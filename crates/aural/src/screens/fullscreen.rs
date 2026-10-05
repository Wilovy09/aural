//! The fullscreen player: a big cover (the album's animated one when that is on and exists),
//! the song, the Música/Letra tabs, the progress and the transport with shuffle and repeat.
//! The Letra tab puts the lyrics beside the player. Back closes it.

use std::sync::Arc;

use freya::prelude::*;
use freya::radio::use_radio;

use crate::chrome::player_bar::Progress;
use crate::cover::{self, Art};
use crate::engine::Repeat;
use crate::library::{self, Song};
use crate::screens::backdrop::Backdrop;
use crate::screens::lyrics::LyricsPanel;
use crate::screens::queue::QueuePanel;
use crate::state::{AppState, Channel, Mode, View};
use crate::ui::{self, Cover, Icon, Variant, color, metrics, text};

#[derive(PartialEq)]
pub struct Fullscreen;

impl Component for Fullscreen {
    fn render(&self) -> impl IntoElement {
        let navigation = use_radio::<AppState, Channel>(Channel::Navigation);
        let now = use_radio::<AppState, Channel>(Channel::Now);
        let motion_on = navigation.read().motion;
        let now_state = now.read().now.clone();
        let song = now_state.song.clone();
        let night_title = song.as_ref().map(|song| song.title.clone());

        let mut frame = use_state(|| None::<Arc<motion::Frame>>);
        let frame_number = use_state(|| 0u64);
        let mut task = use_state(|| None::<TaskHandle>);
        let wanted = use_reactive(&(song.clone(), motion_on));

        // A new song (or turning the option on) drops the old loop and looks the new one up.
        use_side_effect(move || {
            let (song, motion_on) = wanted.read().clone();
            if let Some(old) = task.write().take() {
                old.cancel();
            }
            frame.set(None);
            let (Some(song), true) = (song, motion_on) else {
                return;
            };
            task.set(Some(start(&song, frame, frame_number)));
        });

        let navigation_state = navigation.read();
        let view = navigation_state.view;
        let mode = navigation_state.mode;
        let controls = mode == Mode::Normal;
        let focus = navigation_state.focus;
        drop(navigation_state);
        let (width, height) = ui::viewport();

        // The lyrics view splits the screen like Sonora's on a wide window: the player on the
        // left, the sheet on the right. The music view keeps the player centred.
        let split = view != View::Music;
        let side = match split {
            true => (height * 0.40).min(width * 0.28).min(460.),
            false => (height * 0.44).min(width * 0.34).min(520.),
        };
        let tab_ring = (focus.full_row == 0).then_some(focus.full_tab);
        let ring = |at: usize| focus.full_row == 1 && focus.full_button == at;

        let player = rect()
            .width(match split {
                true => Size::percent(40.),
                false => Size::fill(),
            })
            .height(Size::fill())
            .center()
            .spacing(12.)
            .child(match frame.read().clone() {
                Some(frame) => {
                    cover::view(Some(Art::Motion(frame)), *frame_number.read(), side).into_element()
                }
                None => Cover::new(
                    song.as_ref().and_then(|s| s.cover.clone()),
                    side,
                    metrics::RADIUS * 2.,
                )
                .edge(library::COVER_EDGE)
                .into_element(),
            })
            .map(song, |screen, song| {
                screen
                    .child(
                        ui::line(song.title, text::TITLE, color::FOREGROUND)
                            .font_weight(FontWeight::SEMI_BOLD),
                    )
                    .child(ui::line(song.artist, text::BODY, color::ON_BACKDROP))
            })
            .maybe(controls, |player| {
                player
                    .child(ui::tabs(
                        &[
                            (Icon::Music, "Música"),
                            (Icon::Lyrics, "Letra"),
                            (Icon::Queue, "Cola"),
                        ],
                        match view {
                            View::Music => 0,
                            View::Lyrics => 1,
                            View::Queue => 2,
                        },
                        tab_ring,
                    ))
                    .child(
                        rect()
                            .width(Size::px((side * 1.5).min(420.)))
                            .child(Progress {
                                loading: now_state.loading,
                                ink: color::ON_BACKDROP,
                            }),
                    )
                    .child(
                        rect()
                            .direction(Direction::Horizontal)
                            .cross_align(Alignment::Center)
                            .spacing(match split {
                                true => 2.,
                                false => 10.,
                            })
                            .child(ui::toggle(Icon::Shuffle, now_state.shuffle, ring(0)))
                            .child(ui::button(
                                Variant::Ghost,
                                Some(Icon::Previous),
                                None,
                                ring(1),
                            ))
                            .child(ui::button(
                                Variant::Primary,
                                Some(match now_state.playing {
                                    true => Icon::PauseFilled,
                                    false => Icon::PlayFilled,
                                }),
                                None,
                                ring(2),
                            ))
                            .child(ui::button(Variant::Ghost, Some(Icon::Next), None, ring(3)))
                            .child(ui::toggle(
                                match now_state.repeat {
                                    Repeat::One => Icon::RepeatOne,
                                    _ => Icon::Repeat,
                                },
                                now_state.repeat != Repeat::Off,
                                ring(4),
                            ))
                            .child(
                                rect()
                                    .width(Size::px(1.))
                                    .height(Size::px(metrics::CONTROL))
                                    .margin((0., 4.))
                                    .background(color::GLASS_EDGE),
                            )
                            .child(ui::toggle(Icon::Clear, false, ring(5)))
                            .child(ui::toggle(Icon::Night, false, ring(6))),
                    )
                    .child(ui::line(
                        "Atrás para cerrar",
                        text::TINY,
                        color::ON_BACKDROP,
                    ))
            });

        // Night mode: black, with only the song's name in the bottom-left corner.
        if mode == Mode::Night {
            return rect()
                .expanded()
                .background(Color::BLACK)
                .main_align(Alignment::End)
                .padding(48.)
                .map(night_title, |screen, title| {
                    screen.child(
                        ui::line(title, text::LARGE, color::ON_BACKDROP)
                            .font_weight(FontWeight::SEMI_BOLD),
                    )
                })
                .into_element();
        }

        // The backdrop sits under everything, the player and panel float over it.
        rect()
            .expanded()
            .background(color::BACKGROUND)
            .child(
                rect()
                    .position(Position::new_absolute().top(0.).left(0.))
                    .layer(Layer::Relative(-1))
                    .child(Backdrop),
            )
            .child(
                rect()
                    .expanded()
                    .padding((32., 48., 24., 32.))
                    .direction(Direction::Horizontal)
                    .content(Content::Flex)
                    .spacing(32.)
                    .child(player)
                    .maybe(split, |screen| {
                        screen.child(rect().width(Size::flex(1.)).height(Size::fill()).child(
                            match view {
                                View::Queue => QueuePanel.into_element(),
                                _ => LyricsPanel.into_element(),
                            },
                        ))
                    }),
            )
            .into_element()
    }
}

/// Looks up the animated cover of `song`'s album and plays it into `frame` until the returned
/// task is cancelled, which drops the receiver and stops the decoder.
fn start(
    song: &Song,
    mut frame: State<Option<Arc<motion::Frame>>>,
    mut number: State<u64>,
) -> TaskHandle {
    let (frames, mut latest) = tokio::sync::watch::channel(None);
    cover::start(song, frames, |line| {
        log::info!("cover: {line}");
    });
    spawn(async move {
        while latest.changed().await.is_ok() {
            if let Some(next) = latest.borrow_and_update().clone() {
                frame.set(Some(next));
                *number.write() += 1;
            }
        }
    })
}

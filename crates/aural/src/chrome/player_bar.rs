//! The player bar along the bottom: the song on the left, the transport and progress in the
//! middle, the fullscreen button on the right.

use freya::prelude::*;
use freya::radio::use_radio;

use crate::library::{self, Song};
use crate::state::{AppState, Channel, Zone};
use crate::ui::{self, Cover, Icon, Variant, color, metrics, text};

#[derive(PartialEq)]
pub struct PlayerBar;

impl Component for PlayerBar {
    fn render(&self) -> impl IntoElement {
        let navigation = use_radio::<AppState, Channel>(Channel::Navigation);
        let now = use_radio::<AppState, Channel>(Channel::Now);
        let focus = navigation.read().focus;
        let ring = |button: usize| focus.zone == Zone::Player && focus.player == button;
        let state = now.read();
        let song = state.now.song.clone();
        let playing = state.now.playing;
        let loading = state.now.loading;

        rect()
            .width(Size::fill())
            .height(Size::px(metrics::PLAYER_BAR))
            .background(color::SECONDARY)
            .border(Border::new().fill(color::BORDER).width(BorderWidth {
                top: 1.,
                ..Default::default()
            }))
            .padding((0., 20.))
            .direction(Direction::Horizontal)
            .content(Content::Flex)
            .cross_align(Alignment::Center)
            .spacing(16.)
            .child(rect().width(Size::flex(1.)).child(now_playing(song)))
            .child(
                rect()
                    .width(Size::flex(1.))
                    .max_width(Size::px(560.))
                    .spacing(6.)
                    .cross_align(Alignment::Center)
                    .child(
                        rect()
                            .direction(Direction::Horizontal)
                            .spacing(8.)
                            .child(ui::button(
                                Variant::Ghost,
                                Some(Icon::Previous),
                                None,
                                ring(0),
                            ))
                            .child(ui::button(
                                Variant::Primary,
                                Some(match playing {
                                    true => Icon::PauseFilled,
                                    false => Icon::PlayFilled,
                                }),
                                None,
                                ring(1),
                            ))
                            .child(ui::button(Variant::Ghost, Some(Icon::Next), None, ring(2))),
                    )
                    .child(Progress {
                        loading,
                        ink: color::MUTED_FOREGROUND,
                    }),
            )
            .child(
                rect()
                    .width(Size::flex(1.))
                    .direction(Direction::Horizontal)
                    .main_align(Alignment::End)
                    .child(ui::button(
                        Variant::Ghost,
                        Some(Icon::Maximize),
                        None,
                        ring(3),
                    )),
            )
    }
}

/// The cover, title and artist of the current song.
fn now_playing(song: Option<Song>) -> impl IntoElement {
    let Some(song) = song else {
        return rect()
            .child(ui::line(
                "Nada sonando",
                text::SMALL,
                color::MUTED_FOREGROUND,
            ))
            .into_element();
    };
    rect()
        .direction(Direction::Horizontal)
        .cross_align(Alignment::Center)
        .spacing(12.)
        .child(Cover::new(song.cover.clone(), 42., 4.))
        .child(
            rect()
                .spacing(2.)
                .child(
                    ui::line(song.title, text::BODY, color::FOREGROUND)
                        .font_weight(FontWeight::SEMI_BOLD),
                )
                .child(ui::line(song.artist, text::SMALL, color::MUTED_FOREGROUND)),
        )
        .into_element()
}

/// The elapsed clock, the line and the total, on their own channel so a tick repaints only this.
#[derive(PartialEq)]
pub struct Progress {
    pub loading: bool,
    /// The colour of the clocks: muted on the bar, lighter over the fullscreen backdrop.
    pub ink: Color,
}

impl Component for Progress {
    fn render(&self) -> impl IntoElement {
        let radio = use_radio::<AppState, Channel>(Channel::Position);
        let position = radio.read().position;
        let total = position.total.unwrap_or_default();
        let fraction = match total.is_zero() {
            true => 0.,
            false => (position.elapsed.as_secs_f32() / total.as_secs_f32()).clamp(0., 1.),
        };
        rect()
            .width(Size::fill())
            .direction(Direction::Horizontal)
            .content(Content::Flex)
            .cross_align(Alignment::Center)
            .spacing(8.)
            .child(ui::line(
                clock(position.elapsed, self.loading),
                text::TINY,
                self.ink,
            ))
            .child(
                rect()
                    .width(Size::flex(1.))
                    .height(Size::px(4.))
                    .corner_radius(2.)
                    .background(color::MUTED)
                    .child(
                        rect()
                            .width(Size::percent(fraction * 100.))
                            .height(Size::fill())
                            .corner_radius(2.)
                            .background(color::PROGRESS),
                    ),
            )
            .child(ui::line(
                position.total.map(library::clock).unwrap_or_default(),
                text::TINY,
                self.ink,
            ))
    }
}

fn clock(elapsed: std::time::Duration, loading: bool) -> String {
    match loading {
        true => "…".into(),
        false => library::clock(elapsed),
    }
}

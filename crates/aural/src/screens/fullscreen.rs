//! The fullscreen player: a big cover (the album's animated one when that is on and exists),
//! the song, the Música/Letra tabs, the progress and the transport with shuffle and repeat.
//! The Letra tab puts the lyrics beside the player. Back closes it.

use freya::engine::prelude::Image as SkImage;
use freya::prelude::*;
use freya::radio::use_radio;

use crate::chrome::player_bar::Progress;
use crate::cover::{self, Art};
use crate::engine::Repeat;
use crate::library::{self, Song};
use crate::nav::Target;
use crate::screens::backdrop::Backdrop;
use crate::screens::lyrics::LyricsPanel;
use crate::state::{AppState, Channel, Mode, View};
use crate::ui::{self, Cover, Icon, Variant, color, metrics, text};

#[derive(PartialEq)]
pub struct Fullscreen;

impl Component for Fullscreen {
    fn render(&self) -> impl IntoElement {
        let navigation = use_radio::<AppState, Channel>(Channel::Navigation);
        let now = use_radio::<AppState, Channel>(Channel::Now);
        let motion_on = navigation.read().motion;
        let library = use_radio::<AppState, Channel>(Channel::Library);
        let (song, playback, next) = {
            let now = &now.read().now;
            (now.song.clone(), Playback::from(now), up_next(now))
        };
        let liked = song
            .as_ref()
            .is_some_and(|song| library.read().liked(&song.id));
        let night_title = song.as_ref().map(|song| song.title.clone());

        let mut frame = use_state(|| None::<SkImage>);
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
        let translating = navigation_state.translate;
        let queue_open = navigation_state.queue;
        let controls = mode == Mode::Normal;
        let focus = navigation_state.focus;
        drop(navigation_state);
        let (width, height) = ui::viewport();
        let compact = ui::compact();

        // The lyrics view splits the screen like Sonora's on a wide window: the player on the
        // left, the sheet on the right. The music view keeps the player centred.
        let split = view != View::Music;
        let side = match (compact, split) {
            (true, true) => 52.,
            (true, false) => (width - 48.).min(height * 0.44),
            (false, true) => (height * 0.40).min(width * 0.28).min(460.),
            (false, false) => (height * 0.44).min(width * 0.34).min(520.),
        };
        let tab_ring = (focus.full_row == 0).then_some(focus.full_tab);
        // The transport's two rows share one button index.
        let ring = |at: usize| matches!(focus.full_row, 1 | 3) && focus.full_button == at;
        let press = |at: usize| ui::tap(Target::Transport(at));
        let tabs = ui::tabs(
            &[
                (Icon::Music, "Música"),
                (Icon::Lyrics, "Letra"),
                (Icon::Queue, "Cola"),
            ],
            // Cola stays lit while its panel is open.
            match (queue_open, view) {
                (true, _) => 2,
                (false, View::Music) => 0,
                (false, View::Lyrics) => 1,
            },
            tab_ring,
        );
        let transport = rect()
            .direction(Direction::Horizontal)
            .cross_align(Alignment::Center)
            .spacing(match (compact, split) {
                (true, _) => 4.,
                (false, true) => 2.,
                (false, false) => 10.,
            })
            .child(ui::toggle(Icon::Shuffle, playback.shuffle, ring(0)).on_press(press(0)))
            .child(
                ui::button(Variant::Ghost, Some(Icon::Previous), None, ring(1)).on_press(press(1)),
            )
            .child(
                ui::button(
                    Variant::Primary,
                    Some(match playback.playing {
                        true => Icon::PauseFilled,
                        false => Icon::PlayFilled,
                    }),
                    None,
                    ring(2),
                )
                .on_press(press(2)),
            )
            .child(ui::button(Variant::Ghost, Some(Icon::Next), None, ring(3)).on_press(press(3)))
            .child(
                ui::toggle(
                    match playback.repeat {
                        Repeat::One => Icon::RepeatOne,
                        _ => Icon::Repeat,
                    },
                    playback.repeat != Repeat::Off,
                    ring(4),
                )
                .on_press(press(4)),
            );
        // The modes under the transport: clear, night, translate, like.
        let modes = rect()
            .direction(Direction::Horizontal)
            .cross_align(Alignment::Center)
            .spacing(match split {
                true => 10.,
                false => 18.,
            })
            .child(ui::toggle(Icon::Clear, false, ring(5)).on_press(press(5)))
            .child(ui::toggle(Icon::Night, false, ring(6)).on_press(press(6)))
            .child(ui::toggle(Icon::Translate, translating, ring(7)).on_press(press(7)))
            .child(
                ui::toggle(
                    match liked {
                        true => Icon::HeartFilled,
                        false => Icon::Heart,
                    },
                    liked,
                    ring(8),
                )
                .on_press(press(8)),
            );
        let progress = Progress {
            loading: playback.loading,
            focused: focus.full_row == 2,
            ink: color::ON_BACKDROP,
        };

        if compact && mode != Mode::Night {
            return phone(Phone {
                art: Artwork {
                    frame,
                    number: frame_number,
                    song: song.clone(),
                    side,
                    radius: match split {
                        true => 6.,
                        false => metrics::PLAYER_RADIUS * 2.,
                    },
                }
                .into_element(),
                song,
                view,
                controls,
                tabs,
                progress,
                playback,
                button: matches!(focus.full_row, 1 | 3).then_some(focus.full_button),
                translating,
                liked,
                next: next.clone(),
            })
            .into_element();
        }

        let player = rect()
            .width(match split {
                true => Size::percent(40.),
                false => Size::fill(),
            })
            .height(Size::fill())
            .center()
            .spacing(12.)
            .child(Artwork {
                frame,
                number: frame_number,
                song: song.clone(),
                side,
                radius: metrics::PLAYER_RADIUS * 2.,
            })
            .map(song, |screen, song| {
                screen
                    .child(
                        ui::marquee::Marquee::new(song.title, text::TITLE, color::FOREGROUND)
                            .weight(FontWeight::SEMI_BOLD)
                            .centered(),
                    )
                    .child(ui::line(song.artist, text::BODY, color::ON_BACKDROP))
            })
            .maybe(controls, |player| {
                player
                    .child(tabs)
                    .child(
                        rect()
                            .width(Size::px((side * 1.5).min(420.)))
                            .child(progress),
                    )
                    .child(
                        rect()
                            .cross_align(Alignment::Center)
                            .spacing(6.)
                            .child(transport)
                            .child(modes),
                    )
            });

        // Night mode: black, with only the song's name in the bottom-left corner.
        if mode == Mode::Night {
            return rect()
                .expanded()
                .background(Color::BLACK)
                .on_press(ui::tap(Target::Back))
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

        let shows_next = controls && next.is_some();
        // The backdrop sits under everything, the player and panel float over it.
        rect()
            .expanded()
            .background(color::BACKGROUND)
            .maybe(!controls, |screen| screen.on_press(ui::tap(Target::Back)))
            .child(
                rect()
                    .position(Position::new_absolute().top(0.).left(0.))
                    .layer(Layer::Relative(-1))
                    .child(Backdrop),
            )
            .child(
                rect()
                    .expanded()
                    .padding((20., 48., 24., 32.))
                    .content(Content::Flex)
                    .spacing(4.)
                    // Which device plays, in the top-right corner; pressed, the devices panel.
                    .maybe(controls, |screen| {
                        screen.child(
                            rect()
                                .width(Size::fill())
                                .main_align(Alignment::End)
                                .direction(Direction::Horizontal)
                                .child(crate::chrome::player_bar::DevicesPill {
                                    focused: focus.full_row == 4,
                                    target: Target::Devices,
                                    named: true,
                                    glass: true,
                                }),
                        )
                    })
                    .child(
                        rect()
                            .width(Size::fill())
                            .height(Size::flex(1.))
                            .direction(Direction::Horizontal)
                            .content(Content::Flex)
                            .spacing(32.)
                            // In the music view the card goes under the player, in the bottom-right
                            // corner; floating over the backdrop, it flickered as that repainted.
                            .child(match next.clone().filter(|_| shows_next && !split) {
                                Some(next) => rect()
                                    .width(Size::fill())
                                    .height(Size::fill())
                                    .content(Content::Flex)
                                    .cross_align(Alignment::End)
                                    .child(
                                        rect()
                                            .width(Size::fill())
                                            .height(Size::flex(1.))
                                            .child(player),
                                    )
                                    .child(next_card(&next, 340.))
                                    .into_element(),
                                None => player.into_element(),
                            })
                            .maybe(split, |screen| {
                                screen.child(
                                    rect()
                                        .width(Size::flex(1.))
                                        .height(Size::fill())
                                        .content(Content::Flex)
                                        .cross_align(Alignment::End)
                                        .spacing(16.)
                                        .child(
                                            rect()
                                                .width(Size::fill())
                                                .height(Size::flex(1.))
                                                .child(LyricsPanel),
                                        )
                                        // Under the lyrics, not floating over them: a card over the
                                        // scrolling lines flickers as they repaint.
                                        .map(
                                            next.clone().filter(|_| shows_next),
                                            |column, next| column.child(next_card(&next, 340.)),
                                        ),
                                )
                            }),
                    ),
            )
            .into_element()
    }
}

/// The song after the one playing: the next in the queue, or the first again when repeat
/// starts the queue over. Nothing on repeat-one, which plays the same song again.
fn up_next(now: &crate::state::Now) -> Option<Song> {
    match now.repeat {
        Repeat::One => None,
        Repeat::All => now
            .queue
            .get(now.index + 1)
            .or_else(|| now.queue.first().filter(|_| now.queue.len() > 1))
            .cloned(),
        Repeat::Off => now.queue.get(now.index + 1).cloned(),
    }
}

/// "Después": the next song's cover, title and artist on glass. Pressed, it plays.
fn next_card(song: &Song, width: f32) -> NextCard {
    NextCard {
        song: song.clone(),
        width,
    }
}

/// Its own component so the player's re-renders pass it by: rebuilt with them, the card
/// (and its cover) repainted and flickered.
#[derive(PartialEq)]
struct NextCard {
    song: Song,
    width: f32,
}

impl Component for NextCard {
    fn render(&self) -> impl IntoElement {
        card(&self.song, self.width)
    }
}

fn card(song: &Song, width: f32) -> impl IntoElement {
    rect()
        .width(Size::px(width))
        .height(Size::px(64.))
        .padding((0., 14., 0., 10.))
        .direction(Direction::Horizontal)
        .content(Content::Flex)
        .cross_align(Alignment::Center)
        .spacing(12.)
        // The same glass as the view tabs.
        .corner_radius(metrics::PLAYER_RADIUS + 4.)
        .background(color::GLASS)
        .border(Border::new().fill(color::GLASS_EDGE).width(1.))
        .shadow((0., 8., 24., 0., Color::from_argb(0x33, 0, 0, 0)))
        .on_press(|event: Event<PressEventData>| {
            event.stop_propagation();
            crate::app::control(crate::engine::Command::Next);
        })
        .child(Cover::new(song.cover.clone(), 44., 8.).apple(crate::artwork::Wanted::song(song)))
        .child(
            rect()
                .width(Size::flex(1.))
                .spacing(2.)
                .child(ui::eyebrow("Después").color(color::ON_BACKDROP))
                .child(
                    ui::line(
                        format!("{} · {}", song.title, song.artist),
                        text::BODY,
                        color::FOREGROUND,
                    )
                    .font_weight(FontWeight::SEMI_BOLD)
                    .width(Size::fill()),
                ),
        )
        .child(ui::icon(Icon::Next, 16., color::ON_BACKDROP))
}

/// The transport's state, read from [`crate::state::Now`] without cloning its queue.
#[derive(Clone, Copy)]
struct Playback {
    loading: bool,
    playing: bool,
    shuffle: bool,
    repeat: Repeat,
}

impl From<&crate::state::Now> for Playback {
    fn from(now: &crate::state::Now) -> Self {
        Self {
            loading: now.loading,
            playing: now.playing,
            shuffle: now.shuffle,
            repeat: now.repeat,
        }
    }
}

/// The big cover: the latest motion frame while the loop plays, the still one otherwise. Its
/// own component so each new frame repaints only it, not the whole fullscreen player.
#[derive(PartialEq)]
struct Artwork {
    frame: State<Option<SkImage>>,
    number: State<u64>,
    song: Option<Song>,
    side: f32,
    /// Corner radius of the still cover.
    radius: f32,
}

impl Component for Artwork {
    fn render(&self) -> impl IntoElement {
        match self.frame.read().clone() {
            Some(image) => {
                cover::view(Some(Art::Motion(image)), *self.number.read(), self.side).into_element()
            }
            None => Cover {
                apple: self.song.as_ref().map(crate::artwork::Wanted::song),
                ..Cover::new(
                    self.song.as_ref().and_then(|s| s.cover.clone()),
                    self.side,
                    self.radius,
                )
            }
            .edge(library::COVER_EDGE)
            .into_element(),
        }
    }
}

/// What the phone's fullscreen player is made of.
struct Phone {
    art: Element,
    song: Option<Song>,
    view: View,
    controls: bool,
    tabs: Rect,
    progress: Progress,
    playback: Playback,
    /// The transport button the D-pad is on, if it is on the transport.
    button: Option<usize>,
    /// Whether the lyrics show their translation.
    translating: bool,
    /// Whether the account likes the song.
    liked: bool,
    /// What plays next.
    next: Option<Song>,
}

/// The fullscreen player on a phone, after YouTube Music's and Apple Music's: close on the
/// left and the screen modes on the right along the top; the big cover, the song, the
/// progress and the transport grouped in the middle; the view tabs at the bottom. The lyrics
/// and queue views shrink the cover beside the song and give the rest to the panel.
fn phone(parts: Phone) -> impl IntoElement {
    let Phone {
        art,
        song,
        view,
        controls,
        tabs,
        progress,
        playback,
        button,
        translating,
        liked,
        next,
    } = parts;
    let (top, bottom) = ui::safe();
    let (title, artist) = song
        .map(|song| (song.title, song.artist))
        .unwrap_or_default();
    let ring = |at: usize| button == Some(at);
    let press = |at: usize| ui::tap(Target::Transport(at));
    let names = |size: f32| {
        rect()
            .width(Size::fill())
            .direction(Direction::Horizontal)
            .content(Content::Flex)
            .cross_align(Alignment::Center)
            .spacing(12.)
            .child(
                rect()
                    .width(Size::flex(1.))
                    .spacing(4.)
                    .child(
                        ui::marquee::Marquee::new(title.clone(), size, color::FOREGROUND)
                            .weight(FontWeight::BOLD),
                    )
                    .child(
                        ui::line(artist.clone(), text::BODY + 1., color::ON_BACKDROP)
                            .width(Size::fill()),
                    ),
            )
            // Like it, YouTube Music's heart beside the song.
            .child(
                rect()
                    .width(Size::px(44.))
                    .height(Size::px(44.))
                    .center()
                    .corner_radius(22.)
                    .border(ui::focus_border(ring(8)))
                    .on_press(press(8))
                    .child(ui::heart(liked, 26., color::FOREGROUND, color::ON_BACKDROP)),
            )
    };
    // A round glass button for the top bar.
    let glass = |glyph: Icon, target: Target, focused: bool| {
        rect()
            .width(Size::px(40.))
            .height(Size::px(40.))
            .center()
            .corner_radius(20.)
            .background(color::GLASS)
            .border(ui::focus_border(focused))
            .on_press(ui::tap(target))
            .child(ui::icon(glyph, 20., color::FOREGROUND))
    };
    // A transport button: a bare icon in a round hit area.
    let key = |glyph: Icon, size: f32, ink: Color, at: usize| {
        rect()
            .width(Size::px(52.))
            .height(Size::px(52.))
            .center()
            .corner_radius(26.)
            .background(match ui::ring(ring(at)) {
                true => color::FOCUS_FILL,
                false => Color::TRANSPARENT,
            })
            .border(ui::focus_border(ring(at)))
            .on_press(press(at))
            .child(ui::icon(glyph, size, ink))
    };
    let mode_ink = |on: bool| match on {
        true => color::FOREGROUND,
        false => color::ON_BACKDROP,
    };
    let transport = rect()
        .width(Size::fill())
        .direction(Direction::Horizontal)
        .main_align(Alignment::SpaceBetween)
        .cross_align(Alignment::Center)
        .child(key(Icon::Shuffle, 22., mode_ink(playback.shuffle), 0))
        .child(key(Icon::Previous, 30., color::FOREGROUND, 1))
        .child(
            rect()
                .width(Size::px(68.))
                .height(Size::px(68.))
                .center()
                .corner_radius(34.)
                .background(color::PRIMARY)
                .border(match ui::ring(ring(2)) {
                    true => Border::new().fill(color::FOCUS_ON_PRIMARY).width(3.),
                    false => Border::new().fill(Color::TRANSPARENT).width(3.),
                })
                .on_press(press(2))
                .child(ui::icon(
                    match playback.playing {
                        true => Icon::PauseFilled,
                        false => Icon::PlayFilled,
                    },
                    30.,
                    color::PRIMARY_FOREGROUND,
                )),
        )
        .child(key(Icon::Next, 30., color::FOREGROUND, 3))
        .child(key(
            match playback.repeat {
                Repeat::One => Icon::RepeatOne,
                _ => Icon::Repeat,
            },
            22.,
            mode_ink(playback.repeat != Repeat::Off),
            4,
        ));
    let deck = rect()
        .width(Size::fill())
        .spacing(18.)
        .child(progress)
        .child(transport);

    let body = match view {
        View::Music => rect()
            .width(Size::fill())
            .height(Size::flex(1.))
            .main_align(Alignment::Center)
            .cross_align(Alignment::Center)
            .spacing(28.)
            .child(art)
            .child(
                rect()
                    .width(Size::fill())
                    .spacing(20.)
                    .child(names(text::TITLE))
                    .maybe(controls, |group| group.child(deck)),
            )
            .into_element(),
        _ => rect()
            .width(Size::fill())
            .height(Size::flex(1.))
            .content(Content::Flex)
            .spacing(12.)
            .child(
                rect()
                    .width(Size::fill())
                    .direction(Direction::Horizontal)
                    .content(Content::Flex)
                    .cross_align(Alignment::Center)
                    .spacing(12.)
                    .child(art)
                    .child(rect().width(Size::flex(1.)).child(names(text::LARGE))),
            )
            .child(
                rect()
                    .width(Size::fill())
                    .height(Size::flex(1.))
                    .child(LyricsPanel),
            )
            .maybe(controls, |screen| screen.child(deck))
            .into_element(),
    };
    rect()
        .expanded()
        .background(color::BACKGROUND)
        .maybe(!controls, |screen| screen.on_press(ui::tap(Target::Back)))
        .child(
            rect()
                .position(Position::new_absolute().top(0.).left(0.))
                .layer(Layer::Relative(-1))
                .child(Backdrop),
        )
        .child(
            rect()
                .expanded()
                .padding((12. + top, 24., 16. + bottom, 24.))
                .content(Content::Flex)
                .spacing(16.)
                .maybe(controls, |screen| {
                    screen.child(
                        rect()
                            .width(Size::fill())
                            .direction(Direction::Horizontal)
                            .content(Content::Flex)
                            .spacing(10.)
                            .child(glass(Icon::Close, Target::Back, false))
                            .child(rect().width(Size::flex(1.)))
                            .child(glass(Icon::Cast, Target::Devices, false))
                            .child(glass(Icon::Clear, Target::Transport(5), ring(5)))
                            .child(glass(Icon::Night, Target::Transport(6), ring(6)))
                            .child(
                                glass(Icon::Translate, Target::Transport(7), ring(7)).background(
                                    match translating {
                                        true => color::GLASS_SELECTED,
                                        false => color::GLASS,
                                    },
                                ),
                            ),
                    )
                })
                .child(body)
                .maybe(controls, |screen| {
                    screen
                        // What plays next, over the tabs, in the music view.
                        .map(next.filter(|_| view == View::Music), |screen, next| {
                            screen.child(next_card(&next, ui::viewport().0 - 48.))
                        })
                        .child(rect().width(Size::fill()).center().child(tabs))
                }),
        )
}

/// Looks up the animated cover of `song`'s album and plays it into `frame` until the returned
/// task is cancelled, which drops the receiver and stops the decoder.
fn start(song: &Song, mut frame: State<Option<SkImage>>, mut number: State<u64>) -> TaskHandle {
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

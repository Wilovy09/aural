//! The player bar along the bottom: the song on the left, the transport and progress in the
//! middle, the fullscreen button on the right.

use freya::prelude::*;
use freya::radio::use_radio;

use crate::library::{self, Song};
use crate::nav::Target;
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
        let press = |button: usize| ui::tap(Target::Player(button));
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
            .child(
                rect()
                    .width(Size::flex(1.))
                    .on_press(press(3))
                    .child(now_playing(song)),
            )
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
                            .child(
                                ui::button(Variant::Ghost, Some(Icon::Previous), None, ring(0))
                                    .on_press(press(0)),
                            )
                            .child(
                                ui::button(
                                    Variant::Primary,
                                    Some(match playing {
                                        true => Icon::PauseFilled,
                                        false => Icon::PlayFilled,
                                    }),
                                    None,
                                    ring(1),
                                )
                                .on_press(press(1)),
                            )
                            .child(
                                ui::button(Variant::Ghost, Some(Icon::Next), None, ring(2))
                                    .on_press(press(2)),
                            ),
                    )
                    .child(Progress {
                        loading,
                        focused: false,
                        ink: color::MUTED_FOREGROUND,
                    }),
            )
            .child(
                rect()
                    .width(Size::flex(1.))
                    .direction(Direction::Horizontal)
                    .main_align(Alignment::End)
                    .child(
                        ui::button(Variant::Ghost, Some(Icon::Maximize), None, ring(3))
                            .on_press(press(3)),
                    ),
            )
    }
}

/// The phone's player above the bottom bar, YouTube Music's: a thin progress line over the
/// cover, title and artist, and play or pause. A tap anywhere else opens the fullscreen player.
#[derive(PartialEq)]
pub struct MiniPlayer;

impl Component for MiniPlayer {
    fn render(&self) -> impl IntoElement {
        let navigation = use_radio::<AppState, Channel>(Channel::Navigation);
        let now = use_radio::<AppState, Channel>(Channel::Now);
        let focus = navigation.read().focus;
        let ring = |button: usize| focus.zone == Zone::Player && focus.player == button;
        let state = now.read();
        let Some(song) = state.now.song.clone() else {
            return rect().into_element();
        };
        let playing = state.now.playing;
        drop(state);

        rect()
            .width(Size::fill())
            .background(color::SECONDARY)
            .border(ui::focus_border(ring(3)))
            .on_press(ui::tap(Target::Player(3)))
            .child(Line)
            .child(
                rect()
                    .width(Size::fill())
                    .height(Size::px(MINI))
                    .padding((0., 8., 0., 12.))
                    .direction(Direction::Horizontal)
                    .content(Content::Flex)
                    .cross_align(Alignment::Center)
                    .spacing(12.)
                    .child(
                        Cover::new(song.cover.clone(), 44., 4.)
                            .apple(crate::artwork::Wanted::song(&song)),
                    )
                    .child(
                        rect()
                            .width(Size::flex(1.))
                            .spacing(2.)
                            .child(
                                ui::line(song.title, text::BODY, color::FOREGROUND)
                                    .font_weight(FontWeight::SEMI_BOLD)
                                    .width(Size::fill()),
                            )
                            .child(
                                ui::line(song.artist, text::SMALL, color::MUTED_FOREGROUND)
                                    .width(Size::fill()),
                            ),
                    )
                    .child(
                        rect()
                            .width(Size::px(48.))
                            .height(Size::px(48.))
                            .center()
                            .corner_radius(24.)
                            .border(ui::focus_border(ring(1)))
                            .on_press(ui::tap(Target::Player(1)))
                            .child(ui::icon(
                                match playing {
                                    true => Icon::PauseFilled,
                                    false => Icon::PlayFilled,
                                },
                                24.,
                                color::FOREGROUND,
                            )),
                    ),
            )
            .into_element()
    }
}

/// Height of the mini player's row.
pub const MINI: f32 = 64.;

/// The mini player's progress: a hairline across its top.
#[derive(PartialEq)]
struct Line;

impl Component for Line {
    fn render(&self) -> impl IntoElement {
        let radio = use_radio::<AppState, Channel>(Channel::Position);
        let position = radio.read().position;
        let fraction = match position.total {
            Some(total) if !total.is_zero() => {
                (position.elapsed.as_secs_f32() / total.as_secs_f32()).clamp(0., 1.)
            }
            _ => 0.,
        };
        rect()
            .width(Size::fill())
            .height(Size::px(2.))
            .background(color::MUTED)
            .child(
                rect()
                    .width(Size::percent(fraction * 100.))
                    .height(Size::fill())
                    .background(color::PROGRESS),
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
        .child(Cover::new(song.cover.clone(), 42., 4.).apple(crate::artwork::Wanted::song(&song)))
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
    /// The D-pad is on it: left and right seek.
    pub focused: bool,
    /// The colour of the clocks: muted on the bar, lighter over the fullscreen backdrop.
    pub ink: Color,
}

impl Component for Progress {
    fn render(&self) -> impl IntoElement {
        let radio = use_radio::<AppState, Channel>(Channel::Position);
        let position = radio.read().position;
        let total = position.total.unwrap_or_default();
        // Where a finger or the mouse holds the bar, as a share of it, until it lets go.
        let mut held = use_state(|| None::<f32>);
        let mut area = use_state(Area::default);
        let fraction = match (*held.read(), total.is_zero()) {
            (Some(held), _) => held,
            (None, true) => 0.,
            (None, false) => (position.elapsed.as_secs_f32() / total.as_secs_f32()).clamp(0., 1.),
        };
        let share = move |x: f64| {
            let width = area.read().width().max(1.);
            (x as f32 / width).clamp(0., 1.)
        };
        let shown = match *held.read() {
            Some(held) => total.mul_f32(held),
            None => position.elapsed,
        };
        let grabbing = held.read().is_some() || ui::ring(self.focused);

        rect()
            .width(Size::fill())
            .direction(Direction::Horizontal)
            .content(Content::Flex)
            .cross_align(Alignment::Center)
            .spacing(8.)
            .child(ui::line(clock(shown, self.loading), text::TINY, self.ink))
            .child(
                // A tall, invisible hit area around the thin line, for a finger.
                rect()
                    .width(Size::flex(1.))
                    .height(Size::px(24.))
                    .main_align(Alignment::Center)
                    .on_sized(move |event: Event<SizedEventData>| area.set(event.area))
                    .on_pointer_down(move |event: Event<PointerEventData>| {
                        if total.is_zero() {
                            return;
                        }
                        event.stop_propagation();
                        held.set(Some(share(event.element_location().x)));
                    })
                    .on_global_pointer_move(move |event: Event<PointerEventData>| {
                        if held.peek().is_some() {
                            let x = event.global_location().x - area.peek().min_x() as f64;
                            held.set(Some(share(x)));
                        }
                    })
                    .on_global_pointer_press(move |_: Event<PointerEventData>| {
                        let at = *held.peek();
                        if let Some(at) = at {
                            held.set(None);
                            crate::app::seek(total.mul_f32(at));
                        }
                    })
                    .border(ui::focus_border(self.focused))
                    .corner_radius(6.)
                    .padding((0., 6.))
                    .child(
                        rect()
                            .width(Size::fill())
                            .height(Size::px(match grabbing {
                                true => 6.,
                                false => 4.,
                            }))
                            .corner_radius(3.)
                            .background(color::MUTED)
                            .child(
                                rect()
                                    .width(Size::percent(fraction * 100.))
                                    .height(Size::fill())
                                    .corner_radius(3.)
                                    .background(color::PROGRESS),
                            ),
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

//! The player: an island floating over the bottom edge, faintly lit by the cover of the song
//! playing. The song on the left, the transport and progress in the middle, the music's
//! volume (on a computer), the devices and fullscreen buttons on the right.

use freya::prelude::*;
use freya::radio::use_radio;

use crate::library::{self, Song};
use crate::nav::Target;
use crate::state::{AppState, Channel, PLAYER_LIKE, Zone};
use crate::ui::{self, Cover, Icon, Variant, color, metrics, text};

#[derive(PartialEq)]
pub struct PlayerBar;

impl Component for PlayerBar {
    fn render(&self) -> impl IntoElement {
        let navigation = use_radio::<AppState, Channel>(Channel::Navigation);
        let now = use_radio::<AppState, Channel>(Channel::Now);
        let library = use_radio::<AppState, Channel>(Channel::Library);
        let focus = navigation.read().focus;
        let ring = |button: usize| focus.zone == Zone::Player && focus.player == button;
        let press = |button: usize| ui::tap(Target::Player(button));
        let state = now.read();
        let song = state.now.song.clone();
        let playing = state.now.playing;
        let loading = state.now.loading;
        let light = state.now.light.unwrap_or(color::FOREGROUND);
        let liked = song
            .as_ref()
            .is_some_and(|song| library.read().liked(&song.id));
        let island = match state.now.light {
            Some(light) => ui::tint::mix(color::SECONDARY, light, 0.10),
            None => color::SECONDARY,
        };

        rect().width(Size::fill()).padding((0., 8., 8., 8.)).child(
            rect()
                .width(Size::fill())
                .height(Size::px(metrics::PLAYER_BAR - 8.))
                .corner_radius(metrics::RADIUS_LG)
                .background(island)
                .border(Border::new().fill(color::SIDEBAR_BORDER).width(1.))
                .padding((0., 16.))
                .direction(Direction::Horizontal)
                .content(Content::Flex)
                .cross_align(Alignment::Center)
                .spacing(16.)
                .child(
                    rect()
                        .width(Size::flex(1.))
                        .direction(Direction::Horizontal)
                        .content(Content::Flex)
                        .cross_align(Alignment::Center)
                        .spacing(10.)
                        .maybe(song.is_some(), |left| {
                            left.child(
                                rect()
                                    .width(Size::px(metrics::CONTROL))
                                    .height(Size::px(metrics::CONTROL))
                                    .center()
                                    .corner_radius(metrics::CONTROL / 2.)
                                    .border(ui::focus_border(ring(PLAYER_LIKE)))
                                    .on_press(press(PLAYER_LIKE))
                                    .child(ui::heart(liked, 18., light, color::MUTED_FOREGROUND)),
                            )
                        })
                        .child(
                            rect()
                                .width(Size::flex(1.))
                                .on_press(press(4))
                                .child(now_playing(song)),
                        ),
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
                        .cross_align(Alignment::Center)
                        .spacing(4.)
                        .maybe(
                            cfg!(not(any(target_os = "android", target_os = "ios"))),
                            |right| right.child(VolumeControl),
                        )
                        .child(DevicesPill {
                            focused: ring(3),
                            target: Target::Player(3),
                            named: true,
                            glass: false,
                        })
                        .child(
                            ui::button(Variant::Ghost, Some(Icon::Maximize), None, ring(4))
                                .on_press(press(4)),
                        ),
                ),
        )
    }
}

/// How far one notch of the mouse wheel moves the volume.
const WHEEL_STEP: f32 = 0.05;
/// The wheel delta Freya reports for one notch; a touchpad's pixels scale against it.
const NOTCH: f64 = 53.;

/// The music's volume, apart from the device's: a speaker that mutes and a thin bar like the
/// progress one. Dragging is heard at once; the level is saved when the bar is let go.
#[derive(PartialEq)]
struct VolumeControl;

impl Component for VolumeControl {
    fn render(&self) -> impl IntoElement {
        let radio = use_radio::<AppState, Channel>(Channel::Volume);
        let volume = radio.read().volume;
        let mut held = use_state(|| false);
        let mut area = use_state(Area::default);
        let at = move |x: f64| {
            let width = (area.peek().width() - 2. * VOLUME_PAD).max(1.);
            volume.at((x as f32 / width).clamp(0., 1.))
        };
        let shown = volume.shown();
        let glyph = match shown {
            0. => Icon::VolumeMute,
            ..0.5 => Icon::VolumeLow,
            _ => Icon::VolumeHigh,
        };

        // The track lights up and shows its knob while the pointer is over it or holds it.
        let mut hovered = use_state(|| false);
        let lit = *hovered.read() || *held.read();
        // The track runs inside the hit area's side padding.
        let track = (area.read().width() - 2. * VOLUME_PAD).max(0.);
        let knob = if lit { 12. } else { 0. };

        rect()
            .direction(Direction::Horizontal)
            .cross_align(Alignment::Center)
            .spacing(2.)
            .on_pointer_enter(move |_| hovered.set(true))
            .on_pointer_leave(move |_| hovered.set(false))
            .on_wheel(move |event: Event<WheelEventData>| {
                if event.delta_y == 0. {
                    return;
                }
                event.stop_propagation();
                let step = (event.delta_y / NOTCH) as f32 * WHEEL_STEP;
                crate::app::set_volume(volume.at(volume.shown() + step), true);
            })
            .child(
                rect()
                    .width(Size::px(metrics::CONTROL))
                    .height(Size::px(metrics::CONTROL))
                    .center()
                    .corner_radius(metrics::CONTROL / 2.)
                    .on_press(move |_| crate::app::set_volume(volume.toggled(), true))
                    .child(ui::icon(
                        glyph,
                        18.,
                        match lit {
                            true => color::FOREGROUND,
                            false => color::MUTED_FOREGROUND,
                        },
                    )),
            )
            .child(
                // A tall, invisible hit area around the thin line, as on the progress bar.
                rect()
                    .width(Size::px(108.))
                    .height(Size::px(24.))
                    .main_align(Alignment::Center)
                    .on_sized(move |event: Event<SizedEventData>| area.set(event.area))
                    .on_pointer_down(move |event: Event<PointerEventData>| {
                        event.stop_propagation();
                        held.set(true);
                        let x = event.element_location().x - VOLUME_PAD as f64;
                        crate::app::set_volume(at(x), false);
                    })
                    .on_global_pointer_move(move |event: Event<PointerEventData>| {
                        if *held.peek() {
                            let x = event.global_location().x
                                - area.peek().min_x() as f64
                                - VOLUME_PAD as f64;
                            crate::app::set_volume(at(x), false);
                        }
                    })
                    .on_global_pointer_up(move |_: Event<PointerEventData>| {
                        if *held.peek() {
                            held.set(false);
                            crate::app::set_volume(volume, true);
                        }
                    })
                    .padding((0., VOLUME_PAD))
                    .child(
                        rect()
                            .width(Size::fill())
                            .height(Size::px(4.))
                            .corner_radius(2.)
                            .background(Color::from_argb(0x33, 0xff, 0xff, 0xff))
                            .child(
                                rect()
                                    .width(Size::px(track * shown))
                                    .height(Size::fill())
                                    .corner_radius(2.)
                                    .background(match lit {
                                        true => color::FOREGROUND,
                                        false => Color::from_argb(0xb3, 0xff, 0xff, 0xff),
                                    }),
                            )
                            // The knob, centred on where the fill ends.
                            .maybe(lit, |line| {
                                line.child(
                                    rect()
                                        .position(
                                            Position::new_absolute()
                                                .left(track * shown - knob / 2.)
                                                .top(2. - knob / 2.),
                                        )
                                        .width(Size::px(knob))
                                        .height(Size::px(knob))
                                        .corner_radius(knob / 2.)
                                        .background(color::FOREGROUND)
                                        .shadow((0., 1., 4., 0., Color::from_argb(0x66, 0, 0, 0))),
                                )
                            }),
                    ),
            )
    }
}

/// The side padding of the volume's hit area, around its track.
const VOLUME_PAD: f32 = 6.;

/// The phone's player above the bottom bar, YouTube Music's: a thin progress line over the
/// cover, title and artist, and play or pause. A tap anywhere else opens the fullscreen player.
#[derive(PartialEq)]
pub struct MiniPlayer;

impl Component for MiniPlayer {
    fn render(&self) -> impl IntoElement {
        let navigation = use_radio::<AppState, Channel>(Channel::Navigation);
        let now = use_radio::<AppState, Channel>(Channel::Now);
        let connect = use_radio::<AppState, Channel>(Channel::Connect);
        let library = use_radio::<AppState, Channel>(Channel::Library);
        let remote = connect.read().connect.remote().map(str::to_owned);
        let remote_on = remote.is_some();
        let focus = navigation.read().focus;
        let ring = |button: usize| focus.zone == Zone::Player && focus.player == button;
        let state = now.read();
        let Some(song) = state.now.song.clone() else {
            return rect().into_element();
        };
        let playing = state.now.playing;
        let light = state.now.light.unwrap_or(color::FOREGROUND);
        drop(state);
        let liked = library.read().liked(&song.id);

        rect()
            .width(Size::fill())
            .background(color::SECONDARY)
            .border(ui::focus_border(ring(4)))
            .on_press(ui::tap(Target::Player(4)))
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
                                ui::marquee::Marquee::new(
                                    song.title,
                                    text::BODY,
                                    color::FOREGROUND,
                                )
                                .weight(FontWeight::SEMI_BOLD),
                            )
                            .child(
                                ui::line(song.artist, text::SMALL, color::MUTED_FOREGROUND)
                                    .width(Size::fill()),
                            ),
                    )
                    .child(
                        rect()
                            .width(Size::px(44.))
                            .height(Size::px(48.))
                            .center()
                            .corner_radius(22.)
                            .border(ui::focus_border(ring(PLAYER_LIKE)))
                            .on_press(ui::tap(Target::Player(PLAYER_LIKE)))
                            .child(ui::heart(liked, 22., light, color::MUTED_FOREGROUND)),
                    )
                    // Only the icon while this phone plays: the title keeps the room.
                    .child(DevicesPill {
                        focused: ring(3),
                        target: Target::Player(3),
                        named: remote_on,
                        glass: false,
                    })
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

/// The device the music plays on: this one, or the one it controls. Pressed, the devices
/// panel. On glass over the fullscreen player; plain and smaller in the player bars.
#[derive(PartialEq)]
pub struct DevicesPill {
    pub focused: bool,
    pub target: Target,
    /// Whether it shows the device's name beside the icon.
    pub named: bool,
    /// The fullscreen player's glass and size, rather than the bar's.
    pub glass: bool,
}

impl Component for DevicesPill {
    fn render(&self) -> impl IntoElement {
        let connect = use_radio::<AppState, Channel>(Channel::Connect);
        let name = match connect.read().connect.remote() {
            Some(name) => name.to_owned(),
            None if cfg!(target_os = "macos") => "Esta Mac".to_owned(),
            None if cfg!(target_os = "ios") => "Este iPhone".to_owned(),
            None if cfg!(target_os = "android") && ui::compact() => "Este teléfono".to_owned(),
            None if cfg!(target_os = "android") => "Esta TV".to_owned(),
            None => "Este equipo".to_owned(),
        };
        let glass = self.glass;
        let (side, glyph, size, ink) = match glass {
            true => (metrics::CONTROL + 12., 18., text::BODY, color::FOREGROUND),
            false => (metrics::CONTROL, 16., text::SMALL, color::MUTED_FOREGROUND),
        };
        rect()
            .height(Size::px(side))
            .min_width(Size::px(side))
            .max_width(Size::px(220.))
            .padding(match (self.named, glass) {
                (false, _) => (0., 0., 0., 0.),
                (true, true) => (0., 18., 0., 14.),
                (true, false) => (0., 10., 0., 8.),
            })
            .direction(Direction::Horizontal)
            .main_align(Alignment::Center)
            .cross_align(Alignment::Center)
            .spacing(if glass { 10. } else { 6. })
            .corner_radius(metrics::PLAYER_RADIUS + 4.)
            .maybe(glass, |pill| {
                pill.background(color::GLASS).shadow((
                    0.,
                    8.,
                    24.,
                    0.,
                    Color::from_argb(0x33, 0, 0, 0),
                ))
            })
            .border(match (self.focused, glass) {
                (true, _) => ui::focus_border(true),
                (false, true) => Border::new().fill(color::GLASS_EDGE).width(1.),
                (false, false) => ui::focus_border(false),
            })
            .on_press(ui::tap(self.target))
            .child(ui::icon(Icon::Cast, glyph, ink))
            .maybe(self.named, |pill| {
                pill.child(ui::line(name, size, ink).font_weight(FontWeight::SEMI_BOLD))
            })
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
        .width(Size::fill())
        .direction(Direction::Horizontal)
        .content(Content::Flex)
        .cross_align(Alignment::Center)
        .spacing(12.)
        .child(Cover::new(song.cover.clone(), 42., 4.).apple(crate::artwork::Wanted::song(&song)))
        .child(
            rect()
                .width(Size::flex(1.))
                .spacing(2.)
                .child(
                    ui::marquee::Marquee::new(song.title, text::BODY, color::FOREGROUND)
                        .weight(FontWeight::SEMI_BOLD),
                )
                .child(
                    ui::line(song.artist, text::SMALL, color::MUTED_FOREGROUND).width(Size::fill()),
                ),
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
                    .on_global_pointer_up(move |_: Event<PointerEventData>| {
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

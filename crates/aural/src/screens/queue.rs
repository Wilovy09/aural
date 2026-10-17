//! The queue's panel, over whatever shows like the devices' panel, after YouTube Music's: the whole queue in play
//! order (shuffled when shuffle is on), the songs already played dimmed above the one playing.
//! Any song plays when pressed; the ones coming up also move and leave the queue.

use freya::prelude::*;
use freya::radio::use_radio;

use crate::library::{self, Song};
use crate::state::{AppState, Channel};
use crate::ui::sheet::{Palette, Sheet};
use crate::ui::{self, Cover, Icon, color, metrics, text};

/// Height of one queue row, with the line that marks a drop above it.
const ROW: f32 = 58.;
/// Side of a row's cover.
const THUMB: f32 = 40.;
/// How dim the songs already played are.
const PLAYED: f32 = 0.5;

/// The panel itself: a sheet with the title, a close button and the list.
#[derive(PartialEq)]
pub struct Queue;

impl Component for Queue {
    fn render(&self) -> impl IntoElement {
        let navigation = use_radio::<AppState, Channel>(Channel::Navigation);
        let now = use_radio::<AppState, Channel>(Channel::Now);
        let (open, glass) = {
            let state = navigation.read();
            (state.queue, state.fullscreen)
        };
        let count = now.read().now.queue.len();
        let compact = ui::compact();
        let (top, bottom) = ui::safe();
        let palette = Palette::new(glass);
        Sheet {
            open,
            glass,
            tall: ui::viewport().1 * 0.85,
            close: crate::app::close_queue,
            content: rect()
                .width(Size::fill())
                .height(Size::fill())
                .padding(match compact {
                    true => (14., 20., bottom, 20.),
                    false => (36. + top, 24., bottom, 24.),
                })
                .spacing(16.)
                .content(Content::Flex)
                // Title and close.
                .child(
                    rect()
                        .width(Size::fill())
                        .direction(Direction::Horizontal)
                        .content(Content::Flex)
                        .cross_align(Alignment::Start)
                        .padding((0., 0., 0., 16.))
                        .child(
                            rect()
                                .width(Size::flex(1.))
                                .spacing(6.)
                                .child(
                                    ui::line("Cola", text::TITLE + 4., color::FOREGROUND)
                                        .font_weight(FontWeight::BOLD),
                                )
                                .child(ui::line(
                                    match count {
                                        1 => "1 canción".to_owned(),
                                        count => format!("{count} canciones"),
                                    },
                                    text::SMALL,
                                    color::MUTED_FOREGROUND,
                                )),
                        )
                        .child(
                            rect()
                                .width(Size::px(40.))
                                .height(Size::px(40.))
                                .center()
                                .corner_radius(20.)
                                .background(palette.raised)
                                .on_press(|event: Event<PressEventData>| {
                                    event.stop_propagation();
                                    crate::app::close_queue();
                                })
                                .child(ui::icon(Icon::Close, 18., color::FOREGROUND)),
                        ),
                )
                .child(
                    rect()
                        .width(Size::fill())
                        .height(Size::flex(1.))
                        // Mounted with the sheet, so each opening starts at the song playing.
                        .child(QueuePanel),
                )
                .into_element(),
        }
    }
}

#[derive(PartialEq)]
pub struct QueuePanel;

impl Component for QueuePanel {
    fn render(&self) -> impl IntoElement {
        let now = use_radio::<AppState, Channel>(Channel::Now);
        let state = now.read();
        let queue = state.now.queue.clone();
        let index = state.now.index;
        drop(state);
        let count = queue.len();
        let moving = use_state(|| None::<Moving>);
        let mut scroll = use_scroll_controller(ScrollConfig::default);
        // Opening the queue, or moving on to another song, brings the song playing near the
        // top, the one before it showing above.
        let mut shown = use_state(|| None::<usize>);
        use_side_effect(move || {
            let index = now.read().now.index;
            if *shown.peek() != Some(index) {
                shown.set(Some(index));
                scroll.scroll_to_y(-(index.saturating_sub(1) as f32 * ROW) as i32);
            }
        });

        rect()
            .width(Size::fill())
            .height(Size::fill())
            .spacing(10.)
            .content(Content::Flex)
            .child(
                rect().width(Size::fill()).height(Size::flex(1.)).child(
                    // A finger scrolls through `TouchScroll`, which leaves sideways swipes and
                    // the grips' drags to the rows.
                    ui::touch_scroll::TouchScroll {
                        scroll,
                        content: VirtualScrollView::new_with_data_controlled(
                            (queue, index, moving),
                            |item, (queue, index, moving)| {
                                let at = item.index;
                                let song = &queue[at];
                                match at.cmp(index) {
                                    std::cmp::Ordering::Less => played(at, song).into_element(),
                                    std::cmp::Ordering::Equal => row(at, song, true).into_element(),
                                    std::cmp::Ordering::Greater => Movable {
                                        at,
                                        song: song.clone(),
                                        moving: *moving,
                                        first: index + 1,
                                        last: queue.len() - 1,
                                    }
                                    .into_element(),
                                }
                            },
                            scroll,
                        )
                        .length(count)
                        .item_size(ROW)
                        .show_scrollbar(false)
                        .drag_scrolling(false)
                        .width(Size::fill())
                        .height(Size::fill())
                        .into_element(),
                    },
                ),
            )
            .maybe(index + 1 >= count, |panel| {
                panel.child(ui::line(
                    "No hay más canciones en la cola",
                    text::BODY,
                    color::ON_BACKDROP,
                ))
            })
    }
}

/// Plays the song at `at` when pressed, unless the press ends a swipe.
fn play_at(at: usize) -> impl FnMut(Event<PressEventData>) + 'static {
    move |event: Event<PressEventData>| {
        event.stop_propagation();
        if !ui::swipe::just_swiped() && !ui::long_press::taken() {
            crate::app::play_at(at);
        }
    }
}

/// A song already played, dimmed: pressed, it plays again from there.
fn played(at: usize, song: &Song) -> impl IntoElement {
    rect()
        .key(at)
        .width(Size::fill())
        .height(Size::px(ROW))
        .opacity(PLAYED)
        .on_press(play_at(at))
        .child(row(at, song, false))
}

/// A song being moved by its grip: where it was, where it would land, and where the pointer
/// first pressed.
#[derive(Clone, Copy, PartialEq)]
struct Moving {
    from: usize,
    to: usize,
    y: f64,
}

/// A song coming up: swiped to the left it leaves the queue; dragged by its grip, with a
/// finger or the mouse, it moves up or down, a line marking where it will land.
#[derive(PartialEq)]
struct Movable {
    /// Its place in the queue.
    at: usize,
    song: Song,
    moving: State<Option<Moving>>,
    /// The places songs coming up can move between.
    first: usize,
    last: usize,
}

impl Component for Movable {
    fn render(&self) -> impl IntoElement {
        let (at, first, last) = (self.at, self.first, self.last);
        let mut moving = self.moving;
        let now = *moving.read();
        let held = now.is_some_and(|it| it.from == at);
        // A line where the song would land: above this one when it moves up, below when down.
        let (above, below) = match now {
            Some(it) if it.to == at && it.to < it.from => (true, false),
            Some(it) if it.to == at && it.to > it.from => (false, true),
            _ => (false, false),
        };
        let mark = |on: bool| {
            rect()
                .width(Size::fill())
                .height(Size::px(2.))
                .corner_radius(1.)
                .background(match on {
                    true => color::FOREGROUND,
                    false => Color::TRANSPARENT,
                })
        };
        let grip = rect()
            .width(Size::px(44.))
            .height(Size::px(ROW - 4.))
            .center()
            // The grip keeps its press: dragging it moves the song, it is neither a swipe nor
            // a scroll.
            .on_pointer_down(move |event: Event<PointerEventData>| {
                event.stop_propagation();
                moving.set(Some(Moving {
                    from: at,
                    to: at,
                    y: event.global_location().y,
                }));
            })
            .on_global_pointer_move(move |event: Event<PointerEventData>| {
                // Read first: the borrow must end before the state is written.
                let Some(it) = *moving.peek() else {
                    return;
                };
                if it.from != at {
                    return;
                }
                let rows = ((event.global_location().y - it.y) / ROW as f64).round() as i64;
                let to = (at as i64 + rows).clamp(first as i64, last as i64) as usize;
                ui::swipe::mark();
                if to != it.to {
                    moving.set(Some(Moving { to, ..it }));
                }
            })
            .on_global_pointer_up(move |_: Event<PointerEventData>| {
                let Some(it) = *moving.peek() else {
                    return;
                };
                if it.from != at {
                    return;
                }
                moving.set(None);
                if it.to != it.from {
                    crate::app::move_in_queue(it.from, it.to);
                }
            })
            .child(ui::icon(
                Icon::Grip,
                18.,
                match held {
                    true => color::FOREGROUND,
                    false => color::ON_BACKDROP,
                },
            ));
        let line = rect()
            .width(Size::fill())
            .direction(Direction::Horizontal)
            .content(Content::Flex)
            .cross_align(Alignment::Center)
            .corner_radius(metrics::RADIUS)
            .background(match held {
                true => color::GLASS_SELECTED,
                false => Color::TRANSPARENT,
            })
            .child(
                rect()
                    .width(Size::flex(1.))
                    .on_press(play_at(at))
                    .child(row(at, &self.song, false)),
            )
            .child(grip);
        rect()
            .key(at)
            .width(Size::fill())
            .height(Size::px(ROW))
            .child(mark(above))
            .child(ui::swipe::Swipe {
                content: line.into_element(),
                glyph: Icon::Close,
                label: "Quitar de la cola",
                tint: color::DANGER,
                surface: color::BACKDROP_ROW,
                height: ROW - 4.,
                on_swipe: EventHandler::new(move |_| crate::app::remove_from_queue(at)),
                key: DiffKey::default(),
            })
            .child(mark(below))
    }
}

/// One song of the queue.
fn row(key: usize, song: &Song, current: bool) -> impl IntoElement {
    ui::long_press::hold(rect(), song.clone())
        .key(key)
        .on_secondary_down({
            let song = song.clone();
            move |event: Event<PressEventData>| {
                crate::menu::open(song.clone(), crate::menu::location(&event))
            }
        })
        .height(Size::px(ROW))
        .width(Size::fill())
        .padding((0., 10.))
        .direction(Direction::Horizontal)
        .content(Content::Flex)
        .cross_align(Alignment::Center)
        .spacing(12.)
        .corner_radius(metrics::PLAYER_RADIUS)
        .background(match current {
            true => color::BACKDROP_ROW,
            false => Color::TRANSPARENT,
        })
        .child(Cover::new(song.cover.clone(), THUMB, 4.).apple(crate::artwork::Wanted::song(&song)))
        .child(
            rect()
                .width(Size::flex(1.))
                .spacing(2.)
                .child(
                    ui::line(song.title.clone(), text::BODY, color::FOREGROUND)
                        .font_weight(FontWeight::SEMI_BOLD)
                        .width(Size::fill()),
                )
                .child(
                    ui::line(song.artist.clone(), text::SMALL, color::ON_BACKDROP)
                        .width(Size::fill()),
                ),
        )
        .child(ui::line(
            song.duration.map(library::clock).unwrap_or_default(),
            text::SMALL,
            color::ON_BACKDROP,
        ))
}

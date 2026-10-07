//! The queue panel of the fullscreen player, after Sonora's: the song playing now, then what
//! comes next in play order (shuffled when shuffle is on).

use freya::prelude::*;
use freya::radio::use_radio;

use crate::library::{self, Song};
use crate::state::{AppState, Channel};
use crate::ui::{self, Cover, Icon, color, metrics, text};

/// Height of one queue row, with the line that marks a drop above it.
const ROW: f32 = 58.;
/// Side of a row's cover.
const THUMB: f32 = 40.;

#[derive(PartialEq)]
pub struct QueuePanel;

impl Component for QueuePanel {
    fn render(&self) -> impl IntoElement {
        let now = use_radio::<AppState, Channel>(Channel::Now);
        let state = now.read();
        let current = state.now.queue.get(state.now.index).cloned();
        let upcoming: Vec<Song> = state
            .now
            .queue
            .iter()
            .skip(state.now.index + 1)
            .cloned()
            .collect();
        // Where the first song coming up sits in the queue.
        let first = state.now.index + 1;
        drop(state);
        let count = upcoming.len();
        let moving = use_state(|| None::<Moving>);
        let scroll = use_scroll_controller(ScrollConfig::default);

        rect()
            .width(Size::fill())
            .height(Size::fill())
            .padding((24., 0., 0., 0.))
            .spacing(10.)
            .content(Content::Flex)
            .child(ui::eyebrow("Sonando ahora").color(color::ON_BACKDROP))
            .map(current, |panel, song| panel.child(row(0, &song, true)))
            .child(
                ui::eyebrow("A continuación")
                    .color(color::ON_BACKDROP)
                    .margin((14., 0., 0., 0.)),
            )
            .maybe(count == 0, |panel| {
                panel.child(ui::line(
                    "No hay más canciones en la cola",
                    text::BODY,
                    color::ON_BACKDROP,
                ))
            })
            .child(
                rect().width(Size::fill()).height(Size::flex(1.)).child(
                    // A finger scrolls through `TouchScroll`, which leaves sideways swipes and
                    // the grips' drags to the rows.
                    ui::touch_scroll::TouchScroll {
                        scroll,
                        content: VirtualScrollView::new_with_data_controlled(
                            (upcoming, first, moving),
                            |item, (upcoming, first, moving)| {
                                let index = item.index;
                                Movable {
                                    at: first + index,
                                    song: upcoming[index].clone(),
                                    moving: *moving,
                                    first: *first,
                                    last: first + upcoming.len() - 1,
                                }
                                .into_element()
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
    }
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
    rect()
        .key(key)
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

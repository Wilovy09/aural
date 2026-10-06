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
                // Each song coming up can be dragged onto another to take its place.
                VirtualScrollView::new_with_data((upcoming, first), |index, (upcoming, first)| {
                    Movable {
                        at: first + index,
                        song: upcoming[index].clone(),
                    }
                    .into_element()
                })
                .length(count)
                .item_size(ROW)
                .show_scrollbar(false)
                .width(Size::fill())
                .height(Size::flex(1.)),
            )
    }
}

/// A song coming up: swiped to the left it leaves the queue; dragged by its grip it moves,
/// dropped onto another song's place, which a line marks while it is dragged over.
#[derive(PartialEq)]
struct Movable {
    /// Its place in the queue.
    at: usize,
    song: Song,
}

impl Component for Movable {
    fn render(&self) -> impl IntoElement {
        let mut over = use_state(|| false);
        let at = self.at;
        // The copy that follows the pointer: the row, lifted.
        let lifted = rect()
            .width(Size::px(360.))
            .corner_radius(metrics::RADIUS)
            .background(color::RAISED)
            .shadow((0., 10., 30., 0., Color::from_argb(0x66, 0, 0, 0)))
            .child(row(at, &self.song, false));
        let grip = DragZone::new(
            at,
            rect()
                .width(Size::px(36.))
                .height(Size::px(ROW - 2.))
                .center()
                .child(ui::icon(Icon::Grip, 18., color::ON_BACKDROP))
                .into_element(),
        )
        .drag_element(lifted);
        let line = rect()
            .width(Size::fill())
            .direction(Direction::Horizontal)
            .content(Content::Flex)
            .cross_align(Alignment::Center)
            .child(
                rect()
                    .width(Size::flex(1.))
                    .child(row(at, &self.song, false)),
            )
            // The grip keeps its press: dragging it moves the song, it is not a swipe.
            .child(
                rect()
                    .on_pointer_down(|event: Event<PointerEventData>| event.stop_propagation())
                    .child(grip),
            );
        DropZone::new(
            rect()
                .width(Size::fill())
                .child(
                    rect()
                        .width(Size::fill())
                        .height(Size::px(2.))
                        .corner_radius(1.)
                        .background(match *over.read() {
                            true => color::FOREGROUND,
                            false => Color::TRANSPARENT,
                        }),
                )
                .child(ui::swipe::Swipe {
                    content: line.into_element(),
                    glyph: Icon::Close,
                    label: "Quitar de la cola",
                    tint: color::DANGER,
                    surface: color::BACKDROP_ROW,
                    height: ROW - 2.,
                    on_swipe: EventHandler::new(move |_| crate::app::remove_from_queue(at)),
                    key: DiffKey::default(),
                }),
            move |from: usize| crate::app::move_in_queue(from, at),
        )
        .on_drag_over(move |inside: bool| over.set(inside))
        .key(at)
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

//! The queue panel of the fullscreen player, after Sonora's: the song playing now, then what
//! comes next in play order (shuffled when shuffle is on).

use freya::prelude::*;
use freya::radio::use_radio;

use crate::library::{self, Song};
use crate::state::{AppState, Channel};
use crate::ui::{self, Cover, color, metrics, text};

/// Height of one queue row.
const ROW: f32 = 56.;
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
                VirtualScrollView::new_with_data(upcoming, |index, upcoming| {
                    row(index + 1, &upcoming[index], false).into_element()
                })
                .length(count)
                .item_size(ROW)
                .show_scrollbar(false)
                .width(Size::fill())
                .height(Size::flex(1.)),
            )
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

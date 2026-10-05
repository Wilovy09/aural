//! The playlists or albums of the library as a grid of covers.

use freya::prelude::*;
use freya::radio::use_radio;

use crate::library::Collection;
use crate::state::{AppState, Channel, Load, Page, Spot, Zone};
use crate::ui::{self, Cover, color, metrics, text};

/// Height of one grid row: the cover, two lines of text and the gap.
pub(crate) const ROW: f32 = metrics::CARD + 60.;

#[derive(PartialEq)]
pub struct Cards;

impl Component for Cards {
    fn render(&self) -> impl IntoElement {
        let navigation = use_radio::<AppState, Channel>(Channel::Navigation);
        let library = use_radio::<AppState, Channel>(Channel::Library);
        let state = navigation.read();
        let title = match state.page {
            Page::Albums => "Álbumes",
            _ => "Playlists",
        };
        let focused = match state.focus.content {
            Spot::Card(card) if state.focus.zone == Zone::Content => Some(card),
            _ => None,
        };
        let page = state.page.clone();
        drop(state);

        let load = library.read().library.clone();
        let cards: Vec<Collection> = match (&load, &page) {
            (Load::Ready(library), Page::Albums) => library.albums.clone(),
            (Load::Ready(library), _) => library.playlists.clone(),
            _ => Vec::new(),
        };
        let columns = ui::columns();
        let rows = cards.len().div_ceil(columns);
        let (_, height) = ui::viewport();
        let view = height - metrics::PLAYER_BAR - metrics::INSET * 3.;
        let scroll = ui::use_follow(ui::follow_offset(
            focused.map_or(0, |card| card / columns),
            ROW,
            view,
        ));
        let status = match &load {
            Load::Loading | Load::Idle => Some("Cargando…".to_string()),
            Load::Failed(error) => Some(error.clone()),
            Load::Ready(_) if cards.is_empty() => Some("Nada por aquí todavía".to_string()),
            Load::Ready(_) => None,
        };

        rect()
            .expanded()
            .padding((metrics::INSET, metrics::INSET, 0., metrics::INSET))
            .spacing(16.)
            .content(Content::Flex)
            .child(
                ui::line(title, text::TITLE, color::FOREGROUND).font_weight(FontWeight::SEMI_BOLD),
            )
            .map(status, |page, status| {
                page.child(ui::line(status, text::BODY, color::MUTED_FOREGROUND))
            })
            .child(
                // The data is what the rows depend on: the list compares it, not the builder,
                // to know when to redraw.
                VirtualScrollView::new_with_data_controlled(
                    (cards, focused, columns),
                    |row, (cards, focused, columns)| {
                        let (focused, columns) = (*focused, *columns);
                        rect()
                            .key(row)
                            .height(Size::px(ROW))
                            .direction(Direction::Horizontal)
                            .spacing(metrics::CARD_GAP)
                            .children((0..columns).filter_map(|column| {
                                let index = row * columns + column;
                                let card = cards.get(index)?;
                                Some(tile(index, card, focused == Some(index)).into_element())
                            }))
                            .into()
                    },
                    scroll,
                )
                .length(rows)
                .item_size(ROW)
                .width(Size::fill())
                .height(Size::flex(1.)),
            )
    }
}

/// One playlist or album: its cover, title and subtitle.
pub(crate) fn tile(index: usize, card: &Collection, focused: bool) -> impl IntoElement {
    rect()
        .key(index)
        .width(Size::px(metrics::CARD))
        .spacing(8.)
        .child(
            rect()
                .corner_radius(metrics::RADIUS + 3.)
                .padding(3.)
                .border(ui::focus_border(focused))
                .margin(-3.)
                .child(Cover::new(
                    card.cover.clone(),
                    metrics::CARD - 6.,
                    metrics::RADIUS,
                )),
        )
        .child(
            ui::line(card.title.clone(), text::BODY, color::FOREGROUND)
                .font_weight(FontWeight::SEMI_BOLD)
                .width(Size::fill()),
        )
        .child(
            ui::line(card.subtitle.clone(), text::SMALL, color::MUTED_FOREGROUND)
                .width(Size::fill()),
        )
}

//! The left sidebar: the app's name, the library sections and the account at the bottom.

use freya::prelude::*;
use freya::radio::use_radio;

use crate::nav::Target;
use crate::state::{AppState, Auth, Channel, NAV, Page, Zone};
use crate::ui::{self, Icon, color, metrics, text};

#[derive(PartialEq)]
pub struct Sidebar;

impl Component for Sidebar {
    fn render(&self) -> impl IntoElement {
        let navigation = use_radio::<AppState, Channel>(Channel::Navigation);
        let auth = use_radio::<AppState, Channel>(Channel::Auth);
        let state = navigation.read();
        let focused = state.focus.zone == Zone::Sidebar;
        let current = current(&state);

        let account = match &auth.read().auth {
            Auth::SignedIn(Some(account)) => Some(account.name.clone()),
            _ => None,
        };

        rect()
            .width(Size::px(metrics::SIDEBAR))
            .height(Size::fill())
            .background(color::SIDEBAR)
            .border(
                Border::new()
                    .fill(color::SIDEBAR_BORDER)
                    .width(BorderWidth {
                        right: 1.,
                        ..Default::default()
                    }),
            )
            .padding(12.)
            .spacing(4.)
            .content(Content::Flex)
            .child(
                rect()
                    .padding((4., 12., 16., 12.))
                    .direction(Direction::Horizontal)
                    .cross_align(Alignment::Center)
                    .spacing(8.)
                    .child(ui::logo(28.))
                    .child(
                        ui::line("Aural", text::LARGE, color::FOREGROUND)
                            .font_weight(FontWeight::BOLD),
                    ),
            )
            .child(ui::eyebrow("Tu biblioteca").margin((0., 12., 4., 12.)))
            .children(NAV.iter().enumerate().map(|(index, page)| {
                let (glyph, name) = entry(page);
                let active = *page == current;
                let ring = ui::ring(focused && state.focus.sidebar == index);
                rect()
                    .key(index)
                    .height(Size::px(metrics::CONTROL + 4.))
                    .width(Size::fill())
                    .padding((0., 12.))
                    .direction(Direction::Horizontal)
                    .cross_align(Alignment::Center)
                    .spacing(10.)
                    .corner_radius(metrics::RADIUS)
                    .background(match active {
                        true => color::SIDEBAR_ACCENT,
                        false => Color::TRANSPARENT,
                    })
                    .border(ui::focus_border(ring))
                    .on_press(ui::tap(Target::Sidebar(index)))
                    .child(ui::icon(glyph, metrics::ICON, ink(active || ring)))
                    .child(ui::line(name, text::BODY, ink(active || ring)))
                    .into()
            }))
            .child(rect().height(Size::flex(1.)))
            .map(account, |sidebar, name| {
                sidebar.child(
                    rect()
                        .padding((8., 12.))
                        .direction(Direction::Horizontal)
                        .cross_align(Alignment::Center)
                        .spacing(8.)
                        .child(ui::icon(Icon::User, metrics::ICON, color::MUTED_FOREGROUND))
                        .child(ui::line(name, text::SMALL, color::MUTED_FOREGROUND)),
                )
            })
    }
}

/// The section a page belongs to: a playlist or artist opened from one stays under it.
pub(crate) fn current(state: &AppState) -> Page {
    match &state.page {
        Page::Detail(_) | Page::Artist(_) => {
            state.history.first().cloned().unwrap_or(Page::Playlists)
        }
        page => page.clone(),
    }
}

pub(crate) fn entry(page: &Page) -> (Icon, &'static str) {
    match page {
        Page::Search => (Icon::Search, "Buscar"),
        Page::Songs => (Icon::HeartFilled, "Me gusta"),
        Page::Playlists => (Icon::Queue, "Playlists"),
        Page::Albums => (Icon::Album, "Álbumes"),
        Page::Account | Page::Detail(_) | Page::Artist(_) => (Icon::Settings, "Cuenta"),
    }
}

fn ink(strong: bool) -> Color {
    match strong {
        true => color::FOREGROUND,
        false => color::MUTED_FOREGROUND,
    }
}

//! The sidebar: the app's name, where to go (home and search, then your music), and the
//! account at the foot as an avatar. It shares the page's background; the open section is a
//! soft fill, the D-pad's one is lit with a bar of the cover's light.

use freya::prelude::*;
use freya::radio::use_radio;

use crate::nav::Target;
use crate::state::{AppState, Auth, Channel, NAV, Page, Zone};
use crate::ui::{self, Icon, color, metrics, text};

/// Where the account sits in [`NAV`]: at the foot, apart from the sections.
const ACCOUNT: usize = 5;
/// The entries above the gap: home and search.
const FIND: usize = 2;

#[derive(PartialEq)]
pub struct Sidebar;

impl Component for Sidebar {
    fn render(&self) -> impl IntoElement {
        let navigation = use_radio::<AppState, Channel>(Channel::Navigation);
        let auth = use_radio::<AppState, Channel>(Channel::Auth);
        let now = use_radio::<AppState, Channel>(Channel::Now);
        let state = navigation.read();
        let focused = (state.focus.zone == Zone::Sidebar).then_some(state.focus.sidebar);
        let open = current(&state);
        drop(state);
        let light = now.read().now.light.unwrap_or(color::LIGHT);
        let (account, photo) = match &auth.read().auth {
            Auth::SignedIn(Some(account)) => (account.name.clone(), account.photo.clone()),
            _ => ("Cuenta".to_owned(), None),
        };

        let item = |index: usize| {
            let page = &NAV[index];
            let (glyph, name) = entry(page);
            Entry {
                index,
                glyph,
                name: name.to_owned(),
                active: *page == open,
                focused: focused == Some(index),
                light,
            }
        };

        rect()
            .width(Size::px(metrics::SIDEBAR))
            .height(Size::fill())
            .background(color::SIDEBAR)
            .padding((20., 12., 12., 12.))
            .content(Content::Flex)
            .child(
                rect()
                    .padding((0., 10., 24., 10.))
                    .direction(Direction::Horizontal)
                    .cross_align(Alignment::Center)
                    .spacing(10.)
                    .child(ui::logo(28.))
                    .child(
                        ui::line("Aural", text::LARGE, color::FOREGROUND)
                            .font_weight(FontWeight::BOLD),
                    ),
            )
            .child(
                rect()
                    .spacing(2.)
                    .children((0..FIND).map(|at| item(at).into_element())),
            )
            .child(rect().height(Size::px(20.)))
            .child(
                rect()
                    .spacing(2.)
                    .children((FIND..ACCOUNT).map(|at| item(at).into_element())),
            )
            .child(rect().height(Size::flex(1.)))
            .child(Avatar {
                name: account,
                photo,
                active: open == Page::Account,
                focused: focused == Some(ACCOUNT),
                light,
            })
    }
}

/// One section of the sidebar.
#[derive(PartialEq)]
struct Entry {
    index: usize,
    glyph: Icon,
    name: String,
    active: bool,
    focused: bool,
    light: Color,
}

impl Component for Entry {
    fn render(&self) -> impl IntoElement {
        let mut hovered = use_state(|| false);
        let lit = ui::ring(self.focused);
        let strong = self.active || lit;
        rect()
            .key(self.index)
            .height(Size::px(40.))
            .width(Size::fill())
            .direction(Direction::Horizontal)
            .content(Content::Flex)
            .cross_align(Alignment::Center)
            .corner_radius(metrics::RADIUS_SM)
            .overflow(Overflow::Clip)
            .background(match (lit, self.active, *hovered.read()) {
                (true, _, _) => color::FOCUS_FILL,
                (false, true, _) => color::SIDEBAR_ACCENT,
                (false, false, true) => color::HOVER,
                (false, false, false) => Color::TRANSPARENT,
            })
            .on_pointer_enter(move |_| hovered.set(true))
            .on_pointer_leave(move |_| hovered.set(false))
            .on_press(ui::tap(Target::Sidebar(self.index)))
            .child(bar(lit, self.light))
            .child(
                rect()
                    .width(Size::flex(1.))
                    .padding((0., 10.))
                    .direction(Direction::Horizontal)
                    .cross_align(Alignment::Center)
                    .spacing(12.)
                    .child(ui::icon(self.glyph, 18., ink(strong)))
                    .child(
                        ui::line(self.name.clone(), text::BODY, ink(strong)).font_weight(
                            match strong {
                                true => FontWeight::SEMI_BOLD,
                                false => FontWeight::MEDIUM,
                            },
                        ),
                    ),
            )
    }
}

/// The account at the sidebar's foot: an initial on a disc, and the name.
#[derive(PartialEq)]
struct Avatar {
    name: String,
    photo: Option<String>,
    active: bool,
    focused: bool,
    light: Color,
}

impl Component for Avatar {
    fn render(&self) -> impl IntoElement {
        let mut hovered = use_state(|| false);
        let lit = ui::ring(self.focused);
        rect()
            .height(Size::px(52.))
            .width(Size::fill())
            .direction(Direction::Horizontal)
            .content(Content::Flex)
            .cross_align(Alignment::Center)
            .corner_radius(metrics::RADIUS_SM)
            .overflow(Overflow::Clip)
            .background(match (lit, self.active, *hovered.read()) {
                (true, _, _) => color::FOCUS_FILL,
                (false, true, _) => color::SIDEBAR_ACCENT,
                (false, false, true) => color::HOVER,
                (false, false, false) => Color::TRANSPARENT,
            })
            .on_pointer_enter(move |_| hovered.set(true))
            .on_pointer_leave(move |_| hovered.set(false))
            .on_press(ui::tap(Target::Sidebar(ACCOUNT)))
            .child(bar(lit, self.light))
            .child(
                rect()
                    .width(Size::flex(1.))
                    .padding((0., 8.))
                    .direction(Direction::Horizontal)
                    .content(Content::Flex)
                    .cross_align(Alignment::Center)
                    .spacing(10.)
                    .child(crate::screens::account::avatar(
                        self.photo.clone(),
                        &self.name,
                        32.,
                        self.light,
                    ))
                    .child(
                        rect()
                            .width(Size::flex(1.))
                            .child(
                                ui::line(self.name.clone(), text::BODY, color::FOREGROUND)
                                    .font_weight(FontWeight::MEDIUM)
                                    .width(Size::fill()),
                            )
                            .child(ui::line("Cuenta", text::SMALL, color::FAINT)),
                    ),
            )
    }
}

/// The bar of light at the left edge of what the D-pad is on.
pub(crate) fn bar(lit: bool, light: Color) -> impl IntoElement {
    rect()
        .width(Size::px(3.))
        .height(Size::percent(60.))
        .corner_radius(2.)
        .background(match lit {
            true => light,
            false => Color::TRANSPARENT,
        })
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
        Page::Home => (Icon::Home, "Inicio"),
        Page::Search => (Icon::Search, "Buscar"),
        Page::Songs => (Icon::HeartFilled, "Me gusta"),
        Page::Playlists => (Icon::Queue, "Playlists"),
        Page::Albums => (Icon::Album, "Álbumes"),
        Page::Devices => (Icon::Cast, "Dispositivos"),
        Page::Account | Page::Detail(_) | Page::Artist(_) => (Icon::Settings, "Cuenta"),
    }
}

fn ink(strong: bool) -> Color {
    match strong {
        true => color::FOREGROUND,
        false => color::MUTED_FOREGROUND,
    }
}

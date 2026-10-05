//! The bar over the page on a computer: the search field in the middle (⌘K or Ctrl K from
//! anywhere). A TV has its remote and a phone its tabs, so neither shows it.

use freya::prelude::*;
use freya::radio::{use_radio, use_radio_station};

use crate::state::{AppState, Channel, Page};
use crate::ui::form_input::FormInput;
use crate::ui::{self, Icon};

/// Height of the bar.
pub const HEIGHT: f32 = 64.;

/// Whether this device shows the bar.
pub fn shown() -> bool {
    !cfg!(target_os = "android") && !ui::compact()
}

#[derive(PartialEq)]
pub struct TopBar;

impl Component for TopBar {
    fn render(&self) -> impl IntoElement {
        rect()
            .width(Size::fill())
            .height(Size::px(HEIGHT))
            .padding((0., ui::inset()))
            .center()
            .child(Search)
    }
}

thread_local! {
    /// The field's accessibility id, for ⌘K and Back to hand the keyboard over.
    static FIELD: std::cell::Cell<Option<AccessibilityId>> = const { std::cell::Cell::new(None) };
}

/// Gives the bar's field the keyboard, when the bar is on screen.
pub fn edit() -> bool {
    match (shown(), FIELD.get()) {
        (true, Some(field)) => {
            field.request_focus();
            true
        }
        _ => false,
    }
}

/// Takes the keyboard away from the bar's field.
pub fn leave() {
    if let Some(field) = FIELD.get() {
        field.request_unfocus();
    }
}

/// The one search field on a computer: type and press Enter, and the search page shows what
/// it found. It keeps the search's words, and empties when the search is cleared.
#[derive(PartialEq)]
struct Search;

impl Component for Search {
    fn render(&self) -> impl IntoElement {
        let mut station = use_radio_station::<AppState, Channel>();
        let search = use_radio::<AppState, Channel>(Channel::Search);
        let field = use_a11y();
        FIELD.set(Some(field));
        let focus = use_focus(field);
        // Keys go to the field while it has the keyboard, not to the navigation.
        use_side_effect(move || {
            let typing = focus().is_focused();
            if station.peek().typing != typing {
                station.write_channel(Channel::Navigation).typing = typing;
            }
            // Typing in the field is searching: the search page opens, lit in the sidebar.
            if typing && station.peek().page != Page::Search {
                let mut state = station.write_channel(Channel::Navigation);
                crate::nav::go(&mut state, Page::Search);
                state.focus.sidebar = 1;
            }
        });
        let mut query = use_state(|| station.peek().search.query.clone());
        let stored = use_reactive(&search.read().search.query.clone());
        use_side_effect(move || {
            let stored = stored.read().clone();
            if *query.peek() != stored {
                query.set(stored);
            }
        });
        // The whole bar, for when it is typed in.
        let (width, _) = ui::viewport();
        let room = (width - ui::metrics::SIDEBAR - ui::inset() * 2.).max(440.);
        let shortcut = match cfg!(target_os = "macos") {
            true => "⌘K",
            false => "Ctrl K",
        };

        FormInput::new(query)
            .a11y_id(field)
            .placeholder(format!("¿Qué quieres escuchar?   {shortcut}"))
            .left_icon(Icon::Search.bytes())
            .grow(440.)
            .corner_radius(99.)
            .width(Size::px(room))
            .min_width(Size::px(40.))
            .on_change(move |typed: String| {
                leave();
                if typed.trim().is_empty() {
                    return;
                }
                {
                    let mut state = station.write_channel(Channel::Navigation);
                    if state.page != Page::Search {
                        crate::nav::go(&mut state, Page::Search);
                    }
                }
                crate::screens::search::run(station, typed);
            })
    }
}

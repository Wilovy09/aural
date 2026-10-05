//! The phone's bottom bar, YouTube Music's: the library sections as icons over their names,
//! the open one bright.

use freya::prelude::*;
use freya::radio::use_radio;

use crate::chrome::sidebar::{current, entry};
use crate::nav::Target;
use crate::state::{AppState, Channel, NAV, Zone};
use crate::ui::{self, color, text};

/// Height of the bar.
pub const HEIGHT: f32 = 64.;

#[derive(PartialEq)]
pub struct BottomBar;

impl Component for BottomBar {
    fn render(&self) -> impl IntoElement {
        let navigation = use_radio::<AppState, Channel>(Channel::Navigation);
        let state = navigation.read();
        let open = current(&state);
        let focused = (state.focus.zone == Zone::Sidebar).then_some(state.focus.sidebar);
        drop(state);

        rect()
            .width(Size::fill())
            .height(Size::px(HEIGHT))
            .background(color::SECONDARY)
            .direction(Direction::Horizontal)
            .content(Content::Flex)
            .children(NAV.iter().enumerate().map(|(index, page)| {
                let (glyph, name) = entry(page);
                let active = *page == open;
                let ink = match active {
                    true => color::FOREGROUND,
                    false => color::MUTED_FOREGROUND,
                };
                rect()
                    .key(index)
                    .width(Size::flex(1.))
                    .height(Size::fill())
                    .center()
                    .spacing(4.)
                    .border(ui::focus_border(focused == Some(index)))
                    .on_press(ui::tap(Target::Sidebar(index)))
                    .child(ui::icon(glyph, 22., ink))
                    .child(ui::line(name, text::TINY, ink).font_weight(match active {
                        true => FontWeight::SEMI_BOLD,
                        false => FontWeight::NORMAL,
                    }))
                    .into()
            }))
    }
}

//! The phone's bottom bar, YouTube Music's: the library sections as icons over their names,
//! the open one bright.

use freya::prelude::*;
use freya::radio::use_radio;

use crate::chrome::sidebar::current;
use crate::nav::Target;
use crate::state::{AppState, Channel, NAV, Zone};
use crate::ui::{self, Icon, color, metrics, text};

/// Height of the bar.
pub const HEIGHT: f32 = 64.;

/// The phone's tabs: where in [`NAV`] each goes, and how it is called. Playlists stands for
/// the whole library, whose page switches to albums too.
const TABS: [(usize, Icon, &str); 5] = [
    (0, Icon::Home, "Inicio"),
    (1, Icon::Search, "Buscar"),
    (2, Icon::HeartFilled, "Me gusta"),
    (3, Icon::Library, "Biblioteca"),
    (5, Icon::Settings, "Cuenta"),
];

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
            .background(color::BACKGROUND)
            .border(
                Border::new()
                    .fill(color::SIDEBAR_BORDER)
                    .width(BorderWidth {
                        top: 1.,
                        ..Default::default()
                    }),
            )
            .direction(Direction::Horizontal)
            .content(Content::Flex)
            .children(TABS.iter().map(|(index, glyph, name)| {
                let page = &NAV[*index];
                let active = *page == open || (*index == 3 && open == crate::state::Page::Albums);
                let ink = match active {
                    true => color::FOREGROUND,
                    false => color::MUTED_FOREGROUND,
                };
                rect()
                    .key(*index)
                    .width(Size::flex(1.))
                    .height(Size::fill())
                    .center()
                    .spacing(4.)
                    .corner_radius(metrics::RADIUS)
                    .background(match ui::ring(focused == Some(*index)) {
                        true => color::FOCUS_FILL,
                        false => Color::TRANSPARENT,
                    })
                    .on_press(ui::tap(Target::Sidebar(*index)))
                    .child(ui::icon(*glyph, 22., ink))
                    .child(ui::line(*name, text::TINY, ink).font_weight(match active {
                        true => FontWeight::SEMI_BOLD,
                        false => FontWeight::NORMAL,
                    }))
                    .into()
            }))
    }
}

//! A notice over the player for a moment, Spotify's "Added to queue": a white bar with what
//! happened and, when it can, "Abrir" to see the queue.

use freya::prelude::*;
use freya::radio::use_radio;

use crate::chrome::{bottom_bar, player_bar};
use crate::state::{AppState, Channel};
use crate::ui::{self, color, metrics, text};

#[derive(PartialEq)]
pub struct Notice;

impl Component for Notice {
    fn render(&self) -> impl IntoElement {
        let toast = use_radio::<AppState, Channel>(Channel::Toast);
        let navigation = use_radio::<AppState, Channel>(Channel::Navigation);
        let Some(notice) = toast.read().toast.clone() else {
            return rect().into_element();
        };
        let fullscreen = navigation.read().fullscreen;
        let compact = ui::compact();
        let (width, height) = ui::viewport();
        let (_, safe_bottom) = ui::safe();
        // Just over the player: the mini player and tabs on a phone, the island elsewhere.
        let above = match (fullscreen, compact) {
            (true, _) => 24. + safe_bottom,
            (false, true) => player_bar::MINI + 2. + bottom_bar::HEIGHT + safe_bottom + 8.,
            (false, false) => metrics::PLAYER_BAR + 8.,
        };
        let wide = match compact {
            true => width - 32.,
            false => 420.,
        };
        // Only the bar is placed over the app, so nothing else stops a tap.
        rect()
            .position(
                Position::new_absolute()
                    .left((width - wide) / 2.)
                    .top(height - above - 52.),
            )
            .layer(Layer::Overlay)
            .width(Size::px(wide))
            .height(Size::px(52.))
            .padding((0., 8., 0., 18.))
            .direction(Direction::Horizontal)
            .content(Content::Flex)
            .cross_align(Alignment::Center)
            .corner_radius(metrics::RADIUS_SM)
            .background(color::PRIMARY)
            .shadow((0., 8., 24., 0., Color::from_argb(0x66, 0, 0, 0)))
            .child(
                rect().width(Size::flex(1.)).child(
                    ui::line(notice.text, text::BODY + 1., color::PRIMARY_FOREGROUND)
                        .font_weight(FontWeight::MEDIUM),
                ),
            )
            .maybe(notice.open_queue, |bar| {
                bar.child(
                    rect()
                        .height(Size::px(40.))
                        .padding((0., 12.))
                        .center()
                        .corner_radius(metrics::RADIUS_SM)
                        .on_press(|event: Event<PressEventData>| {
                            event.stop_propagation();
                            crate::app::open_queue();
                        })
                        .child(
                            ui::line("Abrir", text::BODY + 1., color::FOCUS_ON_PRIMARY)
                                .font_weight(FontWeight::BOLD),
                        ),
                )
            })
            .into_element()
    }
}

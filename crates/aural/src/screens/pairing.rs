//! The code another device asks to pair with, shown over everything on this one.

use freya::prelude::*;
use freya::radio::use_radio;

use crate::state::{AppState, Channel};
use crate::ui::{self, Icon, color, text};

#[derive(PartialEq)]
pub struct PairingCode;

impl Component for PairingCode {
    fn render(&self) -> impl IntoElement {
        let connect = use_radio::<AppState, Channel>(Channel::Connect);
        let Some(pairing) = connect.read().connect.pairing.clone() else {
            return rect().into_element();
        };
        let (width, height) = ui::viewport();
        rect()
            .position(Position::new_absolute().top(0.).left(0.))
            .layer(Layer::Overlay)
            .width(Size::px(width))
            .height(Size::px(height))
            .background(Color::from_argb(0xcc, 0, 0, 0))
            .center()
            .child(
                rect()
                    .padding(32.)
                    .spacing(14.)
                    .corner_radius(24.)
                    .background(color::SECONDARY)
                    .border(Border::new().fill(color::BORDER).width(1.))
                    .cross_align(Alignment::Center)
                    .child(ui::icon(Icon::Cast, 36., color::FOREGROUND))
                    .child(
                        ui::line(
                            format!("{} quiere reproducir aquí", pairing.from),
                            text::LARGE,
                            color::FOREGROUND,
                        )
                        .font_weight(FontWeight::SEMI_BOLD),
                    )
                    .child(ui::line(
                        "Escribe este código en ese dispositivo",
                        text::BODY,
                        color::MUTED_FOREGROUND,
                    ))
                    .child(
                        label()
                            .text(
                                pairing
                                    .code
                                    .chars()
                                    .map(String::from)
                                    .collect::<Vec<_>>()
                                    .join(" "),
                            )
                            .font_size(64.)
                            .font_weight(FontWeight::BOLD)
                            .color(color::FOREGROUND),
                    ),
            )
            .into_element()
    }
}

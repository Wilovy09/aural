//! "Nueva playlist", as YouTube Music asks for one: a title, a description, and who can see
//! it (public, unlisted or private), over the page with the rest dimmed.

use std::cell::Cell;

use freya::prelude::*;
use freya::radio::use_radio_station;

use crate::library::Privacy;
use crate::state::{AppState, Channel};
use crate::ui::form_input::FormInput;
use crate::ui::{self, Icon, color, text};

/// The widest the dialog grows.
const WIDTH: f32 = 560.;

thread_local! {
    /// Whether the dialog shows, held by [`NewPlaylist`] once it is mounted.
    static OPEN: Cell<Option<State<bool>>> = const { Cell::new(None) };
}

/// Shows the dialog.
pub fn open() {
    if let Some(mut open) = OPEN.get() {
        open.set(true);
    }
}

/// The dialog, while it shows. Mounted once, at the app's root.
#[derive(PartialEq)]
pub struct NewPlaylist;

impl Component for NewPlaylist {
    fn render(&self) -> impl IntoElement {
        let mut open = use_state(|| false);
        use_hook(|| OPEN.set(Some(open)));
        let mut title = use_state(String::new);
        let mut description = use_state(String::new);
        let mut privacy = use_state(Privacy::default);
        let mut choosing = use_state(|| false);
        // Keys go to the fields while the dialog shows, not to the shortcuts.
        let mut station = use_radio_station::<AppState, Channel>();
        use_side_effect(move || {
            let showing = *open.read();
            if station.peek().typing != showing {
                station.write_channel(Channel::Navigation).typing = showing;
            }
        });
        if !*open.read() {
            return rect().into_element();
        }

        // Closing forgets what was typed, so the next playlist starts blank.
        let mut close = move || {
            open.set(false);
            title.set(String::new());
            description.set(String::new());
            privacy.set(Privacy::default());
            choosing.set(false);
        };
        let ready = !title.read().trim().is_empty();
        let chosen = *privacy.read();
        let (width, height) = ui::viewport();

        rect()
            .layer(Layer::Overlay)
            .position(Position::new_global().left(0.).top(0.))
            .width(Size::px(width))
            .height(Size::px(height))
            .background(Color::from_argb(0xa0, 0, 0, 0))
            .center()
            .on_pointer_down(move |event: Event<PointerEventData>| {
                event.stop_propagation();
                close();
            })
            .on_global_key_down(move |event: Event<KeyboardEventData>| {
                if event.key == Key::Named(NamedKey::Escape) {
                    close();
                }
            })
            .child(
                rect()
                    .width(Size::px(WIDTH.min(width - 32.)))
                    .padding(28.)
                    .spacing(22.)
                    .corner_radius(16.)
                    .background(color::RAISED)
                    .border(Border::new().fill(color::BORDER).width(1.))
                    .shadow((0., 10., 30., 0., Color::from_argb(0x90, 0, 0, 0)))
                    .on_pointer_down(|event: Event<PointerEventData>| event.stop_propagation())
                    .child(
                        ui::line("Nueva playlist", text::TITLE, color::FOREGROUND)
                            .font_weight(FontWeight::BOLD),
                    )
                    .child(
                        FormInput::new(title)
                            .placeholder("Título")
                            .auto_focus(true)
                            .background(color::MUTED)
                            .width(Size::fill()),
                    )
                    .child(
                        FormInput::new(description)
                            .placeholder("Descripción")
                            .background(color::MUTED)
                            .width(Size::fill()),
                    )
                    .child(
                        rect()
                            .width(Size::fill())
                            .spacing(8.)
                            .child(ui::line("Privacidad", text::SMALL, color::MUTED_FOREGROUND))
                            .child(
                                option(chosen, false, false)
                                    .on_press(move |_| choosing.toggle())
                                    .child(ui::icon(
                                        Icon::ChevronDown,
                                        18.,
                                        color::MUTED_FOREGROUND,
                                    )),
                            )
                            .maybe(*choosing.read(), |field| {
                                field.child(
                                    rect()
                                        .width(Size::fill())
                                        .corner_radius(10.)
                                        .background(color::MUTED)
                                        .padding(4.)
                                        .children(Privacy::ALL.into_iter().map(|each| {
                                            option(each, true, each == chosen)
                                                .key(each.name())
                                                .on_press(move |_| {
                                                    privacy.set(each);
                                                    choosing.set(false);
                                                })
                                                .into_element()
                                        })),
                                )
                            }),
                    )
                    .child(
                        rect()
                            .width(Size::fill())
                            .direction(Direction::Horizontal)
                            .main_align(Alignment::End)
                            .cross_align(Alignment::Center)
                            .spacing(12.)
                            .child(
                                rect()
                                    .height(Size::px(42.))
                                    .padding((0., 18.))
                                    .center()
                                    .corner_radius(21.)
                                    .on_press(move |_| close())
                                    .child(ui::line("Cancelar", text::BODY, color::FOREGROUND)),
                            )
                            .child(
                                rect()
                                    .height(Size::px(42.))
                                    .padding((0., 22.))
                                    .center()
                                    .corner_radius(21.)
                                    .background(match ready {
                                        true => color::PRIMARY,
                                        false => color::MUTED,
                                    })
                                    .on_press(move |_| {
                                        let typed = title.read().trim().to_owned();
                                        if typed.is_empty() {
                                            return;
                                        }
                                        crate::app::create_playlist(
                                            typed,
                                            description.read().trim().to_owned(),
                                            *privacy.read(),
                                        );
                                        close();
                                    })
                                    .child(
                                        ui::line(
                                            "Crear",
                                            text::BODY,
                                            match ready {
                                                true => color::PRIMARY_FOREGROUND,
                                                false => color::MUTED_FOREGROUND,
                                            },
                                        )
                                        .font_weight(FontWeight::SEMI_BOLD),
                                    ),
                            ),
                    ),
            )
            .into_element()
    }
}

/// A privacy setting as a row: its icon and name, and when `hinted` what it means. The one
/// chosen is lit in the list.
fn option(privacy: Privacy, hinted: bool, lit: bool) -> Rect {
    let glyph = match privacy {
        Privacy::Public => Icon::Globe,
        Privacy::Unlisted => Icon::Link,
        Privacy::Private => Icon::Lock,
    };
    rect()
        .width(Size::fill())
        .padding((10., 12.))
        .corner_radius(8.)
        .direction(Direction::Horizontal)
        .content(Content::Flex)
        .cross_align(Alignment::Center)
        .spacing(14.)
        .background(match lit {
            true => color::HOVER,
            false => Color::TRANSPARENT,
        })
        .child(ui::icon(glyph, 22., color::FOREGROUND))
        .child(
            rect()
                .width(Size::flex(1.))
                .spacing(2.)
                .child(
                    ui::line(privacy.name(), text::BODY, color::FOREGROUND)
                        .font_weight(FontWeight::SEMI_BOLD),
                )
                .maybe(hinted, |names| {
                    names.child(ui::line(
                        privacy.hint(),
                        text::SMALL,
                        color::MUTED_FOREGROUND,
                    ))
                }),
        )
}

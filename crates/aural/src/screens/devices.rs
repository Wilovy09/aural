//! The devices page of Aural Connect: this device and the other Aurals on the network, the one
//! that plays marked. Choosing another plays there; a device met for the first time asks for
//! the code it shows, typed in the field on top, which otherwise takes an address by hand.

use std::cell::Cell;

use freya::prelude::*;
use freya::radio::{use_radio, use_radio_station};

use crate::connect::{Device, Link};
use crate::nav::Target;
use crate::state::{AppState, Channel, Spot, Zone};
use crate::ui::{self, Icon, color, metrics, text};

/// The port an address typed without one goes to.
const PORT: u16 = 47821;

thread_local! {
    static FIELD: Cell<Option<AccessibilityId>> = const { Cell::new(None) };
}

/// Gives the field the keyboard.
pub fn edit() {
    if let Some(field) = FIELD.get() {
        field.request_focus();
    }
}

/// Takes the keyboard away from the field.
pub fn leave() {
    if let Some(field) = FIELD.get() {
        field.request_unfocus();
    }
}

#[derive(PartialEq)]
pub struct Devices;

impl Component for Devices {
    fn render(&self) -> impl IntoElement {
        let mut station = use_radio_station::<AppState, Channel>();
        let navigation = use_radio::<AppState, Channel>(Channel::Navigation);
        let connect = use_radio::<AppState, Channel>(Channel::Connect);

        let field = use_a11y();
        FIELD.set(Some(field));
        let focus = use_focus(field);
        use_side_effect(move || {
            let typing = focus().is_focused();
            if station.peek().typing != typing {
                station.write_channel(Channel::Navigation).typing = typing;
            }
        });
        let mut entry = use_state(String::new);

        let state = navigation.read();
        let spot = match state.focus.zone == Zone::Content && !ui::touch() {
            true => Some(state.focus.content),
            false => None,
        };
        drop(state);
        let connect = connect.read().connect.clone();
        let asking = match &connect.link {
            Link::NeedCode(name) => Some(name.clone()),
            _ => None,
        };
        let status = match &connect.link {
            Link::Idle => "Reproduciendo en este dispositivo".to_owned(),
            Link::Connecting(name) => format!("Conectando con {name}…"),
            Link::NeedCode(name) => format!("Escribe el código que muestra {name}"),
            Link::Connected(name) => format!("Reproduciendo en {name}"),
            Link::Failed(error) => error.clone(),
        };

        let input = Input::new(entry)
            .a11y_id(field)
            .placeholder(match &asking {
                Some(_) => "Código de 4 dígitos",
                None => "Conectar por dirección (192.168.1.20)",
            })
            .width(Size::fill())
            .flat()
            .theme_colors(InputColorsThemePartial {
                background: Some(Preference::Specific(Color::TRANSPARENT)),
                focus_background: Some(Preference::Specific(Color::TRANSPARENT)),
                color: Some(Preference::Specific(color::FOREGROUND)),
                placeholder_color: Some(Preference::Specific(color::MUTED_FOREGROUND)),
                border_fill: Some(Preference::Specific(Color::TRANSPARENT)),
                focus_border_fill: Some(Preference::Specific(Color::TRANSPARENT)),
            })
            .on_submit(move |typed: String| submit(typed, &mut entry))
            // Android's keyboards send Enter as a "\n" character.
            .on_pre_key_down(move |event: Event<KeyboardEventData>| match &event.key {
                Key::Character(typed) if typed == "\n" || typed == "\r" => {
                    event.stop_propagation();
                    event.prevent_default();
                    submit(entry.peek().clone(), &mut entry);
                    false
                }
                Key::Named(NamedKey::Enter | NamedKey::Escape | NamedKey::Shift) => true,
                Key::Named(NamedKey::Tab) => false,
                _ => {
                    event.stop_propagation();
                    event.prevent_default();
                    true
                }
            });
        let field_border = match (focus().is_focused(), spot == Some(Spot::Action(0))) {
            (true, _) => Border::new().fill(color::FOCUS_ON_PRIMARY).width(2.),
            (false, ring) => ui::focus_border(ring),
        };

        let here = !matches!(connect.link, Link::Connected(_));
        let me = crate::connect::me().name.clone();
        let rows: Vec<Element> = std::iter::once(row(
            0,
            Icon::Music,
            format!("Este dispositivo · {me}"),
            match here {
                true => "Reproduciendo aquí".to_owned(),
                false => "Toca para volver a reproducir aquí".to_owned(),
            },
            here,
            spot == Some(Spot::Row(0)),
        ))
        .chain(connect.devices.iter().enumerate().map(|(at, device)| {
            let (subtitle, active) = describe(device, &connect.link);
            row(
                at + 1,
                Icon::Cast,
                device.name.clone(),
                subtitle,
                active,
                spot == Some(Spot::Row(at + 1)),
            )
        }))
        .map(IntoElement::into_element)
        .collect();

        ScrollView::new()
            .width(Size::fill())
            .height(Size::fill())
            .child(
                rect()
                    .width(Size::fill())
                    .padding(ui::inset())
                    .spacing(16.)
                    .child(
                        ui::line("Dispositivos", text::TITLE, color::FOREGROUND)
                            .font_weight(FontWeight::SEMI_BOLD),
                    )
                    .child(
                        label()
                            .text(status)
                            .font_size(text::BODY)
                            .color(color::MUTED_FOREGROUND)
                            .width(Size::fill()),
                    )
                    .child(
                        rect()
                            .width(Size::fill())
                            .height(Size::px(48.))
                            .padding((0., 14.))
                            .direction(Direction::Horizontal)
                            .content(Content::Flex)
                            .cross_align(Alignment::Center)
                            .spacing(10.)
                            .corner_radius(24.)
                            .background(color::SECONDARY)
                            .border(field_border)
                            .on_press(ui::tap(Target::Content(Spot::Action(0))))
                            .child(ui::icon(
                                match asking {
                                    Some(_) => Icon::User,
                                    None => Icon::Search,
                                },
                                18.,
                                color::MUTED_FOREGROUND,
                            ))
                            .child(rect().width(Size::flex(1.)).child(input)),
                    )
                    .child(rect().width(Size::fill()).spacing(4.).children(rows))
                    .child(
                        label()
                            .text(
                                "Los dispositivos con Aural abierto en tu misma red Wi-Fi \
                                 aparecen aquí. La primera vez, el otro dispositivo muestra un \
                                 código para confirmar.",
                            )
                            .font_size(text::SMALL)
                            .color(color::MUTED_FOREGROUND)
                            .width(Size::fill()),
                    ),
            )
    }
}

/// What a found device's row says, and whether it is the one that plays.
fn describe(device: &Device, link: &Link) -> (String, bool) {
    match link {
        Link::Connected(name) if *name == device.name => ("Reproduciendo aquí".into(), true),
        Link::Connecting(name) if *name == device.name => ("Conectando…".into(), false),
        Link::NeedCode(name) if *name == device.name => ("Esperando el código".into(), false),
        _ => (format!("Aural · {}", device.address), false),
    }
}

/// One device: an icon, its name, a line about it; the one that plays bright.
fn row(
    at: usize,
    glyph: Icon,
    name: String,
    subtitle: String,
    active: bool,
    focused: bool,
) -> impl IntoElement {
    let ink = match active {
        true => color::FOREGROUND,
        false => color::MUTED_FOREGROUND,
    };
    rect()
        .key(at)
        .width(Size::fill())
        .height(Size::px(64.))
        .padding((0., 14.))
        .direction(Direction::Horizontal)
        .content(Content::Flex)
        .cross_align(Alignment::Center)
        .spacing(14.)
        .corner_radius(metrics::RADIUS)
        .background(match (ui::ring(focused), active) {
            (true, _) => color::FOCUS_FILL,
            (false, true) => color::MUTED,
            (false, false) => Color::TRANSPARENT,
        })
        .border(ui::focus_border(focused))
        .on_press(ui::tap(Target::Content(Spot::Row(at))))
        .child(ui::icon(glyph, 22., ink))
        .child(
            rect()
                .width(Size::flex(1.))
                .spacing(2.)
                .child(
                    ui::line(name, text::BODY + 1., color::FOREGROUND)
                        .font_weight(FontWeight::SEMI_BOLD)
                        .width(Size::fill()),
                )
                .child(ui::line(subtitle, text::SMALL, ink).width(Size::fill())),
        )
}

/// What the field does with what was typed: the code a device asked for, or an address to
/// connect to.
fn submit(typed: String, entry: &mut State<String>) {
    leave();
    let typed = typed.trim().to_owned();
    entry.set(String::new());
    if typed.is_empty() {
        return;
    }
    let asking = crate::app::link_needs_code();
    if asking {
        crate::app::pair(&typed);
        return;
    }
    let address = match typed.contains(':') {
        true => typed.clone(),
        false => format!("{typed}:{PORT}"),
    };
    crate::app::connect(Some(Device {
        id: format!("address:{address}"),
        name: typed,
        address,
    }));
}

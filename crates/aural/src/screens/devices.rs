//! The devices panel of Aural Connect, over whatever shows: this device and the other Aurals
//! on the network as cards, the one that plays lit with the cover's light; the pairing code a
//! device met for the first time asks for, in four boxes; and an address to connect to by
//! hand. It slides in from the right and grows on a computer or a TV, and rises from the
//! bottom as a sheet on a phone; the arrows move through it and Back closes it.

use std::cell::Cell;

use freya::prelude::*;
use freya::radio::{use_radio, use_radio_station};

use crate::connect::{Device, Link};
use crate::nav::Target;
use crate::state::{AppState, Channel, Spot};
use crate::ui::sheet::{Palette, Sheet};
use crate::ui::{self, Icon, color, text};

/// The port an address typed without one goes to.
const PORT: u16 = 47821;
/// How many digits a pairing code has.
const DIGITS: usize = 4;

thread_local! {
    static FIELD: Cell<Option<AccessibilityId>> = const { Cell::new(None) };
    static CODE: Cell<Option<AccessibilityId>> = const { Cell::new(None) };
}

/// Gives the keyboard to the address field.
pub fn edit() {
    if let Some(field) = FIELD.get() {
        field.request_focus();
    }
}

/// Gives the keyboard to the pairing code.
pub fn edit_code() {
    if let Some(code) = CODE.get() {
        code.request_focus();
    }
}

/// Takes the keyboard away from the panel's fields.
pub fn leave() {
    for field in [FIELD.get(), CODE.get()].into_iter().flatten() {
        field.request_unfocus();
    }
}

/// The panel, while open. Mounted once, at the app's root.
#[derive(PartialEq)]
pub struct Devices;

impl Component for Devices {
    fn render(&self) -> impl IntoElement {
        let mut station = use_radio_station::<AppState, Channel>();
        let navigation = use_radio::<AppState, Channel>(Channel::Navigation);
        let connect = use_radio::<AppState, Channel>(Channel::Connect);
        let now = use_radio::<AppState, Channel>(Channel::Now);

        let field = use_a11y();
        FIELD.set(Some(field));
        let field_focus = use_focus(field);
        let code_field = use_a11y();
        CODE.set(Some(code_field));
        let code_focus = use_focus(code_field);
        // Keys go to the fields while one has the keyboard, not to the navigation.
        use_side_effect(move || {
            let typing = field_focus().is_focused() || code_focus().is_focused();
            if station.peek().typing != typing {
                station.write_channel(Channel::Navigation).typing = typing;
            }
        });
        let mut entry = use_state(String::new);
        let mut code = use_state(String::new);
        // The code keeps to digits, and goes as soon as it has all of them.
        use_side_effect(move || {
            let typed = code.read().clone();
            let digits: String = typed
                .chars()
                .filter(char::is_ascii_digit)
                .take(DIGITS)
                .collect();
            if digits.len() == DIGITS && crate::app::link_needs_code() {
                code.set(String::new());
                leave();
                crate::app::pair(&digits);
            } else if digits != typed {
                code.set(digits);
            }
        });

        let open = navigation.read().devices.is_some();
        // While it slides out nothing in it holds the focus.
        let spot = navigation.read().devices;
        let ring = |at: Spot| spot == Some(at) && !ui::touch();
        // Over the fullscreen player the panel is frosted glass, not the app's gray.
        let glass = navigation.read().fullscreen;
        let palette = Palette::new(glass);
        let light = now.read().now.light.unwrap_or(color::FOCUS_ON_PRIMARY);
        let connect = connect.read().connect.clone();
        let asking = match &connect.link {
            Link::NeedCode(name) => Some(name.clone()),
            _ => None,
        };
        let subtitle = match &connect.link {
            Link::Connecting(name) => format!("Conectando con {name}…"),
            Link::Failed(error) => error.clone(),
            _ => "Aural Connect · tu red Wi-Fi".to_owned(),
        };

        let here = !matches!(connect.link, Link::Connected(_));
        let me = crate::connect::me().name.clone();
        // What this device is, by its system: a narrow window on a computer is no phone.
        let this_device = match (cfg!(target_os = "ios"), cfg!(target_os = "android")) {
            (true, _) => Icon::Phone,
            (false, true) if ui::compact() => Icon::Phone,
            (false, true) => Icon::Tv,
            (false, false) => Icon::Laptop,
        };
        let cards: Vec<Element> = std::iter::once(card(
            0,
            this_device,
            me,
            match here {
                true => "Reproduciendo aquí".to_owned(),
                false => "Controlando".to_owned(),
            },
            here,
            ring(Spot::Row(0)),
            light,
            palette,
        ))
        .chain(connect.devices.iter().enumerate().map(|(at, device)| {
            let (subtitle, active) = describe(device, &connect.link);
            card(
                at + 1,
                Icon::Cast,
                device.name.clone(),
                subtitle,
                active,
                ring(Spot::Row(at + 1)),
                light,
                palette,
            )
        }))
        .collect();

        let compact = ui::compact();
        let (top, bottom) = ui::safe();
        // A sheet sizes to what it holds, up to most of the screen.
        let rows = 1 + connect.devices.len();
        let sheet_height = rows as f32 * 102. + 470. + bottom;
        let typed = code.read().clone();

        let address = Input::new(entry)
            .a11y_id(field)
            .placeholder("192.168.1.20")
            .width(Size::fill())
            .flat()
            .theme_colors(clear_input(color::FOREGROUND))
            .on_submit(move |typed: String| submit(typed, &mut entry))
            // Android's keyboards send Enter as a "\n" character.
            .on_pre_key_down(move |event: Event<KeyboardEventData>| match &event.key {
                Key::Character(typed) if typed == "\n" || typed == "\r" => {
                    event.stop_propagation();
                    event.prevent_default();
                    // Read first: the borrow must end before `submit` empties the field.
                    let typed = entry.peek().clone();
                    submit(typed, &mut entry);
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
        // Typed into a field laid over the boxes, which show its digits.
        let code_input = Input::new(code)
            .a11y_id(code_field)
            .width(Size::fill())
            .flat()
            .theme_colors(clear_input(Color::TRANSPARENT));

        let boxes = rect()
            .direction(Direction::Horizontal)
            .spacing(12.)
            .children((0..DIGITS).map(|at| {
                let digit = typed.chars().nth(at).map(String::from).unwrap_or_default();
                let current = at == typed.len().min(DIGITS - 1)
                    && (code_focus().is_focused() || ring(Spot::Action(1)));
                rect()
                    .key(at)
                    .width(Size::px(60.))
                    .height(Size::px(72.))
                    .center()
                    .corner_radius(14.)
                    .background(palette.raised)
                    .border(match current {
                        true => Border::new().fill(color::FOREGROUND).width(2.),
                        false => Border::new().fill(palette.edge).width(1.),
                    })
                    .child(ui::line(digit, 30., color::FOREGROUND).font_weight(FontWeight::BOLD))
                    .into_element()
            }));

        Sheet {
            open,
            glass,
            tall: sheet_height,
            close: crate::app::close_devices,
            content: ScrollView::new()
                            .show_scrollbar(false)
                            .width(Size::fill())
                            .height(Size::fill())
                            .child(
                                rect()
                                    .width(Size::fill())
                                    .padding(match compact {
                                        true => (14., 20., 24. + bottom, 20.),
                                        false => (36. + top, 24., 32. + bottom, 24.),
                                    })
                                    .spacing(16.)
                                    // Title and close.
                                    .child(
                                        rect()
                                            .width(Size::fill())
                                            .direction(Direction::Horizontal)
                                            .content(Content::Flex)
                                            .cross_align(Alignment::Start)
                                            .padding((0., 0., 0., 16.))
                                            .child(
                                                rect()
                                                    .width(Size::flex(1.))
                                                    .spacing(6.)
                                                    .child(
                                                        ui::line(
                                                            "Dispositivos",
                                                            text::TITLE + 4.,
                                                            color::FOREGROUND,
                                                        )
                                                        .font_weight(FontWeight::BOLD),
                                                    )
                                                    .child(
                                                        label()
                                                            .text(subtitle)
                                                            .font_size(text::SMALL)
                                                            .color(color::MUTED_FOREGROUND)
                                                            .width(Size::fill()),
                                                    ),
                                            )
                                            .child(
                                                rect()
                                                    .width(Size::px(40.))
                                                    .height(Size::px(40.))
                                                    .center()
                                                    .corner_radius(20.)
                                                    .background(palette.raised)
                                                    .border(ui::focus_border(ring(Spot::Action(2))))
                                                    .on_press(ui::tap(Target::Device(Spot::Action(
                                                        2,
                                                    ))))
                                                    .child(ui::icon(
                                                        Icon::Close,
                                                        18.,
                                                        color::FOREGROUND,
                                                    )),
                                            ),
                                    )
                                    .child(rect().width(Size::fill()).spacing(14.).children(cards))
                                    // Pairing: the code a device met for the first time shows.
                                    .child(
                                        rect()
                                            .width(Size::fill())
                                            .spacing(8.)
                                            .padding((24., 16., 0., 16.))
                                            .child(ui::eyebrow("Vincular un dispositivo nuevo"))
                                            .child(
                                                label()
                                                    .text(match &asking {
                                                        Some(name) => format!(
                                                            "Escribe el código que muestra {name}."
                                                        ),
                                                        None => "El otro Aural muestra un código de 4 dígitos la primera vez que te conectas.".to_owned(),
                                                    })
                                                    .font_size(text::SMALL)
                                                    .color(color::MUTED_FOREGROUND)
                                                    .width(Size::fill()),
                                            )
                                            .child(
                                                rect()
                                                    .margin((8., 0., 0., 0.))
                                                    .opacity(match asking {
                                                        Some(_) => 1.,
                                                        None => 0.45,
                                                    })
                                                    .on_press(ui::tap(Target::Device(
                                                        Spot::Action(1),
                                                    )))
                                                    .child(boxes)
                                                    .child(
                                                        rect()
                                                            .position(
                                                                Position::new_absolute()
                                                                    .top(0.)
                                                                    .left(0.),
                                                            )
                                                            .width(Size::fill())
                                                            .height(Size::px(72.))
                                                            .maybe(asking.is_some(), |slot| {
                                                                slot.child(code_input)
                                                            }),
                                                    ),
                                            ),
                                    )
                                    // Connecting by hand.
                                    .child(
                                        rect()
                                            .width(Size::fill())
                                            .spacing(12.)
                                            .padding((12., 16., 0., 16.))
                                            .child(ui::line(
                                                "o conecta por dirección",
                                                text::SMALL,
                                                color::MUTED_FOREGROUND,
                                            ))
                                            .child(
                                                rect()
                                                    .width(Size::fill())
                                                    .height(Size::px(52.))
                                                    .padding((0., 18.))
                                                    .direction(Direction::Horizontal)
                                                    .content(Content::Flex)
                                                    .cross_align(Alignment::Center)
                                                    .spacing(12.)
                                                    .corner_radius(14.)
                                                    .background(palette.raised)
                                                    .border(
                                                        match (
                                                            field_focus().is_focused(),
                                                            ring(Spot::Action(0)),
                                                        ) {
                                                            (true, _) => Border::new()
                                                                .fill(color::FOCUS_ON_PRIMARY)
                                                                .width(2.),
                                                            (false, lit) => ui::focus_border(lit),
                                                        },
                                                    )
                                                    .on_press(ui::tap(Target::Device(
                                                        Spot::Action(0),
                                                    )))
                                                    .child(ui::icon(
                                                        Icon::Search,
                                                        18.,
                                                        color::MUTED_FOREGROUND,
                                                    ))
                                                    .child(
                                                        rect()
                                                            .width(Size::flex(1.))
                                                            .child(address),
                                                    ),
                                            ),
                                    ),
                            )
            .into_element(),
        }
        .into_element()
    }
}

/// An input with no background or border of its own, its text in `ink`.
fn clear_input(ink: Color) -> InputColorsThemePartial {
    InputColorsThemePartial {
        background: Some(Preference::Specific(Color::TRANSPARENT)),
        focus_background: Some(Preference::Specific(Color::TRANSPARENT)),
        color: Some(Preference::Specific(ink)),
        placeholder_color: Some(Preference::Specific(color::FAINT)),
        border_fill: Some(Preference::Specific(Color::TRANSPARENT)),
        focus_border_fill: Some(Preference::Specific(Color::TRANSPARENT)),
    }
}

/// What a found device's card says, and whether it is the one that plays.
fn describe(device: &Device, link: &Link) -> (String, bool) {
    match link {
        Link::Connected(name) if *name == device.name => ("Reproduciendo aquí".into(), true),
        Link::Connecting(name) if *name == device.name => ("Conectando…".into(), false),
        Link::NeedCode(name) if *name == device.name => ("Esperando el código".into(), false),
        _ => ("Disponible".into(), false),
    }
}

/// One device as a card: its icon in a disc, its name and a line about it. The one that
/// plays takes the cover's light, with the bars of a meter.
fn card(
    at: usize,
    glyph: Icon,
    name: String,
    subtitle: String,
    active: bool,
    focused: bool,
    light: Color,
    palette: Palette,
) -> Element {
    rect()
        .key(at)
        .width(Size::fill())
        .height(Size::px(88.))
        .padding((0., 20.))
        .direction(Direction::Horizontal)
        .content(Content::Flex)
        .cross_align(Alignment::Center)
        .spacing(16.)
        .corner_radius(16.)
        .background(match active {
            true => ui::tint::alpha(light, 0.12),
            false => palette.raised,
        })
        .border(match (ui::ring(focused), active) {
            (true, _) => Border::new().fill(color::FOCUS).width(2.),
            (false, true) => Border::new().fill(ui::tint::alpha(light, 0.6)).width(1.),
            (false, false) => Border::new().fill(Color::TRANSPARENT).width(1.),
        })
        .on_press(ui::tap(Target::Device(Spot::Row(at))))
        .child(
            rect()
                .width(Size::px(48.))
                .height(Size::px(48.))
                .center()
                .corner_radius(24.)
                .background(match active {
                    true => ui::tint::alpha(light, 0.25),
                    false => palette.muted,
                })
                .child(ui::icon(
                    glyph,
                    22.,
                    match active {
                        true => light,
                        false => color::FOREGROUND,
                    },
                )),
        )
        .child(
            rect()
                .width(Size::flex(1.))
                .spacing(4.)
                .child(
                    ui::line(name, text::BODY + 1., color::FOREGROUND)
                        .font_weight(FontWeight::SEMI_BOLD)
                        .width(Size::fill()),
                )
                .child(
                    ui::line(
                        subtitle,
                        text::SMALL,
                        match active {
                            true => light,
                            false => color::MUTED_FOREGROUND,
                        },
                    )
                    .width(Size::fill()),
                ),
        )
        .maybe(active, |card| {
            card.child(
                rect()
                    .direction(Direction::Horizontal)
                    .cross_align(Alignment::End)
                    .spacing(4.)
                    .height(Size::px(22.))
                    .children([14., 22., 10.].into_iter().enumerate().map(|(at, tall)| {
                        rect()
                            .key(at)
                            .width(Size::px(4.))
                            .height(Size::px(tall))
                            .corner_radius(2.)
                            .background(light)
                            .into_element()
                    })),
            )
        })
        .into_element()
}

/// Connects to the address typed, on Aural Connect's port unless it names another.
fn submit(typed: String, entry: &mut State<String>) {
    leave();
    let typed = typed.trim().to_owned();
    entry.set(String::new());
    if typed.is_empty() {
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

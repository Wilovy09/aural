//! The account page, and the sign-in screen shown while nobody is signed in.

use freya::prelude::*;
use freya::radio::use_radio;

use crate::nav::Target;
use crate::state::{AppState, Auth, Channel, Spot, Zone};
use crate::ui::{self, Icon, Variant, color, metrics, text};

/// The account page, as settings: the account with its picture on top, then playback,
/// appearance and devices as grouped rows, and signing out at the foot.
#[derive(PartialEq)]
pub struct Account;

impl Component for Account {
    fn render(&self) -> impl IntoElement {
        let navigation = use_radio::<AppState, Channel>(Channel::Navigation);
        let auth = use_radio::<AppState, Channel>(Channel::Auth);
        let now = use_radio::<AppState, Channel>(Channel::Now);
        let light = now.read().now.light.unwrap_or(color::FOCUS_ON_PRIMARY);
        let state = navigation.read();
        let focus = state.focus;
        let (motion, text_size, interface, confirming) = (
            state.motion,
            state.text,
            state.interface,
            state.confirm_sign_out,
        );
        drop(state);
        let on = |spot: Spot| focus.zone == Zone::Content && focus.content == spot;
        let account = match &auth.read().auth {
            Auth::SignedIn(Some(account)) => Some(account.clone()),
            _ => None,
        };
        let name = account
            .as_ref()
            .map_or("Sin cuenta".to_owned(), |account| account.name.clone());
        let handle = account
            .as_ref()
            .and_then(|account| account.email.clone())
            .unwrap_or_else(|| "YouTube Music".into());
        let photo = account.as_ref().and_then(|account| account.photo.clone());

        ScrollView::new()
            // A phone scrolls under the finger, with no bar to drag.
            .show_scrollbar(!ui::compact())
            .width(Size::fill())
            .height(Size::fill())
            .child(
                rect()
                    .width(Size::fill())
                    .max_width(Size::px(760.))
                    .padding((ui::inset() / 2., ui::inset(), ui::inset(), ui::inset()))
                    .spacing(28.)
                    // Who is signed in.
                    .child(
                        rect()
                            .direction(Direction::Horizontal)
                            .cross_align(Alignment::Center)
                            .spacing(20.)
                            .child(avatar(photo, &name, 88., light))
                            .child(
                                rect()
                                    .spacing(4.)
                                    .child(ui::eyebrow("Cuenta de YouTube Music"))
                                    .child(
                                        ui::line(name.clone(), text::DISPLAY, color::FOREGROUND)
                                            .font_weight(FontWeight::BOLD),
                                    )
                                    .child(ui::line(handle, text::BODY, color::MUTED_FOREGROUND)),
                            ),
                    )
                    .child(group(
                        "Reproducción",
                        vec![
                            row(
                                Icon::Motion,
                                "Portadas animadas",
                                "Se ven en el reproductor a pantalla completa",
                                switch(motion, light),
                                on(Spot::Action(1)),
                                Target::Content(Spot::Action(1)),
                                light,
                            )
                            .into_element(),
                        ],
                    ))
                    .child(group(
                        "Apariencia",
                        vec![
                            sizes(
                                "Tamaño del texto",
                                0,
                                text_size,
                                focus.zone == Zone::Content,
                                focus.content,
                                light,
                            )
                            .into_element(),
                            sizes(
                                "Tamaño de la interfaz",
                                1,
                                interface,
                                focus.zone == Zone::Content,
                                focus.content,
                                light,
                            )
                            .into_element(),
                        ],
                    ))
                    .child(group(
                        "Conexión",
                        vec![
                            row(
                                Icon::Cast,
                                "Dispositivos",
                                "Reproduce en otro Aural de tu red",
                                ui::icon(Icon::Next, 16., color::FAINT).into_element(),
                                on(Spot::Action(2)),
                                Target::Content(Spot::Action(2)),
                                light,
                            )
                            .into_element(),
                        ],
                    ))
                    .child(
                        rect()
                            .width(Size::fill())
                            .height(Size::px(48.))
                            .center()
                            .corner_radius(metrics::RADIUS)
                            .background(match ui::ring(on(Spot::Action(0))) {
                                true => color::FOCUS_FILL,
                                false => color::SECONDARY,
                            })
                            .on_press(ui::tap(Target::Content(Spot::Action(0))))
                            .child(
                                rect()
                                    .direction(Direction::Horizontal)
                                    .cross_align(Alignment::Center)
                                    .spacing(8.)
                                    .child(ui::icon(Icon::LogOut, 16., color::DANGER))
                                    .child(
                                        ui::line(
                                            match (confirming, ui::touch()) {
                                                (true, true) => "¿Cerrar sesión? Toca otra vez",
                                                (true, false) => {
                                                    "¿Cerrar sesión? OK para confirmar"
                                                }
                                                (false, _) => "Cerrar sesión",
                                            },
                                            text::BODY,
                                            color::DANGER,
                                        )
                                        .font_weight(FontWeight::SEMI_BOLD),
                                    ),
                            ),
                    ),
            )
    }
}

/// The account's picture, or its initial on a disc lit by the cover playing.
pub(crate) fn avatar(photo: Option<String>, name: &str, side: f32, light: Color) -> Element {
    match photo {
        Some(url) => ui::Cover::new(Some(url), side, side / 2.).into_element(),
        None => rect()
            .width(Size::px(side))
            .height(Size::px(side))
            .corner_radius(side / 2.)
            .background(ui::tint::alpha(light, 0.35))
            .center()
            .child(
                ui::line(
                    name.chars()
                        .next()
                        .map(|c| c.to_uppercase().to_string())
                        .unwrap_or_default(),
                    side * 0.42,
                    color::FOREGROUND,
                )
                .font_weight(FontWeight::BOLD),
            )
            .into_element(),
    }
}

/// A titled group of settings rows on one raised surface.
fn group(title: &str, rows: Vec<Element>) -> impl IntoElement {
    rect()
        .width(Size::fill())
        .spacing(10.)
        .child(ui::line(title, text::LARGE, color::FOREGROUND).font_weight(FontWeight::BOLD))
        .child(
            rect()
                .width(Size::fill())
                .corner_radius(metrics::RADIUS_LG)
                .overflow(Overflow::Clip)
                .background(color::SECONDARY)
                .border(Border::new().fill(color::SIDEBAR_BORDER).width(1.))
                .children(rows),
        )
}

/// One setting: icon, title over a hint, and its control on the right. The D-pad's one is lit.
fn row(
    glyph: Icon,
    title: &str,
    hint: &str,
    control: Element,
    focused: bool,
    target: Target,
    light: Color,
) -> impl IntoElement {
    let lit = ui::ring(focused);
    rect()
        .width(Size::fill())
        .height(Size::px(68.))
        .direction(Direction::Horizontal)
        .content(Content::Flex)
        .cross_align(Alignment::Center)
        .background(match lit {
            true => color::FOCUS_FILL,
            false => Color::TRANSPARENT,
        })
        .on_press(ui::tap(target))
        .child(crate::chrome::sidebar::bar(lit, light))
        .child(
            rect()
                .width(Size::flex(1.))
                .padding((0., 16.))
                .direction(Direction::Horizontal)
                .content(Content::Flex)
                .cross_align(Alignment::Center)
                .spacing(14.)
                .child(
                    rect()
                        .width(Size::px(36.))
                        .height(Size::px(36.))
                        .corner_radius(metrics::RADIUS_SM)
                        .background(color::MUTED)
                        .center()
                        .child(ui::icon(glyph, 18., color::FOREGROUND)),
                )
                .child(
                    rect()
                        .width(Size::flex(1.))
                        .spacing(2.)
                        .child(
                            ui::line(title, text::BODY, color::FOREGROUND)
                                .font_weight(FontWeight::SEMI_BOLD),
                        )
                        .child(
                            ui::line(hint, text::SMALL, color::MUTED_FOREGROUND)
                                .width(Size::fill()),
                        ),
                )
                .child(control),
        )
}

/// An on/off switch, lit with the cover's light when on.
fn switch(on: bool, light: Color) -> Element {
    rect()
        .width(Size::px(44.))
        .height(Size::px(26.))
        .corner_radius(13.)
        .padding(3.)
        .background(match on {
            true => light,
            false => color::RAISED,
        })
        .main_align(match on {
            true => Alignment::End,
            false => Alignment::Start,
        })
        .direction(Direction::Horizontal)
        .child(
            rect()
                .width(Size::px(20.))
                .height(Size::px(20.))
                .corner_radius(10.)
                .background(color::FOREGROUND),
        )
        .into_element()
}

/// A size setting: its title, then the steps as a segmented control. The chosen step is
/// white; the D-pad's one is lit.
fn sizes(
    title: &str,
    group: usize,
    chosen: crate::settings::Scale,
    in_content: bool,
    spot: Spot,
    light: Color,
) -> impl IntoElement {
    use crate::settings::Scale;
    let row_focused = in_content && matches!(spot, Spot::Cell(at, _) if at == group);
    rect()
        .width(Size::fill())
        .padding((14., 16.))
        .spacing(10.)
        .background(match ui::ring(row_focused) {
            true => color::HOVER,
            false => Color::TRANSPARENT,
        })
        .child(
            rect()
                .direction(Direction::Horizontal)
                .cross_align(Alignment::Center)
                .spacing(14.)
                .child(
                    rect()
                        .width(Size::px(36.))
                        .height(Size::px(36.))
                        .corner_radius(metrics::RADIUS_SM)
                        .background(color::MUTED)
                        .center()
                        .child(
                            ui::line(
                                match group {
                                    0 => "Aa",
                                    _ => "⊞",
                                },
                                text::BODY,
                                color::FOREGROUND,
                            )
                            .font_weight(FontWeight::BOLD),
                        ),
                )
                .child(
                    ui::line(title, text::BODY, color::FOREGROUND)
                        .font_weight(FontWeight::SEMI_BOLD),
                ),
        )
        .child(
            rect()
                .width(Size::fill())
                .padding(4.)
                .direction(Direction::Horizontal)
                .content(Content::Flex)
                .spacing(4.)
                .corner_radius(metrics::RADIUS)
                .background(color::BACKGROUND)
                .children(Scale::ALL.iter().enumerate().map(|(at, scale)| {
                    let picked = *scale == chosen;
                    let lit = ui::ring(in_content && spot == Spot::Cell(group, at));
                    rect()
                        .key(at)
                        .width(Size::flex(1.))
                        .height(Size::px(36.))
                        .center()
                        .corner_radius(metrics::RADIUS_SM)
                        .background(match (picked, lit) {
                            (true, _) => color::PRIMARY,
                            (false, true) => color::RAISED,
                            (false, false) => Color::TRANSPARENT,
                        })
                        .border(match lit {
                            true => Border::new()
                                .fill(light)
                                .width(2.)
                                .alignment(BorderAlignment::Outer),
                            false => Border::new().fill(Color::TRANSPARENT).width(0.),
                        })
                        .on_press(ui::tap(Target::Content(Spot::Cell(group, at))))
                        .child(
                            ui::line(
                                scale.name(),
                                text::LABEL,
                                match picked {
                                    true => color::PRIMARY_FOREGROUND,
                                    false => color::MUTED_FOREGROUND,
                                },
                            )
                            .font_weight(FontWeight::SEMI_BOLD),
                        )
                        .into_element()
                })),
        )
}

/// The full-screen sign-in prompt.
#[derive(PartialEq)]
pub struct SignIn;

impl Component for SignIn {
    fn render(&self) -> impl IntoElement {
        let auth = use_radio::<AppState, Channel>(Channel::Auth);
        let (status, error, busy) = match &auth.read().auth {
            Auth::Checking => (Some("Buscando tu sesión…".to_string()), None, true),
            Auth::SigningIn(step) => (Some(step.clone()), None, true),
            Auth::SignedOut { error } => (None, error.clone(), false),
            Auth::SignedIn(_) => (None, None, true),
        };
        rect()
            .expanded()
            .background(color::BACKGROUND)
            .center()
            .spacing(18.)
            .child(ui::logo(72.))
            .child(
                ui::line("Aural", text::DISPLAY, color::FOREGROUND).font_weight(FontWeight::BOLD),
            )
            .child(
                label()
                    .text("Entra con tu cuenta de YouTube Music para ver tu biblioteca.")
                    .font_size(text::BODY)
                    .color(color::MUTED_FOREGROUND)
                    .text_align(TextAlign::Center)
                    .width(Size::percent(90.)),
            )
            .maybe(!busy, |screen| {
                screen.child(
                    ui::button(
                        Variant::Primary,
                        Some(Icon::User),
                        Some("Iniciar sesión con Google"),
                        true,
                    )
                    .on_press(ui::tap(Target::Content(Spot::Action(0)))),
                )
            })
            .map(status, |screen, status| {
                screen.child(ui::line(status, text::SMALL, color::MUTED_FOREGROUND))
            })
            .map(error, |screen, error| {
                screen.child(ui::line(error, text::SMALL, color::DANGER))
            })
    }
}

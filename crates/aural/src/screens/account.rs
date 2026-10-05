//! The account page, and the sign-in screen shown while nobody is signed in.

use freya::prelude::*;
use freya::radio::use_radio;

use crate::nav::Target;
use crate::state::{AppState, Auth, Channel, Spot, Zone};
use crate::ui::{self, Icon, Variant, color, metrics, text};

/// The account page: who is signed in, and signing out.
#[derive(PartialEq)]
pub struct Account;

impl Component for Account {
    fn render(&self) -> impl IntoElement {
        let navigation = use_radio::<AppState, Channel>(Channel::Navigation);
        let auth = use_radio::<AppState, Channel>(Channel::Auth);
        let focus = navigation.read().focus;
        let ring = |at: usize| focus.zone == Zone::Content && focus.content == Spot::Action(at);
        let motion = navigation.read().motion;
        let confirming = navigation.read().confirm_sign_out;
        let (name, email) = match &auth.read().auth {
            Auth::SignedIn(Some(account)) => (account.name.clone(), account.email.clone()),
            _ => ("Sin cuenta".to_string(), None),
        };

        rect()
            .expanded()
            .padding(ui::inset())
            .spacing(16.)
            .child(
                ui::line("Cuenta", text::TITLE, color::FOREGROUND)
                    .font_weight(FontWeight::SEMI_BOLD),
            )
            .child(
                rect()
                    .direction(Direction::Horizontal)
                    .cross_align(Alignment::Center)
                    .spacing(14.)
                    .padding(16.)
                    .corner_radius(metrics::RADIUS)
                    .background(color::SECONDARY)
                    .child(
                        rect()
                            .width(Size::px(48.))
                            .height(Size::px(48.))
                            .corner_radius(24.)
                            .background(color::MUTED)
                            .center()
                            .child(ui::icon(Icon::User, 22., color::FOREGROUND)),
                    )
                    .child(
                        rect()
                            .spacing(2.)
                            .child(
                                ui::line(name, text::LARGE, color::FOREGROUND)
                                    .font_weight(FontWeight::SEMI_BOLD),
                            )
                            .child(ui::line(
                                email.unwrap_or_else(|| "YouTube Music".into()),
                                text::SMALL,
                                color::MUTED_FOREGROUND,
                            )),
                    ),
            )
            .child(
                rect()
                    // A phone stacks the two buttons.
                    .direction(match ui::compact() {
                        true => Direction::Vertical,
                        false => Direction::Horizontal,
                    })
                    .spacing(8.)
                    .child(
                        ui::button(
                            Variant::Outline,
                            Some(Icon::LogOut),
                            Some(match (confirming, ui::touch()) {
                                (true, true) => "¿Cerrar sesión? Toca otra vez",
                                (true, false) => "¿Cerrar sesión? OK para confirmar",
                                (false, _) => "Cerrar sesión",
                            }),
                            ring(0),
                        )
                        .on_press(ui::tap(Target::Content(Spot::Action(0)))),
                    )
                    .child(
                        ui::button(
                            match motion {
                                true => Variant::Primary,
                                false => Variant::Outline,
                            },
                            Some(Icon::Motion),
                            Some(match motion {
                                true => "Portadas animadas: sí",
                                false => "Portadas animadas: no",
                            }),
                            ring(1),
                        )
                        .on_press(ui::tap(Target::Content(Spot::Action(1)))),
                    ),
            )
            .child(
                label()
                    .text("Las portadas animadas se ven en el reproductor a pantalla completa.")
                    .font_size(text::SMALL)
                    .color(color::MUTED_FOREGROUND)
                    .width(Size::fill()),
            )
    }
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

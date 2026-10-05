//! The account page, and the sign-in screen shown while nobody is signed in.

use freya::prelude::*;
use freya::radio::use_radio;

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
            .padding(metrics::INSET)
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
                    .direction(Direction::Horizontal)
                    .spacing(8.)
                    .child(ui::button(
                        Variant::Outline,
                        Some(Icon::LogOut),
                        Some(match confirming {
                            true => "¿Cerrar sesión? OK para confirmar",
                            false => "Cerrar sesión",
                        }),
                        ring(0),
                    ))
                    .child(ui::button(
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
                    )),
            )
            .child(ui::line(
                "Las portadas animadas se ven en el reproductor a pantalla completa.",
                text::SMALL,
                color::MUTED_FOREGROUND,
            ))
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
            .child(ui::line(
                "Entra con tu cuenta de YouTube Music para ver tu biblioteca.",
                text::BODY,
                color::MUTED_FOREGROUND,
            ))
            .maybe(!busy, |screen| {
                screen.child(ui::button(
                    Variant::Primary,
                    Some(Icon::User),
                    Some("Iniciar sesión con Google"),
                    true,
                ))
            })
            .map(status, |screen, status| {
                screen.child(ui::line(status, text::SMALL, color::MUTED_FOREGROUND))
            })
            .map(error, |screen, error| {
                screen.child(ui::line(error, text::SMALL, color::DANGER))
            })
    }
}

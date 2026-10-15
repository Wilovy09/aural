//! The icon in the menu bar (macOS) or the system tray (Windows, Linux): the song playing,
//! play or pause, the next and previous songs, bringing the window back, and quitting.
//!
//! Its menu runs on the UI thread, where the app's own controls live, and the app keeps its
//! labels up to date with [`follow`].

use std::cell::RefCell;

use freya::prelude::*;
use freya::tray::menu::{Menu, MenuEvent, MenuItem, PredefinedMenuItem};
use freya::tray::{TrayEvent, TrayIconBuilder};

use crate::engine::Command;
use crate::library::Song;

const TOGGLE: &str = "toggle";
const NEXT: &str = "next";
const PREVIOUS: &str = "previous";
const SHOW: &str = "show";
const QUIT: &str = "quit";

/// The menu's items whose labels change: the song playing, and play or pause.
struct Labels {
    now: MenuItem,
    toggle: MenuItem,
}

thread_local! {
    static LABELS: RefCell<Option<Labels>> = const { RefCell::new(None) };
}

/// `config` with the tray icon and its menu.
pub fn add(config: LaunchConfig) -> LaunchConfig {
    config.with_tray(icon, |event, mut context: RendererContext| {
        let TrayEvent::Menu(MenuEvent { id }) = event else {
            return;
        };
        match id.0.as_str() {
            TOGGLE => crate::app::control(Command::Toggle),
            NEXT => crate::app::control(Command::Next),
            PREVIOUS => crate::app::control(Command::Previous),
            SHOW => {
                for window in context.windows_mut().values_mut() {
                    let window = window.window_mut();
                    window.set_visible(true);
                    window.set_minimized(false);
                    window.focus_window();
                }
            }
            QUIT => context.exit(),
            _ => {}
        }
    })
}

fn icon() -> freya::tray::TrayIcon {
    let now = MenuItem::new("Nada sonando", false, None);
    let toggle = MenuItem::with_id(TOGGLE, "Reproducir", true, None);
    let menu = Menu::new();
    let _ = menu.append_items(&[
        &now,
        &PredefinedMenuItem::separator(),
        &toggle,
        &MenuItem::with_id(NEXT, "Siguiente", true, None),
        &MenuItem::with_id(PREVIOUS, "Anterior", true, None),
        &PredefinedMenuItem::separator(),
        &MenuItem::with_id(SHOW, "Mostrar Aural", true, None),
        &MenuItem::with_id(QUIT, "Salir", true, None),
    ]);
    LABELS.set(Some(Labels { now, toggle }));
    TrayIconBuilder::new()
        .with_menu(Box::new(menu))
        .with_tooltip("Aural")
        .with_icon(LaunchConfig::tray_icon(crate::ICON))
        .build()
        .expect("cannot build the tray icon")
}

/// Shows `song` and whether it plays in the menu. Called on the UI thread.
pub fn follow(song: Option<&Song>, playing: bool) {
    LABELS.with_borrow(|labels| {
        let Some(labels) = labels else {
            return;
        };
        labels.now.set_text(match song {
            Some(song) => format!("{} — {}", song.title, song.artist),
            None => "Nada sonando".to_owned(),
        });
        labels.toggle.set_text(match playing {
            true => "Pausar",
            false => "Reproducir",
        });
        labels.toggle.set_enabled(song.is_some());
    });
}

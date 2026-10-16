//! The icon in the menu bar (macOS) or the system tray (Windows, Linux), and its menu: the song
//! playing with its cover, play or pause, the previous and next songs, shuffle and repeat, then
//! bringing the window back and quitting.
//!
//! Its menu runs on the UI thread, where the app's own controls live, and the app keeps it up
//! to date with [`follow`].

use std::cell::RefCell;

use freya::prelude::*;
use freya::tray::menu::{
    CheckMenuItem, Icon, IconMenuItem, Menu, MenuEvent, MenuItem, PredefinedMenuItem,
};
use freya::tray::{TrayEvent, TrayIconBuilder};

use crate::engine::{Command, Repeat};
use crate::library::{self, Song};
use crate::nav::Action;

/// The symbol, white on clear: macOS draws it as a template, in the menu bar's own color.
const ICON: &[u8] = include_bytes!("../../../assets/logos/aural-tray.png");
/// The side of the cover beside the song, in pixels.
const COVER: u32 = 32;
/// The most of "title — artist" the menu shows before cutting it.
const LONGEST: usize = 40;

const NOW: &str = "now";
const TOGGLE: &str = "toggle";
const PREVIOUS: &str = "previous";
const NEXT: &str = "next";
const SHUFFLE: &str = "shuffle";
const REPEAT: &str = "repeat";
const SHOW: &str = "show";
const QUIT: &str = "quit";

/// The menu's items that change with what plays.
struct Items {
    now: IconMenuItem,
    toggle: MenuItem,
    previous: MenuItem,
    next: MenuItem,
    shuffle: CheckMenuItem,
    repeat: CheckMenuItem,
    /// The song whose cover is shown, or is being fetched.
    covered: Option<String>,
}

thread_local! {
    static ITEMS: RefCell<Option<Items>> = const { RefCell::new(None) };
}

/// `config` with the tray icon and its menu.
pub fn add(config: LaunchConfig) -> LaunchConfig {
    config.with_tray(icon, |event, mut context: RendererContext| {
        let TrayEvent::Menu(MenuEvent { id }) = event else {
            return;
        };
        match id.0.as_str() {
            TOGGLE => crate::app::control(Command::Toggle),
            PREVIOUS => crate::app::control(Command::Previous),
            NEXT => crate::app::control(Command::Next),
            SHUFFLE => crate::app::act(Action::ToggleShuffle),
            REPEAT => crate::app::act(Action::CycleRepeat),
            NOW | SHOW => {
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
    let items = Items {
        now: IconMenuItem::with_id(NOW, "Nada sonando", false, None, None),
        toggle: MenuItem::with_id(TOGGLE, "Reproducir", false, None),
        previous: MenuItem::with_id(PREVIOUS, "Anterior", false, None),
        next: MenuItem::with_id(NEXT, "Siguiente", false, None),
        shuffle: CheckMenuItem::with_id(SHUFFLE, "Aleatorio", true, false, None),
        repeat: CheckMenuItem::with_id(REPEAT, "Repetir", true, false, None),
        covered: None,
    };
    let menu = Menu::new();
    let _ = menu.append_items(&[
        &items.now,
        &PredefinedMenuItem::separator(),
        &items.toggle,
        &items.previous,
        &items.next,
        &PredefinedMenuItem::separator(),
        &items.shuffle,
        &items.repeat,
        &PredefinedMenuItem::separator(),
        &MenuItem::with_id(SHOW, "Mostrar Aural", true, None),
        &MenuItem::with_id(QUIT, "Salir", true, None),
    ]);
    ITEMS.set(Some(items));
    TrayIconBuilder::new()
        .with_menu(Box::new(menu))
        .with_tooltip("Aural")
        .with_icon(LaunchConfig::tray_icon(ICON))
        .with_icon_as_template(true)
        .build()
        .expect("cannot build the tray icon")
}

/// Shows in the menu what plays: `song` with its cover, whether it plays, and the shuffle and
/// repeat modes. Called on the UI thread whenever any of them changes.
pub fn follow(song: Option<&Song>, playing: bool, shuffle: bool, repeat: Repeat) {
    let fetch = ITEMS.with_borrow_mut(|items| {
        let items = items.as_mut()?;
        items.now.set_text(match song {
            Some(song) => cut(&format!("{} — {}", song.title, song.artist)),
            None => "Nada sonando".to_owned(),
        });
        items.now.set_enabled(song.is_some());
        items.toggle.set_text(match playing {
            true => "Pausar",
            false => "Reproducir",
        });
        for item in [&items.toggle, &items.previous, &items.next] {
            item.set_enabled(song.is_some());
        }
        items.shuffle.set_checked(shuffle);
        items.repeat.set_checked(repeat != Repeat::Off);
        items.repeat.set_text(match repeat {
            Repeat::One => "Repetir una",
            _ => "Repetir",
        });
        // A new song: its cover replaces the last one once it is in.
        let id = song.map(|song| song.id.clone());
        if items.covered == id {
            return None;
        }
        items.covered = id;
        items.now.set_icon(None);
        song.cloned()
    });
    if let Some(song) = fetch {
        spawn_in_window(async move {
            let Some(icon) = cover(&song).await else {
                return;
            };
            ITEMS.with_borrow(|items| {
                if let Some(items) = items.as_ref()
                    && items.covered.as_deref() == Some(song.id.as_str())
                {
                    items.now.set_icon(Some(icon));
                }
            });
        });
    }
}

/// `song`'s cover as a small menu icon: Apple's when the catalog has it, as everywhere.
async fn cover(song: &Song) -> Option<Icon> {
    let apple = crate::artwork::find(crate::artwork::Wanted::song(song), COVER * 2).await;
    let url = apple.or_else(|| {
        song.cover
            .as_deref()
            .map(|url| library::sized(url, COVER * 2))
    })?;
    let bytes = crate::images::fetch(url).await?;
    let image = image::load_from_memory(&bytes)
        .ok()?
        .resize_to_fill(COVER, COVER, image::imageops::FilterType::Triangle)
        .into_rgba8();
    Icon::from_rgba(image.into_raw(), COVER, COVER).ok()
}

/// `text` cut to what the menu shows, with an ellipsis when something was left out.
fn cut(text: &str) -> String {
    match text.char_indices().nth(LONGEST) {
        Some((at, _)) => format!("{}…", text[..at].trim_end()),
        None => text.to_owned(),
    }
}

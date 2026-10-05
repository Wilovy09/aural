//! Aural: a YouTube Music client in Freya for desktop, Android phones and Android TV.

mod app;
mod artwork;
mod chrome;
mod cover;
mod engine;
mod images;
mod library;
mod login;
mod media;
mod nav;
mod platform;
mod probe;
mod runtime;
mod screens;
mod session;
mod settings;
mod sheets;
mod state;
mod stream;
mod ui;

use freya::prelude::*;

/// The window every platform opens.
pub fn window() -> WindowConfig {
    let window = WindowConfig::new(app::app)
        .with_title("Aural")
        .with_size(1280., 760.)
        .with_background(ui::color::BACKGROUND);
    #[cfg(not(target_os = "android"))]
    let window = window.with_icon(LaunchConfig::window_icon(ICON));
    window
}

/// The app's icon, for the window and the Dock.
#[cfg(not(target_os = "android"))]
const ICON: &[u8] = include_bytes!("../../../assets/logos/aural_icon.png");

/// Shows the icon in the Dock: macOS takes an app's icon from its bundle and ignores the
/// window's, and a binary run on its own has no bundle.
#[cfg(target_os = "macos")]
pub(crate) fn dock_icon() {
    use objc2::AnyThread as _;
    use objc2_app_kit::{NSApplication, NSImage};
    use objc2_foundation::{MainThreadMarker, NSData};

    let Some(main) = MainThreadMarker::new() else {
        return;
    };
    let data = NSData::with_bytes(ICON);
    if let Some(image) = NSImage::initWithData(NSImage::alloc(), &data) {
        // SAFETY: on the main thread, with an image that lives as long as the app holds it.
        unsafe { NSApplication::sharedApplication(main).setApplicationIconImage(Some(&image)) };
    }
}

fn launch_config() -> LaunchConfig {
    ui::fonts(LaunchConfig::new())
        .with_plugin(probe::Probe::default())
        .with_window(window())
}

/// Desktop entry point.
#[cfg(not(target_os = "android"))]
pub fn run() {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();
    launch(launch_config());
}

/// Android entry point, called by `NativeActivity` once `libaural.so` is loaded.
#[cfg(target_os = "android")]
#[unsafe(no_mangle)]
fn android_main(droid: freya::winit::platform::android::activity::AndroidApp) {
    use freya::winit::event_loop::EventLoop;
    use freya::winit::platform::android::EventLoopBuilderExtAndroid as _;

    android_logger::init_once(
        android_logger::Config::default()
            .with_max_level(log::LevelFilter::Info)
            .with_tag("aural"),
    );
    log::info!("aural: android_main, abi {}", std::env::consts::ARCH);

    // winit builds one event loop per process. A window reopened while the process lived on
    // (music kept it) cannot have one, so the music is handed on to a fresh process instead.
    static RAN: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
    if RAN.swap(true, std::sync::atomic::Ordering::SeqCst) {
        // Android brings the activity back up in a new process by itself when this one ends
        // while it opens.
        log::info!("aural: reopened in a living process, starting a fresh one");
        media::hand_off();
        std::process::exit(0);
    }

    login::remember(droid.clone());

    let event_loop = EventLoop::with_user_event()
        .with_android_app(droid.clone())
        .build()
        .expect("cannot build the event loop");

    launch(
        launch_config()
            .with_event_loop(event_loop)
            .with_plugin(freya::android::AndroidPlugin::new(droid)),
    );

    // The window is gone. With music playing the process stays for it, the notification
    // still driving it; otherwise it ends, so the next launch starts clean.
    if !media::playing() {
        log::info!("aural: window closed, nothing playing, ending");
        std::process::exit(0);
    }
    log::info!("aural: window closed, music goes on in the background");
}

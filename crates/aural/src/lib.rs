//! Aural: a YouTube Music client in Freya for desktop, Android phones and Android TV.

mod app;
mod chrome;
mod cover;
mod engine;
mod images;
mod library;
mod login;
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
    WindowConfig::new(app::app)
        .with_title("Aural")
        .with_size(1280., 760.)
        .with_background(ui::color::BACKGROUND)
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
}

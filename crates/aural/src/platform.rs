//! What differs between platforms outside the UI: where the app keeps its files, and
//! keeping the screen on while music plays.

use std::path::PathBuf;

/// The app's private folder, created if missing.
pub fn data_dir() -> PathBuf {
    #[cfg(target_os = "android")]
    let dir = crate::login::data_dir().unwrap_or_else(std::env::temp_dir);
    // A second copy on one computer (to try Aural Connect) can keep its own folder.
    #[cfg(not(target_os = "android"))]
    let dir = std::env::var_os("AURAL_DATA")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| {
            dirs::data_dir()
                .unwrap_or_else(std::env::temp_dir)
                .join("aural")
        });
    let _ = std::fs::create_dir_all(&dir);
    dir
}

/// Keeps the screen from dimming or starting the TV's screensaver while `awake`; lets it
/// again otherwise. Called with whether music is playing.
pub fn keep_awake(awake: bool) {
    // Through Java on Android's UI thread: android-activity's own window-flags call takes a
    // lock the activity's main thread can hold, and froze the app when tried.
    #[cfg(target_os = "android")]
    if let Err(error) = crate::login::keep_screen_on(awake) {
        log::warn!("platform: cannot change keep-screen-on: {error:#}");
    }
    #[cfg(not(target_os = "android"))]
    let _ = awake;
}

/// Holds Wi-Fi's multicast lock, without which Android drops the mDNS announcements of the
/// other devices.
#[cfg(target_os = "android")]
pub fn multicast() -> anyhow::Result<()> {
    crate::login::with_java(|env, activity| {
        let class = crate::login::helper(env, activity, "dev.aural.app.Network")?;
        env.call_static_method(
            &class,
            "multicast",
            "(Landroid/app/Activity;)V",
            &[jni::objects::JValue::Object(activity)],
        )?;
        Ok(())
    })
}

/// The room the notch (or the status bar) and the home indicator take, in physical pixels, read
/// off the app's window: Freya draws under them, as on Android.
#[cfg(target_os = "ios")]
pub fn insets() -> (f32, f32) {
    use objc2::MainThreadMarker;
    use objc2_ui_kit::UIApplication;
    let Some(mtm) = MainThreadMarker::new() else {
        return (0., 0.);
    };
    let application = UIApplication::sharedApplication(mtm);
    #[allow(deprecated)]
    let Some(window) = application
        .keyWindow()
        .or_else(|| application.windows().firstObject())
    else {
        return (0., 0.);
    };
    let insets = window.safeAreaInsets();
    let scale = window.screen().scale();
    ((insets.top * scale) as f32, (insets.bottom * scale) as f32)
}

/// Makes Aural a music player to iOS: the `Playback` category plays with the silent switch on
/// and keeps playing in the background, where the default `SoloAmbient` would mute it.
#[cfg(target_os = "ios")]
pub fn audio_session() {
    use objc2_avf_audio::{AVAudioSession, AVAudioSessionCategoryPlayback};
    // SAFETY: the shared session is a process-wide singleton safe to use from any thread.
    unsafe {
        let session = AVAudioSession::sharedInstance();
        let Some(playback) = AVAudioSessionCategoryPlayback else {
            return;
        };
        if let Err(error) = session.setCategory_error(playback) {
            log::warn!("platform: cannot set the audio category: {error:?}");
        }
        if let Err(error) = session.setActive_error(true) {
            log::warn!("platform: cannot start the audio session: {error:?}");
        }
    }
}

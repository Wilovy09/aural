//! What differs between platforms outside the UI: where the app keeps its files, and
//! keeping the screen on while music plays.

use std::path::PathBuf;

/// The app's private folder, created if missing.
pub fn data_dir() -> PathBuf {
    #[cfg(target_os = "android")]
    let dir = crate::login::data_dir().unwrap_or_else(std::env::temp_dir);
    #[cfg(not(target_os = "android"))]
    let dir = dirs::data_dir()
        .unwrap_or_else(std::env::temp_dir)
        .join("aural");
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

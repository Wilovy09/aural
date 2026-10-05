//! Preferences that outlive the app: kept as JSON in the app's private folder.

use serde::{Deserialize, Serialize};

use crate::platform;

#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    /// Show albums' animated covers in the fullscreen player (opt-in).
    pub motion: bool,
}

/// The saved settings, or the defaults.
pub fn load() -> Settings {
    std::fs::read(platform::data_dir().join("settings.json"))
        .ok()
        .and_then(|body| serde_json::from_slice(&body).ok())
        .unwrap_or_default()
}

pub fn save(settings: Settings) {
    let Ok(body) = serde_json::to_vec(&settings) else {
        return;
    };
    if let Err(error) = std::fs::write(platform::data_dir().join("settings.json"), body) {
        log::warn!("settings: cannot save: {error}");
    }
}

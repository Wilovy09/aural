//! Preferences that outlive the app: kept as JSON in the app's private folder.

use serde::{Deserialize, Serialize};

use crate::platform;

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    /// Show albums' animated covers in the fullscreen player (opt-in).
    pub motion: bool,
    /// How large text is, as a step of [`Scale`].
    pub text: Scale,
    /// How large the whole interface is, as a step of [`Scale`].
    pub interface: Scale,
    /// Show the lyrics' translation under each line.
    pub translate: bool,
    /// Show the song playing as the activity on Discord (desktop only).
    pub discord: bool,
    /// How loud the music plays, apart from the device's volume (desktop only).
    pub volume: crate::engine::Volume,
    /// Play on with similar songs once the queue runs out (opt-in).
    pub autoplay: bool,
    /// Keep the fullscreen player's backdrop moving (opt-in: it costs a TV every frame).
    pub backdrop: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            motion: false,
            text: Scale::default(),
            interface: Scale::default(),
            translate: false,
            discord: true,
            volume: crate::engine::Volume::default(),
            autoplay: false,
            backdrop: false,
        }
    }
}

/// A size step, for text or the whole interface.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Scale {
    Small,
    #[default]
    Normal,
    Large,
    Larger,
}

impl Scale {
    pub const ALL: [Scale; 4] = [Scale::Small, Scale::Normal, Scale::Large, Scale::Larger];

    pub fn name(self) -> &'static str {
        match self {
            Scale::Small => "Pequeño",
            Scale::Normal => "Normal",
            Scale::Large => "Grande",
            Scale::Larger => "Muy grande",
        }
    }

    /// The factor text is multiplied by.
    pub fn text(self) -> f32 {
        match self {
            Scale::Small => 0.9,
            Scale::Normal => 1.,
            Scale::Large => 1.12,
            Scale::Larger => 1.25,
        }
    }

    /// The zoom the whole window is drawn at.
    pub fn zoom(self) -> f32 {
        match self {
            Scale::Small => 0.88,
            Scale::Normal => 1.,
            Scale::Large => 1.15,
            Scale::Larger => 1.3,
        }
    }
}

/// Draws the whole window at `scale`'s zoom: layout and text reflow at the new size.
pub fn apply_interface(scale: Scale) {
    freya::prelude::Platform::get()
        .current_window()
        // On a TV the interface size is applied over the zoom that lays it out like a laptop's.
        .set_custom_scale_factor(scale.zoom() as f64 * platform::tv_zoom().unwrap_or(1.));
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

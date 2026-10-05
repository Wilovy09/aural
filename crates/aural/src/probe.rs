//! Diagnostics: logs window creation and every keyboard event Freya receives, so logcat shows
//! what a remote sends.

use freya_winit::plugins::{FreyaPlugin, PluginEvent, PluginHandle};

#[derive(Default)]
pub struct Probe;

impl FreyaPlugin for Probe {
    fn plugin_id(&self) -> &'static str {
        "aural-probe"
    }

    fn on_event(&mut self, event: &mut PluginEvent, _handle: PluginHandle) {
        match event {
            PluginEvent::WindowCreated { .. } => log::info!("probe: window created"),
            PluginEvent::KeyboardInput {
                key,
                code,
                is_pressed: true,
                ..
            } => {
                log::info!("probe: key {key:?} {code:?}")
            }
            _ => {}
        }
    }
}

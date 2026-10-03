//! Phase 0 diagnostics: logs window creation, every keyboard event Freya receives and a
//! running frame count, so logcat shows whether the event loop is alive.

use freya_winit::plugins::{FreyaPlugin, PluginEvent, PluginHandle};

/// How many frames pass between two frame-count log lines.
const EVERY: u64 = 60;

#[derive(Default)]
pub struct Probe {
    frames: u64,
}

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
                is_pressed,
                ..
            } => {
                log::info!("probe: key {key:?} {code:?} pressed={is_pressed}")
            }
            PluginEvent::AfterRedraw { .. } => {
                self.frames += 1;
                if self.frames % EVERY == 1 {
                    log::info!("probe: frame {}", self.frames);
                }
            }
            _ => {}
        }
    }
}

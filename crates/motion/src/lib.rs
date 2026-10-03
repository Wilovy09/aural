//! Motion artwork: finding an album's short silent loop in the Apple Music catalog and
//! decoding it in hardware on the platforms that have a decoder here.

#[cfg(target_os = "android")]
mod android;
mod lookup;

use std::time::Duration;

pub use lookup::{Loop, MotionArt, MotionQuery, MotionSearch};

/// One decoded frame, ready to draw.
pub struct Frame {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
    pub pts: Duration,
}

/// Whether this platform can decode loops.
pub fn supported() -> bool {
    cfg!(target_os = "android")
}

/// Decodes the loop at `path` forever, calling `show` with each frame at its presentation time
/// until it returns false.
#[cfg(target_os = "android")]
pub fn play(path: &std::path::Path, show: impl FnMut(Frame) -> bool) -> anyhow::Result<()> {
    android::play(path, show)
}

/// Decodes the loop at `path`; no decoder on this platform yet.
#[cfg(not(target_os = "android"))]
pub fn play(_path: &std::path::Path, _show: impl FnMut(Frame) -> bool) -> anyhow::Result<()> {
    anyhow::bail!("no motion decoder on this platform yet")
}

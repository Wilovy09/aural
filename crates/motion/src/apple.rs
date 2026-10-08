//! The decoder for iOS and macOS: `AVAssetReader` reads the fMP4 and decodes it through
//! VideoToolbox, in hardware where the device has it, into BGRA pixel buffers that are copied out
//! as RGBA. A reader runs once through the file, so each pass of the loop opens a new one, with
//! presentation times carried on across every pass.

use std::path::Path;
use std::time::{Duration, Instant};

use anyhow::{Context as _, Result, bail};
use objc2::rc::Retained;
use objc2::runtime::AnyObject;
use objc2_av_foundation::{
    AVAssetReader, AVAssetReaderOutput, AVAssetReaderTrackOutput, AVAssetTrack, AVMediaTypeVideo,
    AVURLAsset,
};
use objc2_core_video::{
    CVPixelBufferGetBaseAddress, CVPixelBufferGetBytesPerRow, CVPixelBufferGetHeight,
    CVPixelBufferGetWidth, CVPixelBufferLockBaseAddress, CVPixelBufferLockFlags,
    CVPixelBufferUnlockBaseAddress, kCVPixelBufferPixelFormatTypeKey, kCVPixelFormatType_32BGRA,
};
use objc2_foundation::{NSDictionary, NSNumber, NSString, NSURL};

use crate::Frame;

/// How late a frame may be before the clock restarts at it.
const LATE: Duration = Duration::from_millis(250);
/// The gap assumed between the last frame of a pass and the first of the next.
const FRAME: Duration = Duration::from_micros(33_333);

/// Decodes `path` in a loop, handing every frame to `show` at its presentation time, until
/// `show` returns false.
pub fn play(path: &Path, mut show: impl FnMut(Frame) -> bool) -> Result<()> {
    let path = path.to_str().context("the loop's path is not UTF-8")?;
    let url = NSURL::fileURLWithPath(&NSString::from_str(path));
    // SAFETY: a file url and no options.
    let asset = unsafe { AVURLAsset::URLAssetWithURL_options(&url, None) };
    let kind = unsafe { AVMediaTypeVideo }.context("AVFoundation has no video media type")?;
    // The asset is a local file, so its tracks are there to read without waiting.
    #[allow(deprecated)]
    let track = unsafe { asset.tracksWithMediaType(kind) }
        .firstObject()
        .context("the loop has no video track")?;
    // BGRA is the one 8-bit RGB layout every decoder hands out without a conversion pass.
    let format = NSNumber::new_u32(kCVPixelFormatType_32BGRA);
    // SAFETY: CFString and NSString are toll-free bridged.
    let key: &NSString =
        unsafe { &*(kCVPixelBufferPixelFormatTypeKey as *const _ as *const NSString) };
    let value: &AnyObject = &format;
    let settings = NSDictionary::from_slices(&[key], &[value]);

    let mut started = Instant::now();
    let mut offset = Duration::ZERO;
    let mut last = Duration::ZERO;
    let mut logged = false;
    loop {
        let output = reader(&asset, &track, &settings)?;
        let mut frames = 0u32;
        // SAFETY: the reader started and owns `output`; it is read on this thread only.
        while let Some(sample) = unsafe { output.1.copyNextSampleBuffer() } {
            let Some(image) = (unsafe { sample.image_buffer() }) else {
                continue;
            };
            let seconds = unsafe { sample.presentation_time_stamp().seconds() };
            let pts = offset + Duration::from_secs_f64(seconds.max(0.));
            last = last.max(pts);
            let due = started + pts;
            let now = Instant::now();
            match due.checked_duration_since(now) {
                Some(wait) => std::thread::sleep(wait),
                // Far behind (a slow start, a stall): restart the clock here instead of racing
                // to catch up.
                None if now - due > LATE => started = now - pts,
                None => {}
            }
            let frame = convert(&image, pts)?;
            if !logged {
                log::info!(
                    "motion: decoding {}x{} with AVFoundation",
                    frame.width,
                    frame.height
                );
                logged = true;
            }
            frames += 1;
            if !show(frame) {
                return Ok(());
            }
        }
        if frames == 0 {
            bail!("the decoder gave no frames ({:?})", unsafe {
                output.0.status()
            });
        }
        // End of a pass: keep the clock running past the last frame.
        offset = last + FRAME;
    }
}

/// A reader started over the whole of `track`, and its output.
fn reader(
    asset: &AVURLAsset,
    track: &AVAssetTrack,
    settings: &NSDictionary<NSString, AnyObject>,
) -> Result<(Retained<AVAssetReader>, Retained<AVAssetReaderTrackOutput>)> {
    // SAFETY: plain AVFoundation calls on objects this thread made.
    unsafe {
        let reader = AVAssetReader::assetReaderWithAsset_error(asset)
            .map_err(|error| anyhow::anyhow!("{}", error.localizedDescription()))
            .context("cannot read the loop")?;
        let output = AVAssetReaderTrackOutput::assetReaderTrackOutputWithTrack_outputSettings(
            track,
            Some(settings),
        );
        // Each frame is copied out at once, so the decoder's own buffers can be handed over.
        output.setAlwaysCopiesSampleData(false);
        let as_output: &AVAssetReaderOutput = &output;
        reader.addOutput(as_output);
        if !reader.startReading() {
            bail!("the reader would not start ({:?})", reader.status());
        }
        Ok((reader, output))
    }
}

/// Copies a BGRA pixel buffer out as tightly packed RGBA.
fn convert(image: &objc2_core_video::CVPixelBuffer, pts: Duration) -> Result<Frame> {
    // SAFETY: a live pixel buffer, unlocked below with the same flags.
    if unsafe { CVPixelBufferLockBaseAddress(image, CVPixelBufferLockFlags::ReadOnly) } != 0 {
        bail!("cannot lock the frame");
    }
    let width = CVPixelBufferGetWidth(image);
    let height = CVPixelBufferGetHeight(image);
    let stride = CVPixelBufferGetBytesPerRow(image);
    let base = CVPixelBufferGetBaseAddress(image) as *const u8;
    let mut rgba = Vec::with_capacity(width * height * 4);
    if !base.is_null() {
        for row in 0..height {
            // SAFETY: the buffer is locked and `stride` bytes long per row, `height` rows tall.
            let line = unsafe { std::slice::from_raw_parts(base.add(row * stride), width * 4) };
            for pixel in line.chunks_exact(4) {
                rgba.extend_from_slice(&[pixel[2], pixel[1], pixel[0], pixel[3]]);
            }
        }
    }
    // SAFETY: locked above with the same flags.
    unsafe { CVPixelBufferUnlockBaseAddress(image, CVPixelBufferLockFlags::ReadOnly) };
    if base.is_null() {
        bail!("the frame has no pixels");
    }
    Ok(Frame {
        width: width as u32,
        height: height as u32,
        rgba,
        pts,
    })
}

//! The Android decoder: `AMediaExtractor` reads the fMP4, `AMediaCodec` decodes it in hardware
//! into an `AImageReader` surface, and each `YUV_420_888` image is converted to RGBA. The file
//! loops forever, with presentation times carried on across every pass.

use std::fs::File;
use std::os::fd::AsRawFd as _;
use std::path::Path;
use std::ptr::NonNull;
use std::time::{Duration, Instant};

use anyhow::{Context as _, Result, bail};
use ndk::media::image_reader::{AcquireResult, Image, ImageFormat, ImageReader};
use ndk::media::media_codec::{
    DequeuedInputBufferResult, DequeuedOutputBufferInfoResult, MediaCodec, MediaCodecDirection,
};
use ndk::media::media_format::MediaFormat;
use ndk_sys as ffi;

use crate::Frame;

/// How long one codec call waits for a buffer before the loop moves on.
const WAIT: Duration = Duration::from_millis(5);
/// How long a rendered output gets to show up in the image reader.
const ARRIVAL: Duration = Duration::from_millis(100);
/// How late a frame may be before the clock restarts at it.
const LATE: Duration = Duration::from_millis(250);
/// The gap assumed between the last frame of a pass and the first of the next.
const FRAME_US: i64 = 33_333;

/// Owns the extractor so it is deleted however decoding ends.
struct Extractor(NonNull<ffi::AMediaExtractor>);

impl Drop for Extractor {
    fn drop(&mut self) {
        // SAFETY: the pointer came from AMediaExtractor_new and is deleted once.
        unsafe { ffi::AMediaExtractor_delete(self.0.as_ptr()) };
    }
}

/// Decodes `path` in a loop, handing every frame to `show` at its presentation time, until
/// `show` returns false.
pub fn play(path: &Path, mut show: impl FnMut(Frame) -> bool) -> Result<()> {
    let file = File::open(path).context("cannot open the loop")?;
    let length = file.metadata().context("cannot read the loop size")?.len();

    // SAFETY: plain NDK calls on a fresh extractor and an open file descriptor.
    let extractor = Extractor(
        NonNull::new(unsafe { ffi::AMediaExtractor_new() }).context("no media extractor")?,
    );
    let raw = extractor.0.as_ptr();
    let status =
        unsafe { ffi::AMediaExtractor_setDataSourceFd(raw, file.as_raw_fd(), 0, length as i64) };
    if status.0 != 0 {
        bail!("the extractor refused the loop ({})", status.0);
    }

    let (track, mut format, mime) = video_track(raw)?;
    unsafe { ffi::AMediaExtractor_selectTrack(raw, track) };
    let width = format.i32("width").context("the loop has no width")?;
    let height = format.i32("height").context("the loop has no height")?;
    // The reader hands out frames at the decoder's size; keep the requested format as is.
    let _ = &mut format;

    let reader = ImageReader::new(width, height, ImageFormat::YUV_420_888, 4)
        .context("cannot create the image reader")?;
    let window = reader.window().context("the image reader has no surface")?;
    let codec = MediaCodec::from_decoder_type(&mime).context("no decoder for the loop")?;
    codec
        .configure(&format, Some(&window), MediaCodecDirection::Decoder)
        .context("cannot configure the decoder")?;
    codec.start().context("cannot start the decoder")?;
    log::info!(
        "motion: decoding {width}x{height} {mime} with {}",
        codec.name().unwrap_or_default()
    );

    let mut started = Instant::now();
    let mut offset_us = 0i64;
    let mut last_us = 0i64;
    let mut sample = vec![0u8; 0];
    loop {
        // Keep every free input buffer filled, so decoding never waits on the extractor.
        while let DequeuedInputBufferResult::Buffer(mut input) =
            codec.dequeue_input_buffer(Duration::ZERO)?
        {
            let capacity = input.buffer_mut().len();
            sample.resize(capacity, 0);
            let mut size =
                unsafe { ffi::AMediaExtractor_readSampleData(raw, sample.as_mut_ptr(), capacity) };
            if size < 0 {
                // End of a pass: rewind and keep the clock running past the last frame.
                unsafe {
                    ffi::AMediaExtractor_seekTo(
                        raw,
                        0,
                        ffi::SeekMode::AMEDIAEXTRACTOR_SEEK_PREVIOUS_SYNC,
                    )
                };
                offset_us = last_us + FRAME_US;
                size = unsafe {
                    ffi::AMediaExtractor_readSampleData(raw, sample.as_mut_ptr(), capacity)
                };
            }
            let size = usize::try_from(size).context("the loop has no samples")?;
            let time = unsafe { ffi::AMediaExtractor_getSampleTime(raw) } + offset_us;
            last_us = last_us.max(time);
            for (to, from) in input.buffer_mut()[..size].iter_mut().zip(&sample[..size]) {
                to.write(*from);
            }
            codec.queue_input_buffer(input, 0, size, time as u64, 0)?;
            unsafe { ffi::AMediaExtractor_advance(raw) };
        }

        let DequeuedOutputBufferInfoResult::Buffer(output) = codec.dequeue_output_buffer(WAIT)?
        else {
            continue;
        };
        let pts = Duration::from_micros(output.info().presentation_time_us().max(0) as u64);
        let due = started + pts;
        let now = Instant::now();
        match due.checked_duration_since(now) {
            Some(wait) => std::thread::sleep(wait),
            // Far behind (a slow start, a stall): restart the clock here instead of racing to
            // catch up.
            None if now - due > LATE => started = now - pts,
            None => {}
        }
        codec.release_output_buffer(output, true)?;
        let Some(image) = arrive(&reader)? else {
            continue;
        };
        if !show(convert(&image, pts)?) {
            break;
        }
    }
    codec.stop().ok();
    Ok(())
}

/// The first video track of the file: its index, format and mime type.
fn video_track(raw: *mut ffi::AMediaExtractor) -> Result<(usize, MediaFormat, String)> {
    let tracks = unsafe { ffi::AMediaExtractor_getTrackCount(raw) };
    for track in 0..tracks {
        let pointer = unsafe { ffi::AMediaExtractor_getTrackFormat(raw, track) };
        let Some(pointer) = NonNull::new(pointer) else {
            continue;
        };
        // SAFETY: getTrackFormat hands over a new format the wrapper deletes.
        let mut format = unsafe { MediaFormat::from_ptr(pointer) };
        let mime = format.str("mime").unwrap_or_default().to_owned();
        if mime.starts_with("video/") {
            return Ok((track, format, mime));
        }
    }
    bail!("the loop has no video track")
}

/// Waits briefly for the frame just rendered to reach the reader.
fn arrive(reader: &ImageReader) -> Result<Option<Image>> {
    let since = Instant::now();
    while since.elapsed() < ARRIVAL {
        match reader.acquire_latest_image()? {
            AcquireResult::Image(image) => return Ok(Some(image)),
            AcquireResult::NoBufferAvailable | AcquireResult::MaxImagesAcquired => {
                std::thread::sleep(Duration::from_millis(1))
            }
        }
    }
    Ok(None)
}

/// A `YUV_420_888` image as RGBA, with BT.709 limited-range coefficients (what Apple's loops use).
fn convert(image: &Image, pts: Duration) -> Result<Frame> {
    let crop = image.crop_rect()?;
    let width = (crop.right - crop.left) as usize;
    let height = (crop.bottom - crop.top) as usize;
    let (y, u, v) = (
        image.plane_data(0)?,
        image.plane_data(1)?,
        image.plane_data(2)?,
    );
    let y_row = image.plane_row_stride(0)? as usize;
    let uv_row = image.plane_row_stride(1)? as usize;
    let uv_step = image.plane_pixel_stride(1)? as usize;
    let (left, top) = (crop.left as usize, crop.top as usize);

    let mut rgba = vec![0u8; width * height * 4];
    for row in 0..height {
        let ys = (top + row) * y_row + left;
        let uvs = ((top + row) / 2) * uv_row;
        for col in 0..width {
            let luma = (i32::from(y[ys + col]) - 16) * 298;
            let at = uvs + ((left + col) / 2) * uv_step;
            let cb = i32::from(u[at]) - 128;
            let cr = i32::from(v[at]) - 128;
            let red = (luma + 459 * cr + 128) >> 8;
            let green = (luma - 55 * cb - 136 * cr + 128) >> 8;
            let blue = (luma + 541 * cb + 128) >> 8;
            let out = (row * width + col) * 4;
            rgba[out] = red.clamp(0, 255) as u8;
            rgba[out + 1] = green.clamp(0, 255) as u8;
            rgba[out + 2] = blue.clamp(0, 255) as u8;
            rgba[out + 3] = 255;
        }
    }
    Ok(Frame {
        width: width as u32,
        height: height as u32,
        rgba,
        pts,
    })
}

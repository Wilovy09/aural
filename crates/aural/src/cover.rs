//! Phase 0 cover: the still cover and, once a song plays, its album's motion artwork found in
//! the Apple Music catalog, downloaded and decoded in hardware, replacing the still one.

use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use anyhow::{Context as _, Result};
use freya::engine::prelude::{
    AlphaType, ClipOp, ColorType, FilterMode, ImageInfo, MipmapMode, Paint, SamplingOptions,
    SkData, SkRRect, SkRect, raster_from_data,
};
use freya::prelude::*;
use tokio::sync::watch;

/// The side, in pixels, of the rendition asked for.
const EDGE: u32 = 486;
/// How often the decode rate is reported.
const RATE_EVERY: Duration = Duration::from_secs(5);

/// The latest decoded frame, shared with the UI.
pub type Latest = watch::Sender<Option<Arc<motion::Frame>>>;

/// Looks up the motion artwork of `song`'s album and, when there is one, downloads and
/// decodes it on its own thread. Steps go to `say`, frames to `frames`.
pub fn start(song: &crate::library::Song, frames: Latest, say: impl Fn(String) + Send + 'static) {
    // Apple's catalog matches on the lead artist and the album, as Sonora asks it: the joined
    // artists or a missing album make most songs miss.
    let artist = song.artist.split(", ").next().unwrap_or_default();
    let query = motion::MotionQuery::new(&song.title, artist, song.album.as_deref(), song.duration);
    std::thread::spawn(move || {
        if let Err(error) = run(&query, &say, &frames) {
            say(format!("sin animated cover: {error:#}"));
        }
    });
}

fn run(query: &motion::MotionQuery, say: &dyn Fn(String), frames: &Latest) -> Result<()> {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .context("cannot start tokio")?;
    let search = motion::MotionSearch::default();

    let started = Instant::now();
    say("buscando animated cover".into());
    let art = runtime
        .block_on(search.find(query))?
        .context("el álbum no tiene")?;
    if !motion::supported() {
        anyhow::bail!("no hay decoder en esta plataforma");
    }
    let found = runtime.block_on(search.fetch(&art, EDGE))?;
    let path = folder().join("motion.mp4");
    std::fs::write(&path, &found.bytes).context("cannot write the loop")?;
    say(format!(
        "animated cover {} px, {} KiB en {:?}",
        found.side,
        found.bytes.len() / 1024,
        started.elapsed()
    ));

    let mut count = 0u32;
    let mut window = Instant::now();
    motion::play(&path, |frame| {
        count += 1;
        if window.elapsed() >= RATE_EVERY {
            say(format!(
                "animated cover {}x{} a {:.1} fps",
                frame.width,
                frame.height,
                f64::from(count) / window.elapsed().as_secs_f64()
            ));
            count = 0;
            window = Instant::now();
        }
        frames.send_replace(Some(Arc::new(frame)));
        !frames.is_closed()
    })
}

/// Corner radius of the cover square.
const RADIUS: f32 = 20.;

/// Where the downloaded loop is written.
fn folder() -> PathBuf {
    #[cfg(target_os = "android")]
    if let Some(dir) = crate::login::data_dir() {
        return dir;
    }
    std::env::temp_dir()
}

/// What the cover square shows: the latest motion frame.
#[derive(Clone)]
pub enum Art {
    Motion(Arc<motion::Frame>),
}

/// The cover square with rounded corners. `number` changes with every motion frame so Freya
/// repaints the canvas, whose callback always compares equal.
pub fn view(art: Option<Art>, number: u64, side: f32) -> impl IntoElement {
    rect()
        .width(Size::px(side))
        .height(Size::px(side))
        .corner_radius(RADIUS)
        .background((28, 28, 36))
        .maybe_child(art.map(|art| {
            canvas(RenderCallback::new(move |context: &mut CanvasContext| {
                let image = match &art {
                    Art::Motion(frame) => {
                        let info = ImageInfo::new(
                            (frame.width as i32, frame.height as i32),
                            ColorType::RGBA8888,
                            AlphaType::Opaque,
                            None,
                        );
                        raster_from_data(
                            &info,
                            SkData::new_copy(&frame.rgba),
                            frame.width as usize * 4,
                        )
                    }
                };
                let Some(image) = image else {
                    return;
                };
                let target = SkRect::from_wh(context.size.width, context.size.height);
                context.canvas.save();
                context.canvas.clip_rrect(
                    SkRRect::new_rect_xy(target, RADIUS, RADIUS),
                    ClipOp::Intersect,
                    true,
                );
                context.canvas.draw_image_rect_with_sampling_options(
                    &image,
                    None,
                    target,
                    SamplingOptions::new(FilterMode::Linear, MipmapMode::None),
                    &Paint::default(),
                );
                context.canvas.restore();
            }))
            .key(number)
            .expanded()
        }))
}

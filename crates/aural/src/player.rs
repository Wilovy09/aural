//! Phase 0 player check: searches one song on YouTube Music as a guest, shows its still cover,
//! plays it through rodio, and once it sounds starts the motion artwork lookup for its album.

use std::io::Cursor;
use std::sync::Arc;
use std::time::Instant;

use anyhow::{Context as _, Result};
use tokio::sync::mpsc::UnboundedSender;

use crate::cover;

/// The search the check plays the first song of.
const QUERY: &str = "ENVIDIA Chino Pacas";
/// The side the still cover is asked for, in pixels.
const COVER_EDGE: u32 = 544;

/// What the player tells the UI.
pub enum Event {
    /// A step of the playback, shown as the status line.
    Status(String),
    /// The song found, shown under the cover.
    Track { title: String, artist: String },
    /// The still cover, still encoded (JPEG or WebP).
    Still(Arc<Vec<u8>>),
    /// A step of the motion artwork lookup, shown under the track.
    Motion(String),
}

/// Runs the check on its own thread; events go to `events` and motion frames to `frames`.
pub fn play(events: UnboundedSender<Event>, frames: cover::Latest) {
    std::thread::spawn(move || {
        let say = |line: String| {
            log::info!("player: {line}");
            let _ = events.send(Event::Status(line));
        };
        if let Err(error) = run(&events, &say, frames) {
            say(format!("error: {error:#}"));
        }
    });
}

fn run(events: &UnboundedSender<Event>, say: &dyn Fn(String), frames: cover::Latest) -> Result<()> {
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .context("cannot start tokio")?;
    let api = ytmusic::YtMusic::anonymous();

    say(format!("buscando \"{QUERY}\""));
    let tracks = runtime.block_on(api.search_songs(QUERY))?;
    let track = tracks
        .into_iter()
        .find(|t| t.video_id.is_some())
        .context("no results")?;
    let id = track.video_id.clone().unwrap_or_default();
    let artist = track
        .artists
        .first()
        .map(|a| a.name.clone())
        .unwrap_or_default();
    let _ = events.send(Event::Track {
        title: track.title.clone(),
        artist: artist.clone(),
    });

    if let Some(url) = ytmusic::best_thumbnail(&track.thumbnails).map(|t| sized(&t.url)) {
        match runtime.block_on(still(&api, &url)) {
            Ok(bytes) => {
                let _ = events.send(Event::Still(Arc::new(bytes)));
            }
            Err(error) => log::warn!("player: no still cover: {error:#}"),
        }
    }

    let started = Instant::now();
    say("descargando audio".into());
    let (format, bytes) = runtime.block_on(api.load_audio(&id))?;
    say(format!(
        "{} {} kbps, {} KiB en {:?}",
        format.codec,
        format.bitrate / 1000,
        bytes.len() / 1024,
        started.elapsed()
    ));

    let stream = rodio::OutputStreamBuilder::open_default_stream().context("no audio output")?;
    let sink = rodio::Sink::connect_new(stream.mixer());
    let source = rodio::Decoder::builder()
        .with_data(Cursor::new(bytes))
        .with_hint("mp4")
        .with_seekable(false)
        .build()
        .context("cannot decode")?;
    sink.append(source);
    say(format!("▶ reproduciendo {}", track.title));

    let motion_events = events.clone();
    cover::start(&track.title, &artist, track.duration, frames, move |line| {
        log::info!("cover: {line}");
        let _ = motion_events.send(Event::Motion(line));
    });

    sink.sleep_until_end();
    say("terminó".into());
    Ok(())
}

/// The cover url asked at `COVER_EDGE`: Google's image host takes the size after the last `=`.
fn sized(url: &str) -> String {
    match url.rsplit_once('=') {
        Some((base, _)) if url.contains("googleusercontent.com") => {
            format!("{base}=w{COVER_EDGE}-h{COVER_EDGE}-l90-rj")
        }
        _ => url.to_owned(),
    }
}

async fn still(api: &ytmusic::YtMusic, url: &str) -> Result<Vec<u8>> {
    let response = api.client().get(url).send().await?.error_for_status()?;
    Ok(response.bytes().await?.to_vec())
}

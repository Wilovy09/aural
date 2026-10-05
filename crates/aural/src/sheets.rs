//! Looks up the lyrics of the song that starts playing: every provider at once, the best
//! sheet shown as soon as it arrives and replaced when a better one does. Kept in memory per
//! song, so going back to a song does not search again.

use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

use freya::prelude::*;
use freya::radio::RadioStation;

use crate::library::Song;
use crate::runtime;
use crate::state::{AppState, Channel, Load, Sheet};

/// How long a fallback sheet waits for Apple Music's before showing.
const APPLE_WAIT: std::time::Duration = std::time::Duration::from_secs(3);

/// The best sheet found per song id, `None` when no provider had one.
static FOUND: OnceLock<
    Mutex<HashMap<String, Option<(lyrics::Lyrics, &'static str, Vec<String>)>>>,
> = OnceLock::new();

fn found() -> &'static Mutex<HashMap<String, Option<(lyrics::Lyrics, &'static str, Vec<String>)>>> {
    FOUND.get_or_init(Mutex::default)
}

/// Starts the lookup for `song` unless its sheet is already shown or known.
pub fn look_up(mut station: RadioStation<AppState, Channel>, song: Song) {
    if station.peek().sheet.song.as_deref() == Some(song.id.as_str()) {
        return;
    }
    if let Some(known) = found()
        .lock()
        .ok()
        .and_then(|found| found.get(&song.id).cloned())
    {
        station.write_channel(Channel::Lyrics).sheet = sheet(&song, known);
        return;
    }
    station.write_channel(Channel::Lyrics).sheet = Sheet {
        song: Some(song.id.clone()),
        lyrics: Load::Loading,
        source: None,
        writers: Vec::new(),
    };

    let query = lyrics::LyricsQuery {
        title: song.title.clone(),
        artist: song
            .artist
            .split(", ")
            .next()
            .unwrap_or_default()
            .to_owned(),
        album: song.album.clone(),
        duration: song.duration.unwrap_or_default(),
        track: Some(lyrics::TrackKey {
            provider: "youtube",
            id: song.id.clone(),
        }),
    };
    let (sender, mut batches) = tokio::sync::mpsc::unbounded_channel();
    // Until Apple Music answers, no other provider's sheet shows, so Apple's does not replace
    // a fallback a moment later. If it takes longer than `APPLE_WAIT`, this stands in for its
    // answer and the fallbacks may show.
    let timer = sender.clone();
    runtime::spawn(async move {
        tokio::time::sleep(APPLE_WAIT).await;
        let _ = timer.send((lyrics::APPLE, Vec::new()));
    });
    runtime::spawn(lyrics::gather(lyrics::providers(), query.clone(), sender));
    spawn(async move {
        let mut hits = Vec::new();
        let mut best = None;
        let mut apple_answered = false;
        while let Some((provider, batch)) = batches.recv().await {
            // A newer song took over: dropping the receiver stops the remaining requests.
            if station.peek().sheet.song.as_deref() != Some(song.id.as_str()) {
                return;
            }
            apple_answered |= provider == lyrics::APPLE;
            hits.extend(batch);
            let ranked = lyrics::ordered(&query, hits.clone());
            best = ranked
                .into_iter()
                .find(|hit| !hit.lyrics.is_empty())
                .map(|hit| (hit.lyrics, hit.source, hit.writers));
            let settled = apple_answered
                || best
                    .as_ref()
                    .is_some_and(|(_, source, _)| *source == lyrics::APPLE);
            if settled && let Some(best) = best.clone() {
                station.write_channel(Channel::Lyrics).sheet = sheet(&song, Some(best));
            }
        }
        if let Some((lyrics, source, _)) = &best {
            let first = match lyrics {
                lyrics::Lyrics::Synced { lines } => lines.first().map(|line| line.start),
                lyrics::Lyrics::Plain { .. } => None,
            };
            log::info!(
                "lyrics: {source} for {}, synced {}, worded {}, first line at {first:?}",
                song.id,
                lyrics.synced(),
                lyrics.worded()
            );
        }
        if let Ok(mut found) = found().lock() {
            found.insert(song.id.clone(), best.clone());
        }
        if best.is_none() && station.peek().sheet.song.as_deref() == Some(song.id.as_str()) {
            station.write_channel(Channel::Lyrics).sheet = sheet(&song, None);
        }
    });
}

fn sheet(song: &Song, best: Option<(lyrics::Lyrics, &'static str, Vec<String>)>) -> Sheet {
    match best {
        Some((lyrics, source, writers)) => Sheet {
            song: Some(song.id.clone()),
            lyrics: Load::Ready(lyrics),
            source: Some(source),
            writers,
        },
        None => Sheet {
            song: Some(song.id.clone()),
            lyrics: Load::Failed("Esta canción no tiene letra".into()),
            source: None,
            writers: Vec::new(),
        },
    }
}

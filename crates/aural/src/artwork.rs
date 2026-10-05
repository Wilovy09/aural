//! Apple Music's covers in place of YouTube's: every song and album cover is looked up in the
//! Apple Music catalog first, and YouTube's thumbnail only stands in until it answers or when
//! it has none.
//!
//! The catalog takes about one request a second, so a lookup is made only for a cover on
//! screen, and every answer (a cover or none) is kept on disk: a list seen once shows Apple's
//! covers straight away from then on.

use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

use crate::{platform, runtime};

/// What a cover shows, as the catalog is asked about it.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Wanted {
    /// A song's album cover, found through the song.
    Song {
        title: String,
        artist: String,
        album: Option<String>,
        seconds: Option<u64>,
    },
    /// An album's cover, found by its name and artist.
    Album { title: String, artist: String },
}

impl Wanted {
    /// The cover of `song`. Apple's catalog matches on the lead artist.
    pub fn song(song: &crate::library::Song) -> Self {
        Self::Song {
            title: song.title.clone(),
            artist: lead(&song.artist),
            album: song.album.clone(),
            seconds: song.duration.map(|duration| duration.as_secs()),
        }
    }

    /// The cover of an album by `artist`.
    pub fn album(title: &str, artist: &str) -> Self {
        Self::Album {
            title: title.to_owned(),
            artist: lead(artist),
        }
    }

    /// The cover of an album card, when it names its artist (an artist page's cards name the
    /// kind and year instead, and playlists are YouTube's own).
    pub fn collection(card: &crate::library::Collection) -> Option<Self> {
        (card.kind == crate::library::Kind::Album && !card.subtitle.contains(" • "))
            .then(|| Self::album(&card.title, &card.subtitle))
    }

    /// What the answer is kept under: songs of one album share it.
    fn key(&self) -> String {
        match self {
            Self::Song {
                title,
                artist,
                album,
                ..
            } => format!(
                "{}|{}",
                artist.to_lowercase(),
                album.as_deref().unwrap_or(title).to_lowercase()
            ),
            Self::Album { title, artist } => {
                format!("{}|{}", artist.to_lowercase(), title.to_lowercase())
            }
        }
    }
}

fn lead(artist: &str) -> String {
    artist
        .split([',', '&'])
        .next()
        .unwrap_or_default()
        .trim()
        .to_owned()
}

/// The answers so far: a url template for a cover, `None` for an album Apple does not have.
fn known() -> &'static Mutex<HashMap<String, Option<String>>> {
    static KNOWN: OnceLock<Mutex<HashMap<String, Option<String>>>> = OnceLock::new();
    KNOWN.get_or_init(|| {
        let saved = std::fs::read(file())
            .ok()
            .and_then(|body| serde_json::from_slice(&body).ok())
            .unwrap_or_default();
        Mutex::new(saved)
    })
}

fn file() -> std::path::PathBuf {
    platform::data_dir().join("artwork.json")
}

/// The catalog search every lookup goes through, so they share its pace.
fn search() -> &'static motion::MotionSearch {
    static SEARCH: OnceLock<motion::MotionSearch> = OnceLock::new();
    SEARCH.get_or_init(motion::MotionSearch::default)
}

/// What is already known about `wanted`: `Some(Some(url))` for Apple's cover at `edge` px,
/// `Some(None)` when Apple has none, `None` when it has not been asked yet.
pub fn cached(wanted: &Wanted, edge: u32) -> Option<Option<String>> {
    let known = known().lock().ok()?;
    known
        .get(&wanted.key())
        .map(|template| template.as_deref().map(|template| sized(template, edge)))
}

/// Asks the catalog for `wanted`'s cover and remembers the answer. Apple's cover at `edge` px,
/// or `None` when it has none or could not be asked (then it is asked again next time).
/// Dropping the future gives up the lookup, so covers scrolled past do not hold the queue.
pub async fn find(wanted: Wanted, edge: u32) -> Option<String> {
    if let Some(answer) = cached(&wanted, edge) {
        return answer;
    }
    let lookup = Abort(runtime::spawn(async move {
        let search = search();
        let answer = match &wanted {
            Wanted::Song {
                title,
                artist,
                album,
                seconds,
            } => {
                let query = motion::MotionQuery::new(
                    title,
                    artist,
                    album.as_deref(),
                    seconds.map(std::time::Duration::from_secs),
                );
                search.artwork(&query).await
            }
            Wanted::Album { title, artist } => search.album_artwork(title, artist).await,
        };
        let template = match answer {
            Ok(template) => template,
            Err(error) => {
                log::debug!("artwork: {error:#}");
                return None;
            }
        };
        if let Ok(mut known) = known().lock() {
            known.insert(wanted.key(), template.clone());
            if let Ok(body) = serde_json::to_vec(&*known) {
                let _ = std::fs::write(file(), body);
            }
        }
        template
    }));
    let template = lookup.await?;
    Some(sized(&template, edge))
}

/// A lookup on the io runtime that is stopped when whoever waits for it lets go.
struct Abort(tokio::task::JoinHandle<Option<String>>);

impl Drop for Abort {
    fn drop(&mut self) {
        self.0.abort();
    }
}

impl std::future::Future for Abort {
    type Output = Option<String>;

    fn poll(
        mut self: std::pin::Pin<&mut Self>,
        context: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Self::Output> {
        std::pin::Pin::new(&mut self.0)
            .poll(context)
            .map(|joined| joined.ok().flatten())
    }
}

/// A catalog url template asked at `edge` px square.
fn sized(template: &str, edge: u32) -> String {
    template
        .replace("{w}", &edge.to_string())
        .replace("{h}", &edge.to_string())
        .replace("{f}", "jpg")
}

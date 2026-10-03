//! Finds Apple Music's motion artwork, the short silent loop some albums carry, for any track.
//!
//! It needs no account: the catalog answers the same public bearer token the web player uses.
//! A track that already comes from Apple is looked up by its album id; any other track is found
//! by name with the same scoring the Better Lyrics artwork service uses, see [`matching`]. A
//! miss is `Ok(None)` and worth remembering; an `Err` is transient and worth trying again later.

mod auth;
mod hls;
mod matching;
mod playlist;

use std::sync::Arc;
use std::time::{Duration, Instant};

use anyhow::{Context as _, Result, bail};
use reqwest::StatusCode;
use serde_json::Value;
use tokio::sync::Mutex;

use auth::AGENT;

pub use playlist::Loop;

/// The catalog host music.apple.com itself calls.
const API: &str = "https://amp-api.music.apple.com/v1";

/// The origin Apple expects next to the web player's token.
const ORIGIN: &str = "https://music.apple.com";

/// The storefront used until a signed-in Apple account names its own.
const STOREFRONT: &str = "us";

/// How many songs one search reads. Enough for the right edition to be among them.
const RESULTS: &str = "10";

/// The quietest pace between two catalog requests, the one the artwork service keeps for the
/// same public token.
const SPACING: Duration = Duration::from_secs(1);

/// How long to stay away from the catalog after it says too many requests.
const COOLDOWN: Duration = Duration::from_secs(60);

/// What a lookup searches by. Built from a track with [`MotionQuery::for_track`].
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct MotionQuery {
    title: String,
    artist: String,
    album: Option<String>,
    duration: Option<Duration>,
    apple_album: Option<String>,
}

/// An album's motion artwork: the HLS master playlist that lists every rendition of the loop.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MotionArt {
    pub album_id: String,
    pub master: String,
}

/// Looks motion artwork up in the Apple Music catalog. Cheap to clone; clones share one pace.
#[derive(Clone)]
pub struct MotionSearch {
    http: reqwest::Client,
    storefront: Arc<str>,
    pace: Arc<Mutex<Pace>>,
}

/// When the next catalog request may go out.
struct Pace {
    next: Instant,
}

impl MotionQuery {
    /// A query by name alone, for a track with no catalog album id to go by.
    pub fn new(title: &str, artist: &str, album: Option<&str>, duration: Option<Duration>) -> Self {
        Self {
            title: title.to_owned(),
            artist: artist.to_owned(),
            album: album.map(str::to_owned).filter(|album| !album.is_empty()),
            duration: duration.filter(|duration| !duration.is_zero()),
            apple_album: None,
        }
    }

    /// A key every track of one album shares, for caching what a lookup found.
    pub fn key(&self) -> String {
        match &self.apple_album {
            Some(id) => format!("apple:{id}"),
            None => format!(
                "{}|{}",
                matching::normalize(&self.artist),
                matching::normalize(self.album.as_deref().unwrap_or(&self.title))
            ),
        }
    }
}

impl Default for MotionSearch {
    fn default() -> Self {
        Self::new(STOREFRONT)
    }
}

impl MotionSearch {
    /// A search against `storefront`, a two-letter Apple storefront such as `us`.
    pub fn new(storefront: &str) -> Self {
        let http = reqwest::Client::builder()
            .user_agent(AGENT)
            .build()
            .unwrap_or_default();
        Self {
            http,
            storefront: storefront.into(),
            pace: Arc::new(Mutex::new(Pace {
                next: Instant::now(),
            })),
        }
    }

    /// The motion artwork of the album `query` belongs to, or `None` when it has none or no
    /// catalog song is a close enough match to trust.
    pub async fn find(&self, query: &MotionQuery) -> Result<Option<MotionArt>> {
        let album = match &query.apple_album {
            Some(id) => id.clone(),
            None => match self.search(query).await? {
                Some(id) => id,
                None => return Ok(None),
            },
        };
        self.album(&album).await
    }

    /// Downloads the H.264 rendition whose side covers `edge` physical pixels.
    pub async fn fetch(&self, art: &MotionArt, edge: u32) -> Result<Loop> {
        playlist::fetch(&self.http, &art.master, edge).await
    }

    /// The album id of the best scoring catalog song for `query`.
    async fn search(&self, query: &MotionQuery) -> Result<Option<String>> {
        // Brackets such as "(Official Video)" would narrow the search; scoring still sees them.
        let term = format!("{} {}", searchable(&query.title), query.artist);
        let path = format!("/catalog/{}/search", self.storefront);
        let answered = self
            .get(
                &path,
                &[
                    ("term", term.trim()),
                    ("types", "songs"),
                    ("limit", RESULTS),
                ],
            )
            .await?;
        let Some(answered) = answered else {
            return Ok(None);
        };
        let songs = answered
            .pointer("/results/songs/data")
            .and_then(Value::as_array)
            .map(|rows| {
                rows.iter()
                    .filter_map(matching::Song::read)
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        let best = matching::best(
            &songs,
            &query.title,
            &query.artist,
            query.album.as_deref(),
            query.duration,
        );
        if best.is_none() {
            log::debug!(
                "motion: no catalog match for {} by {}",
                query.title,
                query.artist
            );
        }
        Ok(best.map(|song| song.album_id.clone()))
    }

    /// The motion artwork of catalog album `id`, read from its `editorialVideo`.
    async fn album(&self, id: &str) -> Result<Option<MotionArt>> {
        let path = format!("/catalog/{}/albums/{id}", self.storefront);
        let Some(answered) = self.get(&path, &[("extend", "editorialVideo")]).await? else {
            return Ok(None);
        };
        let video = answered.pointer("/data/0/attributes/editorialVideo");
        let master = ["motionDetailSquare", "motionSquareVideo1x1"]
            .into_iter()
            .find_map(|kind| {
                video?
                    .pointer(&format!("/{kind}/video"))
                    .and_then(Value::as_str)
            })
            .filter(|url| url.starts_with("https://"));
        Ok(master.map(|master| MotionArt {
            album_id: id.to_owned(),
            master: master.to_owned(),
        }))
    }

    /// One catalog request, paced, with one retry on a stale token. `Ok(None)` is a 404.
    async fn get(&self, path: &str, query: &[(&str, &str)]) -> Result<Option<Value>> {
        for attempt in 0..2 {
            self.wait().await;
            let bearer = auth::bearer(&self.http).await?;
            let response = self
                .http
                .get(format!("{API}{path}"))
                .query(query)
                .bearer_auth(&bearer)
                .header(reqwest::header::ORIGIN, ORIGIN)
                .header(reqwest::header::REFERER, format!("{ORIGIN}/"))
                .send()
                .await
                .context("cannot reach the apple music catalog")?;
            match response.status() {
                StatusCode::UNAUTHORIZED if attempt == 0 => {
                    auth::forget_bearer(&bearer);
                    continue;
                }
                StatusCode::NOT_FOUND => return Ok(None),
                StatusCode::TOO_MANY_REQUESTS => {
                    self.pace.lock().await.next = Instant::now() + COOLDOWN;
                    bail!("the apple music catalog is rate limiting");
                }
                status if !status.is_success() => {
                    bail!("the apple music catalog answered with status {status}");
                }
                _ => {}
            }
            let body = response
                .json()
                .await
                .context("cannot read the apple music catalog")?;
            return Ok(Some(body));
        }
        bail!("the apple music catalog refused a fresh token")
    }

    /// Holds the caller until the pace allows the next request.
    async fn wait(&self) {
        let mut pace = self.pace.lock().await;
        let now = Instant::now();
        if pace.next > now {
            tokio::time::sleep(pace.next - now).await;
        }
        pace.next = Instant::now() + SPACING;
    }
}

/// `text` without anything in brackets, which would only narrow a catalog search.
fn searchable(text: &str) -> String {
    let mut kept = String::with_capacity(text.len());
    let mut depth = 0usize;
    for character in text.chars() {
        match character {
            '(' | '[' => depth += 1,
            ')' | ']' => depth = depth.saturating_sub(1),
            _ if depth == 0 => kept.push(character),
            _ => {}
        }
    }
    kept.split_whitespace().collect::<Vec<_>>().join(" ")
}

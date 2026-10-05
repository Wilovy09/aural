//! Lyrics for any song, ported from Sonora: the data model (lines, words, background lanes,
//! romanization), LRC and TTML parsing, the providers (Apple Music via Binimum, Musixmatch,
//! YouTube Music, LRCLIB, NetEase, KuGou) and the ranking that picks between their answers.

mod binimum;
mod escape;
mod kugou;
mod lrclib;
pub mod lyrics;
mod musixmatch;
mod netease;
mod youtube;

use std::sync::Arc;
use std::time::Duration;

use anyhow::Result;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use tokio::task::JoinSet;

pub use lyrics::{active, active_word, instrumental};

/// A sheet of lyrics: plain text, or lines timed to the song.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Lyrics {
    Plain {
        text: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        romanized: Option<RomanizedText>,
    },
    Synced {
        lines: Arc<[LyricsLine]>,
    },
}

impl Lyrics {
    pub fn plain(text: impl Into<String>) -> Self {
        let text = text.into();
        let romanized = crate::lyrics::romanize::plain(&text);
        Self::Plain { text, romanized }
    }

    pub fn synced(&self) -> bool {
        matches!(self, Self::Synced { .. })
    }

    pub fn worded(&self) -> bool {
        match self {
            Self::Plain { .. } => false,
            Self::Synced { lines } => lines.iter().any(LyricsLine::worded),
        }
    }

    pub fn is_empty(&self) -> bool {
        match self {
            Self::Plain { text, .. } => text.trim().is_empty(),
            Self::Synced { lines } => lines.is_empty(),
        }
    }

    pub fn span(&self) -> Option<Duration> {
        let Self::Synced { lines } = self else {
            return None;
        };
        lines
            .iter()
            .map(|line| line.end.unwrap_or(line.start))
            .max()
    }
}

/// One timed line: its text, its words when the sheet is word-synced, background vocals on
/// their own lanes, and which voice sings it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct LyricsLine {
    pub start: Duration,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub end: Option<Duration>,
    pub text: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub romanized: Option<RomanizedText>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub words: Option<Vec<LyricsWord>>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub secondary: Vec<LyricsLane>,
    #[serde(default, skip_serializing_if = "Voice::lead")]
    pub voice: Voice,
}

impl LyricsLine {
    pub fn worded(&self) -> bool {
        self.words.as_ref().is_some_and(|words| !words.is_empty())
            || self.secondary.iter().any(LyricsLane::worded)
    }

    pub fn sung_end(&self) -> Option<Duration> {
        let primary = self
            .words
            .as_ref()
            .and_then(|words| words.iter().rev().find(|word| !word.text.trim().is_empty()))
            .map(|word| word.end.max(word.start).max(self.start))
            .or(self.end);
        self.secondary
            .iter()
            .filter_map(LyricsLane::sung_end)
            .chain(primary)
            .max()
    }
}

/// A background vocal sung alongside a line.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct LyricsLane {
    pub start: Duration,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub end: Option<Duration>,
    pub text: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub romanized: Option<RomanizedText>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub words: Option<Vec<LyricsWord>>,
}

impl LyricsLane {
    pub fn worded(&self) -> bool {
        self.words.as_ref().is_some_and(|words| !words.is_empty())
    }

    pub fn sung_end(&self) -> Option<Duration> {
        self.words
            .as_ref()
            .and_then(|words| words.iter().rev().find(|word| !word.text.trim().is_empty()))
            .map(|word| word.end.max(word.start).max(self.start))
            .or(self.end)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RomanizedText {
    pub text: String,
    pub writing_system: WritingSystem,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum WritingSystem {
    Japanese,
    Chinese,
    Korean,
    Cyrillic,
    Greek,
    Arabic,
    Other,
}

impl WritingSystem {
    pub const ALL: [Self; 7] = [
        Self::Japanese,
        Self::Chinese,
        Self::Korean,
        Self::Cyrillic,
        Self::Greek,
        Self::Arabic,
        Self::Other,
    ];
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Voice {
    #[default]
    Lead,
    Counter,
}

impl Voice {
    pub fn lead(&self) -> bool {
        matches!(self, Self::Lead)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct LyricsWord {
    pub start: Duration,
    pub end: Duration,
    pub text: String,
}

/// A provider's own id for the song, when the caller has one.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TrackKey {
    pub provider: &'static str,
    pub id: String,
}

/// What a lookup searches by.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LyricsQuery {
    pub title: String,
    pub artist: String,
    pub album: Option<String>,
    pub duration: Duration,
    pub track: Option<TrackKey>,
}

impl LyricsQuery {
    pub fn id_for(&self, provider: &str) -> Option<&str> {
        self.track
            .as_ref()
            .filter(|track| track.provider == provider)
            .map(|track| track.id.as_str())
    }
}

/// One provider's answer, with what it says the song is so the ranking can check it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LyricsHit {
    pub source: &'static str,
    pub trust: u32,
    pub lyrics: Lyrics,
    pub instrumental: bool,
    pub title: String,
    pub artist: String,
    pub album: Option<String>,
    pub duration: Option<Duration>,
    pub writers: Vec<String>,
}

/// A catalogue song a provider matched, before its sheet is fetched.
#[derive(Clone, Debug)]
pub(crate) struct Track {
    pub id: Option<String>,
    pub name: String,
    pub artists: String,
    pub album: String,
    pub duration: Duration,
}

#[async_trait]
pub trait LyricsProvider: Send + Sync {
    fn name(&self) -> &'static str;
    async fn search(&self, query: &LyricsQuery) -> Result<Vec<LyricsHit>>;
}

/// Every provider, in no particular order; the ranking decides.
pub fn providers() -> Vec<Arc<dyn LyricsProvider>> {
    vec![
        Arc::new(binimum::Binimum::new()),
        Arc::new(musixmatch::Musixmatch::new()),
        Arc::new(youtube::YouTubeLyrics::new()),
        Arc::new(lrclib::LrcLib::new()),
        Arc::new(netease::NetEase::new()),
        Arc::new(kugou::Kugou::new()),
    ]
}

/// The source name Apple Music's sheets (through Binimum) carry.
pub const APPLE: &str = "Apple Music";

/// Asks every provider for `query` at once and sends each one's name and hits as they arrive,
/// so the first good sheet can show before the slow providers answer. Stops early when
/// `sender` closes.
pub async fn gather(
    providers: Vec<Arc<dyn LyricsProvider>>,
    query: LyricsQuery,
    sender: tokio::sync::mpsc::UnboundedSender<(&'static str, Vec<LyricsHit>)>,
) {
    let mut tasks = JoinSet::new();
    for provider in providers {
        let query = query.clone();
        tasks.spawn(async move {
            let hits = provider
                .search(&query)
                .await
                .inspect_err(|error| {
                    log::warn!("lyrics: {} did not answer: {error:#}", provider.name())
                })
                .unwrap_or_default();
            (provider.name(), hits)
        });
    }
    while !tasks.is_empty() {
        tokio::select! {
            _ = sender.closed() => break,
            found = tasks.join_next() => {
                if let Some(Ok(found)) = found {
                    sender.send(found).ok();
                }
            }
        }
    }
}

/// `hits` best first, with weak word timings re-timed against the best line-synced sheet.
/// Apple Music's sheet, when there is one for this song, comes first whatever the scores.
pub fn ordered(query: &LyricsQuery, hits: Vec<LyricsHit>) -> Vec<LyricsHit> {
    let mut ranked = lyrics::rank(query, hits);
    lyrics::reshape(&mut ranked);
    if let Some(apple) = ranked
        .iter()
        .position(|hit| hit.source == APPLE && !hit.lyrics.is_empty())
    {
        let hit = ranked.remove(apple);
        ranked.insert(0, hit);
    }
    ranked
}

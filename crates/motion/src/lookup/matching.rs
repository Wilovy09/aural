//! Picks the catalog song a track most likely is, ported from the Better Lyrics artwork
//! service (`src/search.ts`) so Sonora finds the same albums it does.
//!
//! Durations narrow the field first, a title has to share real words with the result, and the
//! rest is a weighted similarity of title, artist and album with penalties for a remix, a live
//! take or an acoustic version nobody asked for.

use std::collections::HashMap;
use std::time::Duration;

use serde_json::Value;
use unicode_normalization::UnicodeNormalization as _;
use unicode_normalization::char::is_combining_mark;

/// The lowest score still trusted. Below it a wrong album is likelier than the right one.
const THRESHOLD: f64 = 0.6;

/// How far a result's length may be from the track's and still count as the same recording.
const DURATION_DELTA: Duration = Duration::from_millis(2000);

/// Words too common to prove two titles are about the same song.
const STOPWORDS: &[&str] = &[
    "the", "and", "feat", "with", "you", "for", "from", "version",
];

/// One catalog song, reduced to what scoring reads.
#[derive(Debug)]
pub(super) struct Song {
    pub album_id: String,
    /// The catalog's artwork url template, with `{w}` and `{h}` for the size.
    pub artwork: Option<String>,
    name: String,
    artist: String,
    album: String,
    duration: Option<Duration>,
}

impl Song {
    /// Reads one row of a catalog search. A row with no album id it can name is skipped.
    pub fn read(row: &Value) -> Option<Self> {
        let attributes = row.get("attributes")?;
        let text = |key: &str| {
            attributes
                .get(key)
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_owned()
        };
        let album_id = row
            .pointer("/relationships/albums/data/0/id")
            .and_then(Value::as_str)
            .map(str::to_owned)
            .or_else(|| album_from_url(&text("url")))?;
        Some(Self {
            album_id,
            artwork: artwork(attributes),
            name: text("name"),
            artist: text("artistName"),
            album: text("albumName"),
            duration: attributes
                .get("durationInMillis")
                .and_then(Value::as_u64)
                .map(Duration::from_millis),
        })
    }
}

/// The artwork url template of a catalog row's attributes.
pub(super) fn artwork(attributes: &Value) -> Option<String> {
    attributes
        .pointer("/artwork/url")
        .and_then(Value::as_str)
        .filter(|url| url.starts_with("https://"))
        .map(str::to_owned)
}

/// The best scoring song for the wanted title and artist, if any clears [`THRESHOLD`].
pub(super) fn best<'a>(
    songs: &'a [Song],
    title: &str,
    artist: &str,
    album: Option<&str>,
    duration: Option<Duration>,
) -> Option<&'a Song> {
    let near: Vec<&Song> = match duration {
        Some(wanted) => songs
            .iter()
            .filter(|song| {
                song.duration
                    .is_some_and(|it| it.abs_diff(wanted) <= DURATION_DELTA)
            })
            .collect(),
        None => Vec::new(),
    };
    let field: Vec<&Song> = match near.is_empty() {
        true => songs.iter().collect(),
        false => near,
    };
    field
        .into_iter()
        .filter_map(|song| score(song, title, artist, album).map(|score| (song, score)))
        .filter(|(_, score)| *score >= THRESHOLD)
        .max_by(|(_, one), (_, other)| one.total_cmp(other))
        .map(|(song, _)| song)
}

/// How alike `song` is to what was asked for, or `None` when the titles share nothing real.
fn score(song: &Song, title: &str, artist: &str, album: Option<&str>) -> Option<f64> {
    let name = normalize(&song.name);
    let found_artist = normalize(&song.artist);
    let found_album = normalize(&song.album);
    let wanted = normalize(title);
    let wanted_artist = normalize(artist);

    if !title_matches(&wanted, &name) && !title_matches(&wanted, &found_album) {
        return None;
    }

    let title_score = similarity(&name, &wanted);
    let artist_score = similarity(&found_artist, &wanted_artist);
    let mut score = match album.map(normalize) {
        Some(wanted_album) => {
            title_score * 0.5
                + artist_score * 0.375
                + similarity(&found_album, &wanted_album) * 0.125
        }
        None => title_score * (50. / 87.5) + artist_score * (37.5 / 87.5),
    };

    let unasked = |word: &str| !wanted.contains(word) && name.contains(word);
    if unasked("remix") {
        score -= 0.15;
    }
    if unasked("live") {
        score -= 0.10;
    }
    if unasked("acoustic") {
        score -= 0.075;
    }
    Some(score)
}

/// Whether two normalised titles share more than letters: one holds the other, or they share a
/// word of three letters or more that is not a stopword. Letter-frequency similarity alone rates
/// unrelated titles as close. Titles outside the Latin script always pass, since their words
/// are not separated the same way.
fn title_matches(wanted: &str, found: &str) -> bool {
    if wanted.is_empty() || found.is_empty() {
        return false;
    }
    if !latin(wanted) || !latin(found) {
        return true;
    }
    let one = wanted.replace(' ', "");
    let other = found.replace(' ', "");
    if other.contains(&one) || (other.chars().count() >= 3 && one.contains(&other)) {
        return true;
    }
    let words: Vec<&str> = found.split(' ').collect();
    wanted.split(' ').any(|word| {
        word.chars().count() >= 3 && !STOPWORDS.contains(&word) && words.contains(&word)
    })
}

/// 1.0 for equal strings, 0.7 and up when one holds the other, otherwise the Dice coefficient
/// of their letter counts.
fn similarity(one: &str, other: &str) -> f64 {
    if one == other {
        return 1.;
    }
    let (one_len, other_len) = (one.chars().count(), other.chars().count());
    if one.contains(other) || other.contains(one) {
        let (short, long) = (one_len.min(other_len), one_len.max(other_len));
        return 0.7 + 0.3 * short as f64 / long as f64;
    }
    if one_len + other_len == 0 {
        return 0.;
    }
    let counts = |text: &str| {
        let mut counted = HashMap::new();
        for letter in text.chars() {
            *counted.entry(letter).or_insert(0usize) += 1;
        }
        counted
    };
    let theirs = counts(other);
    let shared: usize = counts(one)
        .into_iter()
        .map(|(letter, count)| count.min(theirs.get(&letter).copied().unwrap_or(0)))
        .sum();
    (shared * 2) as f64 / (one_len + other_len) as f64
}

/// Lowercase, without accents or punctuation, with every run of spaces made one.
pub(super) fn normalize(text: &str) -> String {
    let stripped: String = text
        .nfkd()
        .flat_map(char::to_lowercase)
        .filter(|letter| !is_combining_mark(*letter))
        .filter(|letter| letter.is_alphanumeric() || letter.is_whitespace())
        .nfc()
        .collect();
    stripped.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Whether every letter of `text` is Latin. Accents are already gone after [`normalize`], so the
/// Latin blocks left to allow are the basic, extended and fullwidth ones.
fn latin(text: &str) -> bool {
    text.chars()
        .filter(|letter| letter.is_alphabetic())
        .all(|letter| {
            matches!(letter as u32,
            0x41..=0x5A | 0x61..=0x7A | 0xAA | 0xBA | 0xC0..=0x24F | 0x1E00..=0x1EFF
            | 0x2C60..=0x2C7F | 0xA720..=0xA7FF | 0xFF21..=0xFF3A | 0xFF41..=0xFF5A)
        })
}

/// The album id in a catalog song url such as `https://music.apple.com/us/album/name/123?i=4`.
fn album_from_url(url: &str) -> Option<String> {
    let (_, rest) = url.split_once("/album/")?;
    let path = rest.split(['?', '#']).next()?;
    // The id is the last numeric part, since an album can be named "1989".
    path.split('/')
        .rev()
        .find(|part| !part.is_empty() && part.bytes().all(|byte| byte.is_ascii_digit()))
        .map(str::to_owned)
}

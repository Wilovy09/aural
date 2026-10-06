//! A sheet's lines in the listener's language, from the best source that has them: the
//! sheet's own (Apple Music's carry some), then people's translations from Musixmatch, then a
//! machine translation, marked as such.

use std::time::Duration;

use anyhow::{Context as _, Result};
use serde_json::Value;

use crate::{LyricsLine, LyricsQuery};

/// What a translation is and where it came from.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Translation {
    /// One entry per line of the sheet, `None` for a line left as it is.
    pub lines: Vec<Option<String>>,
    pub source: &'static str,
    /// Whether a machine made it rather than people.
    pub machine: bool,
}

const MACHINE: &str = "https://translate.googleapis.com/translate_a/single";
/// Musixmatch's mobile API: unlike the desktop one, it still hands out people's translations.
const MOBILE: &str = "https://apic-appmobile.musixmatch.com/ws/1.1";
const MOBILE_APP: &str = "mac-ios-v2.0";
/// How much text one machine request carries.
const CHUNK: usize = 1500;

/// `lines` translated into the listener's language, or `None` when no source has them or they
/// already are in it.
pub async fn translate(query: &LyricsQuery, lines: &[LyricsLine]) -> Option<Translation> {
    let language = crate::language();
    if lines.iter().any(|line| line.translation.is_some()) {
        return Some(Translation {
            lines: lines.iter().map(|line| line.translation.clone()).collect(),
            source: crate::APPLE,
            machine: false,
        });
    }
    match people(query, &language).await {
        Ok(Some(pairs)) => {
            let found = matched(lines, &pairs);
            if found.iter().filter(|line| line.is_some()).count() * 2 >= sung(lines) {
                return Some(Translation {
                    lines: found,
                    source: "Musixmatch",
                    machine: false,
                });
            }
        }
        Ok(None) => {}
        Err(error) => log::info!("lyrics: no musixmatch translation: {error:#}"),
    }
    match machine(lines, &language).await {
        Ok(found) => found.map(|lines| Translation {
            lines,
            source: "Google Translate",
            machine: true,
        }),
        Err(error) => {
            log::warn!("lyrics: cannot translate: {error:#}");
            None
        }
    }
}

/// The song's lines as people translated them into `language` on Musixmatch, as pairs of
/// the line and its translation.
async fn people(query: &LyricsQuery, language: &str) -> Result<Option<Vec<(String, String)>>> {
    let http = reqwest::Client::builder()
        .timeout(Duration::from_secs(10))
        .user_agent("Mozilla/5.0")
        .build()
        .unwrap_or_default();
    let token = mobile_token(&http).await?;
    let duration = query.duration.as_secs().to_string();
    let mut params = vec![
        ("app_id", MOBILE_APP),
        ("format", "json"),
        ("usertoken", token.as_str()),
        ("q_track", query.title.as_str()),
        ("q_artist", query.artist.as_str()),
    ];
    if !query.duration.is_zero() {
        params.push(("q_duration", duration.as_str()));
    }
    let matched = mobile(&http, "matcher.track.get", &params).await?;
    let Some(id) = matched
        .pointer("/message/body/track/track_id")
        .and_then(Value::as_u64)
    else {
        return Ok(None);
    };
    let id = id.to_string();
    let found = mobile(
        &http,
        "crowd.track.translations.get",
        &[
            ("app_id", MOBILE_APP),
            ("format", "json"),
            ("usertoken", token.as_str()),
            ("track_id", id.as_str()),
            ("selected_language", language),
            ("translation_fields_set", "minimal"),
            ("comment_format", "text"),
        ],
    )
    .await?;
    let pairs: Vec<(String, String)> = found
        .pointer("/message/body/translations_list")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|entry| {
            let translation = entry.get("translation")?;
            let text = |key: &str| {
                translation
                    .get(key)
                    .and_then(Value::as_str)
                    .map(str::trim)
                    .filter(|text| !text.is_empty())
            };
            let original = text("matched_line").or_else(|| text("snippet"))?;
            Some((original.to_owned(), text("description")?.to_owned()))
        })
        .collect();
    Ok((!pairs.is_empty()).then_some(pairs))
}

/// One call to the mobile API, failing on what it answers in its own header.
async fn mobile(http: &reqwest::Client, method: &str, params: &[(&str, &str)]) -> Result<Value> {
    let answer: Value = http
        .get(format!("{MOBILE}/{method}"))
        .query(params)
        .send()
        .await
        .context("cannot reach musixmatch")?
        .json()
        .await
        .context("cannot read musixmatch's answer")?;
    match answer
        .pointer("/message/header/status_code")
        .and_then(Value::as_i64)
    {
        Some(200) => Ok(answer),
        Some(401) => {
            // The token went stale or was refused: the next song asks for a new one.
            let _ = std::fs::remove_file(token_file());
            anyhow::bail!("musixmatch refused the token")
        }
        Some(404) => Ok(Value::Null),
        status => anyhow::bail!("musixmatch answered {status:?}"),
    }
}

/// The mobile API's token, kept on disk: asking for one too often brings a captcha.
async fn mobile_token(http: &reqwest::Client) -> Result<String> {
    if let Ok(kept) = std::fs::read_to_string(token_file()) {
        let kept = kept.trim().to_owned();
        if !kept.is_empty() {
            return Ok(kept);
        }
    }
    let answer = mobile(
        http,
        "token.get",
        &[("app_id", MOBILE_APP), ("format", "json")],
    )
    .await?;
    let token = answer
        .pointer("/message/body/user_token")
        .and_then(Value::as_str)
        .filter(|token| !token.contains("UpgradeOnly"))
        .context("musixmatch handed out no token")?
        .to_owned();
    let path = token_file();
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let _ = std::fs::write(path, &token);
    Ok(token)
}

fn token_file() -> std::path::PathBuf {
    dirs::cache_dir()
        .unwrap_or_else(std::env::temp_dir)
        .join("aural")
        .join("musixmatch-mobile")
}

/// How many lines have words to translate.
fn sung(lines: &[LyricsLine]) -> usize {
    lines
        .iter()
        .filter(|line| !line.text.trim().is_empty())
        .count()
}

/// The translation of each line: the pair whose original reads most like it, if it reads
/// like it at all. Sources spell lines a little differently ("pleasin'" and "pleasing").
fn matched(lines: &[LyricsLine], pairs: &[(String, String)]) -> Vec<Option<String>> {
    let originals: Vec<String> = pairs.iter().map(|(original, _)| normal(original)).collect();
    lines
        .iter()
        .map(|line| {
            let wanted = normal(&line.text);
            if wanted.is_empty() {
                return None;
            }
            originals
                .iter()
                .enumerate()
                .map(|(at, original)| (at, alike(&wanted, original)))
                .filter(|(_, score)| *score >= 0.8)
                .max_by(|a, b| a.1.total_cmp(&b.1))
                .map(|(at, _)| pairs[at].1.clone())
        })
        .collect()
}

/// How alike two normalised lines are, from 0 to 1, by the pairs of letters they share.
fn alike(a: &str, b: &str) -> f32 {
    if a == b {
        return 1.;
    }
    let pairs = |text: &str| -> Vec<(char, char)> {
        let chars: Vec<char> = text.chars().collect();
        chars.windows(2).map(|pair| (pair[0], pair[1])).collect()
    };
    let (mut left, right) = (pairs(a), pairs(b));
    if left.is_empty() || right.is_empty() {
        return 0.;
    }
    let total = left.len() + right.len();
    let mut shared = 0;
    for pair in &right {
        if let Some(at) = left.iter().position(|it| it == pair) {
            left.swap_remove(at);
            shared += 1;
        }
    }
    2. * shared as f32 / total as f32
}

/// `text` without case, punctuation or spacing, for matching lines across sources.
fn normal(text: &str) -> String {
    text.chars()
        .filter(|c| c.is_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect()
}

/// `lines` through a machine translation, `None` when they already are in `language`.
async fn machine(lines: &[LyricsLine], language: &str) -> Result<Option<Vec<Option<String>>>> {
    let http = reqwest::Client::builder()
        .timeout(Duration::from_secs(10))
        .build()
        .unwrap_or_default();
    let texts: Vec<&str> = lines.iter().map(|line| line.text.trim()).collect();
    let mut out: Vec<Option<String>> = vec![None; lines.len()];
    let mut start = 0;
    while start < texts.len() {
        // A chunk of whole lines, one per line of the request.
        let mut end = start;
        let mut size = 0;
        while end < texts.len() && (end == start || size + texts[end].len() < CHUNK) {
            size += texts[end].len() + 1;
            end += 1;
        }
        let chunk: Vec<&str> = texts[start..end].to_vec();
        let joined = chunk.join("\n");
        let answer: Value = http
            .get(MACHINE)
            .query(&[
                ("client", "gtx"),
                ("sl", "auto"),
                ("tl", language),
                ("dt", "t"),
                ("q", joined.as_str()),
            ])
            .send()
            .await
            .context("cannot reach the translator")?
            .error_for_status()
            .context("the translator refused")?
            .json()
            .await
            .context("cannot read the translation")?;
        // Already in the listener's language: nothing to show.
        let detected = answer.get(2).and_then(Value::as_str).unwrap_or_default();
        if start == 0 && detected.to_lowercase().starts_with(language) {
            return Ok(None);
        }
        let translated: String = answer
            .get(0)
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(|segment| segment.get(0).and_then(Value::as_str))
            .collect();
        let parts: Vec<&str> = translated.split('\n').collect();
        if parts.len() == chunk.len() {
            for (at, part) in parts.into_iter().enumerate() {
                let part = part.trim();
                if !part.is_empty() && !chunk[at].is_empty() && normal(part) != normal(chunk[at]) {
                    out[start + at] = Some(part.to_owned());
                }
            }
        }
        start = end;
    }
    Ok(out.iter().any(Option::is_some).then_some(out))
}

#[cfg(test)]
mod tests {
    use super::normal;

    #[test]
    fn spellings_of_one_line_match() {
        assert!(
            super::alike(
                &normal("People-pleasin' planet"),
                &normal("People-pleasing planet")
            ) > 0.8
        );
        assert!(
            super::alike(
                &normal("I'll take the beach"),
                &normal("You could have the mountains")
            ) < 0.5
        );
    }

    #[test]
    fn lines_match_whatever_the_punctuation() {
        assert_eq!(
            normal("Push comes to shove, ah"),
            normal("push comes to shove ah!")
        );
    }
}

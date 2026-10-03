//! Turns a motion artwork master playlist into one small file a platform decoder can open.
//!
//! Apple lists each loop in H.264 and HEVC at several sizes. Every rendition is a single fMP4
//! addressed by byte ranges, so the whole loop is one ranged GET once the right rendition is
//! known: the smallest H.264 one that still covers the side it will be drawn at.

use anyhow::{Context as _, Result, bail};
use bytes::Bytes;

use super::hls::{absolute, attributes};

/// The largest loop body accepted. A 1080 px rendition is about 12 MB; this only bounds what a
/// wrong answer can cost.
const CEILING: u64 = 32 << 20;

/// A downloaded loop, ready to be written to disk and opened by a decoder.
pub struct Loop {
    pub bytes: Bytes,
    pub side: u32,
}

/// One rendition a master playlist lists.
#[derive(Debug, PartialEq, Eq)]
struct Variant {
    codecs: String,
    side: u32,
    bandwidth: u64,
    uri: String,
}

/// Fetches the master, picks a rendition for `edge` and downloads its whole file.
pub(super) async fn fetch(http: &reqwest::Client, master: &str, edge: u32) -> Result<Loop> {
    let listed = text(http, master).await?;
    let variants = variants(&listed, master);
    let chosen = pick(&variants, edge).context("the motion artwork has no h.264 rendition")?;
    let media = text(http, &chosen.uri).await?;
    let (file, end) = layout(&media, &chosen.uri)?;
    if end > CEILING {
        bail!("the motion artwork is {end} bytes, over the ceiling");
    }
    let response = http
        .get(&file)
        .header(reqwest::header::RANGE, format!("bytes=0-{}", end - 1))
        .send()
        .await
        .context("cannot reach the motion artwork")?
        .error_for_status()
        .context("the motion artwork was refused")?;
    let bytes = response
        .bytes()
        .await
        .context("cannot read the motion artwork")?;
    if (bytes.len() as u64) < end {
        bail!("the motion artwork arrived short");
    }
    Ok(Loop {
        bytes,
        side: chosen.side,
    })
}

async fn text(http: &reqwest::Client, url: &str) -> Result<String> {
    http.get(url)
        .send()
        .await
        .context("cannot reach a motion artwork playlist")?
        .error_for_status()
        .context("a motion artwork playlist was refused")?
        .text()
        .await
        .context("cannot read a motion artwork playlist")
}

/// The renditions of a master playlist: each `EXT-X-STREAM-INF` and the uri on the line after.
fn variants(master: &str, base: &str) -> Vec<Variant> {
    let mut found = Vec::new();
    let mut lines = master.lines().map(str::trim);
    while let Some(line) = lines.next() {
        let Some(rest) = line.strip_prefix("#EXT-X-STREAM-INF:") else {
            continue;
        };
        let Some(uri) = lines
            .next()
            .filter(|uri| !uri.is_empty() && !uri.starts_with('#'))
        else {
            continue;
        };
        let read = attributes(rest);
        let value = |wanted: &str| {
            read.iter()
                .find(|(name, _)| name == wanted)
                .map(|(_, value)| value.as_str())
        };
        let side = value("RESOLUTION")
            .and_then(|it| it.split_once('x'))
            .and_then(|(width, height)| Some(width.parse::<u32>().ok()?.min(height.parse().ok()?)));
        let Some(side) = side else {
            continue;
        };
        found.push(Variant {
            codecs: value("CODECS").unwrap_or_default().to_owned(),
            side,
            bandwidth: value("BANDWIDTH")
                .and_then(|it| it.parse().ok())
                .unwrap_or(0),
            uri: absolute(uri, base),
        });
    }
    found
}

/// The smallest H.264 rendition at least `edge` on a side, or the largest when none is. Among
/// equal sides the lighter stream wins, since the loop is drawn under a cover-sized box anyway.
fn pick(variants: &[Variant], edge: u32) -> Option<&Variant> {
    let h264 = || variants.iter().filter(|it| it.codecs.starts_with("avc1"));
    h264()
        .filter(|it| it.side >= edge)
        .min_by_key(|it| (it.side, it.bandwidth))
        .or_else(|| h264().max_by_key(|it| (it.side, std::cmp::Reverse(it.bandwidth))))
}

/// The one file a media playlist addresses and the byte its last range ends at. A playlist
/// whose segments live in different files is refused, since the decoder opens one file.
fn layout(media: &str, base: &str) -> Result<(String, u64)> {
    let mut file: Option<String> = None;
    let mut end = 0u64;
    let mut pending: Option<u64> = None;
    let mut claim = |uri: &str, last: u64| -> Result<()> {
        let uri = absolute(uri, base);
        match &file {
            Some(known) if *known != uri => bail!("the motion artwork spans more than one file"),
            Some(_) => {}
            None => file = Some(uri),
        }
        end = end.max(last);
        Ok(())
    };
    for line in media.lines().map(str::trim) {
        if let Some(rest) = line.strip_prefix("#EXT-X-MAP:") {
            let read = attributes(rest);
            let value = |wanted: &str| {
                read.iter()
                    .find(|(name, _)| name == wanted)
                    .map(|(_, value)| value.clone())
            };
            let uri = value("URI").context("the motion artwork map names no file")?;
            let last = value("BYTERANGE")
                .as_deref()
                .and_then(range_end)
                .unwrap_or(0);
            claim(&uri, last)?;
        } else if let Some(rest) = line.strip_prefix("#EXT-X-BYTERANGE:") {
            pending = range_end(rest);
        } else if !line.is_empty() && !line.starts_with('#') {
            let last = pending
                .take()
                .context("the motion artwork segment has no byte range")?;
            claim(line, last)?;
        }
    }
    let file = file.context("the motion artwork playlist names no file")?;
    if end == 0 {
        bail!("the motion artwork playlist names no byte ranges");
    }
    Ok((file, end))
}

/// Where a `length@offset` byte range ends.
fn range_end(range: &str) -> Option<u64> {
    let (length, offset) = range.trim().split_once('@')?;
    length
        .parse::<u64>()
        .ok()?
        .checked_add(offset.parse().ok()?)
}

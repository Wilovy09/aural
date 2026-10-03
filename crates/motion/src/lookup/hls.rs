//! The few HLS playlist rules more than one module reads by: tag attributes and relative urls.

/// The `NAME=VALUE` pairs of an HLS tag, with quoted values unwrapped. A comma inside quotes
/// belongs to the value.
pub(super) fn attributes(line: &str) -> Vec<(String, String)> {
    let mut found = Vec::new();
    let mut rest = line;
    while !rest.is_empty() {
        let Some((name, tail)) = rest.split_once('=') else {
            break;
        };
        let (value, tail) = match tail.strip_prefix('"') {
            Some(quoted) => match quoted.split_once('"') {
                Some((value, tail)) => (value, tail.strip_prefix(',').unwrap_or(tail)),
                None => (quoted, ""),
            },
            None => match tail.split_once(',') {
                Some((value, tail)) => (value, tail),
                None => (tail, ""),
            },
        };
        found.push((name.trim().to_owned(), value.to_owned()));
        rest = tail;
    }
    found
}

/// Resolves a playlist-relative url against the playlist's own.
pub(super) fn absolute(url: &str, base: &str) -> String {
    if url.starts_with("http://") || url.starts_with("https://") {
        return url.to_owned();
    }
    match base.rsplit_once('/') {
        Some((root, _)) => format!("{root}/{url}"),
        None => url.to_owned(),
    }
}

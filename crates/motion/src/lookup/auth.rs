//! The Apple Music web player's public bearer token, read off music.apple.com. It is the same
//! for every visitor and lasts months; it is read off the page rather than registered for,
//! which makes it the brittle half of the lookup and the reason it is isolated here.

use std::sync::RwLock;

use anyhow::{Context as _, Result};

/// A bearer token supplied by hand, which skips reading one off the page.
pub const BEARER_ENV: &str = "AURAL_APPLE_BEARER_TOKEN";

/// What a browser calls itself. Apple's web endpoints answer a browser, and a bare reqwest
/// agent gets a different page.
pub(super) const AGENT: &str = "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/17.5 Safari/605.1.15";

/// The page carrying the web player's bearer token, either in its own markup or in the script
/// bundle it names.
const PLAYER: &str = "https://music.apple.com/";

/// The bearer token this process is using. It is the same for every visitor and lasts months,
/// so it is read once, and only read again after [`forget_bearer`].
static BEARER: RwLock<Option<String>> = RwLock::new(None);

/// The web player's bearer token: the environment's, the one already read, or one read off the
/// page now.
pub async fn bearer(http: &reqwest::Client) -> Result<String> {
    if let Some(token) = std::env::var(BEARER_ENV).ok().filter(|it| !it.is_empty()) {
        return Ok(token);
    }
    if let Some(token) = BEARER.read().ok().and_then(|held| held.clone()) {
        return Ok(token);
    }
    let token = read_bearer(http).await?;
    log::debug!("apple: read a bearer token of {} characters", token.len());
    let Ok(mut held) = BEARER.write() else {
        return Ok(token);
    };
    Ok(held.get_or_insert(token).clone())
}

/// Drops `stale` after Apple refused it, so the next [`bearer`] reads the page again. A token
/// another caller already replaced is left alone.
pub fn forget_bearer(stale: &str) {
    if let Ok(mut held) = BEARER.write()
        && held.as_deref() == Some(stale)
    {
        *held = None;
    }
}

/// Reads the token off music.apple.com: out of the page itself, or out of the script bundle the
/// page names. Apple has moved it between the two, so both are tried before giving up.
async fn read_bearer(http: &reqwest::Client) -> Result<String> {
    let html = http
        .get(PLAYER)
        .header(reqwest::header::USER_AGENT, AGENT)
        .send()
        .await
        .context("cannot reach music.apple.com")?
        .error_for_status()
        .context("music.apple.com refused the request")?
        .text()
        .await
        .context("cannot read music.apple.com")?;

    if let Some(token) = jwt(&html) {
        return Ok(token.to_owned());
    }
    let path = bundle(&html).context("music.apple.com names no script bundle")?;
    let script = http
        .get(format!("https://music.apple.com{path}"))
        .header(reqwest::header::USER_AGENT, AGENT)
        .send()
        .await
        .context("cannot fetch the apple music script bundle")?
        .error_for_status()
        .context("apple refused the script bundle")?
        .text()
        .await
        .context("cannot read the apple music script bundle")?;
    jwt(&script)
        .map(str::to_owned)
        .context("the apple music script bundle carries no bearer token")
}

/// The path of the script bundle the page names, `/assets/index~<hash>.js`.
fn bundle(html: &str) -> Option<&str> {
    let at = html.find("/assets/index~")?;
    let rest = &html[at..];
    let end = rest.find(".js")? + ".js".len();
    let path = rest.get(..end)?;
    (!path["/assets/".len()..].contains('/')).then_some(path)
}

/// The first JWT in a body: three dot-separated base64url runs starting with the `eyJ` that a
/// JSON header always encodes to. Length is what tells a token from an incidental match.
fn jwt(body: &str) -> Option<&str> {
    let mut from = 0usize;
    while let Some(at) = body[from..].find("eyJ") {
        let start = from + at;
        let len = body[start..]
            .chars()
            .take_while(|letter| {
                letter.is_ascii_alphanumeric() || matches!(letter, '-' | '_' | '.')
            })
            .count();
        let token = &body[start..start + len];
        if token.matches('.').count() == 2 && token.len() >= 100 {
            return Some(token);
        }
        from = start + 3;
    }
    None
}

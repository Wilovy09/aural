//! The YouTube Music session: signing in through the web window, remembering the account on
//! disk, restoring it at launch and keeping its cookies fresh.
//!
//! The stored file holds the account's cookies in plain JSON inside the app's private folder.
//! Encrypting it with the platform keystore is still to do.

use std::path::PathBuf;
use std::sync::{Arc, Weak};
use std::time::Duration;

use anyhow::{Context as _, Result, bail};
use serde::{Deserialize, Serialize};
use ytmusic::YtMusic;

use crate::{login, platform};

/// The cookies that prove a session is signed in.
const PROOF: [&str; 2] = ["SAPISID", "__Secure-3PAPISID"];
/// How often a signed-in session asks Google for fresh cookies, the cadence of an open tab.
const ROTATION: Duration = Duration::from_secs(10 * 60);
/// Account slots Google may hold one browser session for.
const SLOTS: usize = 8;

/// Who is signed in, shown in the sidebar.
#[derive(Clone, Debug, PartialEq)]
pub struct Account {
    pub name: String,
    pub email: Option<String>,
    pub photo: Option<String>,
}

/// What is written to disk for a signed-in account.
#[derive(Serialize, Deserialize)]
struct Saved {
    cookies: String,
    authuser: usize,
    page_id: Option<String>,
}

/// A live session: the client every request goes through and whose account it is.
#[derive(Clone)]
pub struct Session {
    pub api: Arc<YtMusic>,
    pub account: Option<Account>,
}

impl Session {
    /// A session with no account, for browsing and playing as a guest.
    pub fn guest() -> Self {
        Self {
            api: Arc::new(client(YtMusic::anonymous())),
            account: None,
        }
    }
}

/// The stored account, if one was saved and still answers. Runs on the caller's runtime.
pub async fn restore() -> Result<Option<Session>> {
    let Ok(body) = std::fs::read(file()) else {
        return Ok(None);
    };
    let saved: Saved = serde_json::from_slice(&body).context("cannot read the saved session")?;
    let api = authed(&saved);
    let stale = std::fs::metadata(cookie_file())
        .and_then(|meta| meta.modified())
        .ok()
        .and_then(|at| at.elapsed().ok())
        .is_none_or(|elapsed| elapsed >= ROTATION);
    if stale && let Err(error) = api.rotate_cookies().await {
        log::warn!("session: cannot rotate the stored cookies: {error:#}");
    }
    let profile = match api.profile().await {
        Ok(profile) => profile,
        Err(error) => {
            log::warn!("session: the saved account no longer answers: {error:#}");
            return Ok(None);
        }
    };
    Ok(Some(start(api, profile)))
}

/// Opens the sign-in window, then picks the account the cookies belong to and saves it. Blocks,
/// so it runs on a worker thread; `say` hears each step.
pub fn sign_in(say: &dyn Fn(String)) -> Result<Session> {
    let header = match login::supported() {
        true => login::web_sign_in(say)?,
        false => manual_cookies()?,
    };
    if !PROOF
        .iter()
        .any(|name| header.contains(&format!("{name}=")))
    {
        bail!("the sign-in brought back no YouTube session");
    }
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .context("cannot start tokio")?;
    say("buscando tu cuenta".into());
    let saved = runtime.block_on(choose(&header))?;
    let body = serde_json::to_vec(&saved).context("cannot encode the session")?;
    std::fs::write(file(), body).context("cannot save the session")?;
    // Rotated cookies of an earlier account would replace the ones just brought back.
    let _ = std::fs::remove_file(cookie_file());
    runtime
        .block_on(restore())?
        .context("the new session does not answer")
}

/// Forgets the stored account.
pub fn sign_out() {
    let _ = std::fs::remove_file(file());
    let _ = std::fs::remove_file(cookie_file());
}

/// The first account slot that answers with a profile. Google lists every signed-in account
/// of the browser session under `authuser` 0, 1, …; a brand account adds a page id.
async fn choose(header: &str) -> Result<Saved> {
    for authuser in 0..SLOTS {
        let api = YtMusic::with_cookies(header).as_user(authuser);
        let Ok(identities) = api.identities().await else {
            continue;
        };
        if let Some(identity) = identities.into_iter().next() {
            log::info!("session: account slot {authuser} answers");
            return Ok(Saved {
                cookies: header.to_owned(),
                authuser,
                page_id: identity.page_id,
            });
        }
    }
    bail!("no account answered for these cookies")
}

/// A client for `saved`, writing rotated cookies back to disk.
fn authed(saved: &Saved) -> YtMusic {
    let mut api = YtMusic::with_cookies(saved.cookies.clone()).as_user(saved.authuser);
    if let Some(page) = &saved.page_id {
        api = api.as_page(page.clone());
    }
    client(api.persist_cookies(cookie_file()))
}

/// The settings every client shares: the player script cache.
fn client(api: YtMusic) -> YtMusic {
    api.cache_player(platform::data_dir().join("player.json"))
}

fn start(api: YtMusic, profile: ytmusic::Profile) -> Session {
    let api = Arc::new(api);
    keep_fresh(Arc::downgrade(&api));
    Session {
        api,
        account: Some(Account {
            name: profile.name,
            email: profile.email,
            photo: ytmusic::best_thumbnail(&profile.thumbnails).map(|thumb| thumb.url.clone()),
        }),
    }
}

/// Rotates the session's cookies on a schedule until the client is dropped.
fn keep_fresh(api: Weak<YtMusic>) {
    std::thread::spawn(move || {
        let Ok(runtime) = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
        else {
            return;
        };
        loop {
            std::thread::sleep(ROTATION);
            let Some(api) = api.upgrade() else {
                return;
            };
            if let Err(error) = runtime.block_on(api.rotate_cookies()) {
                log::warn!("session: cannot rotate cookies: {error:#}");
            }
        }
    });
}

/// Where the rotating cookie store is saved.
fn cookie_file() -> PathBuf {
    platform::data_dir().join("cookies.json")
}

/// Where the account is saved.
fn file() -> PathBuf {
    platform::data_dir().join("session.json")
}

/// The cookie header for a platform with no sign-in window yet: `AURAL_COOKIES`, or the
/// `cookies.txt` file in the data folder.
fn manual_cookies() -> Result<String> {
    if let Ok(header) = std::env::var("AURAL_COOKIES") {
        return Ok(header);
    }
    let path = platform::data_dir().join("cookies.txt");
    std::fs::read_to_string(&path)
        .map(|header| header.trim().to_owned())
        .with_context(|| {
            format!(
                "no sign-in window here; put a Cookie header in {}",
                path.display()
            )
        })
}

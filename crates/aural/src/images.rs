//! Cover images: fetched once per url and size, kept in memory, handed to Freya's
//! `ImageViewer` as bytes so it decodes them off the UI thread at the size they are drawn.
//! Fetching goes through rustls (`ytmusic`'s client), not Freya's remote loader.

use std::collections::HashMap;
use std::hash::{DefaultHasher, Hash as _, Hasher as _};
use std::sync::{Arc, Mutex, OnceLock};

use bytes::Bytes;
use tokio::sync::Semaphore;

use crate::runtime;

/// Downloads running at once.
const PARALLEL: usize = 6;
/// Images kept in memory before the oldest go. Covers are small (~20 KB at 226 px).
const KEEP: usize = 400;

#[derive(Default)]
struct Cache {
    ready: HashMap<String, Bytes>,
    order: Vec<String>,
}

static CACHE: OnceLock<Mutex<Cache>> = OnceLock::new();
static GATE: OnceLock<Arc<Semaphore>> = OnceLock::new();
static HTTP: OnceLock<reqwest::Client> = OnceLock::new();

/// The bytes of `url` if already fetched.
pub fn cached(url: &str) -> Option<Bytes> {
    CACHE.get()?.lock().ok()?.ready.get(url).cloned()
}

/// Fetches `url`, or returns the cached copy.
pub async fn fetch(url: String) -> Option<Bytes> {
    if let Some(bytes) = cached(&url) {
        return Some(bytes);
    }
    runtime::spawn(async move {
        let gate = GATE
            .get_or_init(|| Arc::new(Semaphore::new(PARALLEL)))
            .clone();
        let _permit = gate.acquire().await.ok()?;
        let http = HTTP.get_or_init(|| ytmusic::YtMusic::anonymous().client().clone());
        let bytes = http
            .get(&url)
            .send()
            .await
            .ok()?
            .error_for_status()
            .ok()?
            .bytes()
            .await
            .ok()?;
        let mut cache = CACHE.get_or_init(Mutex::default).lock().ok()?;
        if cache.order.len() >= KEEP {
            let oldest = cache.order.remove(0);
            cache.ready.remove(&oldest);
        }
        cache.order.push(url.clone());
        cache.ready.insert(url, bytes.clone());
        Some(bytes)
    })
    .await
    .ok()
    .flatten()
}

/// A stable id for `url`, the key Freya caches the decoded image under.
pub fn key(url: &str) -> u64 {
    let mut hasher = DefaultHasher::new();
    url.hash(&mut hasher);
    hasher.finish()
}

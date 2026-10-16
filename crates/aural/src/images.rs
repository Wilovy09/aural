//! Cover images: fetched once per url and size, kept in memory and on disk, handed to Freya's
//! `ImageViewer` as bytes so it decodes them off the UI thread at the size they are drawn.
//! Fetching goes through rustls (`ytmusic`'s client), not Freya's remote loader.
//!
//! The disk copy lets the next session draw covers without downloading them again. It lives in
//! the cache folder, keyed by a stable hash of the url, and is trimmed to [`DISK`] once a
//! session, the oldest files first.

use std::collections::HashMap;
use std::hash::{DefaultHasher, Hash as _, Hasher as _};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, Once, OnceLock};

use bytes::Bytes;
use tokio::sync::Semaphore;

use crate::{platform, runtime};

/// Downloads running at once.
const PARALLEL: usize = 6;
/// Images kept in memory before the oldest go. Covers are small (~20 KB at 226 px).
const KEEP: usize = 400;
/// The most the covers on disk may take; past it, the oldest go until a fifth is free.
const DISK: u64 = 150 * 1024 * 1024;

#[derive(Default)]
struct Cache {
    ready: HashMap<String, Bytes>,
    order: Vec<String>,
}

static CACHE: OnceLock<Mutex<Cache>> = OnceLock::new();
static GATE: OnceLock<Arc<Semaphore>> = OnceLock::new();
static HTTP: OnceLock<reqwest::Client> = OnceLock::new();
static TRIMMED: Once = Once::new();

/// The bytes of `url` if already in memory.
pub fn cached(url: &str) -> Option<Bytes> {
    CACHE.get()?.lock().ok()?.ready.get(url).cloned()
}

/// Fetches `url`: from memory, else from disk, else from the network (and keeps it on disk).
pub async fn fetch(url: String) -> Option<Bytes> {
    if let Some(bytes) = cached(&url) {
        return Some(bytes);
    }
    TRIMMED.call_once(|| {
        runtime::spawn(async {
            let _ = tokio::task::spawn_blocking(|| trim(&folder())).await;
        });
    });
    runtime::spawn(async move {
        let path = folder().join(format!("{:016x}", stable_hash(&url)));
        let stored = tokio::task::spawn_blocking({
            let path = path.clone();
            move || std::fs::read(path).ok().filter(|bytes| !bytes.is_empty())
        })
        .await
        .ok()
        .flatten();
        let bytes = match stored {
            Some(bytes) => Bytes::from(bytes),
            None => {
                let bytes = download(&url).await?;
                let kept = bytes.clone();
                tokio::task::spawn_blocking(move || store(&path, &kept));
                bytes
            }
        };
        remember(url, bytes.clone());
        Some(bytes)
    })
    .await
    .ok()
    .flatten()
}

/// What the covers kept on disk take, in bytes.
pub fn disk_size() -> u64 {
    std::fs::read_dir(folder())
        .map(|entries| {
            entries
                .flatten()
                .filter_map(|entry| entry.metadata().ok())
                .map(|meta| meta.len())
                .sum()
        })
        .unwrap_or(0)
}

/// Forgets every cover, in memory and on disk, returning the bytes freed. Covers are fetched
/// again as they show.
pub fn clear() -> u64 {
    let freed = disk_size();
    if let Some(cache) = CACHE.get()
        && let Ok(mut cache) = cache.lock()
    {
        *cache = Cache::default();
    }
    if let Err(error) = std::fs::remove_dir_all(folder())
        && error.kind() != std::io::ErrorKind::NotFound
    {
        log::warn!("images: cannot clear the covers on disk: {error}");
    }
    freed
}

/// `bytes` as the app shows a size: "3.6 MB", "850 KB".
pub fn describe(bytes: u64) -> String {
    match bytes {
        0..1_000_000 => format!("{} KB", bytes.div_ceil(1000)),
        _ => format!("{:.1} MB", bytes as f64 / 1_000_000.),
    }
}

/// A stable id for `url`, the key Freya caches the decoded image under.
pub fn key(url: &str) -> u64 {
    let mut hasher = DefaultHasher::new();
    url.hash(&mut hasher);
    hasher.finish()
}

async fn download(url: &str) -> Option<Bytes> {
    let gate = GATE
        .get_or_init(|| Arc::new(Semaphore::new(PARALLEL)))
        .clone();
    let _permit = gate.acquire().await.ok()?;
    let http = HTTP.get_or_init(|| ytmusic::YtMusic::anonymous().client().clone());
    http.get(url)
        .send()
        .await
        .ok()?
        .error_for_status()
        .ok()?
        .bytes()
        .await
        .ok()
}

fn remember(url: String, bytes: Bytes) {
    let Ok(mut cache) = CACHE.get_or_init(Mutex::default).lock() else {
        return;
    };
    if cache.ready.contains_key(&url) {
        return;
    }
    if cache.order.len() >= KEEP {
        let oldest = cache.order.remove(0);
        cache.ready.remove(&oldest);
    }
    cache.order.push(url.clone());
    cache.ready.insert(url, bytes);
}

fn folder() -> PathBuf {
    platform::cache_dir().join("covers")
}

/// Writes `bytes` at `path` whole or not at all: a half-written cover would never load.
fn store(path: &Path, bytes: &[u8]) {
    let Some(parent) = path.parent() else {
        return;
    };
    let partial = path.with_extension("part");
    let written = std::fs::create_dir_all(parent)
        .and_then(|()| std::fs::write(&partial, bytes))
        .and_then(|()| std::fs::rename(&partial, path));
    if let Err(error) = written {
        log::warn!("images: cannot keep a cover on disk: {error}");
        let _ = std::fs::remove_file(&partial);
    }
}

/// Removes the oldest covers in `folder` while they take more than [`DISK`].
fn trim(folder: &Path) {
    let Ok(entries) = std::fs::read_dir(folder) else {
        return;
    };
    let mut files: Vec<(std::time::SystemTime, u64, PathBuf)> = entries
        .flatten()
        .filter_map(|entry| {
            let meta = entry.metadata().ok()?;
            Some((meta.modified().ok()?, meta.len(), entry.path()))
        })
        .collect();
    let mut total: u64 = files.iter().map(|(_, len, _)| len).sum();
    if total <= DISK {
        return;
    }
    files.sort_by_key(|(modified, ..)| *modified);
    let goal = DISK / 5 * 4;
    for (_, len, path) in files {
        if total <= goal {
            break;
        }
        if std::fs::remove_file(&path).is_ok() {
            total = total.saturating_sub(len);
        }
    }
    log::info!(
        "images: trimmed the covers on disk to {} MB",
        total / 1024 / 1024
    );
}

/// FNV-1a: the same hash for the same url on every run and every Rust version, which
/// `DefaultHasher` does not promise, so the disk copy outlives app updates.
fn stable_hash(text: &str) -> u64 {
    text.bytes().fold(0xcbf2_9ce4_8422_2325, |hash, byte| {
        (hash ^ u64::from(byte)).wrapping_mul(0x0100_0000_01b3)
    })
}

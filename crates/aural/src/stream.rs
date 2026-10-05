//! A song downloaded while it plays, after Sonora's `stream.rs`: the decoder reads from the
//! front of one buffer while a task on the io runtime keeps filling the back, so playback
//! starts after a short preroll instead of after the whole file.

use std::io::{self, Read, Seek, SeekFrom};
use std::sync::{Arc, Condvar, Mutex, Weak};
use std::time::Duration;

use anyhow::{Result, bail};
use tokio::sync::watch;
use ytmusic::AudioStream;

use crate::runtime;

/// How much has to be in before the decoder starts. At ~130 kbps this is many seconds of
/// audio, and the download outruns playback, so the lead only grows.
pub const PREROLL: usize = 256 * 1024;
/// How long a read waits for bytes that have not arrived. A wait there is silence, so it is
/// bounded: a dead connection must not hold the output forever.
const PATIENCE: Duration = Duration::from_secs(20);
/// The most one song may buffer, a ceiling on what a wrong length can cost.
const CEILING: usize = 128 * 1024 * 1024;

#[derive(Default)]
struct Fill {
    data: Vec<u8>,
    complete: bool,
    error: Option<String>,
}

struct Shared {
    fill: Mutex<Fill>,
    grew: Condvar,
    /// How many bytes have arrived, for async waiters.
    arrived: watch::Sender<usize>,
}

/// One song's download. Cheap to clone; the download stops once every clone and reader is gone.
#[derive(Clone)]
pub struct Stream {
    shared: Arc<Shared>,
    total: Option<u64>,
}

impl Stream {
    /// Starts pulling `audio` into a new buffer on the io runtime.
    pub fn pulling(mut audio: AudioStream) -> Self {
        let total = audio.total();
        let (arrived, _) = watch::channel(0);
        let shared = Arc::new(Shared {
            fill: Mutex::new(Fill::default()),
            grew: Condvar::new(),
            arrived,
        });
        let weak: Weak<Shared> = Arc::downgrade(&shared);
        runtime::spawn(async move {
            loop {
                let next = audio.chunk().await;
                let Some(shared) = weak.upgrade() else {
                    return;
                };
                let Ok(mut fill) = shared.fill.lock() else {
                    return;
                };
                let done = match next {
                    Ok(Some(chunk)) if fill.data.len() + chunk.len() <= CEILING => {
                        fill.data.extend_from_slice(&chunk);
                        false
                    }
                    Ok(Some(_)) => {
                        fill.error = Some("the song is larger than the ceiling".into());
                        true
                    }
                    Ok(None) => {
                        fill.complete = true;
                        true
                    }
                    Err(error) => {
                        fill.error = Some(format!("{error:#}"));
                        true
                    }
                };
                let length = fill.data.len();
                drop(fill);
                shared.grew.notify_all();
                shared.arrived.send_replace(length);
                if done {
                    return;
                }
            }
        });
        Self { shared, total }
    }

    /// Waits until the preroll is in, or the song ended sooner. Fails when the download did.
    pub async fn primed(&self) -> Result<()> {
        let mut arrived = self.shared.arrived.subscribe();
        loop {
            {
                let fill = self
                    .shared
                    .fill
                    .lock()
                    .map_err(|_| anyhow::anyhow!("poisoned"))?;
                if let Some(error) = &fill.error {
                    bail!("the download failed: {error}");
                }
                if fill.complete || fill.data.len() >= PREROLL {
                    return Ok(());
                }
            }
            if arrived.changed().await.is_err() {
                bail!("the download stopped");
            }
        }
    }

    /// The song's size, when the server said.
    pub fn total(&self) -> Option<u64> {
        self.total
    }

    /// A reader from the start of the song.
    pub fn reader(&self) -> Reader {
        Reader {
            shared: self.shared.clone(),
            total: self.total,
            at: 0,
        }
    }
}

/// Reads a [`Stream`] as a file, blocking until the bytes asked for arrive.
pub struct Reader {
    shared: Arc<Shared>,
    total: Option<u64>,
    at: u64,
}

impl Read for Reader {
    fn read(&mut self, out: &mut [u8]) -> io::Result<usize> {
        let mut fill = self
            .shared
            .fill
            .lock()
            .map_err(|_| io::Error::other("the stream lock is poisoned"))?;
        loop {
            let at = self.at as usize;
            if at < fill.data.len() {
                let count = out.len().min(fill.data.len() - at);
                out[..count].copy_from_slice(&fill.data[at..at + count]);
                self.at += count as u64;
                return Ok(count);
            }
            if fill.complete {
                return Ok(0);
            }
            if let Some(error) = &fill.error {
                return Err(io::Error::other(error.clone()));
            }
            let (next, waited) = self
                .shared
                .grew
                .wait_timeout(fill, PATIENCE)
                .map_err(|_| io::Error::other("the stream lock is poisoned"))?;
            fill = next;
            if waited.timed_out() && at >= fill.data.len() {
                return Err(io::Error::new(
                    io::ErrorKind::TimedOut,
                    "the download stalled",
                ));
            }
        }
    }
}

impl Seek for Reader {
    fn seek(&mut self, to: SeekFrom) -> io::Result<u64> {
        let target = match to {
            SeekFrom::Start(offset) => Some(offset),
            SeekFrom::Current(delta) => self.at.checked_add_signed(delta),
            SeekFrom::End(delta) => self.total.and_then(|total| total.checked_add_signed(delta)),
        };
        let target = target.ok_or_else(|| io::Error::other("cannot seek there"))?;
        self.at = target;
        Ok(target)
    }
}

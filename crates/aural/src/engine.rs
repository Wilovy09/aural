//! The playback engine: one thread that owns the audio output and the queue. The UI sends it
//! [`Command`]s and hears back [`Update`]s; it never touches rodio itself.
//!
//! Like Sonora's engine, a song plays while it downloads (see [`crate::stream`]), the next one
//! is fetched before the current one ends and queued on the same sink so there is no gap, and
//! every song is brought to the same loudness with the level YouTube reports for it.

use std::sync::Arc;
use std::sync::mpsc::{Receiver, RecvTimeoutError, Sender, channel};
use std::time::Duration;

use anyhow::{Context as _, Result};
use rodio::Source as _;
use tokio::sync::{mpsc::UnboundedSender, oneshot};
use ytmusic::YtMusic;

use crate::library::Song;
use crate::runtime;
use crate::stream::Stream;

/// How often the position is reported while a song plays.
const TICK: Duration = Duration::from_millis(250);
/// How far into a song "previous" restarts it instead of going back one.
const RESTART_AFTER: Duration = Duration::from_secs(3);
/// How long the stream metadata and the preroll may take before a load gives up.
const PATIENCE: Duration = Duration::from_secs(15);
/// How long before the end of a song the next one starts loading.
const PRELOAD_BEFORE: Duration = Duration::from_secs(25);
/// The loudness YouTube's `loudnessDb` is measured against; 0 dB there is this many LUFS.
const REFERENCE_LUFS: f32 = -14.0;
/// The loudness every song is brought to.
const TARGET_LUFS: f32 = -14.0;
/// The most a quiet song is raised, as a gain factor.
const BOOST_CAP: f32 = 2.0;

/// What the UI asks of the engine.
pub enum Command {
    /// Replace the queue and start at `index`.
    Play {
        queue: Vec<Song>,
        index: usize,
    },
    Toggle,
    Next,
    Previous,
    /// Use this client from now on, after signing in or out.
    Client(Arc<YtMusic>),
    /// Turn shuffle on or off; the current song keeps playing either way.
    Shuffle(bool),
    Repeat(Repeat),
    /// Jump to this point of the current song.
    Seek(Duration),
}

/// What happens when a song ends.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum Repeat {
    /// Go on through the queue and stop at its end.
    #[default]
    Off,
    /// Go on through the queue and start it over at its end.
    All,
    /// Play the same song again.
    One,
}

impl Repeat {
    /// The mode after this one, in the order the button cycles through.
    pub fn next(self) -> Self {
        match self {
            Repeat::Off => Repeat::All,
            Repeat::All => Repeat::One,
            Repeat::One => Repeat::Off,
        }
    }
}

/// What the engine tells the UI.
#[derive(Clone, Debug)]
pub enum Update {
    /// A song was chosen and is loading.
    Loading(Song),
    /// The song started; `playing` changes with every toggle.
    Playing(bool),
    /// How far into the current song, and how long it is.
    Position(Duration, Option<Duration>),
    /// The queue as it will play, and which song of it is current.
    Queue(Vec<Song>, usize),
    /// The queue ran out.
    Stopped,
    Error(String),
}

/// A handle the UI keeps to drive the engine. Cheap to clone.
#[derive(Clone)]
pub struct Engine {
    commands: Sender<Command>,
}

impl PartialEq for Engine {
    fn eq(&self, _other: &Self) -> bool {
        true
    }
}

impl Engine {
    /// Starts the engine thread with `api`; updates go to `updates`.
    pub fn start(api: Arc<YtMusic>, updates: UnboundedSender<Update>) -> Self {
        let (commands, inbox) = channel();
        std::thread::spawn(move || {
            if let Err(error) = run(api, inbox, &updates) {
                log::error!("engine: {error:#}");
                let _ = updates.send(Update::Error(format!("{error:#}")));
            }
        });
        Self { commands }
    }

    pub fn send(&self, command: Command) {
        let _ = self.commands.send(command);
    }
}

/// A song ready to play: its download, primed, and what to play it at.
struct Loaded {
    stream: Stream,
    duration: Option<Duration>,
    gain: f32,
}

/// The song on the sink and, once fetched, the one queued behind it.
struct Playing {
    sink: rodio::Sink,
    duration: Option<Duration>,
    /// The next song's index, queued on the sink right behind the current one.
    queued: Option<(usize, Option<Duration>)>,
    /// The next song's load, running.
    preload: Option<(usize, oneshot::Receiver<Result<Loaded>>)>,
    /// The sink's position when the current song took over from the one before it.
    offset: Duration,
    /// Kept so the downloads live as long as their decoders.
    _streams: Vec<Stream>,
}

fn run(
    mut api: Arc<YtMusic>,
    inbox: Receiver<Command>,
    updates: &UnboundedSender<Update>,
) -> Result<()> {
    let output = rodio::OutputStreamBuilder::open_default_stream().context("no audio output")?;
    let say = |update: Update| {
        // Android's controls follow from here, so they stay right in the background.
        crate::media::observe(&update);
        crate::connect::observe(&update);
        let _ = updates.send(update);
    };

    let mut queue: Vec<Song> = Vec::new();
    // The queue as it was given, to go back to when shuffle is turned off.
    let mut original: Vec<Song> = Vec::new();
    let mut index = 0usize;
    let mut current: Option<Playing> = None;
    let mut shuffled = false;
    let mut repeat = Repeat::Off;

    loop {
        let command = match inbox.recv_timeout(TICK) {
            Ok(command) => Some(command),
            Err(RecvTimeoutError::Timeout) => None,
            Err(RecvTimeoutError::Disconnected) => return Ok(()),
        };
        let mut jump: Option<usize> = None;
        match command {
            Some(Command::Play {
                queue: next,
                index: at,
            }) => {
                original = next.clone();
                queue = next;
                jump = Some(at);
                if shuffled && !queue.is_empty() {
                    let first = at.min(queue.len() - 1);
                    queue = shuffle_from(&original, first);
                    jump = Some(0);
                }
            }
            Some(Command::Toggle) => {
                if let Some(current) = &current {
                    match current.sink.is_paused() {
                        true => current.sink.play(),
                        false => current.sink.pause(),
                    }
                    say(Update::Playing(!current.sink.is_paused()));
                }
            }
            Some(Command::Next) => {
                jump = Some(following(index, queue.len(), repeat, true).unwrap_or(queue.len()))
            }
            Some(Command::Previous) => {
                let restart = current
                    .as_ref()
                    .is_some_and(|current| elapsed(current) > RESTART_AFTER);
                jump = Some(match restart {
                    true => index,
                    false => index.saturating_sub(1),
                });
            }
            Some(Command::Seek(to)) => {
                if let Some(playing) = current.as_mut() {
                    let to = playing
                        .duration
                        .map_or(to, |duration| to.min(duration.saturating_sub(TICK)));
                    match playing.sink.try_seek(to) {
                        // The sink now counts from the point sought, in the current song.
                        Ok(()) => {
                            playing.offset = Duration::ZERO;
                            say(Update::Position(to, playing.duration));
                        }
                        Err(error) => log::warn!("engine: cannot seek: {error}"),
                    }
                }
            }
            Some(Command::Client(next)) => api = next,
            Some(Command::Repeat(mode)) => repeat = mode,
            Some(Command::Shuffle(on)) if on != shuffled => {
                shuffled = on;
                let playing_id = queue.get(index).map(|song| song.id.clone());
                match on {
                    true if !original.is_empty() => {
                        let at = playing_id
                            .and_then(|id| original.iter().position(|song| song.id == id))
                            .unwrap_or(0);
                        queue = shuffle_from(&original, at);
                        index = 0;
                    }
                    true => {}
                    false => {
                        index = playing_id
                            .and_then(|id| original.iter().position(|song| song.id == id))
                            .unwrap_or(0);
                        queue = original.clone();
                    }
                }
                // What comes next changed: a fetched next song no longer fits.
                if let Some(playing) = current.as_mut() {
                    playing.preload = None;
                }
                say(Update::Queue(queue.clone(), index));
            }
            Some(Command::Shuffle(_)) => {}
            None => {}
        }

        if let Some(at) = jump {
            if let Some(old) = current.take() {
                old.sink.stop();
            }
            let Some(song) = queue.get(at).cloned() else {
                say(Update::Stopped);
                continue;
            };
            index = at;
            say(Update::Queue(queue.clone(), index));
            current = begin(&output, &api, &song, &say);
            continue;
        }

        // Out of songs on the sink: move on.
        if current.as_ref().is_some_and(|playing| playing.sink.empty()) {
            current = None;
            match following(index, queue.len(), repeat, false)
                .and_then(|next| queue.get(next).cloned().map(|song| (next, song)))
            {
                Some((next, song)) => {
                    index = next;
                    say(Update::Queue(queue.clone(), index));
                    current = begin(&output, &api, &song, &say);
                }
                None => say(Update::Stopped),
            }
            continue;
        }

        let Some(playing) = current.as_mut() else {
            continue;
        };

        // The queued song took over: the sink holds one source again.
        if let Some((next, duration)) = playing.queued
            && playing.sink.len() <= 1
        {
            index = next;
            playing.queued = None;
            playing.duration = duration;
            playing.offset = playing.sink.get_pos();
            say(Update::Queue(queue.clone(), index));
            if let Some(song) = queue.get(index) {
                say(Update::Loading(song.clone()));
                say(Update::Playing(!playing.sink.is_paused()));
            }
        }

        let elapsed = elapsed(playing);
        if !playing.sink.is_paused() {
            say(Update::Position(elapsed, playing.duration));
        }

        // Near the end, fetch the next song; once it is in, queue it right behind.
        let near_end = playing
            .duration
            .is_some_and(|duration| duration.saturating_sub(elapsed) < PRELOAD_BEFORE);
        if near_end
            && playing.queued.is_none()
            && playing.preload.is_none()
            && let Some(next) = following(index, queue.len(), repeat, false)
            && let Some(song) = queue.get(next).cloned()
        {
            let (done, ready) = oneshot::channel();
            let api = api.clone();
            runtime::spawn(async move {
                let _ = done.send(load(api, song).await);
            });
            playing.preload = Some((next, ready));
        }
        if let Some((next, ready)) = playing.preload.as_mut() {
            match ready.try_recv() {
                Ok(Ok(loaded)) => {
                    let next = *next;
                    match decoder(&loaded) {
                        Ok(source) => {
                            playing.sink.append(source.amplify(loaded.gain));
                            playing.queued = Some((next, loaded.duration));
                            playing._streams.push(loaded.stream);
                            log::info!("engine: queued the next song gaplessly");
                        }
                        Err(error) => log::warn!("engine: cannot queue the next song: {error:#}"),
                    }
                    playing.preload = None;
                }
                Ok(Err(error)) => {
                    log::warn!("engine: cannot preload the next song: {error:#}");
                    playing.preload = None;
                }
                Err(oneshot::error::TryRecvError::Empty) => {}
                Err(oneshot::error::TryRecvError::Closed) => playing.preload = None,
            }
        }
    }
}

/// The song after `index` in a queue of `length`: the same one again on repeat-one (unless
/// the user skipped), the first again on repeat-all, nothing past the end otherwise.
fn following(index: usize, length: usize, repeat: Repeat, skipped: bool) -> Option<usize> {
    match repeat {
        Repeat::One if !skipped => Some(index),
        _ if index + 1 < length => Some(index + 1),
        Repeat::All if length > 0 => Some(0),
        _ => None,
    }
}

/// `songs` with the one at `first` first and the rest in random order.
pub fn shuffle_from(songs: &[Song], first: usize) -> Vec<Song> {
    let mut rest: Vec<Song> = songs
        .iter()
        .enumerate()
        .filter(|(at, _)| *at != first)
        .map(|(_, song)| song.clone())
        .collect();
    shuffle(&mut rest);
    songs.get(first).cloned().into_iter().chain(rest).collect()
}

/// Shuffles `songs` in place with a time-seeded xorshift; nothing stronger is needed.
pub fn shuffle(songs: &mut [Song]) {
    let mut seed = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|elapsed| elapsed.as_nanos() as u64)
        .unwrap_or(0x9e37_79b9)
        | 1;
    for at in (1..songs.len()).rev() {
        seed ^= seed << 13;
        seed ^= seed >> 7;
        seed ^= seed << 17;
        songs.swap(at, (seed % (at as u64 + 1)) as usize);
    }
}

/// How far into the current song. rodio's position may or may not restart when a queued
/// source takes over; the offset taken at the switch covers both.
fn elapsed(playing: &Playing) -> Duration {
    let raw = playing.sink.get_pos();
    match raw >= playing.offset {
        true => raw - playing.offset,
        false => raw,
    }
}

/// Loads `song` and starts it on a fresh sink, telling the UI either way.
fn begin(
    output: &rodio::OutputStream,
    api: &Arc<YtMusic>,
    song: &Song,
    say: &dyn Fn(Update),
) -> Option<Playing> {
    say(Update::Loading(song.clone()));
    let started =
        runtime::block_on(load(api.clone(), song.clone())).and_then(|loaded| start(output, loaded));
    match started {
        Ok(playing) => {
            say(Update::Playing(true));
            Some(playing)
        }
        Err(error) => {
            fail(say, song, error);
            None
        }
    }
}

fn fail(say: &dyn Fn(Update), song: &Song, error: anyhow::Error) {
    log::warn!("engine: cannot play {}: {error:#}", song.id);
    say(Update::Error(format!(
        "no se pudo reproducir {}",
        song.title
    )));
}

/// Opens `song`'s audio and waits for the preroll.
async fn load(api: Arc<YtMusic>, song: Song) -> Result<Loaded> {
    let started = std::time::Instant::now();
    let (format, audio) = tokio::time::timeout(PATIENCE, api.open_audio(&song.id))
        .await
        .context("the stream took too long to open")??;
    let stream = Stream::pulling(audio);
    tokio::time::timeout(PATIENCE, stream.primed())
        .await
        .context("the preroll took too long")??;
    let gain = format
        .loudness_db
        .map(|db| {
            10f32
                .powf((TARGET_LUFS - (REFERENCE_LUFS + db)) / 20.)
                .min(BOOST_CAP)
        })
        .unwrap_or(1.);
    log::info!(
        "engine: {} {} kbps, {:.1} MiB, gain {gain:.2}, ready in {:?}",
        format.codec,
        format.bitrate / 1000,
        stream.total().unwrap_or_default() as f64 / (1024. * 1024.),
        started.elapsed()
    );
    Ok(Loaded {
        stream,
        duration: format.duration.or(song.duration),
        gain,
    })
}

/// A decoder over a loaded song's download.
fn decoder(loaded: &Loaded) -> Result<rodio::Decoder<crate::stream::Reader>> {
    let mut builder = rodio::Decoder::builder()
        .with_data(loaded.stream.reader())
        .with_hint("mp4")
        // Seeking ahead of the download only waits for the bytes: the reader blocks until
        // they arrive, and the download outruns playback.
        .with_seekable(true);
    if let Some(total) = loaded.stream.total() {
        builder = builder.with_byte_len(total);
    }
    builder.build().context("cannot decode")
}

/// Starts `loaded` on a new sink.
fn start(output: &rodio::OutputStream, loaded: Loaded) -> Result<Playing> {
    let source = decoder(&loaded)?;
    let sink = rodio::Sink::connect_new(output.mixer());
    sink.append(source.amplify(loaded.gain));
    Ok(Playing {
        sink,
        duration: loaded.duration,
        queued: None,
        preload: None,
        offset: Duration::ZERO,
        _streams: vec![loaded.stream],
    })
}

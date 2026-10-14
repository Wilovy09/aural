//! Discord's activity: "Listening to Aural" on the profile, with the song, its cover and how far
//! it is, through the Discord app running on the same computer. Phones have no Discord app to
//! talk to, so this is for the desktop only.
//!
//! It follows the engine's updates on the engine's own thread, as the media controls do, and
//! talks to Discord from a thread of its own, since every message waits for Discord's answer.

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Condvar, Mutex, Once};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use discord_rich_presence::{DiscordIpc, DiscordIpcClient, activity};

use crate::engine::Update;
use crate::library::{self, Song};
use crate::{artwork, runtime};

/// The Aural application on Discord's developer portal, whose name the activity takes.
const APPLICATION_ID: &str = "1559256203278946304";
/// How long the song must stay put before Discord hears of it, so skipping through songs
/// sends only the last.
const SETTLE: Duration = Duration::from_secs(1);
/// How long to wait before trying again when Discord is closed.
const RETRY: Duration = Duration::from_secs(3);
/// How far the song's start may move before Discord is told again: a seek, not the clock.
const DRIFT: i64 = 2;
/// The longest text Discord takes in a field, in UTF-16 units.
const TEXT_LIMIT: usize = 128;
/// The side of the cover Discord shows.
const ART_EDGE: u32 = 512;

/// What the profile should show: the song, and since when it plays if it does.
#[derive(Clone, Debug, PartialEq)]
struct Activity {
    song: Song,
    /// The Unix time the song started, counted back from its position.
    started: i64,
}

/// What the engine said last, to work the activity out of.
struct Heard {
    song: Song,
    playing: bool,
    position: Duration,
    /// When `position` was true.
    at: Instant,
}

impl Heard {
    fn activity(&self) -> Option<Activity> {
        self.playing.then(|| Activity {
            song: self.song.clone(),
            started: now() - (self.position + self.at.elapsed()).as_secs() as i64,
        })
    }
}

/// What the worker should show, and a count bumped on every change so it notices.
struct Wanted {
    change: u64,
    activity: Option<Activity>,
}

static ENABLED: AtomicBool = AtomicBool::new(true);
static HEARD: Mutex<Option<Heard>> = Mutex::new(None);
static WANTED: Mutex<Wanted> = Mutex::new(Wanted {
    change: 0,
    activity: None,
});
static CHANGED: Condvar = Condvar::new();
static WORKER: Once = Once::new();

/// Turns the activity on or off, as the setting says.
pub fn enable(enabled: bool) {
    log::info!("discord: activity {}", if enabled { "on" } else { "off" });
    ENABLED.store(enabled, Ordering::SeqCst);
    let activity = HEARD
        .lock()
        .ok()
        .and_then(|heard| heard.as_ref().and_then(Heard::activity));
    want(activity);
}

/// Follows one engine update. Runs on the engine's thread.
pub fn observe(update: &Update) {
    let Ok(mut heard) = HEARD.lock() else {
        return;
    };
    match update {
        Update::Loading(song) => {
            *heard = Some(Heard {
                song: song.clone(),
                playing: false,
                position: Duration::ZERO,
                at: Instant::now(),
            });
        }
        Update::Playing(playing) => {
            let Some(now) = heard.as_mut() else {
                return;
            };
            if now.playing {
                now.position += now.at.elapsed();
            }
            now.at = Instant::now();
            now.playing = *playing;
        }
        Update::Position(elapsed, _) => {
            let Some(now) = heard.as_mut() else {
                return;
            };
            now.position = *elapsed;
            now.at = Instant::now();
        }
        Update::Stopped => *heard = None,
        Update::Queue(..) | Update::Error(_) => return,
    }
    want(heard.as_ref().and_then(Heard::activity));
}

/// Hands `activity` to the worker unless it already has it, or one that differs only by the
/// clock's drift.
fn want(activity: Option<Activity>) {
    let activity = activity.filter(|_| ENABLED.load(Ordering::SeqCst));
    let Ok(mut wanted) = WANTED.lock() else {
        return;
    };
    let same = match (&wanted.activity, &activity) {
        (None, None) => true,
        (Some(old), Some(new)) => {
            old.song.id == new.song.id && old.started.abs_diff(new.started) <= DRIFT as u64
        }
        _ => false,
    };
    if same {
        return;
    }
    wanted.activity = activity;
    wanted.change += 1;
    drop(wanted);
    WORKER.call_once(|| {
        std::thread::spawn(work);
    });
    CHANGED.notify_all();
}

/// Why an activity did not land.
enum Failure {
    /// Discord is closed or went away: worth trying again.
    Unreachable(String),
    /// Discord turned the activity down: trying again would not help.
    Refused(String),
}

/// Sends each activity wanted to Discord, once it has settled.
fn work() {
    log::debug!("discord: worker started for application {APPLICATION_ID}");
    let mut client = DiscordIpcClient::new(APPLICATION_ID);
    let mut connected = false;
    // `None` until something reached Discord, so the first answer is always sent.
    let mut shown: Option<Option<Activity>> = None;
    let mut seen = 0;
    let mut retrying = false;
    let mut covers: HashMap<String, Option<String>> = HashMap::new();
    let mut reported: Option<String> = None;

    loop {
        let Ok(mut wanted) = WANTED.lock() else {
            return;
        };
        while wanted.change == seen {
            if !retrying {
                wanted = match CHANGED.wait(wanted) {
                    Ok(wanted) => wanted,
                    Err(_) => return,
                };
                continue;
            }
            let Ok((again, waited)) = CHANGED.wait_timeout(wanted, RETRY) else {
                return;
            };
            wanted = again;
            if waited.timed_out() {
                break;
            }
        }
        seen = wanted.change;
        drop(wanted);

        std::thread::sleep(SETTLE);
        let Ok(wanted) = WANTED.lock() else {
            return;
        };
        if wanted.change != seen {
            continue;
        }
        let activity = wanted.activity.clone();
        drop(wanted);
        if shown.as_ref() == Some(&activity) && !retrying {
            continue;
        }

        let cover = activity
            .as_ref()
            .and_then(|activity| cover(&mut covers, &activity.song));
        let sent = send(&mut client, &mut connected, activity.as_ref(), cover);
        retrying = matches!(sent, Err(Failure::Unreachable(_)));
        match sent {
            Ok(()) => {
                match &activity {
                    Some(activity) => log::debug!(
                        "discord: showing {} ({}) by {}",
                        activity.song.title,
                        activity.song.id,
                        activity.song.artist
                    ),
                    None => log::debug!("discord: activity cleared"),
                }
                reported = None;
            }
            // Said once, however long it keeps failing: Discord being closed is normal.
            Err(Failure::Unreachable(failure)) => {
                if reported.as_ref() != Some(&failure) {
                    log::info!(
                        "discord: cannot reach Discord, retrying every {}s: {failure}",
                        RETRY.as_secs()
                    );
                    reported = Some(failure);
                }
            }
            Err(Failure::Refused(failure)) => {
                if reported.as_ref() != Some(&failure) {
                    log::warn!("discord: Discord refused the activity: {failure}");
                    reported = Some(failure);
                }
            }
        }
        if !retrying {
            shown = Some(activity);
        }
    }
}

/// Offers `activity` to Discord, or clears it, connecting first when needed and once more for
/// a connection gone stale.
fn send(
    client: &mut DiscordIpcClient,
    connected: &mut bool,
    activity: Option<&Activity>,
    cover: Option<String>,
) -> Result<(), Failure> {
    if *connected {
        match offer(client, activity, cover.as_deref()) {
            Err(Failure::Unreachable(error)) => {
                log::info!("discord: connection lost, reconnecting: {error}");
                *connected = false;
            }
            result => return result,
        }
    }
    // Nothing to clear on a Discord that was never told anything.
    if activity.is_none() && !*connected {
        return Ok(());
    }
    log::debug!("discord: connecting to the local Discord app");
    client
        .connect()
        .map_err(|error| Failure::Unreachable(error.to_string()))?;
    log::info!("discord: connected");
    *connected = true;
    offer(client, activity, cover.as_deref())
}

fn offer(
    client: &mut DiscordIpcClient,
    activity: Option<&Activity>,
    cover: Option<&str>,
) -> Result<(), Failure> {
    let sent = match activity {
        Some(activity) => client.set_activity(payload(activity, cover)),
        None => client.clear_activity(),
    };
    sent.map_err(|error| Failure::Unreachable(error.to_string()))?;
    let (_, answer) = client
        .recv()
        .map_err(|error| Failure::Unreachable(error.to_string()))?;
    if answer.get("evt").and_then(|event| event.as_str()) != Some("ERROR") {
        return Ok(());
    }
    let message = answer
        .pointer("/data/message")
        .and_then(|message| message.as_str())
        .unwrap_or("Discord turned the activity down");
    Err(Failure::Refused(message.to_owned()))
}

/// The activity as Discord takes it: the title over the artist, the cover with the album
/// under it, the bar of how far the song is, and a link to it.
fn payload<'a>(activity: &'a Activity, cover: Option<&'a str>) -> activity::Activity<'a> {
    let song = &activity.song;
    let mut payload = activity::Activity::new()
        .activity_type(activity::ActivityType::Listening)
        .details(fit(&song.title).unwrap_or_else(|| "Música".to_owned()));
    if let Some(artist) = fit(&song.artist) {
        payload = payload.state(artist);
    }
    if let Some(cover) = cover {
        let mut assets = activity::Assets::new().large_image(cover);
        if let Some(album) = song.album.as_deref().and_then(fit) {
            assets = assets.large_text(album);
        }
        payload = payload.assets(assets);
    }
    let mut timestamps = activity::Timestamps::new().start(activity.started);
    if let Some(duration) = song.duration.filter(|duration| !duration.is_zero()) {
        timestamps = timestamps.end(activity.started + duration.as_secs() as i64);
    }
    payload
        .timestamps(timestamps)
        .buttons(vec![activity::Button::new(
            "Escuchar en YouTube Music",
            format!("https://music.youtube.com/watch?v={}", song.id),
        )])
}

/// The cover of `song` at a public https address, as Discord fetches it itself: Apple's when
/// it has one, as everywhere, or YouTube's. Asked once a song.
fn cover(covers: &mut HashMap<String, Option<String>>, song: &Song) -> Option<String> {
    if let Some(cover) = covers.get(&song.id) {
        return cover.clone();
    }
    let apple = runtime::block_on(artwork::find(artwork::Wanted::song(song), ART_EDGE));
    let youtube = song
        .cover
        .as_deref()
        .map(|url| library::sized(url, ART_EDGE));
    let from = match (&apple, &youtube) {
        (Some(_), _) => "apple",
        (None, Some(_)) => "youtube",
        (None, None) => "none",
    };
    let cover = apple.or(youtube).filter(|url| url.starts_with("https://"));
    log::debug!(
        "discord: cover for {} from {from}{}",
        song.id,
        if cover.is_none() && from != "none" {
            " (not https, dropped)"
        } else {
            ""
        }
    );
    covers.insert(song.id.clone(), cover.clone());
    cover
}

/// Cuts `text` to what Discord takes in a field, or `None` if nothing is left. Discord wants
/// two characters at least, so a lone one gets a no-break space after it.
fn fit(text: &str) -> Option<String> {
    let mut units = 0;
    let mut fitted: String = text
        .trim()
        .chars()
        .take_while(|character| {
            units += character.len_utf16();
            units <= TEXT_LIMIT
        })
        .collect();
    fitted.truncate(fitted.trim_end().len());
    match fitted.chars().count() {
        0 => None,
        1 => Some(fitted + "\u{a0}"),
        _ => Some(fitted),
    }
}

fn now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64
}

//! The system's media controls. On Android, the player in quick settings, on the lock screen
//! and in the notification shade, through `dev.aural.app.Media` (see `android/java`); on iOS,
//! Now Playing, in Control Center and on the lock screen. It follows the engine's updates on
//! the engine's own thread, so it stays right while the app is in the background, and hands
//! the buttons pressed there back to the engine.
//!
//! Elsewhere it does nothing yet.

use std::sync::Mutex;
use std::time::{Duration, Instant};

use crate::engine::{Command, Engine, Update};
use crate::library::{self, Song};
use crate::{artwork, images, runtime};

/// The side of the cover handed to the system.
const ART_EDGE: u32 = 512;
/// How far the position may drift from where it should be before Android is told again.
/// Android moves the bar on its own from the last position and the play state.
const DRIFT: Duration = Duration::from_millis(1500);
/// How often the buttons pressed are read.
const LISTEN: Duration = Duration::from_millis(250);

/// What the system was last shown.
struct Shown {
    song: Song,
    playing: bool,
    position: Duration,
    total: Option<Duration>,
    /// When `position` was true.
    at: Instant,
    /// The cover's bytes, once fetched.
    art: Option<Vec<u8>>,
}

static SHOWN: Mutex<Option<Shown>> = Mutex::new(None);
/// The queue as it plays and the current song's place in it, for a hand-off.
static QUEUE: Mutex<(Vec<Song>, usize)> = Mutex::new((Vec::new(), 0));

/// What a new process picks up from the one before it: the queue and where it was.
#[derive(serde::Serialize, serde::Deserialize)]
pub struct Resume {
    pub queue: Vec<Song>,
    pub index: usize,
    pub position: Duration,
}

fn resume_file() -> std::path::PathBuf {
    crate::platform::data_dir().join("resume.json")
}

/// Whether music is playing right now.
#[cfg_attr(not(target_os = "android"), allow(dead_code))]
pub fn playing() -> bool {
    SHOWN
        .lock()
        .ok()
        .and_then(|shown| shown.as_ref().map(|shown| shown.playing))
        .unwrap_or(false)
}

#[cfg_attr(not(target_os = "android"), allow(dead_code))]
/// Writes down what plays, for the process that takes over. Only a song still playing is
/// handed on.
pub fn hand_off() {
    let position = match SHOWN.lock() {
        Ok(shown) => match shown.as_ref() {
            Some(shown) if shown.playing => expected(shown),
            _ => return,
        },
        Err(_) => return,
    };
    let Ok(queue) = QUEUE.lock() else {
        return;
    };
    let resume = Resume {
        queue: queue.0.clone(),
        index: queue.1,
        position,
    };
    if let Ok(body) = serde_json::to_vec(&resume) {
        let _ = std::fs::write(resume_file(), body);
    }
}

/// What the process before this one was playing, once: the file is gone after.
pub fn take_resume() -> Option<Resume> {
    let body = std::fs::read(resume_file()).ok()?;
    let _ = std::fs::remove_file(resume_file());
    serde_json::from_slice::<Resume>(&body)
        .ok()
        .filter(|resume| resume.index < resume.queue.len())
}

/// Follows one engine update. Runs on the engine's thread.
pub fn observe(update: &Update) {
    let Ok(mut shown) = SHOWN.lock() else {
        return;
    };
    match update {
        Update::Loading(song) => {
            let same = shown.as_ref().is_some_and(|shown| shown.song.id == song.id);
            let art = shown.take().filter(|_| same).and_then(|shown| shown.art);
            let fetch = art.is_none();
            *shown = Some(Shown {
                song: song.clone(),
                playing: false,
                position: Duration::ZERO,
                total: song.duration,
                at: Instant::now(),
                art,
            });
            publish(shown.as_ref());
            if fetch {
                cover(song.clone());
            }
        }
        Update::Playing(playing) => {
            if let Some(now) = shown.as_mut() {
                now.position = expected(now);
                now.at = Instant::now();
                now.playing = *playing;
            }
            publish(shown.as_ref());
        }
        Update::Position(elapsed, total) => {
            let Some(now) = shown.as_mut() else {
                return;
            };
            let jumped = expected(now).abs_diff(*elapsed) > DRIFT;
            let learned = now.total.is_none() && total.is_some();
            now.position = *elapsed;
            now.at = Instant::now();
            now.total = total.or(now.total);
            if jumped || learned {
                publish(shown.as_ref());
            }
        }
        Update::Stopped => {
            *shown = None;
            stop();
        }
        Update::Queue(queue, index) => {
            if let Ok(mut held) = QUEUE.lock() {
                *held = (queue.clone(), *index);
            }
        }
        Update::Error(_) => {}
    }
}

/// Where the song should be now, by the clock since it was last told.
fn expected(shown: &Shown) -> Duration {
    match shown.playing {
        true => shown.position + shown.at.elapsed(),
        false => shown.position,
    }
}

/// Fetches `song`'s cover (Apple's first, as everywhere) and shows it once it is in.
fn cover(song: Song) {
    runtime::spawn(async move {
        let apple = artwork::find(artwork::Wanted::song(&song), ART_EDGE).await;
        let youtube = song
            .cover
            .as_deref()
            .map(|url| library::sized(url, ART_EDGE));
        let Some(url) = apple.or(youtube) else {
            return;
        };
        let Some(bytes) = images::fetch(url).await else {
            return;
        };
        let Ok(mut shown) = SHOWN.lock() else {
            return;
        };
        if let Some(now) = shown.as_mut().filter(|now| now.song.id == song.id) {
            now.position = expected(now);
            now.at = Instant::now();
            now.art = Some(bytes.to_vec());
            publish(shown.as_ref());
        }
    });
}

/// Reads the buttons pressed in the system's controls and sends them to `engine`, on a thread
/// of its own so they work with the app in the background.
pub fn listen(engine: Engine) {
    #[cfg(target_os = "ios")]
    buttons();
    #[cfg(any(target_os = "android", target_os = "ios"))]
    std::thread::spawn(move || {
        loop {
            std::thread::sleep(LISTEN);
            while let Some(pressed) = take() {
                if let Some(command) = command(&pressed) {
                    engine.send(command);
                }
            }
        }
    });
    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    let _ = (engine, LISTEN);
}

#[cfg_attr(not(any(target_os = "android", target_os = "ios")), allow(dead_code))]
/// The engine command a button stands for. Play and pause only toggle when they change
/// something, since a headset may send either whatever the state.
fn command(pressed: &str) -> Option<Command> {
    let playing = SHOWN
        .lock()
        .ok()
        .and_then(|shown| shown.as_ref().map(|shown| shown.playing))
        .unwrap_or(false);
    match pressed {
        "play" if !playing => Some(Command::Toggle),
        "pause" if playing => Some(Command::Toggle),
        "toggle" => Some(Command::Toggle),
        "next" => Some(Command::Next),
        "previous" => Some(Command::Previous),
        seek => seek
            .strip_prefix("seek:")
            .and_then(|millis| millis.parse().ok())
            .map(|millis| Command::Seek(Duration::from_millis(millis))),
    }
}

#[cfg(target_os = "android")]
fn publish(shown: Option<&Shown>) {
    use jni::objects::{JObject, JValue};

    let Some(shown) = shown else {
        return;
    };
    let result = crate::login::with_java(|env, activity| {
        let class = crate::login::helper(env, activity, "dev.aural.app.Media")?;
        let title = env.new_string(&shown.song.title)?;
        let artist = env.new_string(&shown.song.artist)?;
        let art = match &shown.art {
            Some(bytes) => JObject::from(env.byte_array_from_slice(bytes)?),
            None => JObject::null(),
        };
        let millis = |duration: Duration| duration.as_millis() as i64;
        env.call_static_method(
            &class,
            "update",
            "(Landroid/app/Activity;Ljava/lang/String;Ljava/lang/String;[BJJZ)V",
            &[
                JValue::Object(activity),
                JValue::Object(&title),
                JValue::Object(&artist),
                JValue::Object(&art),
                JValue::Long(shown.total.map(millis).unwrap_or(0)),
                JValue::Long(millis(expected(shown))),
                JValue::Bool(shown.playing.into()),
            ],
        )?;
        Ok(())
    });
    if let Err(error) = result {
        log::warn!("media: cannot update the controls: {error:#}");
    }
}

#[cfg(target_os = "android")]
fn stop() {
    use jni::objects::JValue;

    let result = crate::login::with_java(|env, activity| {
        let class = crate::login::helper(env, activity, "dev.aural.app.Media")?;
        env.call_static_method(
            &class,
            "stop",
            "(Landroid/app/Activity;)V",
            &[JValue::Object(activity)],
        )?;
        Ok(())
    });
    if let Err(error) = result {
        log::warn!("media: cannot take the controls down: {error:#}");
    }
}

#[cfg(target_os = "android")]
fn take() -> Option<String> {
    use jni::objects::JString;

    crate::login::with_java(|env, activity| {
        let class = crate::login::helper(env, activity, "dev.aural.app.Media")?;
        let pressed = env
            .call_static_method(&class, "take", "()Ljava/lang/String;", &[])?
            .l()?;
        if pressed.is_null() {
            return Ok(None);
        }
        Ok(Some(env.get_string(&JString::from(pressed))?.into()))
    })
    .unwrap_or_else(|error| {
        log::warn!("media: cannot read the buttons: {error:#}");
        None
    })
}

/// The buttons pressed in Now Playing, waiting for `listen` to read them.
#[cfg(target_os = "ios")]
static PRESSED: Mutex<std::collections::VecDeque<String>> =
    Mutex::new(std::collections::VecDeque::new());

/// Queues a button press for the engine, as Now Playing and audio interruptions send them.
#[cfg(target_os = "ios")]
pub fn press(button: &str) {
    if let Ok(mut pressed) = PRESSED.lock() {
        pressed.push_back(button.to_string());
    }
}

#[cfg(target_os = "ios")]
fn take() -> Option<String> {
    PRESSED.lock().ok()?.pop_front()
}

/// Shows Now Playing's buttons, each queuing its press.
#[cfg(target_os = "ios")]
fn buttons() {
    use std::ptr::NonNull;

    use block2::RcBlock;
    use objc2::rc::Retained;
    use objc2_media_player::{
        MPChangePlaybackPositionCommandEvent, MPRemoteCommand, MPRemoteCommandCenter,
        MPRemoteCommandEvent, MPRemoteCommandHandlerStatus,
    };

    let on = |command: &MPRemoteCommand, handler: RcBlock<_>| {
        // SAFETY: the command center keeps the handler for as long as the app runs.
        unsafe {
            command.setEnabled(true);
            let _ = command.addTargetWithHandler(&handler);
        }
    };
    let button = |command: Retained<MPRemoteCommand>, name: &'static str| {
        on(
            &command,
            RcBlock::new(move |_: NonNull<MPRemoteCommandEvent>| {
                press(name);
                MPRemoteCommandHandlerStatus::Success
            }),
        );
    };

    // SAFETY: the shared command center is a process-wide singleton.
    let center = unsafe { MPRemoteCommandCenter::sharedCommandCenter() };
    // SAFETY: as above.
    unsafe {
        button(center.playCommand(), "play");
        button(center.pauseCommand(), "pause");
        button(center.togglePlayPauseCommand(), "toggle");
        button(center.nextTrackCommand(), "next");
        button(center.previousTrackCommand(), "previous");
        on(
            &center.changePlaybackPositionCommand(),
            RcBlock::new(|event: NonNull<MPRemoteCommandEvent>| {
                // SAFETY: the position command hands its own kind of event.
                let event = event.cast::<MPChangePlaybackPositionCommandEvent>();
                let seconds = event.as_ref().positionTime();
                press(&format!("seek:{}", (seconds.max(0.) * 1000.) as u64));
                MPRemoteCommandHandlerStatus::Success
            }),
        );
    }
}

#[cfg(target_os = "ios")]
fn publish(shown: Option<&Shown>) {
    use objc2::rc::Retained;
    use objc2::runtime::AnyObject;
    use objc2_foundation::{NSDictionary, NSNumber, NSString};
    use objc2_media_player::{
        MPMediaItemPropertyArtist, MPMediaItemPropertyArtwork, MPMediaItemPropertyPlaybackDuration,
        MPMediaItemPropertyTitle, MPNowPlayingInfoCenter,
        MPNowPlayingInfoPropertyElapsedPlaybackTime, MPNowPlayingInfoPropertyPlaybackRate,
    };

    let Some(shown) = shown else {
        return;
    };
    let seconds = |duration: Duration| NSNumber::numberWithDouble(duration.as_secs_f64()).into();
    // SAFETY: the keys are constants MediaPlayer defines.
    let mut info: Vec<(&NSString, Retained<AnyObject>)> = unsafe {
        vec![
            (
                MPMediaItemPropertyTitle,
                NSString::from_str(&shown.song.title).into(),
            ),
            (
                MPMediaItemPropertyArtist,
                NSString::from_str(&shown.song.artist).into(),
            ),
            (
                MPNowPlayingInfoPropertyElapsedPlaybackTime,
                seconds(expected(shown)),
            ),
            (
                MPNowPlayingInfoPropertyPlaybackRate,
                NSNumber::numberWithDouble(if shown.playing { 1. } else { 0. }).into(),
            ),
        ]
    };
    if let Some(total) = shown.total {
        // SAFETY: as above.
        info.push((
            unsafe { MPMediaItemPropertyPlaybackDuration },
            seconds(total),
        ));
    }
    if let Some(artwork) = shown.art.as_deref().and_then(artwork) {
        // SAFETY: as above.
        info.push((unsafe { MPMediaItemPropertyArtwork }, artwork));
    }
    let (keys, values): (Vec<_>, Vec<_>) = info.into_iter().unzip();
    let info = NSDictionary::from_retained_objects(&keys, &values);
    // SAFETY: the default center takes a dictionary of the properties above, from any thread.
    unsafe { MPNowPlayingInfoCenter::defaultCenter().setNowPlayingInfo(Some(&info)) };
}

/// The cover as Now Playing takes it: an `MPMediaItemArtwork` handing out a `UIImage`.
#[cfg(target_os = "ios")]
fn artwork(bytes: &[u8]) -> Option<objc2::rc::Retained<objc2::runtime::AnyObject>> {
    use std::ptr::NonNull;

    use block2::RcBlock;
    use objc2::rc::Retained;
    use objc2::runtime::AnyObject;
    use objc2::{AnyThread as _, class, msg_send};
    use objc2_core_foundation::CGSize;
    use objc2_foundation::NSData;
    use objc2_media_player::MPMediaItemArtwork;

    let data = NSData::with_bytes(bytes);
    // SAFETY: `imageWithData:` takes any data and returns nil for what is not an image.
    let image: Option<Retained<AnyObject>> =
        unsafe { msg_send![class!(UIImage), imageWithData: &*data] };
    let image = image?;
    // SAFETY: a `UIImage` has a size.
    let size: CGSize = unsafe { msg_send![&*image, size] };
    let handler = RcBlock::new(move |_: CGSize| NonNull::from(&*image));
    // SAFETY: the handler returns a `UIImage` that lives as long as the artwork holds it.
    let artwork: Retained<MPMediaItemArtwork> = unsafe {
        msg_send![MPMediaItemArtwork::alloc(), initWithBoundsSize: size, requestHandler: &*handler]
    };
    Some(artwork.into())
}

#[cfg(target_os = "ios")]
fn stop() {
    // SAFETY: the default center takes no info at all, from any thread.
    unsafe { objc2_media_player::MPNowPlayingInfoCenter::defaultCenter().setNowPlayingInfo(None) };
}

#[cfg(not(any(target_os = "android", target_os = "ios")))]
fn publish(_shown: Option<&Shown>) {}

#[cfg(not(any(target_os = "android", target_os = "ios")))]
fn stop() {}

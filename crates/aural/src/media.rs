//! Android's media controls: the player in quick settings, on the lock screen and in the
//! notification shade, through `dev.aural.app.Media` (see `android/java`). It follows the
//! engine's updates on the engine's own thread, so it stays right while the app is in the
//! background, and hands the buttons pressed there back to the engine.
//!
//! Elsewhere it does nothing yet.

use std::sync::Mutex;
use std::time::{Duration, Instant};

use crate::engine::{Command, Engine, Update};
use crate::library::{self, Song};
use crate::{artwork, images, runtime};

/// The side of the cover handed to Android.
const ART_EDGE: u32 = 512;
/// How far the position may drift from where it should be before Android is told again.
/// Android moves the bar on its own from the last position and the play state.
const DRIFT: Duration = Duration::from_millis(1500);
/// How often the buttons pressed are read.
const LISTEN: Duration = Duration::from_millis(250);

/// What Android was last shown.
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
        Update::Queue(..) | Update::Error(_) => {}
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

/// Reads the buttons pressed in Android's controls and sends them to `engine`, on a thread of
/// its own so they work with the app in the background.
pub fn listen(engine: Engine) {
    #[cfg(target_os = "android")]
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
    #[cfg(not(target_os = "android"))]
    let _ = (engine, LISTEN);
}

#[cfg_attr(not(target_os = "android"), allow(dead_code))]
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

#[cfg(not(target_os = "android"))]
fn publish(_shown: Option<&Shown>) {}

#[cfg(not(target_os = "android"))]
fn stop() {}

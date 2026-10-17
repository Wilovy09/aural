//! What differs between platforms outside the UI: where the app keeps its files, and
//! keeping the screen on while music plays.

use std::path::PathBuf;

/// The app's private folder, created if missing.
pub fn data_dir() -> PathBuf {
    #[cfg(target_os = "android")]
    let dir = crate::login::data_dir().unwrap_or_else(std::env::temp_dir);
    // A second copy on one computer (to try Aural Connect) can keep its own folder.
    #[cfg(not(target_os = "android"))]
    let dir = std::env::var_os("AURAL_DATA")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| {
            dirs::data_dir()
                .unwrap_or_else(std::env::temp_dir)
                .join("aural")
        });
    let _ = std::fs::create_dir_all(&dir);
    dir
}

/// Where the app keeps what it can fetch again, such as covers: the system's cache folder on a
/// computer, which the system may empty, and the app's private folder on a phone.
pub fn cache_dir() -> PathBuf {
    #[cfg(any(target_os = "android", target_os = "ios"))]
    let dir = data_dir().join("cache");
    // A second copy on one computer keeps its cache beside its data, as it does the rest.
    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    let dir = match std::env::var_os("AURAL_DATA") {
        Some(data) => PathBuf::from(data).join("cache"),
        None => dirs::cache_dir()
            .map(|dir| dir.join("aural"))
            .unwrap_or_else(|| data_dir().join("cache")),
    };
    dir
}

/// Keeps the screen from dimming or starting the TV's screensaver while `awake`; lets it
/// again otherwise. Called with whether music is playing.
pub fn keep_awake(awake: bool) {
    // Through Java on Android's UI thread: android-activity's own window-flags call takes a
    // lock the activity's main thread can hold, and froze the app when tried.
    #[cfg(target_os = "android")]
    if let Err(error) = crate::login::keep_screen_on(awake) {
        log::warn!("platform: cannot change keep-screen-on: {error:#}");
    }
    #[cfg(not(target_os = "android"))]
    let _ = awake;
}

/// How wide, in logical pixels, a TV's interface is laid out: a TV reports a phone's density
/// (2× on a 1080p screen), which leaves a desktop layout under 1000 pixels to work with.
const TV_WIDTH: f64 = 1280.;

/// The zoom that lays a TV's interface out [`TV_WIDTH`] wide, or `None` off a TV. Asked once.
pub fn tv_zoom() -> Option<f64> {
    #[cfg(target_os = "android")]
    {
        static ZOOM: std::sync::OnceLock<Option<f64>> = std::sync::OnceLock::new();
        *ZOOM.get_or_init(|| match android_tv_zoom() {
            Ok(zoom) => zoom,
            Err(error) => {
                log::warn!("platform: cannot tell whether this is a TV: {error:#}");
                None
            }
        })
    }
    #[cfg(not(target_os = "android"))]
    None
}

/// Whether Android says this is a TV (`UI_MODE_TYPE_TELEVISION`), and if so the zoom off its
/// screen's width and density.
#[cfg(target_os = "android")]
fn android_tv_zoom() -> anyhow::Result<Option<f64>> {
    use jni::objects::JValue;
    crate::login::with_java(|env, activity| {
        let name = env.new_string("uimode")?;
        let manager = env
            .call_method(
                activity,
                "getSystemService",
                "(Ljava/lang/String;)Ljava/lang/Object;",
                &[JValue::Object(&name)],
            )?
            .l()?;
        let mode = env
            .call_method(&manager, "getCurrentModeType", "()I", &[])?
            .i()?;
        // UiModeManager.UI_MODE_TYPE_TELEVISION
        if mode != 4 {
            return Ok(None);
        }
        let resources = env
            .call_method(activity, "getResources", "()Landroid/content/res/Resources;", &[])?
            .l()?;
        let metrics = env
            .call_method(
                &resources,
                "getDisplayMetrics",
                "()Landroid/util/DisplayMetrics;",
                &[],
            )?
            .l()?;
        let width = env.get_field(&metrics, "widthPixels", "I")?.i()?;
        let height = env.get_field(&metrics, "heightPixels", "I")?.i()?;
        let density = env.get_field(&metrics, "density", "F")?.f()?;
        let wide = width.max(height) as f64;
        let zoom = wide / (TV_WIDTH * density.max(0.5) as f64);
        log::info!("platform: a TV, {width}×{height} at {density}×, zoomed to {zoom:.2}");
        Ok(Some(zoom))
    })
}

/// Holds Wi-Fi's multicast lock, without which Android drops the mDNS announcements of the
/// other devices.
#[cfg(target_os = "android")]
pub fn multicast() -> anyhow::Result<()> {
    crate::login::with_java(|env, activity| {
        let class = crate::login::helper(env, activity, "dev.aural.app.Network")?;
        env.call_static_method(
            &class,
            "multicast",
            "(Landroid/app/Activity;)V",
            &[jni::objects::JValue::Object(activity)],
        )?;
        Ok(())
    })
}

/// The room the notch (or the status bar) and the home indicator take, in physical pixels, read
/// off the app's window: Freya draws under them, as on Android.
#[cfg(target_os = "ios")]
pub fn insets() -> (f32, f32) {
    use objc2::MainThreadMarker;
    use objc2_ui_kit::UIApplication;
    let Some(mtm) = MainThreadMarker::new() else {
        return (0., 0.);
    };
    let application = UIApplication::sharedApplication(mtm);
    #[allow(deprecated)]
    let Some(window) = application
        .keyWindow()
        .or_else(|| application.windows().firstObject())
    else {
        return (0., 0.);
    };
    let insets = window.safeAreaInsets();
    let scale = window.screen().scale();
    ((insets.top * scale) as f32, (insets.bottom * scale) as f32)
}

/// Set once a call, Siri or an alarm that took the audio over has ended.
#[cfg(target_os = "ios")]
static INTERRUPTION_ENDED: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(false);

/// Whether an interruption ended since last asked: iOS stopped the audio output for it, and
/// the engine starts it again.
#[cfg(target_os = "ios")]
pub fn interruption_ended() -> bool {
    INTERRUPTION_ENDED.swap(false, std::sync::atomic::Ordering::SeqCst)
}

/// Makes Aural a music player to iOS: the `Playback` category plays with the silent switch on
/// and keeps playing in the background, where the default `SoloAmbient` would mute it. Also
/// follows interruptions, which stop the audio output: the music pauses when one begins, and
/// plays on after it when iOS says so.
#[cfg(target_os = "ios")]
pub fn audio_session() {
    use std::ptr::NonNull;

    use block2::RcBlock;
    use objc2_avf_audio::{
        AVAudioSession, AVAudioSessionCategoryPlayback, AVAudioSessionInterruptionNotification,
        AVAudioSessionInterruptionOptionKey, AVAudioSessionInterruptionTypeKey,
    };
    use objc2_foundation::{NSNotification, NSNotificationCenter, NSNumber, NSString};

    /// `AVAudioSessionInterruptionType`'s and `AVAudioSessionInterruptionOptions`' values.
    const BEGAN: usize = 1;
    const ENDED: usize = 0;
    const SHOULD_RESUME: usize = 1;

    // SAFETY: the shared session is a process-wide singleton safe to use from any thread.
    let session = unsafe { AVAudioSession::sharedInstance() };
    // SAFETY: the categories are constants AVFAudio defines.
    if let Some(playback) = unsafe { AVAudioSessionCategoryPlayback } {
        // SAFETY: as above.
        if let Err(error) = unsafe { session.setCategory_error(playback) } {
            log::warn!("platform: cannot set the audio category: {error:?}");
        }
    }
    activate();

    let observer = RcBlock::new(|notification: NonNull<NSNotification>| {
        // SAFETY: the notification center hands a valid notification for the call.
        let Some(info) = unsafe { notification.as_ref() }.userInfo() else {
            return;
        };
        let number = |key: Option<&NSString>| {
            info.objectForKey(key?)
                .and_then(|value| value.downcast::<NSNumber>().ok())
                .map(|number| number.unsignedIntegerValue())
        };
        // SAFETY: the keys are constants AVFAudio defines.
        let (kind, options) = unsafe {
            (
                number(AVAudioSessionInterruptionTypeKey),
                number(AVAudioSessionInterruptionOptionKey).unwrap_or(0),
            )
        };
        match kind {
            Some(BEGAN) => crate::media::press("pause"),
            Some(ENDED) => {
                activate();
                INTERRUPTION_ENDED.store(true, std::sync::atomic::Ordering::SeqCst);
                if options & SHOULD_RESUME != 0 {
                    crate::media::press("play");
                }
            }
            _ => {}
        }
    });
    // SAFETY: the block only touches thread-safe state, and the observer lives as long as
    // the app, as it is never removed.
    unsafe {
        let observer = NSNotificationCenter::defaultCenter()
            .addObserverForName_object_queue_usingBlock(
                AVAudioSessionInterruptionNotification,
                Some(&session),
                None,
                &observer,
            );
        std::mem::forget(observer);
    }
}

/// Makes the audio session active, which an interruption undoes.
#[cfg(target_os = "ios")]
fn activate() {
    // SAFETY: the shared session is a process-wide singleton safe to use from any thread.
    if let Err(error) =
        unsafe { objc2_avf_audio::AVAudioSession::sharedInstance().setActive_error(true) }
    {
        log::warn!("platform: cannot start the audio session: {error:?}");
    }
}

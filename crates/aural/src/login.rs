//! Google sign-in for YouTube Music: on Android, Google's page in a WebView
//! (`dev.aural.app.Login`, see `android/java`) until the YouTube proof cookies show up. Cookie
//! values are never logged or shown.

use std::time::{Duration, Instant};

use anyhow::Result;

/// The same sign-in Sonora opens: Google's page, told to come back to YouTube Music.
const SIGN_IN_URL: &str = "https://accounts.google.com/ServiceLogin?ltmpl=music&service=youtube&passive=true&continue=https%3A%2F%2Fwww.youtube.com%2Fsignin%3Faction_handle_signin%3Dtrue%26next%3Dhttps%253A%252F%252Fmusic.youtube.com%252F";
/// How often the window's state is read.
const POLL: Duration = Duration::from_millis(500);
/// How long the user gets to finish signing in.
const PATIENCE: Duration = Duration::from_secs(10 * 60);

/// Opens the sign-in window and blocks until it hands back the `Cookie` header, reporting each
/// page it lands on to `say`.
pub fn web_sign_in(say: &dyn Fn(String)) -> Result<String> {
    platform::sign_in(say)
}

/// Whether this platform has a sign-in window.
pub fn supported() -> bool {
    cfg!(target_os = "android")
}

#[cfg(target_os = "android")]
pub use platform::{data_dir, keep_screen_on, remember};

#[cfg(target_os = "android")]
mod platform {
    use std::sync::OnceLock;

    use anyhow::Context as _;

    use freya::winit::platform::android::activity::AndroidApp;
    use jni::JavaVM;
    use jni::objects::{JClass, JObject, JString, JValue};

    use super::*;

    const CLASS: &str = "dev.aural.app.Login";

    /// The running activity, kept by `android_main` for JNI calls from worker threads.
    static APP: OnceLock<AndroidApp> = OnceLock::new();

    /// Remembers the activity `android_main` was started with.
    pub fn remember(app: AndroidApp) {
        let _ = APP.set(app);
    }

    /// The app's private data folder.
    pub fn data_dir() -> Option<std::path::PathBuf> {
        APP.get()?.internal_data_path()
    }

    /// Adds or clears the window's keep-screen-on flag through `dev.aural.app.Screen`, which
    /// changes it on Android's UI thread.
    pub fn keep_screen_on(on: bool) -> Result<()> {
        let app = APP.get().context("android_main has not run")?;
        // SAFETY: android-activity hands out the process JavaVM.
        let vm = unsafe { JavaVM::from_raw(app.vm_as_ptr().cast()) }.context("no JavaVM")?;
        let mut env = vm
            .attach_current_thread()
            .context("cannot attach to the JVM")?;
        // SAFETY: a global reference to the activity, valid while the activity is alive.
        let activity = unsafe { JObject::from_raw(app.activity_as_ptr().cast()) };
        let loader = env
            .call_method(
                &activity,
                "getClassLoader",
                "()Ljava/lang/ClassLoader;",
                &[],
            )?
            .l()?;
        let name = env.new_string("dev.aural.app.Screen")?;
        let class: JClass = env
            .call_method(
                &loader,
                "loadClass",
                "(Ljava/lang/String;)Ljava/lang/Class;",
                &[JValue::Object(&name)],
            )?
            .l()?
            .into();
        env.call_static_method(
            &class,
            "keepOn",
            "(Landroid/app/Activity;Z)V",
            &[JValue::Object(&activity), JValue::Bool(on.into())],
        )?;
        Ok(())
    }

    /// Opens the window and waits for the cookie header, reporting each page it lands on.
    pub fn sign_in(say: &dyn Fn(String)) -> Result<String> {
        let app = APP.get().context("android_main has not run")?;
        // SAFETY: android-activity hands out the process JavaVM.
        let vm = unsafe { JavaVM::from_raw(app.vm_as_ptr().cast()) }.context("no JavaVM")?;
        let mut env = vm
            .attach_current_thread()
            .context("cannot attach to the JVM")?;
        // SAFETY: a global reference to the activity, valid while the activity is alive.
        let activity = unsafe { JObject::from_raw(app.activity_as_ptr().cast()) };

        let loader = env
            .call_method(
                &activity,
                "getClassLoader",
                "()Ljava/lang/ClassLoader;",
                &[],
            )?
            .l()?;
        let name = env.new_string(CLASS)?;
        let class: JClass = env
            .call_method(
                &loader,
                "loadClass",
                "(Ljava/lang/String;)Ljava/lang/Class;",
                &[JValue::Object(&name)],
            )?
            .l()?
            .into();

        let url = env.new_string(SIGN_IN_URL)?;
        env.call_static_method(
            &class,
            "open",
            "(Landroid/app/Activity;Ljava/lang/String;)V",
            &[JValue::Object(&activity), JValue::Object(&url)],
        )?;

        let started = Instant::now();
        let mut last = String::new();
        while started.elapsed() < PATIENCE {
            std::thread::sleep(POLL);
            let state = string(&mut env, &class, "state")?.unwrap_or_default();
            if state != last {
                say(state.clone());
                last = state.clone();
            }
            if state == "cancelled" || state.starts_with("error") {
                anyhow::bail!("sign-in ended: {state}");
            }
            if let Some(header) = string(&mut env, &class, "take")? {
                return Ok(header);
            }
        }
        anyhow::bail!("sign-in timed out")
    }

    fn string(env: &mut jni::JNIEnv, class: &JClass, method: &str) -> Result<Option<String>> {
        let value = env
            .call_static_method(class, method, "()Ljava/lang/String;", &[])?
            .l()?;
        if value.is_null() {
            return Ok(None);
        }
        let value = JString::from(value);
        Ok(Some(env.get_string(&value)?.into()))
    }
}

#[cfg(not(target_os = "android"))]
mod platform {
    use super::*;

    pub fn sign_in(_say: &dyn Fn(String)) -> Result<String> {
        let _ = (SIGN_IN_URL, POLL, PATIENCE, Instant::now());
        anyhow::bail!("no sign-in window on this platform yet")
    }
}

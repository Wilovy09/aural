//! Phase 0 sign-in check: opens Google's sign-in in an Android WebView (`dev.aural.app.Login`,
//! see `android/java`), waits for the YouTube proof cookies and asks YouTube Music who they
//! belong to. Cookie values are never logged or shown, only their count and the account name.

use std::time::{Duration, Instant};

use anyhow::{Context as _, Result};
use tokio::sync::mpsc::UnboundedSender;

/// The same sign-in Sonora opens: Google's page, told to come back to YouTube Music.
const SIGN_IN_URL: &str = "https://accounts.google.com/ServiceLogin?ltmpl=music&service=youtube&passive=true&continue=https%3A%2F%2Fwww.youtube.com%2Fsignin%3Faction_handle_signin%3Dtrue%26next%3Dhttps%253A%252F%252Fmusic.youtube.com%252F";
/// How often the window's state is read.
const POLL: Duration = Duration::from_millis(500);
/// How long the user gets to finish signing in.
const PATIENCE: Duration = Duration::from_secs(10 * 60);

/// Runs the check on its own thread; every step is sent to `report`.
pub fn check(report: UnboundedSender<String>) {
    std::thread::spawn(move || {
        let say = |line: String| {
            log::info!("login: {line}");
            let _ = report.send(line);
        };
        if let Err(error) = run(&say) {
            say(format!("error: {error:#}"));
        }
    });
}

fn run(say: &dyn Fn(String)) -> Result<()> {
    let header = platform::sign_in(say)?;
    let count = header.split(';').filter(|pair| pair.contains('=')).count();
    say(format!(
        "cookies recibidas ({count}), verificando la cuenta"
    ));

    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .context("cannot start tokio")?;
    let api = ytmusic::YtMusic::with_cookies(header);
    let identities = runtime
        .block_on(api.identities())
        .context("cannot list accounts")?;
    let names: Vec<_> = identities.iter().map(|i| i.profile.name.as_str()).collect();
    say(format!(
        "sesión OK: {} ({} cuentas)",
        names.join(", "),
        names.len()
    ));
    Ok(())
}

#[cfg(target_os = "android")]
pub use platform::{data_dir, remember};

#[cfg(target_os = "android")]
mod platform {
    use std::sync::OnceLock;

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
        anyhow::bail!("this check only runs on Android")
    }
}

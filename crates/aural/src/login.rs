//! Google sign-in for YouTube Music: Google's page in a browser window until the YouTube proof
//! cookies show up. On Android that is a WebView (`dev.aural.app.Login`, see `android/java`);
//! on macOS, Windows and Linux it is the `webview` crate's native window (WebKit, WebView2,
//! WebKitGTK) over a throwaway session. Cookie values are never logged or shown.

use std::time::{Duration, Instant};

use anyhow::Result;

/// The same sign-in Sonora opens: Google's page, told to come back to YouTube Music.
const SIGN_IN_URL: &str = "https://accounts.google.com/ServiceLogin?ltmpl=music&service=youtube&passive=true&continue=https%3A%2F%2Fwww.youtube.com%2Fsignin%3Faction_handle_signin%3Dtrue%26next%3Dhttps%253A%252F%252Fmusic.youtube.com%252F";
/// How often the window's state is read.
const POLL: Duration = Duration::from_millis(500);
/// How long the user gets to finish signing in.
const PATIENCE: Duration = Duration::from_secs(10 * 60);

/// Whether this platform has a sign-in window. On Linux it depends on webkit2gtk being
/// installed.
pub fn supported() -> bool {
    #[cfg(target_os = "android")]
    return true;
    #[cfg(not(target_os = "android"))]
    return webview::supported();
}

/// Opens the sign-in window and blocks until it hands back the `Cookie` header, reporting each
/// page it lands on to `say`. Runs on a worker thread.
#[cfg(target_os = "android")]
pub fn web_sign_in(say: &dyn Fn(String)) -> Result<String> {
    platform::sign_in(say)
}

#[cfg(not(target_os = "android"))]
pub use platform::window_sign_in;

#[cfg(target_os = "android")]
pub use platform::{data_dir, helper, insets, keep_screen_on, remember, with_java};

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

    /// The room the visible system bars take at the top and bottom, in physical pixels: the
    /// window is drawn edge to edge, under them. Asked once, when the window has been laid out.
    pub fn insets() -> (f32, f32) {
        static INSETS: OnceLock<(f32, f32)> = OnceLock::new();
        if let Some(insets) = INSETS.get() {
            return *insets;
        }
        match ask_insets() {
            Ok(Some(insets)) => *INSETS.get_or_init(|| insets),
            Ok(None) => (0., 0.),
            Err(error) => {
                log::warn!("insets: {error:#}");
                *INSETS.get_or_init(|| (0., 0.))
            }
        }
    }

    fn ask_insets() -> Result<Option<(f32, f32)>> {
        let app = APP.get().context("android_main has not run")?;
        // SAFETY: android-activity hands out the process JavaVM.
        let vm = unsafe { JavaVM::from_raw(app.vm_as_ptr().cast()) }.context("no JavaVM")?;
        let mut env = vm
            .attach_current_thread()
            .context("cannot attach to the JVM")?;
        // SAFETY: a global reference to the activity, valid while the activity is alive.
        let activity = unsafe { JObject::from_raw(app.activity_as_ptr().cast()) };
        let class = screen(&mut env, &activity)?;
        let found = env
            .call_static_method(
                &class,
                "insets",
                "(Landroid/app/Activity;)[I",
                &[JValue::Object(&activity)],
            )?
            .l()?;
        if found.is_null() {
            return Ok(None);
        }
        let array = jni::objects::JIntArray::from(found);
        let mut pair = [0i32; 2];
        env.get_int_array_region(&array, 0, &mut pair)?;
        Ok(Some((pair[0] as f32, pair[1] as f32)))
    }

    /// `dev.aural.app.Screen`, through the activity's class loader.
    fn screen<'local>(env: &mut jni::JNIEnv<'local>, activity: &JObject) -> Result<JClass<'local>> {
        helper(env, activity, "dev.aural.app.Screen")
    }

    /// Runs `job` with this thread attached to the JVM and the activity at hand, for the Java
    /// helpers in `android/java`.
    pub fn with_java<R>(job: impl FnOnce(&mut jni::JNIEnv, &JObject) -> Result<R>) -> Result<R> {
        let app = APP.get().context("android_main has not run")?;
        // SAFETY: android-activity hands out the process JavaVM.
        let vm = unsafe { JavaVM::from_raw(app.vm_as_ptr().cast()) }.context("no JavaVM")?;
        let mut env = vm
            .attach_current_thread()
            .context("cannot attach to the JVM")?;
        // SAFETY: a global reference to the activity, valid while the activity is alive.
        let activity = unsafe { JObject::from_raw(app.activity_as_ptr().cast()) };
        job(&mut env, &activity)
    }

    /// One of the Java helpers, `name` such as `dev.aural.app.Media`, through the activity's
    /// class loader: the system one does not know the app's classes.
    pub fn helper<'local>(
        env: &mut jni::JNIEnv<'local>,
        activity: &JObject,
        name: &str,
    ) -> Result<JClass<'local>> {
        let loader = env
            .call_method(activity, "getClassLoader", "()Ljava/lang/ClassLoader;", &[])?
            .l()?;
        let name = env.new_string(name)?;
        Ok(env
            .call_method(
                &loader,
                "loadClass",
                "(Ljava/lang/String;)Ljava/lang/Class;",
                &[JValue::Object(&name)],
            )?
            .l()?
            .into())
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
        let class = screen(&mut env, &activity)?;
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

    /// The YouTube Music page the sign-in comes back to, and the domain its cookies live on.
    const LANDING: &str = "music.youtube.com";
    const DOMAIN: &str = "youtube.com";

    /// Opens the native sign-in window and waits for the `Cookie` header. The window belongs to
    /// the main thread, so this runs on Freya's executor there and sleeps on the io runtime.
    pub async fn window_sign_in(say: impl Fn(String)) -> Result<String> {
        let mut page = webview::Page::open(webview::Target {
            url: SIGN_IN_URL.to_owned(),
            landing: LANDING.to_owned(),
            domain: DOMAIN.to_owned(),
            proof: crate::session::PROOF.map(str::to_owned).to_vec(),
            title: "Aural · Iniciar sesión".to_owned(),
            agent: None,
            script: None,
        })?;
        say("Inicia sesión en la ventana de Google".into());
        let started = Instant::now();
        while started.elapsed() < PATIENCE {
            let _ = crate::runtime::spawn(async { tokio::time::sleep(POLL).await }).await;
            match page.poll() {
                webview::Poll::Pending => {}
                webview::Poll::Closed => anyhow::bail!("sign-in cancelled"),
                webview::Poll::Cookies(header) => return Ok(header),
            }
        }
        anyhow::bail!("sign-in timed out")
    }
}

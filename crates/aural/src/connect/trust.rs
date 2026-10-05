//! Who this device is, and the devices it has been paired with, kept on disk.

use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

use serde::{Deserialize, Serialize};

use crate::platform;

/// This device as others see it.
pub struct Me {
    pub id: String,
    pub name: String,
}

/// This device: an id made once and kept, and a name people recognise.
pub fn me() -> &'static Me {
    static ME: OnceLock<Me> = OnceLock::new();
    ME.get_or_init(|| {
        let path = platform::data_dir().join("device-id");
        let id = std::fs::read_to_string(&path)
            .ok()
            .map(|id| id.trim().to_owned())
            .filter(|id| !id.is_empty())
            .unwrap_or_else(|| {
                let id = format!("{:016x}", super::random());
                let _ = std::fs::write(&path, &id);
                id
            });
        Me {
            id,
            name: name(),
        }
    })
}

/// The device's name: its model on Android, the computer's name elsewhere.
fn name() -> String {
    if let Ok(name) = std::env::var("AURAL_NAME") {
        return name;
    }
    #[cfg(target_os = "android")]
    if let Some(model) = model() {
        return model;
    }
    let host = gethostname::gethostname().to_string_lossy().into_owned();
    let host = host.trim_end_matches(".local").to_owned();
    match host.is_empty() {
        true => "Aural".to_owned(),
        false => host,
    }
}

#[cfg(target_os = "android")]
fn model() -> Option<String> {
    crate::login::with_java(|env, _| {
        let value = env
            .get_static_field("android/os/Build", "MODEL", "Ljava/lang/String;")?
            .l()?;
        Ok(env.get_string(&jni::objects::JString::from(value))?.into())
    })
    .ok()
}

#[derive(Default, Serialize, Deserialize)]
struct Book {
    /// Tokens this device was given by hosts, by host id.
    hosts: HashMap<String, String>,
    /// Tokens this device gave controllers, by controller id.
    controllers: HashMap<String, String>,
}

fn book() -> &'static Mutex<Book> {
    static BOOK: OnceLock<Mutex<Book>> = OnceLock::new();
    BOOK.get_or_init(|| {
        let saved = std::fs::read(file())
            .ok()
            .and_then(|body| serde_json::from_slice(&body).ok())
            .unwrap_or_default();
        Mutex::new(saved)
    })
}

fn file() -> std::path::PathBuf {
    platform::data_dir().join("connect.json")
}

fn save(book: &Book) {
    if let Ok(body) = serde_json::to_vec(book) {
        let _ = std::fs::write(file(), body);
    }
}

/// The token host `id` gave this device, if they were paired.
pub fn host_token(id: &str) -> Option<String> {
    book().lock().ok()?.hosts.get(id).cloned()
}

/// Remembers the token host `id` gave this device.
pub fn keep_host(id: &str, token: &str) {
    if let Ok(mut book) = book().lock() {
        book.hosts.insert(id.to_owned(), token.to_owned());
        save(&book);
    }
}

/// Whether controller `id` was paired with this device and holds `token`.
pub fn trusted(id: &str, token: &str) -> bool {
    book()
        .lock()
        .ok()
        .and_then(|book| book.controllers.get(id).map(|kept| kept == token))
        .unwrap_or(false)
}

/// Pairs controller `id` with this device and returns the token it will say hello with.
pub fn trust(id: &str) -> String {
    let token = format!("{:016x}{:016x}", super::random(), super::random());
    if let Ok(mut book) = book().lock() {
        book.controllers.insert(id.to_owned(), token.clone());
        save(&book);
    }
    token
}

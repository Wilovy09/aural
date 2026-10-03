#![cfg_attr(
    all(not(debug_assertions), target_os = "windows"),
    windows_subsystem = "windows"
)]

#[cfg(not(target_os = "android"))]
fn main() {
    aural::run();
}

#[cfg(target_os = "android")]
fn main() {}

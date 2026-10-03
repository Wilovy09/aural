//! Aural phase 0 spike: a Freya window that reports which keys reach the app and moves a
//! selection across a grid with the arrow keys, so a TV remote's D-pad can be checked on device.

use freya::prelude::*;

mod cover;
mod login;
mod player;
mod probe;

/// Tiles per row in the D-pad test grid.
const COLUMNS: usize = 4;
/// Tiles in the D-pad test grid.
const TILES: usize = 12;
/// How many received keys the log keeps on screen.
const LOG: usize = 10;

/// Side of the cover square.
const COVER: f32 = 280.;

const BACKGROUND: (u8, u8, u8) = (12, 12, 16);
const SURFACE: (u8, u8, u8) = (28, 28, 36);
const ACCENT: (u8, u8, u8) = (255, 92, 122);
const MUTED: (u8, u8, u8) = (150, 150, 165);

/// The window every platform opens.
pub fn window() -> WindowConfig {
    WindowConfig::new(app)
        .with_title("Aural")
        .with_size(1280., 720.)
        .with_background(BACKGROUND)
}

/// Desktop entry point.
#[cfg(not(target_os = "android"))]
pub fn run() {
    env_logger::init();
    launch(
        LaunchConfig::new()
            .with_plugin(probe::Probe::default())
            .with_window(window()),
    );
}

/// Android entry point, called by `NativeActivity` once `libaural.so` is loaded.
#[cfg(target_os = "android")]
#[unsafe(no_mangle)]
fn android_main(droid: freya::winit::platform::android::activity::AndroidApp) {
    use freya::winit::event_loop::EventLoop;
    use freya::winit::platform::android::EventLoopBuilderExtAndroid as _;

    android_logger::init_once(
        android_logger::Config::default()
            .with_max_level(log::LevelFilter::Info)
            .with_tag("aural"),
    );
    log::info!("aural: android_main, abi {}", std::env::consts::ARCH);

    login::remember(droid.clone());

    let event_loop = EventLoop::with_user_event()
        .with_android_app(droid.clone())
        .build()
        .expect("cannot build the event loop");

    launch(
        LaunchConfig::new()
            .with_event_loop(event_loop)
            .with_plugin(freya::android::AndroidPlugin::new(droid))
            .with_plugin(probe::Probe::default())
            .with_window(window()),
    );
}

fn app() -> impl IntoElement {
    let mut selected = use_state(|| 0usize);
    let mut log = use_state(Vec::<String>::new);
    let mut presses = use_state(|| 0usize);
    let mut status = use_state(|| String::from("OK en 1: reproducir · 2: login de Google"));
    let mut playing = use_state(|| false);
    let mut song = use_state(|| None::<(String, String)>);
    let mut motion_status = use_state(String::new);
    let mut art = use_state(|| None::<cover::Art>);
    let mut art_number = use_state(|| 0u64);
    let mut player_on = use_state(|| false);

    let mut start_player = move || {
        if *player_on.peek() {
            return;
        }
        player_on.set(true);
        let (sender, mut receiver) = tokio::sync::mpsc::unbounded_channel();
        let (frames, mut latest) = tokio::sync::watch::channel(None);
        player::play(sender, frames);
        spawn(async move {
            while let Some(event) = receiver.recv().await {
                match event {
                    player::Event::Status(line) => status.set(line),
                    player::Event::Track { title, artist } => song.set(Some((title, artist))),
                    player::Event::Still(bytes) => {
                        art.set(Some(cover::Art::Still(bytes)));
                        *art_number.write() += 1;
                    }
                    player::Event::Motion(line) => motion_status.set(line),
                }
            }
            player_on.set(false);
        });
        spawn(async move {
            while latest.changed().await.is_ok() {
                if let Some(frame) = latest.borrow_and_update().clone() {
                    art.set(Some(cover::Art::Motion(frame)));
                    *art_number.write() += 1;
                }
            }
        });
    };

    let mut start = move |check: fn(tokio::sync::mpsc::UnboundedSender<String>)| {
        if *playing.peek() {
            return;
        }
        playing.set(true);
        let (sender, mut receiver) = tokio::sync::mpsc::unbounded_channel();
        check(sender);
        spawn(async move {
            while let Some(line) = receiver.recv().await {
                status.set(line);
            }
            playing.set(false);
        });
    };

    let on_key = move |e: Event<KeyboardEventData>| {
        let entry = format!("{:?}  ·  {:?}", e.key, e.code);
        log::info!("aural: key {entry}");
        {
            let mut log = log.write();
            log.insert(0, entry);
            log.truncate(LOG);
        }
        let current = *selected.read();
        let next = match &e.key {
            Key::Named(NamedKey::ArrowLeft) if current % COLUMNS > 0 => Some(current - 1),
            Key::Named(NamedKey::ArrowRight) if current % COLUMNS < COLUMNS - 1 => {
                Some((current + 1).min(TILES - 1))
            }
            Key::Named(NamedKey::ArrowUp) if current >= COLUMNS => Some(current - COLUMNS),
            Key::Named(NamedKey::ArrowDown) if current + COLUMNS < TILES => Some(current + COLUMNS),
            _ => None,
        };
        if let Some(next) = next {
            selected.set(next);
        }
        if matches!(e.key, Key::Named(NamedKey::Enter)) {
            *presses.write() += 1;
        }
        match (&e.key, current) {
            (Key::Named(NamedKey::Enter), 1) => start(login::check),
            (Key::Named(NamedKey::Enter | NamedKey::MediaPlayPause | NamedKey::MediaPlay), _) => {
                start_player()
            }
            _ => {}
        }
    };

    let rows = (0..TILES.div_ceil(COLUMNS)).map(|row| {
        rect()
            .key(row)
            .direction(Direction::Horizontal)
            .spacing(16.)
            .children((0..COLUMNS).map(|col| {
                let index = row * COLUMNS + col;
                tile(index, index == *selected.read()).into_element()
            }))
            .into()
    });

    rect()
        .expanded()
        .background(BACKGROUND)
        .padding(48.)
        .spacing(24.)
        .on_global_key_down(on_key)
        .child(
            label()
                .text("Aural · fase 0")
                .font_size(40.)
                .font_weight(FontWeight::BOLD)
                .color((255, 255, 255)),
        )
        .child(
            label()
                .text(format!(
                    "{} · {} · Enter pulsado {} veces",
                    std::env::consts::OS,
                    std::env::consts::ARCH,
                    presses.read()
                ))
                .font_size(18.)
                .color(MUTED),
        )
        .child(
            label()
                .text(status.read().clone())
                .font_size(20.)
                .color(ACCENT),
        )
        .child(
            rect()
                .direction(Direction::Horizontal)
                .spacing(32.)
                .child(rect().spacing(16.).children(rows))
                .child(now_playing(
                    art.read().clone(),
                    *art_number.read(),
                    song.read().clone(),
                    motion_status.read().clone(),
                ))
                .child(
                    rect()
                        .width(Size::fill())
                        .padding(20.)
                        .spacing(6.)
                        .corner_radius(16.)
                        .background(SURFACE)
                        .child(
                            label()
                                .text("Teclas recibidas")
                                .font_size(20.)
                                .color(ACCENT),
                        )
                        .children(log.read().iter().enumerate().map(|(i, entry)| {
                            label()
                                .key(i)
                                .text(entry.clone())
                                .font_size(16.)
                                .color((230, 230, 235))
                                .into()
                        })),
                ),
        )
}

/// One cell of the D-pad grid; the selected one grows and takes the accent border.
fn tile(index: usize, selected: bool) -> impl IntoElement {
    rect()
        .key(index)
        .width(Size::px(120.))
        .height(Size::px(120.))
        .corner_radius(20.)
        .center()
        .background(SURFACE)
        .maybe(selected, |r| {
            r.background(ACCENT)
                .border(Border::new().fill((255, 255, 255)).width(3.))
                .shadow((0., 8., 24., 0., (255, 92, 122, 90)))
        })
        .child(
            label()
                .text(match index {
                    0 => "1 Play".to_string(),
                    1 => "2 Login".to_string(),
                    _ => format!("{}", index + 1),
                })
                .font_size(22.)
                .font_weight(FontWeight::BOLD)
                .color((255, 255, 255)),
        )
}

/// The cover with the song and the motion artwork state under it.
fn now_playing(
    art: Option<cover::Art>,
    number: u64,
    song: Option<(String, String)>,
    motion: String,
) -> impl IntoElement {
    let (title, artist) = song.unwrap_or_default();
    rect()
        .width(Size::px(COVER))
        .spacing(8.)
        .child(cover::view(art, number, COVER))
        .child(
            label()
                .text(title)
                .font_size(22.)
                .font_weight(FontWeight::BOLD)
                .color((255, 255, 255)),
        )
        .child(label().text(artist).font_size(18.).color(MUTED))
        .child(label().text(motion).font_size(14.).color(ACCENT))
}

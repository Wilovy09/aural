//! The app's root: it owns the state station, restores the session, runs the engine, turns
//! keys into navigation and actions, and lays out the shell.

use std::sync::{Arc, RwLock};

use freya::prelude::*;
use freya::radio::{RadioStation, use_init_radio_station, use_radio};
use ytmusic::YtMusic;

use crate::chrome::{
    bottom_bar::BottomBar,
    player_bar::{MiniPlayer, PlayerBar},
    sidebar::Sidebar,
};
use crate::engine::{Command, Engine, Update};
use crate::library::{self, Collection};
use crate::nav::{self, Action, Press};
use crate::screens::{
    account::{Account, SignIn},
    cards::Cards,
    fullscreen::Fullscreen,
    songs::Songs,
};
use crate::session::{self, Session};
use crate::state::{AppState, Auth, Channel, Load, Page, Zone};
use crate::{runtime, ui};

type Station = RadioStation<AppState, Channel>;

/// The client every request goes through; swapped when signing in or out.
static CLIENT: RwLock<Option<Arc<YtMusic>>> = RwLock::new(None);

pub fn client() -> Arc<YtMusic> {
    CLIENT
        .read()
        .ok()
        .and_then(|held| held.clone())
        .unwrap_or_else(|| Session::guest().api)
}

thread_local! {
    /// The station and engine a tap acts on, kept by the root for press handlers anywhere.
    static TAPS: std::cell::RefCell<Option<(Station, Engine)>> = const { std::cell::RefCell::new(None) };
}

/// Does what the D-pad would on `target`: moves the focus there and presses OK.
pub fn tap(target: nav::Target) {
    let Some((mut station, engine)) = TAPS.with_borrow(Clone::clone) else {
        return;
    };
    ui::set_touch(true);
    if !matches!(station.peek().auth, Auth::SignedIn(_)) {
        if matches!(station.peek().auth, Auth::SignedOut { .. }) {
            sign_in(station, engine);
        }
        return;
    }
    // A tap anywhere but the search field takes the keyboard away from it.
    if station.peek().typing && target != nav::Target::Content(crate::state::Spot::Action(0)) {
        crate::screens::search::leave();
    }
    let columns = ui::columns();
    let action = {
        let mut state = station.write_channel(Channel::Navigation);
        nav::tap(&mut state, target, columns)
    };
    perform(station, &engine, action);
}

/// Jumps to `to` in the song playing, from the progress bar.
pub fn seek(to: std::time::Duration) {
    let Some((mut station, engine)) = TAPS.with_borrow(Clone::clone) else {
        return;
    };
    station.write_channel(Channel::Position).position.elapsed = to;
    player(station, &engine, Command::Seek(to));
}

/// Sends `command` to the player that plays: this device's engine, or the device this one
/// controls through Aural Connect.
fn player(station: Station, engine: &Engine, command: Command) {
    if station.peek().connect.remote().is_none() {
        return engine.send(command);
    }
    let remote = match command {
        Command::Play { queue, index } => crate::connect::Remote::Play { queue, index },
        Command::Toggle => crate::connect::Remote::Toggle,
        Command::Next => crate::connect::Remote::Next,
        Command::Previous => crate::connect::Remote::Previous,
        Command::Seek(to) => crate::connect::Remote::Seek {
            millis: to.as_millis() as u64,
        },
        Command::Shuffle(on) => crate::connect::Remote::Shuffle { on },
        Command::Repeat(mode) => crate::connect::Remote::Repeat { mode },
        Command::Client(_) => return engine.send(command),
    };
    crate::connect::command(remote);
}

thread_local! {
    /// Where Connect's events go, for the devices page to start a link with.
    static CONNECT: std::cell::RefCell<Option<crate::connect::Events>> = const { std::cell::RefCell::new(None) };
}

/// Plays on `device` from now on, or on this device again with `None`.
pub fn connect(device: Option<crate::connect::Device>) {
    let Some((mut station, engine)) = TAPS.with_borrow(Clone::clone) else {
        return;
    };
    let Some(events) = CONNECT.with_borrow(Clone::clone) else {
        return;
    };
    let was_remote = station.peek().connect.remote().is_some();
    crate::connect::disconnect();
    match device {
        Some(device) => {
            // This device stops playing: the other one takes over.
            if !was_remote && station.peek().now.playing {
                engine.send(Command::Toggle);
            }
            crate::connect::connect(device, events);
        }
        None => {
            station.write_channel(Channel::Connect).connect.link = crate::connect::Link::Idle;
            if was_remote {
                forget_remote(station);
            }
        }
    }
}

/// Clears what the other device played, so this one's player starts empty again.
fn forget_remote(mut station: Station) {
    station.write_channel(Channel::Now).now = Default::default();
    station.write_channel(Channel::Position).position = Default::default();
}

/// Whether the device being connected to waits for its code.
pub fn link_needs_code() -> bool {
    TAPS.with_borrow(|taps| {
        taps.as_ref().is_some_and(|(station, _)| {
            matches!(station.peek().connect.link, crate::connect::Link::NeedCode(_))
        })
    })
}

/// Sends the code shown on the other device.
pub fn pair(code: &str) {
    crate::connect::pair(code);
}

/// Follows one of Connect's events.
fn follow(mut station: Station, engine: &Engine, event: crate::connect::Event) {
    use crate::connect::{Event, Link};
    match event {
        Event::Devices(devices) => station.write_channel(Channel::Connect).connect.devices = devices,
        Event::Link(link) => {
            let was_remote = station.peek().connect.remote().is_some();
            let lost = was_remote && !matches!(link, Link::Connected(_));
            station.write_channel(Channel::Connect).connect.link = link;
            if lost {
                forget_remote(station);
            }
        }
        Event::Pairing(pairing) => station.write_channel(Channel::Connect).connect.pairing = pairing,
        Event::State(state) => {
            if station.peek().connect.remote().is_none() {
                return;
            }
            let new_song = state.song.as_ref().map(|song| &song.id)
                != station.peek().now.song.as_ref().map(|song| &song.id);
            if new_song && let Some(song) = state.song.clone() {
                crate::sheets::look_up(station, song);
            }
            {
                let mut now = station.write_channel(Channel::Now);
                now.now.song = state.song;
                now.now.playing = state.playing;
                now.now.loading = state.loading;
                now.now.index = state.index;
                now.now.shuffle = state.shuffle;
                now.now.repeat = state.repeat;
            }
            let mut position = station.write_channel(Channel::Position);
            position.position.elapsed = std::time::Duration::from_millis(state.elapsed_millis);
            position.position.total = state.total_millis.map(std::time::Duration::from_millis);
        }
        Event::Queue(queue, index) => {
            if station.peek().connect.remote().is_none() {
                return;
            }
            let mut now = station.write_channel(Channel::Now);
            now.now.queue = queue;
            now.now.index = index;
        }
        // A controller asks this device's player.
        Event::Remote(remote) => match remote {
            crate::connect::Remote::Shuffle { on } => {
                let mut state = station.write_channel(Channel::Now);
                state.now.shuffle = on;
                crate::connect::modes(on, state.now.repeat);
                engine.send(Command::Shuffle(on));
            }
            crate::connect::Remote::Repeat { mode } => {
                let mut state = station.write_channel(Channel::Now);
                state.now.repeat = mode;
                crate::connect::modes(state.now.shuffle, mode);
                engine.send(Command::Repeat(mode));
            }
            other => {
                if let Some(command) = crate::connect::host_command(&other) {
                    engine.send(command);
                }
            }
        },
    }
}

fn set_client(api: Arc<YtMusic>, engine: &Engine) {
    if let Ok(mut held) = CLIENT.write() {
        *held = Some(api.clone());
    }
    engine.send(Command::Client(api));
}

pub fn app() -> impl IntoElement {
    let station = use_init_radio_station::<AppState, Channel>(|| AppState {
        motion: crate::settings::load().motion,
        ..AppState::default()
    });
    let auth = use_radio::<AppState, Channel>(Channel::Auth);
    let navigation = use_radio::<AppState, Channel>(Channel::Navigation);

    let engine = use_hook(move || boot(station));
    use_hook({
        let engine = engine.clone();
        move || TAPS.set(Some((station, engine)))
    });
    // A phone starts without focus rings; the first key press brings them back.
    use_hook(|| ui::set_touch(ui::compact()));
    #[cfg(target_os = "macos")]
    use_hook(crate::dock_icon);

    let on_key = {
        let engine = engine.clone();
        move |event: Event<KeyboardEventData>| {
            let Some(press) = Press::from_key(&event.key) else {
                return;
            };
            // Back is also the phone's back gesture: it keeps a finger's session ringless.
            if ui::touch() && press != Press::Back {
                ui::set_touch(false);
                // Repaint the focus ring the taps had hidden.
                let mut station = station;
                station.write_channel(Channel::Navigation);
            }
            let signed_in = matches!(station.peek().auth, Auth::SignedIn(_));
            if !signed_in {
                if press == Press::Ok && matches!(station.peek().auth, Auth::SignedOut { .. }) {
                    sign_in(station, engine.clone());
                }
                return;
            }
            // The search field has the keyboard: only Back reaches the navigation, to leave it.
            if station.peek().typing {
                if press == Press::Back {
                    crate::screens::search::leave();
                    crate::screens::devices::leave();
                }
                return;
            }
            let columns = ui::columns();
            let action = {
                let mut station = station;
                let mut state = station.write_channel(Channel::Navigation);
                nav::press(&mut state, press, columns)
            };
            perform(station, &engine, action);
        }
    };

    let signed_in = matches!(auth.read().auth, Auth::SignedIn(_));
    let fullscreen = navigation.read().fullscreen;
    let page = navigation.read().page.clone();

    let content = match page {
        Page::Songs | Page::Detail(_) => Songs.into_element(),
        Page::Artist(_) => crate::screens::artist::Artist.into_element(),
        Page::Playlists | Page::Albums => Cards.into_element(),
        Page::Account => Account.into_element(),
        Page::Search => crate::screens::search::Search.into_element(),
        Page::Devices => crate::screens::devices::Devices.into_element(),
    };
    let shell = match ui::compact() {
        // A phone: the page over a mini player and the bottom bar.
        true => {
            let (top, bottom) = ui::safe();
            rect()
                .expanded()
                .content(Content::Flex)
                .padding((top, 0., 0., 0.))
                .child(
                    rect()
                        .width(Size::fill())
                        .height(Size::flex(1.))
                        .child(content),
                )
                .child(MiniPlayer)
                .child(BottomBar)
                .child(
                    rect()
                        .width(Size::fill())
                        .height(Size::px(bottom))
                        .background(ui::color::SECONDARY),
                )
        }
        false => rect()
            .expanded()
            .content(Content::Flex)
            .child(
                rect()
                    .width(Size::fill())
                    .height(Size::flex(1.))
                    .direction(Direction::Horizontal)
                    .content(Content::Flex)
                    .child(Sidebar)
                    .child(
                        rect()
                            .width(Size::flex(1.))
                            .height(Size::fill())
                            .child(content),
                    ),
            )
            .child(PlayerBar),
    };

    rect()
        .expanded()
        .background(ui::color::BACKGROUND)
        .color(ui::color::FOREGROUND)
        .on_global_key_down(on_key)
        .child(match (signed_in, fullscreen) {
            (false, _) => SignIn.into_element(),
            (true, true) => Fullscreen.into_element(),
            (true, false) => shell.into_element(),
        })
        .child(crate::screens::pairing::PairingCode)
}

/// Starts the engine and restores the saved account; returns the engine handle.
fn boot(station: Station) -> Engine {
    let (updates, mut inbox) = tokio::sync::mpsc::unbounded_channel();
    let engine = Engine::start(client(), updates);
    crate::media::listen(engine.clone());

    let (events, mut heard) = tokio::sync::mpsc::unbounded_channel();
    CONNECT.set(Some(events.clone()));
    crate::connect::start(events);
    let connect_engine = engine.clone();
    spawn_forever(async move {
        while let Some(event) = heard.recv().await {
            follow(station, &connect_engine, event);
        }
    });
    // The process before this one handed its music on: carry on where it was.
    if let Some(resume) = crate::media::take_resume() {
        engine.send(Command::Play {
            queue: resume.queue,
            index: resume.index,
        });
        engine.send(Command::Seek(resume.position));
    }

    let mut playback = station;
    spawn_forever(async move {
        while let Some(update) = inbox.recv().await {
            apply(&mut playback, update);
        }
    });

    let engine_for_restore = engine.clone();
    let mut auth = station;
    spawn_forever(async move {
        let restored = runtime::spawn(session::restore()).await.ok();
        match restored {
            Some(Ok(Some(session))) => {
                set_client(session.api.clone(), &engine_for_restore);
                auth.write_channel(Channel::Auth).auth = Auth::SignedIn(session.account.clone());
                load_library(station);
            }
            Some(Ok(None)) | None => {
                auth.write_channel(Channel::Auth).auth = Auth::SignedOut { error: None };
            }
            Some(Err(error)) => {
                auth.write_channel(Channel::Auth).auth = Auth::SignedOut {
                    error: Some(format!("{error:#}")),
                };
            }
        }
    });
    engine
}

/// Applies one engine update to the state.
fn apply(station: &mut Station, update: Update) {
    // While another device plays, the player shows that one's state, not this engine's.
    if station.peek().connect.remote().is_some() {
        return;
    }
    match update {
        Update::Loading(song) => {
            crate::sheets::look_up(*station, song.clone());
            let mut state = station.write_channel(Channel::Now);
            state.now.song = Some(song);
            state.now.loading = true;
            state.now.error = None;
        }
        Update::Playing(playing) => {
            crate::platform::keep_awake(playing);
            let mut state = station.write_channel(Channel::Now);
            state.now.loading = false;
            state.now.playing = playing;
        }
        Update::Position(elapsed, total) => {
            let mut state = station.write_channel(Channel::Position);
            state.position.elapsed = elapsed;
            state.position.total = total;
        }
        Update::Queue(queue, index) => {
            let mut state = station.write_channel(Channel::Now);
            state.now.queue = queue;
            state.now.index = index;
        }
        Update::Stopped => {
            crate::platform::keep_awake(false);
            station.write_channel(Channel::Now).now.playing = false;
        }
        Update::Error(error) => {
            let mut state = station.write_channel(Channel::Now);
            state.now.loading = false;
            state.now.error = Some(error);
        }
    }
}

/// Does what a key press asked beyond moving the focus.
fn perform(station: Station, engine: &Engine, action: Action) {
    match action {
        Action::None => {}
        Action::Play { queue, index } => player(station, engine, Command::Play { queue, index }),
        Action::Shuffle(mut queue) => {
            crate::engine::shuffle(&mut queue);
            player(station, engine, Command::Play { queue, index: 0 });
        }
        Action::Toggle => player(station, engine, Command::Toggle),
        Action::Next => player(station, engine, Command::Next),
        Action::Previous => player(station, engine, Command::Previous),
        Action::Connect(device) => {
            let device = device.and_then(|at| station.peek().connect.devices.get(at).cloned());
            connect(device);
        }
        Action::EditField => crate::screens::devices::edit(),
        Action::Go(_) => {}
        Action::Open(collection) => open(station, collection),
        Action::SeekBy(seconds) => {
            let position = station.peek().position;
            let to = match seconds < 0 {
                true => position
                    .elapsed
                    .saturating_sub(std::time::Duration::from_secs(seconds.unsigned_abs())),
                false => position.elapsed + std::time::Duration::from_secs(seconds as u64),
            };
            seek(match position.total {
                Some(total) => to.min(total),
                None => to,
            });
        }
        Action::SignIn => sign_in(station, engine.clone()),
        Action::SignOut => sign_out(station, engine),
        Action::ToggleShuffle => {
            let mut station = station;
            let (shuffle, repeat) = {
                let mut state = station.write_channel(Channel::Now);
                state.now.shuffle = !state.now.shuffle;
                (state.now.shuffle, state.now.repeat)
            };
            crate::connect::modes(shuffle, repeat);
            player(station, engine, Command::Shuffle(shuffle));
        }
        Action::CycleRepeat => {
            let mut station = station;
            let (shuffle, repeat) = {
                let mut state = station.write_channel(Channel::Now);
                state.now.repeat = state.now.repeat.next();
                (state.now.shuffle, state.now.repeat)
            };
            crate::connect::modes(shuffle, repeat);
            player(station, engine, Command::Repeat(repeat));
        }
        Action::EditSearch => crate::screens::search::edit(),
        Action::ClearSearch => {
            let mut station = station;
            station.write_channel(Channel::Search).search = Default::default();
            station.write_channel(Channel::Navigation).focus.content =
                crate::state::Spot::Action(0);
        }
        Action::ToggleMotion => {
            let mut station = station;
            let mut state = station.write_channel(Channel::Navigation);
            state.motion = !state.motion;
            crate::settings::save(crate::settings::Settings {
                motion: state.motion,
            });
        }
    }
}

/// Loads the library into the state.
fn load_library(mut station: Station) {
    station.write_channel(Channel::Library).library = Load::Loading;
    spawn_forever(async move {
        let api = client();
        let loaded = runtime::spawn(async move { library::load(&api).await }).await;
        station.write_channel(Channel::Library).library = match loaded {
            Ok(Ok(library)) => Load::Ready(library),
            Ok(Err(error)) => Load::Failed(format!("{error:#}")),
            Err(error) => Load::Failed(error.to_string()),
        };
        // The content may have been entered before anything loaded.
        let mut state = station.write_channel(Channel::Navigation);
        if state.focus.zone == Zone::Content {
            state.focus.content = nav::first(&state);
        }
    });
}

/// Shows a playlist or album and loads its tracks.
fn open(mut station: Station, collection: Collection) {
    {
        let mut state = station.write_channel(Channel::Navigation);
        nav::open(&mut state, collection.clone());
    }
    if collection.kind == library::Kind::Artist {
        return open_artist(station, collection);
    }
    station.write_channel(Channel::Detail).detail = Load::Loading;
    spawn_forever(async move {
        let api = client();
        let wanted = collection.clone();
        let loaded = runtime::spawn(async move { library::tracks(&api, &wanted).await }).await;
        let still_open = matches!(&station.peek().page, Page::Detail(open) if *open == collection);
        if !still_open {
            return;
        }
        station.write_channel(Channel::Detail).detail = match loaded {
            Ok(Ok(songs)) => Load::Ready(songs),
            Ok(Err(error)) => Load::Failed(format!("{error:#}")),
            Err(error) => Load::Failed(error.to_string()),
        };
    });
}

/// Loads an artist's page into the state.
fn open_artist(mut station: Station, artist: Collection) {
    station.write_channel(Channel::Detail).artist = Load::Loading;
    spawn_forever(async move {
        let api = client();
        let id = artist.id.clone();
        let loaded = runtime::spawn(async move { library::artist_page(&api, &id).await }).await;
        let still_open = matches!(&station.peek().page, Page::Artist(open) if *open == artist);
        if !still_open {
            return;
        }
        station.write_channel(Channel::Detail).artist = match loaded {
            Ok(Ok(page)) => Load::Ready(page),
            Ok(Err(error)) => Load::Failed(format!("{error:#}")),
            Err(error) => Load::Failed(error.to_string()),
        };
    });
}

/// Opens the sign-in window and, when it brings an account back, loads its library.
fn sign_in(mut station: Station, engine: Engine) {
    station.write_channel(Channel::Auth).auth = Auth::SigningIn("Abriendo Google…".into());
    let (steps, mut heard) = tokio::sync::mpsc::unbounded_channel::<String>();
    let mut progress = station;
    spawn_forever(async move {
        while let Some(step) = heard.recv().await {
            progress.write_channel(Channel::Auth).auth = Auth::SigningIn(step);
        }
    });
    spawn_forever(async move {
        let say = move |step: String| {
            let _ = steps.send(step);
        };
        let result = match cookies(say.clone()).await {
            Ok(header) => runtime::blocking(move || session::finish(&header, &say))
                .await
                .map_err(anyhow::Error::from),
            Err(error) => Ok(Err(error)),
        };
        match result {
            Ok(Ok(session)) => {
                set_client(session.api.clone(), &engine);
                station.write_channel(Channel::Auth).auth = Auth::SignedIn(session.account.clone());
                {
                    let mut state = station.write_channel(Channel::Navigation);
                    nav::go(&mut state, Page::Songs);
                    state.focus.zone = Zone::Sidebar;
                }
                load_library(station);
            }
            Ok(Err(error)) => {
                station.write_channel(Channel::Auth).auth = Auth::SignedOut {
                    error: Some(format!("{error:#}")),
                };
            }
            Err(error) => {
                station.write_channel(Channel::Auth).auth = Auth::SignedOut {
                    error: Some(error.to_string()),
                };
            }
        }
    });
}

/// The `Cookie` header of a fresh sign-in. The desktop window lives on the main thread, so it is
/// driven from here; Android's and the pasted header block, so they go to a worker.
async fn cookies(say: impl Fn(String) + Clone + Send + 'static) -> anyhow::Result<String> {
    #[cfg(not(target_os = "android"))]
    if crate::login::supported() {
        return crate::login::window_sign_in(say).await;
    }
    runtime::blocking(move || session::cookies(&say)).await?
}

fn sign_out(mut station: Station, engine: &Engine) {
    session::sign_out();
    set_client(Session::guest().api, engine);
    station.write_channel(Channel::Library).library = Load::Idle;
    station.write_channel(Channel::Auth).auth = Auth::SignedOut { error: None };
}

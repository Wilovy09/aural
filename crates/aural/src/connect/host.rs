//! The host side: a WebSocket server other Aurals control this one through. A controller
//! says hello with the token it was given when paired; one that was not shows a code on this
//! screen and has to send it back. Every controller hears what plays here.

use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

use anyhow::{Context as _, Result};
use futures_util::{SinkExt as _, StreamExt as _};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::broadcast;
use tokio_tungstenite::tungstenite::Message;

use super::protocol::{Remote, Snapshot, ToController, ToHost};
use super::{Event, Events, Pairing, trust};
use crate::engine::{Repeat, Update};
use crate::library::Song;

/// How long a pairing code holds.
const CODE_FOR: Duration = Duration::from_secs(120);
/// How many wrong codes a controller may send.
const TRIES: usize = 3;
/// How often the position goes out to controllers while a song plays.
const POSITION_EVERY: Duration = Duration::from_millis(500);

/// What plays here, as controllers are told.
struct Playing {
    state: Snapshot,
    queue: Vec<Song>,
    /// When the position last went out.
    sent: Instant,
}

static PLAYING: Mutex<Option<Playing>> = Mutex::new(None);

/// Every message for every controller, already serialised.
fn outbox() -> &'static broadcast::Sender<String> {
    static OUTBOX: OnceLock<broadcast::Sender<String>> = OnceLock::new();
    OUTBOX.get_or_init(|| broadcast::channel(64).0)
}

fn with_playing<R>(job: impl FnOnce(&mut Playing) -> R) -> Option<R> {
    let mut held = PLAYING.lock().ok()?;
    let playing = held.get_or_insert_with(|| Playing {
        state: Snapshot::default(),
        queue: Vec::new(),
        sent: Instant::now(),
    });
    Some(job(playing))
}

fn tell(message: &ToController) {
    if let Ok(text) = serde_json::to_string(message) {
        let _ = outbox().send(text);
    }
}

/// Follows one update of this device's engine, on the engine's thread.
pub fn observe(update: &Update) {
    let message = with_playing(|playing| {
        let state = &mut playing.state;
        match update {
            Update::Loading(song) => {
                state.song = Some(song.clone());
                state.loading = true;
                state.elapsed_millis = 0;
                state.total_millis = song.duration.map(|duration| duration.as_millis() as u64);
            }
            Update::Playing(on) => {
                state.loading = false;
                state.playing = *on;
            }
            Update::Position(elapsed, total) => {
                state.elapsed_millis = elapsed.as_millis() as u64;
                state.total_millis = total
                    .map(|total| total.as_millis() as u64)
                    .or(state.total_millis);
                if playing.sent.elapsed() < POSITION_EVERY {
                    return None;
                }
            }
            Update::Queue(queue, index) => {
                state.index = *index;
                playing.queue = queue.clone();
                return Some(vec![
                    ToController::Queue {
                        queue: queue.clone(),
                        index: *index,
                    },
                    ToController::State {
                        state: state.clone(),
                    },
                ]);
            }
            Update::Stopped => {
                state.playing = false;
                state.loading = false;
            }
            Update::Error(_) => state.loading = false,
        }
        playing.sent = Instant::now();
        Some(vec![ToController::State {
            state: state.clone(),
        }])
    })
    .flatten();
    for message in message.into_iter().flatten() {
        tell(&message);
    }
}

/// Notes this device's shuffle and repeat, which its engine does not report.
pub fn modes(shuffle: bool, repeat: Repeat) {
    let state = with_playing(|playing| {
        playing.state.shuffle = shuffle;
        playing.state.repeat = repeat;
        playing.state.clone()
    });
    if let Some(state) = state {
        tell(&ToController::State { state });
    }
}

/// Listens for controllers on `port` (any free one if it is taken) and returns the port.
pub async fn serve(port: u16, events: Events) -> Result<u16> {
    let listener = match TcpListener::bind(("0.0.0.0", port)).await {
        Ok(listener) => listener,
        Err(_) => TcpListener::bind(("0.0.0.0", 0))
            .await
            .context("cannot listen for controllers")?,
    };
    let port = listener.local_addr()?.port();
    tokio::spawn(async move {
        loop {
            let Ok((stream, from)) = listener.accept().await else {
                continue;
            };
            let events = events.clone();
            tokio::spawn(async move {
                if let Err(error) = welcome(stream, events.clone()).await {
                    log::info!("connect: controller {from} left: {error:#}");
                }
                let _ = events.send(Event::Pairing(None));
            });
        }
    });
    Ok(port)
}

/// One controller, from its hello to its leaving.
async fn welcome(stream: TcpStream, events: Events) -> Result<()> {
    let mut socket = tokio_tungstenite::accept_async(stream)
        .await
        .context("not a websocket")?;
    let send = |message: &ToController| -> Result<Message> {
        Ok(Message::text(serde_json::to_string(message)?))
    };
    let Some(ToHost::Hello { id, name, token }) = next(&mut socket).await? else {
        anyhow::bail!("no hello");
    };

    let known = token.is_some_and(|token| trust::trusted(&id, &token));
    if !known {
        let code = format!("{:04}", super::random() % 10_000);
        let _ = events.send(Event::Pairing(Some(Pairing {
            code: code.clone(),
            from: name.clone(),
        })));
        socket.send(send(&ToController::NeedCode)?).await?;
        let until = Instant::now() + CODE_FOR;
        let mut tries = 0;
        loop {
            let left = until.saturating_duration_since(Instant::now());
            let answer = tokio::time::timeout(left, next(&mut socket))
                .await
                .context("the code expired")??;
            match answer {
                Some(ToHost::Pair { code: sent }) if sent.trim() == code => break,
                Some(ToHost::Pair { .. }) if tries + 1 < TRIES => {
                    tries += 1;
                    socket.send(send(&ToController::NeedCode)?).await?;
                }
                _ => {
                    let reason = "código incorrecto".to_owned();
                    socket
                        .send(send(&ToController::Denied { reason })?)
                        .await?;
                    anyhow::bail!("wrong code");
                }
            }
        }
        let _ = events.send(Event::Pairing(None));
        let token = trust::trust(&id);
        socket.send(send(&ToController::Paired { token })?).await?;
    }

    let me = trust::me();
    socket
        .send(send(&ToController::Welcome {
            id: me.id.clone(),
            name: me.name.clone(),
        })?)
        .await?;
    log::info!("connect: {name} controls this device");
    let mut heard = outbox().subscribe();
    // What plays now, before the updates.
    let (state, queue) = with_playing(|playing| (playing.state.clone(), playing.queue.clone()))
        .unwrap_or_default();
    socket
        .send(send(&ToController::Queue {
            queue,
            index: state.index,
        })?)
        .await?;
    socket.send(send(&ToController::State { state })?).await?;

    loop {
        tokio::select! {
            incoming = next(&mut socket) => match incoming? {
                Some(ToHost::Command { remote }) => {
                    let _ = events.send(Event::Remote(remote));
                }
                Some(_) => {}
                None => return Ok(()),
            },
            outgoing = heard.recv() => match outgoing {
                Ok(text) => socket.send(Message::text(text)).await?,
                Err(broadcast::error::RecvError::Lagged(_)) => {}
                Err(broadcast::error::RecvError::Closed) => return Ok(()),
            },
        }
    }
}

/// The next message from a controller, `None` once it has gone.
async fn next(
    socket: &mut tokio_tungstenite::WebSocketStream<TcpStream>,
) -> Result<Option<ToHost>> {
    loop {
        match socket.next().await {
            None | Some(Ok(Message::Close(_))) => return Ok(None),
            Some(Ok(Message::Text(text))) => return Ok(Some(serde_json::from_str(&text)?)),
            Some(Ok(_)) => continue,
            Some(Err(error)) => return Err(error.into()),
        }
    }
}

/// What a controller's command does here: `Remote` as the engine's command.
pub fn command(remote: &Remote) -> Option<crate::engine::Command> {
    use crate::engine::Command;
    Some(match remote {
        Remote::Play { queue, index } => Command::Play {
            queue: queue.clone(),
            index: *index,
        },
        Remote::Toggle => Command::Toggle,
        Remote::Next => Command::Next,
        Remote::Previous => Command::Previous,
        Remote::Seek { millis } => Command::Seek(Duration::from_millis(*millis)),
        Remote::Shuffle { .. } | Remote::Repeat { .. } => return None,
    })
}

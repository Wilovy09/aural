//! What Aurals say to each other over a WebSocket, one JSON message a frame.

use serde::{Deserialize, Serialize};

use crate::engine::Repeat;
use crate::library::Song;

/// From a controller to the Aural it plays on.
#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ToHost {
    /// Who is asking, with the token it was given when paired, if it was.
    Hello {
        id: String,
        name: String,
        token: Option<String>,
    },
    /// The code shown on the host's screen.
    Pair {
        code: String,
    },
    Command {
        remote: Remote,
    },
}

/// From the host back to a controller.
#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ToController {
    /// The controller is in; state follows.
    Welcome {
        id: String,
        name: String,
    },
    /// The host shows a code; the controller should send it.
    NeedCode,
    /// The code was right: the token to say hello with from now on.
    Paired {
        token: String,
    },
    Denied {
        reason: String,
    },
    State {
        state: Snapshot,
    },
    /// The queue, apart from the state since it is large and changes seldom.
    Queue {
        queue: Vec<Song>,
        index: usize,
    },
}

/// What a controller asks the host's player to do.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "do", rename_all = "snake_case")]
pub enum Remote {
    Play { queue: Vec<Song>, index: usize },
    Toggle,
    Next,
    Previous,
    Seek { millis: u64 },
    Shuffle { on: bool },
    Repeat { mode: Repeat },
}

/// What plays on the host.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Snapshot {
    pub song: Option<Song>,
    pub playing: bool,
    pub loading: bool,
    pub index: usize,
    pub shuffle: bool,
    pub repeat: Repeat,
    pub elapsed_millis: u64,
    pub total_millis: Option<u64>,
}

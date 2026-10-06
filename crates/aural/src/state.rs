//! The app state the UI renders, in one Freya Radio station. Each channel is a part that changes
//! on its own, so a position tick repaints the progress bar and not the library.

use std::time::Duration;

use freya::radio::RadioChannel;

use crate::library::{Collection, Library, Song};
use crate::session::Account;

/// Where the account stands.
#[derive(Clone, Debug, PartialEq)]
pub enum Auth {
    /// Looking for a saved account at launch.
    Checking,
    /// No account; the sign-in screen shows.
    SignedOut {
        error: Option<String>,
    },
    /// The sign-in window is open; the last step it reported.
    SigningIn(String),
    SignedIn(Option<Account>),
}

/// Something loaded from the network.
#[derive(Clone, Debug, PartialEq, Default)]
pub enum Load<T> {
    #[default]
    Idle,
    Loading,
    Ready(T),
    Failed(String),
}

impl<T> Load<T> {
    pub fn ready(&self) -> Option<&T> {
        match self {
            Load::Ready(value) => Some(value),
            _ => None,
        }
    }
}

/// The screen the content area shows.
#[derive(Clone, Debug, PartialEq)]
pub enum Page {
    /// The account's home feed.
    Home,
    Search,
    Songs,
    Playlists,
    Albums,
    Detail(Collection),
    Artist(Collection),
    Account,
    /// The other Aurals to play on.
    Devices,
}

/// The sidebar's entries, top to bottom.
pub const NAV: [Page; 6] = [
    Page::Home,
    Page::Search,
    Page::Songs,
    Page::Playlists,
    Page::Albums,
    Page::Account,
];

/// The part of the screen the D-pad is in.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Zone {
    Sidebar,
    Content,
    Player,
}

/// What inside the content area the D-pad is on.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Spot {
    /// A button of the page header (Play, Shuffle, Sign in…).
    Action(usize),
    /// A track row.
    Row(usize),
    /// A card of a grid.
    Card(usize),
    /// A search filter chip.
    Chip(usize),
    /// The search's best match.
    Best,
    /// A search result: group (0 songs, 1 artists, 2 albums, 3 playlists) and index.
    Cell(usize, usize),
}

/// Where the D-pad is on every part, so moving back to a part lands where it was.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Focus {
    pub zone: Zone,
    pub sidebar: usize,
    pub content: Spot,
    pub player: usize,
    /// In the fullscreen player: the view tabs (row 0), the transport (row 1) or the
    /// progress (row 2), where left and right seek.
    pub full_row: usize,
    /// The view tab the D-pad is on.
    pub full_tab: usize,
    /// The transport button the D-pad is on: shuffle, previous, play, next, repeat, clear,
    /// night.
    pub full_button: usize,
}

impl Default for Focus {
    fn default() -> Self {
        Self {
            zone: Zone::Sidebar,
            sidebar: 0,
            content: Spot::Action(0),
            player: 1,
            full_row: 1,
            full_tab: 0,
            full_button: 2,
        }
    }
}

/// How many top songs an artist's page lists.
pub const ARTIST_TOP: usize = 5;

/// The player bar's buttons, left to right.
pub const PLAYER_BUTTONS: usize = 6;
/// The player bar's like button: drawn beside the song, on the left of the transport.
pub const PLAYER_LIKE: usize = 5;
/// The fullscreen transport's buttons, left to right: shuffle, previous, play, next, repeat,
/// clear the screen, night mode.
pub const TRANSPORT_BUTTONS: usize = 9;

/// How much of the fullscreen player shows.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Mode {
    /// Everything.
    #[default]
    Normal,
    /// The cover and the lyrics or queue, without tabs, progress or buttons.
    Clear,
    /// Black, with only the song's name in the bottom-left corner.
    Night,
}

/// What the fullscreen player shows beside the controls.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum View {
    /// The cover alone, centred.
    #[default]
    Music,
    /// The cover beside the lyrics.
    Lyrics,
    /// The cover beside what plays next.
    Queue,
}

/// What is playing.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Now {
    pub song: Option<Song>,
    pub loading: bool,
    pub playing: bool,
    pub error: Option<String>,
    pub shuffle: bool,
    pub repeat: crate::engine::Repeat,
    /// The queue in play order and the current song's place in it.
    pub queue: Vec<Song>,
    pub index: usize,
    /// The light the song's cover casts, tinting the shell.
    pub light: Option<freya::prelude::Color>,
}

/// The lyrics of the song that is playing.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Sheet {
    /// The song the sheet is for.
    pub song: Option<String>,
    pub lyrics: Load<lyrics::Lyrics>,
    /// Which provider it came from.
    pub source: Option<&'static str>,
    /// Who wrote the song, when the provider says.
    pub writers: Vec<String>,
    /// The lines in the listener's language, while translating is on.
    pub translation: Load<Option<lyrics::Translation>>,
}

/// How far into the song, kept apart because it ticks four times a second.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Position {
    pub elapsed: Duration,
    pub total: Option<Duration>,
}

/// A search and what it found.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Search {
    pub query: String,
    pub results: Load<crate::library::Results>,
    pub filter: Filter,
}

/// Which kind of result the search shows, the chips above the results.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Filter {
    #[default]
    All,
    Songs,
    Artists,
    Albums,
    Playlists,
}

impl Filter {
    pub const ALL: [Filter; 5] = [
        Filter::All,
        Filter::Songs,
        Filter::Artists,
        Filter::Albums,
        Filter::Playlists,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Filter::All => "Todo",
            Filter::Songs => "Canciones",
            Filter::Artists => "Artistas",
            Filter::Albums => "Álbumes",
            Filter::Playlists => "Playlists",
        }
    }

    /// The result group a filter shows alone: songs, artists, albums, playlists.
    pub fn group(self) -> Option<usize> {
        match self {
            Filter::All => None,
            Filter::Songs => Some(0),
            Filter::Artists => Some(1),
            Filter::Albums => Some(2),
            Filter::Playlists => Some(3),
        }
    }
}

#[derive(Default)]
pub struct AppState {
    pub auth: Auth,
    pub library: Load<Library>,
    /// The home feed's shelves.
    pub home: Load<Vec<crate::library::HomeShelf>>,
    pub detail: Load<Vec<Song>>,
    pub artist: Load<crate::library::ArtistPage>,
    pub page: Page,
    /// Pages to return to with Back.
    pub history: Vec<Page>,
    pub focus: Focus,
    pub fullscreen: bool,
    pub view: View,
    pub mode: Mode,
    /// Whether the player shows albums' animated covers (opt-in).
    pub motion: bool,
    /// The text and interface size settings.
    pub text: crate::settings::Scale,
    pub interface: crate::settings::Scale,
    /// Whether the lyrics show their translation under each line.
    pub translate: bool,
    /// Sign out was pressed once and waits for a second OK.
    pub confirm_sign_out: bool,
    pub now: Now,
    pub position: Position,
    pub sheet: Sheet,
    pub search: Search,
    /// The search field has the keyboard: keys go to it, not to the D-pad navigation.
    pub typing: bool,
    pub connect: Connect,
    /// A short notice over the player, Spotify's "Added to queue".
    pub toast: Option<Toast>,
}

/// A notice that shows for a moment and goes.
#[derive(Clone, Debug, PartialEq)]
pub struct Toast {
    pub text: String,
    /// Whether it offers to open the queue.
    pub open_queue: bool,
    /// Tells one notice from the next, so an old timer does not hide a new one.
    pub id: u64,
}

/// Aural Connect: the devices around, this device's link to one, a code it shows.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Connect {
    pub devices: Vec<crate::connect::Device>,
    pub link: crate::connect::Link,
    pub pairing: Option<crate::connect::Pairing>,
}

impl Connect {
    /// The device this one plays on, when it plays on another.
    pub fn remote(&self) -> Option<&str> {
        match &self.link {
            crate::connect::Link::Connected(name) => Some(name),
            _ => None,
        }
    }
}

impl Default for Auth {
    fn default() -> Self {
        Auth::Checking
    }
}

impl Default for Page {
    fn default() -> Self {
        Page::Home
    }
}

impl AppState {
    /// Whether the account likes the song `id`.
    pub fn liked(&self, id: &str) -> bool {
        self.library
            .ready()
            .is_some_and(|library| library.liked.iter().any(|song| song.id == id))
    }

    /// The songs of the page on screen, when it lists songs.
    pub fn songs(&self) -> Option<&[Song]> {
        match &self.page {
            Page::Songs => self.library.ready().map(|library| library.liked.as_slice()),
            Page::Detail(_) => self.detail.ready().map(Vec::as_slice),
            Page::Artist(_) => self
                .artist
                .ready()
                .map(|page| &page.top[..page.top.len().min(ARTIST_TOP)]),
            _ => None,
        }
    }

    /// The cards of the page on screen, when it is a grid.
    pub fn cards(&self) -> Option<&[Collection]> {
        let library = self.library.ready()?;
        match &self.page {
            Page::Playlists => Some(&library.playlists),
            Page::Albums => Some(&library.albums),
            _ => None,
        }
    }

    /// How many results each search group holds: songs, artists, albums and playlists.
    pub fn search_groups(&self) -> [usize; 4] {
        match self.search.results.ready() {
            Some(found) => [
                found.songs.len(),
                found.artists.len(),
                found.albums.len(),
                found.playlists.len(),
            ],
            None => [0; 4],
        }
    }

    /// How many header buttons the page has.
    pub fn actions(&self) -> usize {
        match &self.page {
            Page::Songs | Page::Detail(_) => 2,
            Page::Account => 3,
            Page::Artist(_) => 2,
            Page::Search => 2,
            Page::Devices => 1,
            Page::Home => 0,
            Page::Playlists | Page::Albums => 0,
        }
    }
}

#[derive(PartialEq, Eq, Clone, Copy, Debug, Hash, PartialOrd, Ord)]
pub enum Channel {
    Auth,
    Library,
    Detail,
    /// The page, history, focus and fullscreen: everything a key press moves.
    Navigation,
    Now,
    Position,
    Lyrics,
    Search,
    Connect,
    Toast,
}

impl RadioChannel<AppState> for Channel {}

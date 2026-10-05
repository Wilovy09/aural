//! What a remote key does: moving the focus between the sidebar, the content and the player
//! bar, and turning OK into an [`Action`]. Pure state changes, so the TV's D-pad, a keyboard's
//! arrows and the tests all go through the same rules.

use freya::prelude::{Key, NamedKey};

use crate::library::{Collection, Song};
use crate::state::{
    AppState, Filter, Mode, NAV, PLAYER_BUTTONS, Page, Spot, TRANSPORT_BUTTONS, View, Zone,
};

/// What a key press asks the app to do beyond moving the focus.
#[derive(Debug, PartialEq)]
pub enum Action {
    None,
    Play {
        queue: Vec<Song>,
        index: usize,
    },
    Shuffle(Vec<Song>),
    Toggle,
    Next,
    Previous,
    Go(Page),
    Open(Collection),
    SignIn,
    SignOut,
    ToggleMotion,
    ToggleShuffle,
    CycleRepeat,
    /// Give the search field the keyboard.
    EditSearch,
    /// Empty the search field and its results.
    ClearSearch,
}

/// The arrow keys, OK and Back, whatever the device calls them.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Press {
    Up,
    Down,
    Left,
    Right,
    Ok,
    Back,
}

impl Press {
    pub fn from_key(key: &Key) -> Option<Self> {
        Some(match key {
            Key::Named(NamedKey::ArrowUp) => Press::Up,
            Key::Named(NamedKey::ArrowDown) => Press::Down,
            Key::Named(NamedKey::ArrowLeft) => Press::Left,
            Key::Named(NamedKey::ArrowRight) => Press::Right,
            Key::Named(NamedKey::Enter) => Press::Ok,
            Key::Named(NamedKey::BrowserBack | NamedKey::Escape | NamedKey::GoBack) => Press::Back,
            _ => return None,
        })
    }
}

/// Moves the focus for `press` and returns what else should happen. `columns` is how many
/// cards a grid row holds right now.
pub fn press(state: &mut AppState, press: Press, columns: usize) -> Action {
    // Sign out asks twice; any other key between the two presses cancels it.
    let confirming = std::mem::take(&mut state.confirm_sign_out);
    if confirming
        && press == Press::Ok
        && state.focus.zone == Zone::Content
        && state.page == Page::Account
        && state.focus.content == Spot::Action(0)
    {
        return Action::SignOut;
    }
    if state.fullscreen {
        return fullscreen(state, press);
    }
    if press == Press::Back {
        return back(state);
    }
    match state.focus.zone {
        Zone::Sidebar => sidebar(state, press),
        Zone::Content => content(state, press, columns.max(1)),
        Zone::Player => player(state, press),
    }
}

/// Opens `page` from the sidebar, forgetting the way back.
pub fn go(state: &mut AppState, page: Page) {
    state.history.clear();
    state.page = page;
    state.focus.content = first(state);
}

/// Opens a playlist or album, remembering the page to return to.
pub fn open(state: &mut AppState, collection: Collection) {
    let page = match collection.kind {
        crate::library::Kind::Artist => Page::Artist(collection),
        _ => Page::Detail(collection),
    };
    let from = std::mem::replace(&mut state.page, page);
    state.history.push(from);
    state.focus.zone = Zone::Content;
    state.focus.content = first(state);
}

fn back(state: &mut AppState) -> Action {
    if let Some(page) = state.history.pop() {
        state.page = page;
        state.focus.content = first(state);
    } else if state.focus.zone != Zone::Sidebar {
        state.focus.zone = Zone::Sidebar;
    }
    Action::None
}

fn fullscreen(state: &mut AppState, press: Press) -> Action {
    // With the controls hidden, any key brings them back and does nothing else.
    if state.mode != Mode::Normal {
        state.mode = Mode::Normal;
        return Action::None;
    }
    let focus = &mut state.focus;
    match (focus.full_row, press) {
        (_, Press::Back) => state.fullscreen = false,
        // The view tabs: Música, Letra, Cola.
        (0, Press::Left) => focus.full_tab = focus.full_tab.saturating_sub(1),
        (0, Press::Right) => focus.full_tab = (focus.full_tab + 1).min(2),
        (0, Press::Down) => focus.full_row = 1,
        (0, Press::Ok) => {
            state.view = match focus.full_tab {
                0 => View::Music,
                1 => View::Lyrics,
                _ => View::Queue,
            }
        }
        (0, Press::Up) => {}
        // The transport: shuffle, previous, play, next, repeat.
        (_, Press::Left) => focus.full_button = focus.full_button.saturating_sub(1),
        (_, Press::Right) => focus.full_button = (focus.full_button + 1).min(TRANSPORT_BUTTONS - 1),
        (_, Press::Up) => {
            focus.full_row = 0;
            focus.full_tab = match state.view {
                View::Music => 0,
                View::Lyrics => 1,
                View::Queue => 2,
            };
        }
        (_, Press::Down) => state.fullscreen = false,
        (_, Press::Ok) => {
            return match focus.full_button {
                0 => Action::ToggleShuffle,
                1 => Action::Previous,
                2 => Action::Toggle,
                3 => Action::Next,
                4 => Action::CycleRepeat,
                5 => {
                    state.mode = Mode::Clear;
                    Action::None
                }
                _ => {
                    state.mode = Mode::Night;
                    Action::None
                }
            };
        }
    }
    Action::None
}

fn sidebar(state: &mut AppState, press: Press) -> Action {
    let at = &mut state.focus.sidebar;
    match press {
        Press::Up => *at = at.saturating_sub(1),
        Press::Down if *at + 1 < NAV.len() => *at += 1,
        Press::Down => state.focus.zone = Zone::Player,
        Press::Right => enter_content(state),
        Press::Ok => {
            let page = NAV[*at].clone();
            go(state, page.clone());
            state.focus.zone = Zone::Content;
            return Action::Go(page);
        }
        Press::Left | Press::Back => {}
    }
    Action::None
}

fn content(state: &mut AppState, press: Press, columns: usize) -> Action {
    if state.page == Page::Search {
        return search(state, press);
    }
    if matches!(state.page, Page::Artist(_)) {
        return artist(state, press);
    }
    let actions = state.actions();
    let rows = state.songs().map_or(0, <[Song]>::len);
    let cards = state.cards().map_or(0, <[Collection]>::len);
    let spot = state.focus.content;
    let next = match (spot, press) {
        (Spot::Action(i), Press::Left) if i > 0 => Spot::Action(i - 1),
        (Spot::Action(i), Press::Right) if i + 1 < actions => Spot::Action(i + 1),
        (Spot::Action(_), Press::Down) if rows > 0 => Spot::Row(0),
        (Spot::Action(_), Press::Down) if cards > 0 => Spot::Card(0),

        (Spot::Row(r), Press::Up) if r > 0 => Spot::Row(r - 1),
        (Spot::Row(_), Press::Up) if actions > 0 => Spot::Action(0),
        (Spot::Row(r), Press::Down) if r + 1 < rows => Spot::Row(r + 1),

        (Spot::Card(c), Press::Left) if c % columns > 0 => Spot::Card(c - 1),
        (Spot::Card(c), Press::Right) if c + 1 < cards && (c + 1) % columns != 0 => {
            Spot::Card(c + 1)
        }
        (Spot::Card(c), Press::Up) if c >= columns => Spot::Card(c - columns),
        (Spot::Card(_), Press::Up) if actions > 0 => Spot::Action(0),
        (Spot::Card(c), Press::Down) if c + columns < cards => Spot::Card(c + columns),
        (Spot::Card(c), Press::Down) if c / columns + 1 < cards.div_ceil(columns) => {
            Spot::Card(cards - 1)
        }

        (_, Press::Left) => {
            state.focus.zone = Zone::Sidebar;
            return Action::None;
        }
        (_, Press::Down) => {
            state.focus.zone = Zone::Player;
            return Action::None;
        }
        (Spot::Action(0), Press::Ok)
            if state.page == Page::Account
                && matches!(state.auth, crate::state::Auth::SignedIn(_)) =>
        {
            state.confirm_sign_out = true;
            return Action::None;
        }
        (_, Press::Ok) => return content_action(state, spot),
        _ => spot,
    };
    state.focus.content = next;
    Action::None
}

fn content_action(state: &AppState, spot: Spot) -> Action {
    match (spot, &state.page) {
        (Spot::Row(index), _) => match state.songs() {
            Some(songs) => Action::Play {
                queue: songs.to_vec(),
                index,
            },
            None => Action::None,
        },
        (Spot::Best | Spot::Cell(..) | Spot::Chip(_), _) => search_action(state, spot),
        (Spot::Card(index), _) => match state.cards().and_then(|cards| cards.get(index)) {
            Some(card) => Action::Open(card.clone()),
            None => Action::None,
        },
        (Spot::Action(1), Page::Account) => Action::ToggleMotion,
        (Spot::Action(_), Page::Account) => match state.auth {
            crate::state::Auth::SignedIn(_) => Action::SignOut,
            _ => Action::SignIn,
        },
        (Spot::Action(button), _) => match state.songs() {
            Some(songs) if !songs.is_empty() && button == 0 => Action::Play {
                queue: songs.to_vec(),
                index: 0,
            },
            Some(songs) if !songs.is_empty() => Action::Shuffle(songs.to_vec()),
            _ => Action::None,
        },
    }
}

fn player(state: &mut AppState, press: Press) -> Action {
    let at = &mut state.focus.player;
    match press {
        Press::Left => *at = at.saturating_sub(1),
        Press::Right => *at = (*at + 1).min(PLAYER_BUTTONS - 1),
        Press::Up => enter_content(state),
        Press::Ok if *at == 3 => {
            if state.now.song.is_some() {
                state.fullscreen = true;
                state.focus.full_row = 1;
                state.focus.full_button = 2;
            }
        }
        Press::Ok => return player_action(*at),
        Press::Down | Press::Back => {}
    }
    Action::None
}

fn player_action(button: usize) -> Action {
    match button {
        0 => Action::Previous,
        1 => Action::Toggle,
        _ => Action::Next,
    }
}

/// Moves into the content area, onto the spot it held or the page's first one.
fn enter_content(state: &mut AppState) {
    state.focus.zone = Zone::Content;
    if !valid(state, state.focus.content) {
        state.focus.content = first(state);
    }
}

/// The first spot of the page on screen.
pub fn first(state: &AppState) -> Spot {
    if state.actions() > 0 {
        Spot::Action(0)
    } else if state.cards().is_some() {
        Spot::Card(0)
    } else {
        Spot::Row(0)
    }
}

fn valid(state: &AppState, spot: Spot) -> bool {
    match spot {
        Spot::Action(i) => i < state.actions(),
        Spot::Row(r) => state.songs().is_some_and(|songs| r < songs.len()),
        Spot::Card(c) => state.cards().is_some_and(|cards| c < cards.len()),
        Spot::Chip(at) => at < Filter::ALL.len(),
        Spot::Best => state
            .search
            .results
            .ready()
            .is_some_and(|found| found.best.is_some()),
        Spot::Cell(group, at) => state
            .search_groups()
            .get(group)
            .is_some_and(|length| at < *length),
    }
}

/// How many songs the "Todo" view lists beside the best match.
pub const TOP_SONGS: usize = 4;

/// The search page. Above everything, the field and its clear button, then the filter chips.
/// "Todo" shows the best match beside the top songs, then a shelf each of artists, albums
/// and playlists; a filter shows one group alone, songs as a list and the rest as a grid.
fn search(state: &mut AppState, press: Press) -> Action {
    let lengths = state.search_groups();
    let filter = state.search.filter;
    let has_results = lengths.iter().any(|length| *length > 0);
    let best = state
        .search
        .results
        .ready()
        .is_some_and(|found| found.best.is_some());
    let chip = Filter::ALL.iter().position(|f| *f == filter).unwrap_or(0);
    let columns = crate::ui::columns();

    // "Todo": the rows of the page from the top, and the first spot of each.
    let top = match (best, lengths[0] > 0) {
        (true, _) => Some(Spot::Best),
        (false, true) => Some(Spot::Cell(0, 0)),
        (false, false) => None,
    };
    let shelves: Vec<usize> = (1..4).filter(|group| lengths[*group] > 0).collect();
    let first_content = match filter.group() {
        None => top.or(shelves.first().map(|group| Spot::Cell(*group, 0))),
        Some(group) if lengths[group] > 0 => Some(Spot::Cell(group, 0)),
        Some(_) => None,
    };

    let spot = state.focus.content;
    let next = match (spot, press) {
        // The field and its clear button.
        (Spot::Action(0), Press::Right) if !state.search.query.is_empty() => Spot::Action(1),
        (Spot::Action(1), Press::Left) => Spot::Action(0),
        (Spot::Action(_), Press::Down) if has_results => Spot::Chip(chip),
        (Spot::Action(0), Press::Ok) => return Action::EditSearch,
        (Spot::Action(_), Press::Ok) => return Action::ClearSearch,

        // The filter chips: OK shows that group.
        (Spot::Chip(at), Press::Left) if at > 0 => Spot::Chip(at - 1),
        (Spot::Chip(at), Press::Right) if at + 1 < Filter::ALL.len() => Spot::Chip(at + 1),
        (Spot::Chip(_), Press::Up) => Spot::Action(0),
        (Spot::Chip(_), Press::Down) => first_content.unwrap_or(spot),
        (Spot::Chip(at), Press::Ok) => {
            state.search.filter = Filter::ALL[at];
            return Action::None;
        }

        (Spot::Best | Spot::Cell(..), Press::Ok) => return search_action(state, spot),

        // "Todo": the best match and the top songs beside it.
        (Spot::Best, Press::Right) if lengths[0] > 0 => Spot::Cell(0, 0),
        (Spot::Best, Press::Up) => Spot::Chip(chip),
        (Spot::Best, Press::Down) => match shelves.first() {
            Some(group) => Spot::Cell(*group, 0),
            None => spot,
        },
        (Spot::Cell(0, row), Press::Up) if filter == Filter::All && row > 0 => {
            Spot::Cell(0, row - 1)
        }
        (Spot::Cell(0, _), Press::Up) if filter == Filter::All => Spot::Chip(chip),
        (Spot::Cell(0, row), Press::Down)
            if filter == Filter::All && row + 1 < lengths[0].min(TOP_SONGS) =>
        {
            Spot::Cell(0, row + 1)
        }
        (Spot::Cell(0, _), Press::Down) if filter == Filter::All => match shelves.first() {
            Some(group) => Spot::Cell(*group, 0),
            None => spot,
        },
        (Spot::Cell(0, _), Press::Left) if filter == Filter::All && best => Spot::Best,

        // "Todo": the shelves, left and right along one, up and down between them.
        (Spot::Cell(group, at), Press::Left) if filter == Filter::All && group > 0 && at > 0 => {
            Spot::Cell(group, at - 1)
        }
        (Spot::Cell(group, at), Press::Right)
            if filter == Filter::All && group > 0 && at + 1 < lengths[group] =>
        {
            Spot::Cell(group, at + 1)
        }
        (Spot::Cell(group, at), Press::Up) if filter == Filter::All && group > 0 => {
            let place = shelves
                .iter()
                .position(|shelf| *shelf == group)
                .unwrap_or(0);
            match place {
                0 => top.unwrap_or(Spot::Chip(chip)),
                _ => {
                    let above = shelves[place - 1];
                    Spot::Cell(above, at.min(lengths[above] - 1))
                }
            }
        }
        (Spot::Cell(group, at), Press::Down) if filter == Filter::All && group > 0 => {
            let place = shelves
                .iter()
                .position(|shelf| *shelf == group)
                .unwrap_or(0);
            match shelves.get(place + 1) {
                Some(below) => Spot::Cell(*below, at.min(lengths[*below] - 1)),
                None => {
                    state.focus.zone = Zone::Player;
                    return Action::None;
                }
            }
        }

        // A filter on songs: one list.
        (Spot::Cell(0, row), Press::Up) if row > 0 => Spot::Cell(0, row - 1),
        (Spot::Cell(0, _), Press::Up) => Spot::Chip(chip),
        (Spot::Cell(0, row), Press::Down) if row + 1 < lengths[0] => Spot::Cell(0, row + 1),

        // A filter on artists, albums or playlists: a grid.
        (Spot::Cell(group, at), Press::Left) if group > 0 && at % columns > 0 => {
            Spot::Cell(group, at - 1)
        }
        (Spot::Cell(group, at), Press::Right)
            if group > 0 && at + 1 < lengths[group] && (at + 1) % columns != 0 =>
        {
            Spot::Cell(group, at + 1)
        }
        (Spot::Cell(group, at), Press::Up) if group > 0 && at >= columns => {
            Spot::Cell(group, at - columns)
        }
        (Spot::Cell(group, _), Press::Up) if group > 0 => Spot::Chip(chip),
        (Spot::Cell(group, at), Press::Down) if group > 0 && at + columns < lengths[group] => {
            Spot::Cell(group, at + columns)
        }

        (_, Press::Left) => {
            state.focus.zone = Zone::Sidebar;
            return Action::None;
        }
        (_, Press::Down) => {
            state.focus.zone = Zone::Player;
            return Action::None;
        }
        _ => spot,
    };
    state.focus.content = next;
    Action::None
}

/// What OK does on a search result: play a song with the songs found as the queue, open an
/// artist, album or playlist.
fn search_action(state: &AppState, spot: Spot) -> Action {
    let Some(found) = state.search.results.ready() else {
        return Action::None;
    };
    let open = |group: &[crate::library::Collection], at: usize| match group.get(at) {
        Some(found) => Action::Open(found.clone()),
        None => Action::None,
    };
    match spot {
        Spot::Best => match &found.best {
            Some(crate::library::Best::Artist(artist)) => Action::Open(artist.clone()),
            Some(crate::library::Best::Song(song)) => Action::Play {
                queue: found.songs.clone(),
                index: found
                    .songs
                    .iter()
                    .position(|found| found.id == song.id)
                    .unwrap_or(0),
            },
            None => Action::None,
        },
        Spot::Cell(0, row) => Action::Play {
            queue: found.songs.clone(),
            index: row,
        },
        Spot::Cell(1, at) => open(&found.artists, at),
        Spot::Cell(2, at) => open(&found.albums, at),
        Spot::Cell(_, at) => open(&found.playlists, at),
        _ => Action::None,
    }
}

/// An artist's page: Play and Shuffle, the top songs, then a shelf per row of the page.
fn artist(state: &mut AppState, press: Press) -> Action {
    let rows = state.songs().map_or(0, <[Song]>::len);
    let lengths: Vec<usize> = state
        .artist
        .ready()
        .map(|page| page.shelves.iter().map(|shelf| shelf.items.len()).collect())
        .unwrap_or_default();
    let spot = state.focus.content;
    let next = match (spot, press) {
        (Spot::Action(0), Press::Right) => Spot::Action(1),
        (Spot::Action(1), Press::Left) => Spot::Action(0),
        (Spot::Action(_), Press::Down) if rows > 0 => Spot::Row(0),
        (Spot::Action(_), Press::Down) if !lengths.is_empty() => Spot::Cell(0, 0),

        (Spot::Row(row), Press::Up) if row > 0 => Spot::Row(row - 1),
        (Spot::Row(_), Press::Up) => Spot::Action(0),
        (Spot::Row(row), Press::Down) if row + 1 < rows => Spot::Row(row + 1),
        (Spot::Row(_), Press::Down) if !lengths.is_empty() => Spot::Cell(0, 0),

        (Spot::Cell(shelf, at), Press::Left) if at > 0 => Spot::Cell(shelf, at - 1),
        (Spot::Cell(shelf, at), Press::Right) if at + 1 < lengths[shelf] => {
            Spot::Cell(shelf, at + 1)
        }
        (Spot::Cell(shelf, at), Press::Up) if shelf > 0 => {
            Spot::Cell(shelf - 1, at.min(lengths[shelf - 1] - 1))
        }
        (Spot::Cell(_, _), Press::Up) if rows > 0 => Spot::Row(rows - 1),
        (Spot::Cell(_, _), Press::Up) => Spot::Action(0),
        (Spot::Cell(shelf, at), Press::Down) if shelf + 1 < lengths.len() => {
            Spot::Cell(shelf + 1, at.min(lengths[shelf + 1] - 1))
        }
        (Spot::Cell(shelf, at), Press::Ok) => {
            return match state
                .artist
                .ready()
                .and_then(|page| page.shelves.get(shelf))
                .and_then(|shelf| shelf.items.get(at))
            {
                Some(item) => Action::Open(item.clone()),
                None => Action::None,
            };
        }
        (_, Press::Ok) => return content_action(state, spot),

        (_, Press::Left) => {
            state.focus.zone = Zone::Sidebar;
            return Action::None;
        }
        (_, Press::Down) => {
            state.focus.zone = Zone::Player;
            return Action::None;
        }
        _ => spot,
    };
    state.focus.content = next;
    Action::None
}

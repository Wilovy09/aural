//! The search page, mixing Spotify's and YouTube Music's: filter chips under the field; under
//! "Todo" the best match as a large card beside the top songs, then a shelf of large covers
//! each for artists, albums and playlists; under a filter, that group alone, songs as a list
//! and the rest as a grid. On a TV, OK on the field opens Android's keyboard; Enter searches
//! and Back leaves the field.

use std::cell::Cell;

use freya::prelude::*;
use freya::radio::{RadioStation, use_radio, use_radio_station};

use crate::library::{self, Best, Collection, Kind, Song};
use crate::nav::TOP_SONGS;
use crate::runtime;
use crate::screens::cards;
use crate::state::{AppState, Channel, Filter, Load, Spot, Zone};
use crate::ui::{self, Cover, Icon, color, metrics, text};

/// Height of a song row.
const ROW: f32 = 60.;
/// Side of a song row's cover.
const THUMB: f32 = 44.;
/// Side of the best match's picture.
const BEST: f32 = 120.;
/// Height of a section title.
const TITLE: f32 = 44.;
/// The field and the chips above the results, with their gaps.
const HEAD: f32 = metrics::INSET + 48. + 18. + 40. + 28.;
/// The best match and the top songs beside it.
const TOP: f32 = TITLE + TOP_SONGS as f32 * ROW;
/// One shelf: its title and a row of covers.
const SHELF: f32 = TITLE + cards::ROW + 12.;

thread_local! {
    /// The field's accessibility id, so a key press can hand it the keyboard.
    static FIELD: Cell<Option<AccessibilityId>> = const { Cell::new(None) };
}

/// Gives the search field the keyboard (Android shows its own on a TV).
pub fn edit() {
    if let Some(field) = FIELD.get() {
        field.request_focus();
    }
}

/// Takes the keyboard away from the search field.
pub fn leave() {
    if let Some(field) = FIELD.get() {
        field.request_unfocus();
    }
}

#[derive(PartialEq)]
pub struct Search;

impl Component for Search {
    fn render(&self) -> impl IntoElement {
        let mut station = use_radio_station::<AppState, Channel>();
        let navigation = use_radio::<AppState, Channel>(Channel::Navigation);
        let search = use_radio::<AppState, Channel>(Channel::Search);
        let now = use_radio::<AppState, Channel>(Channel::Now);

        let field = use_a11y();
        FIELD.set(Some(field));
        let focus = use_focus(field);
        // Typing follows the field's focus, however it got it (OK on a TV, a click, a tap).
        use_side_effect(move || {
            let typing = focus().is_focused();
            if station.peek().typing != typing {
                station.write_channel(Channel::Navigation).typing = typing;
            }
        });
        // The field shows the stored query, and empties when it is cleared.
        let mut query = use_state(|| station.peek().search.query.clone());
        let stored = use_reactive(&search.read().search.query.clone());
        use_side_effect(move || {
            let stored = stored.read().clone();
            if stored.is_empty() && !query.peek().is_empty() {
                query.set(stored);
            }
        });

        let state = navigation.read();
        let spot = match state.focus.zone == Zone::Content {
            true => Some(state.focus.content),
            false => None,
        };
        drop(state);
        let found = search.read().search.results.clone();
        let filter = search.read().search.filter;
        let playing = now.read().now.song.as_ref().map(|song| song.id.clone());
        let columns = ui::columns();

        let shelves: Vec<usize> = match found.ready() {
            Some(found) => [
                (1, found.artists.len()),
                (2, found.albums.len()),
                (3, found.playlists.len()),
            ]
            .into_iter()
            .filter(|(_, length)| *length > 0)
            .map(|(group, _)| group)
            .collect(),
            None => Vec::new(),
        };
        let target = match (spot, filter) {
            (Some(Spot::Cell(group, _)), Filter::All) if group > 0 => {
                let place = shelves
                    .iter()
                    .position(|shelf| *shelf == group)
                    .unwrap_or(0);
                HEAD + TOP + 24. + place as f32 * SHELF
            }
            (Some(Spot::Cell(0, row)), Filter::Songs) => HEAD + row as f32 * ROW,
            (Some(Spot::Cell(_, at)), _) if filter != Filter::All => {
                HEAD + (at / columns) as f32 * cards::ROW
            }
            _ => 0.,
        };
        let (_, height) = ui::viewport();
        let view = height - metrics::PLAYER_BAR;
        let scroll = ui::use_follow((target - view / 4.).max(0.));

        let status = match &found {
            Load::Idle => Some("Escribe y pulsa Enter para buscar".to_string()),
            Load::Loading => Some("Buscando…".to_string()),
            Load::Failed(error) => Some(error.clone()),
            Load::Ready(found)
                if found.songs.is_empty()
                    && found.artists.is_empty()
                    && found.albums.is_empty()
                    && found.playlists.is_empty() =>
            {
                Some("No se encontró nada".to_string())
            }
            Load::Ready(_) => None,
        };

        let input = Input::new(query)
            .a11y_id(field)
            .placeholder("Canciones, artistas, álbumes, playlists…")
            .width(Size::fill())
            .flat()
            .theme_colors(InputColorsThemePartial {
                background: Some(Preference::Specific(Color::TRANSPARENT)),
                focus_background: Some(Preference::Specific(Color::TRANSPARENT)),
                color: Some(Preference::Specific(color::FOREGROUND)),
                placeholder_color: Some(Preference::Specific(color::MUTED_FOREGROUND)),
                border_fill: Some(Preference::Specific(Color::TRANSPARENT)),
                focus_border_fill: Some(Preference::Specific(Color::TRANSPARENT)),
            })
            .on_submit(move |submitted: String| {
                leave();
                run(station, submitted);
            })
            // Android's keyboards send Enter as a "\n" character rather than the Enter key the
            // input submits on: catch it here instead of typing a newline.
            .on_pre_key_down(move |event: Event<KeyboardEventData>| match &event.key {
                Key::Character(typed) if typed == "\n" || typed == "\r" => {
                    event.stop_propagation();
                    event.prevent_default();
                    leave();
                    run(station, query.peek().clone());
                    false
                }
                Key::Named(NamedKey::Enter | NamedKey::Escape | NamedKey::Shift) => true,
                Key::Named(NamedKey::Tab) => false,
                _ => {
                    event.stop_propagation();
                    event.prevent_default();
                    true
                }
            });

        let field_border = match (focus().is_focused(), spot == Some(Spot::Action(0))) {
            (true, _) => Border::new().fill(color::FOCUS_ON_PRIMARY).width(2.),
            (false, ring) => ui::focus_border(ring),
        };
        let has_query = !query.read().is_empty();
        let results = found.ready().cloned();

        ScrollView::new_controlled(scroll)
            .width(Size::fill())
            .height(Size::fill())
            .child(
                rect()
                    .width(Size::fill())
                    .padding(metrics::INSET)
                    .spacing(18.)
                    .child(
                        rect()
                            .width(Size::fill())
                            .height(Size::px(48.))
                            .padding((0., 8., 0., 14.))
                            .direction(Direction::Horizontal)
                            .content(Content::Flex)
                            .cross_align(Alignment::Center)
                            .spacing(10.)
                            .corner_radius(24.)
                            .background(color::SECONDARY)
                            .border(field_border)
                            .child(ui::icon(Icon::Search, 18., color::MUTED_FOREGROUND))
                            .child(rect().width(Size::flex(1.)).child(input))
                            .maybe(has_query, |row| {
                                row.child(
                                    rect()
                                        .width(Size::px(32.))
                                        .height(Size::px(32.))
                                        .center()
                                        .corner_radius(16.)
                                        .border(ui::focus_border(spot == Some(Spot::Action(1))))
                                        .child(ui::icon(Icon::Close, 18., color::FOREGROUND)),
                                )
                            }),
                    )
                    .maybe(results.is_some(), |page| page.child(chips(filter, spot)))
                    .map(status, |page, status| {
                        page.child(ui::line(status, text::BODY, color::MUTED_FOREGROUND))
                    })
                    .map(results, |page, found| {
                        let view = View {
                            found: &found,
                            spot,
                            playing: playing.as_deref(),
                            columns,
                        };
                        match filter {
                            Filter::All => page.children(view.everything(&shelves)),
                            Filter::Songs => page.child(view.songs_list()),
                            other => page.child(view.grid(other.group().unwrap_or(1))),
                        }
                    }),
            )
    }
}

/// The filter chips: the chosen one filled white, the D-pad's one ringed.
fn chips(filter: Filter, spot: Option<Spot>) -> impl IntoElement {
    rect()
        .direction(Direction::Horizontal)
        .spacing(10.)
        .children(Filter::ALL.iter().enumerate().map(|(at, chip)| {
            let chosen = *chip == filter;
            rect()
                .key(at)
                .height(Size::px(40.))
                .padding((0., 18.))
                .center()
                .corner_radius(20.)
                .background(match chosen {
                    true => color::PRIMARY,
                    false => color::MUTED,
                })
                .border(match spot == Some(Spot::Chip(at)) {
                    true => Border::new().fill(color::FOCUS_ON_PRIMARY).width(3.),
                    false => Border::new().fill(Color::TRANSPARENT).width(3.),
                })
                .child(
                    ui::line(
                        chip.name(),
                        text::BODY,
                        match chosen {
                            true => color::PRIMARY_FOREGROUND,
                            false => color::FOREGROUND,
                        },
                    )
                    .font_weight(FontWeight::SEMI_BOLD),
                )
                .into()
        }))
}

/// What the results are drawn from.
struct View<'a> {
    found: &'a library::Results,
    spot: Option<Spot>,
    playing: Option<&'a str>,
    columns: usize,
}

impl View<'_> {
    /// "Todo": the best match beside the top songs, then the shelves.
    fn everything(&self, shelves: &[usize]) -> Vec<Element> {
        let found = self.found;
        let mut parts = Vec::new();
        let songs: Vec<Element> = found
            .songs
            .iter()
            .take(TOP_SONGS)
            .enumerate()
            .map(|(row, song)| self.song(row, song, false).into_element())
            .collect();
        parts.push(
            rect()
                .width(Size::fill())
                .direction(Direction::Horizontal)
                .content(Content::Flex)
                .spacing(28.)
                .map(found.best.clone(), |top, best| {
                    top.child(
                        rect()
                            .width(Size::flex(0.42))
                            .child(section("Mejor resultado"))
                            .child(best_card(&best, self.spot == Some(Spot::Best))),
                    )
                })
                .maybe(!songs.is_empty(), |top| {
                    top.child(
                        rect()
                            .width(Size::flex(0.58))
                            .child(section("Canciones"))
                            .children(songs),
                    )
                })
                .into(),
        );
        for group in shelves {
            let (title, items) = self.group(*group);
            parts.push(self.shelf(title, *group, items).into_element());
        }
        parts
    }

    /// The title and the results of an artists, albums or playlists group.
    fn group(&self, group: usize) -> (&'static str, &[Collection]) {
        match group {
            1 => ("Artistas", &self.found.artists),
            2 => ("Álbumes", &self.found.albums),
            _ => ("Playlists", &self.found.playlists),
        }
    }

    /// A row of large covers. It slides along with the D-pad, so the focused one is in view.
    fn shelf(&self, title: &str, group: usize, items: &[Collection]) -> impl IntoElement {
        let focused = match self.spot {
            Some(Spot::Cell(at_group, at)) if at_group == group => Some(at),
            _ => None,
        };
        let start = focused.map_or(0, |at| (at + 1).saturating_sub(self.columns));
        rect()
            .width(Size::fill())
            .margin((24., 0., 0., 0.))
            .child(section(title))
            .child(
                rect()
                    .direction(Direction::Horizontal)
                    .spacing(metrics::CARD_GAP)
                    .children(
                        items
                            .iter()
                            .enumerate()
                            .skip(start)
                            .take(self.columns)
                            .map(|(at, item)| tile(at, item, focused == Some(at)).into_element()),
                    ),
            )
    }

    /// A filter on songs: every song, YouTube Music's row with its kind and album.
    fn songs_list(&self) -> impl IntoElement {
        rect().width(Size::fill()).children(
            self.found
                .songs
                .iter()
                .enumerate()
                .map(|(row, song)| self.song(row, song, true).into_element()),
        )
    }

    /// A filter on artists, albums or playlists: a grid of large covers.
    fn grid(&self, group: usize) -> impl IntoElement {
        let (_, items) = self.group(group);
        let focused = match self.spot {
            Some(Spot::Cell(at_group, at)) if at_group == group => Some(at),
            _ => None,
        };
        rect().width(Size::fill()).children(
            items
                .chunks(self.columns)
                .enumerate()
                .map(|(row, chunk)| {
                    rect()
                        .key(row)
                        .height(Size::px(cards::ROW))
                        .direction(Direction::Horizontal)
                        .spacing(metrics::CARD_GAP)
                        .children(chunk.iter().enumerate().map(|(column, item)| {
                            let at = row * self.columns + column;
                            tile(at, item, focused == Some(at)).into_element()
                        }))
                        .into()
                })
                .collect::<Vec<Element>>(),
        )
    }

    /// A song row: cover, title, a subtitle and the duration. `detailed` adds YouTube Music's
    /// "Canción • artist • album".
    fn song(&self, row: usize, song: &Song, detailed: bool) -> impl IntoElement {
        let focused = self.spot == Some(Spot::Cell(0, row));
        let playing = self.playing == Some(song.id.as_str());
        let subtitle = match (detailed, &song.album) {
            (true, Some(album)) => format!("Canción • {} • {album}", song.artist),
            (true, None) => format!("Canción • {}", song.artist),
            (false, _) => song.artist.clone(),
        };
        rect()
            .key(row)
            .width(Size::fill())
            .height(Size::px(ROW))
            .padding((0., 10.))
            .direction(Direction::Horizontal)
            .content(Content::Flex)
            .cross_align(Alignment::Center)
            .spacing(14.)
            .corner_radius(metrics::RADIUS)
            .background(match focused {
                true => color::FOCUS_FILL,
                false => Color::TRANSPARENT,
            })
            .border(ui::focus_border(focused))
            .child(Cover::new(song.cover.clone(), THUMB, 4.))
            .child(
                rect()
                    .width(Size::flex(1.))
                    .spacing(3.)
                    .child(
                        ui::line(
                            song.title.clone(),
                            text::LARGE - 2.,
                            match playing {
                                true => color::PRIMARY,
                                false => color::FOREGROUND,
                            },
                        )
                        .width(Size::fill()),
                    )
                    .child(
                        ui::line(subtitle, text::LABEL, color::MUTED_FOREGROUND)
                            .width(Size::fill()),
                    ),
            )
            .child(ui::line(
                song.duration.map(library::clock).unwrap_or_default(),
                text::LABEL,
                color::MUTED_FOREGROUND,
            ))
    }
}

/// A section title, Spotify's large bold heading.
fn section(title: &str) -> impl IntoElement {
    rect()
        .height(Size::px(TITLE))
        .main_align(Alignment::Center)
        .child(ui::line(title, text::TITLE, color::FOREGROUND).font_weight(FontWeight::BOLD))
}

/// The best match: Spotify's large card, a big picture over a big name and its kind.
fn best_card(best: &Best, focused: bool) -> impl IntoElement {
    let (cover, round, kind, title, detail) = match best {
        Best::Artist(artist) => (
            artist.cover.clone(),
            true,
            "Artista",
            artist.title.clone(),
            artist.subtitle.clone(),
        ),
        Best::Song(song) => (
            song.cover.clone(),
            false,
            "Canción",
            song.title.clone(),
            song.artist.clone(),
        ),
    };
    rect()
        .width(Size::fill())
        .height(Size::px(TOP_SONGS as f32 * ROW))
        .padding(20.)
        .spacing(12.)
        .main_align(Alignment::Center)
        .corner_radius(metrics::RADIUS + 4.)
        .background(color::SECONDARY)
        .border(ui::focus_border(focused))
        .child(Cover::new(
            cover,
            BEST,
            match round {
                true => BEST / 2.,
                false => metrics::RADIUS,
            },
        ))
        .child(
            ui::line(title, text::DISPLAY + 6., color::FOREGROUND)
                .font_weight(FontWeight::BOLD)
                .width(Size::fill()),
        )
        .child(ui::line(
            match detail.is_empty() {
                true => kind.to_string(),
                false => format!("{kind} • {detail}"),
            },
            text::BODY,
            color::MUTED_FOREGROUND,
        ))
}

/// A large cover for a shelf or grid: an artist's round, an album's or playlist's square.
pub(crate) fn tile(at: usize, item: &Collection, focused: bool) -> impl IntoElement {
    let side = metrics::CARD - 6.;
    let round = item.kind == Kind::Artist;
    let subtitle = match item.kind {
        Kind::Artist => "Artista".to_string(),
        // An artist's page labels its own (Sencillo • 2026); a search names the artist.
        Kind::Album if item.subtitle.contains(" • ") => item.subtitle.clone(),
        Kind::Album => format!("Álbum • {}", item.subtitle),
        Kind::Playlist => format!("Playlist • {}", item.subtitle),
    };
    rect()
        .key(at)
        .width(Size::px(metrics::CARD))
        .spacing(8.)
        .child(
            rect()
                .corner_radius(match round {
                    true => side / 2. + 3.,
                    false => metrics::RADIUS + 3.,
                })
                .padding(3.)
                .margin(-3.)
                .border(ui::focus_border(focused))
                .child(Cover::new(
                    item.cover.clone(),
                    side,
                    match round {
                        true => side / 2.,
                        false => metrics::RADIUS,
                    },
                )),
        )
        .child(
            ui::line(item.title.clone(), text::BODY, color::FOREGROUND)
                .font_weight(FontWeight::SEMI_BOLD)
                .width(Size::fill()),
        )
        .child(ui::line(subtitle, text::SMALL, color::MUTED_FOREGROUND).width(Size::fill()))
}

/// Searches for `query` and puts what it finds in the state.
pub fn run(mut station: RadioStation<AppState, Channel>, query: String) {
    let query = query.trim().to_owned();
    if query.is_empty() {
        return;
    }
    {
        let mut state = station.write_channel(Channel::Search);
        state.search.query = query.clone();
        state.search.results = Load::Loading;
        state.search.filter = Filter::All;
    }
    spawn(async move {
        let api = crate::app::client();
        let wanted = query.clone();
        let found = runtime::spawn(async move { library::search(&api, &wanted).await }).await;
        if station.peek().search.query != query {
            return;
        }
        let found = match found {
            Ok(Ok(found)) => found,
            Ok(Err(error)) => {
                station.write_channel(Channel::Search).search.results =
                    Load::Failed(format!("{error:#}"));
                return;
            }
            Err(error) => {
                station.write_channel(Channel::Search).search.results =
                    Load::Failed(error.to_string());
                return;
            }
        };
        let landing = match (found.best.is_some(), found.songs.is_empty()) {
            (true, _) => Spot::Best,
            (false, false) => Spot::Cell(0, 0),
            (false, true) => Spot::Chip(0),
        };
        station.write_channel(Channel::Search).search.results = Load::Ready(found);
        // Land on the best match, ready for OK.
        let mut state = station.write_channel(Channel::Navigation);
        state.focus.zone = Zone::Content;
        state.focus.content = landing;
    });
}

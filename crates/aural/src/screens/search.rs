//! The search page, mixing Spotify's and YouTube Music's: filter chips under the field; under
//! "Todo" the best match as a large card beside the top songs, then a shelf of large covers
//! each for artists, albums and playlists; under a filter, that group alone, songs as a list
//! and the rest as a grid. On a TV, OK on the field opens Android's keyboard; Enter searches
//! and Back leaves the field.

use std::cell::Cell;

use freya::prelude::*;
use freya::radio::{RadioStation, use_radio, use_radio_station};

use crate::library::{self, Best, Collection, Kind, Song};
use crate::nav::{TOP_SONGS, Target};
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
fn shelf_height() -> f32 {
    TITLE + cards::row() + 12.
}

thread_local! {
    /// The field's accessibility id, so a key press can hand it the keyboard.
    static FIELD: Cell<Option<AccessibilityId>> = const { Cell::new(None) };
    /// The field was asked for the keyboard before it was on screen.
    static PENDING: Cell<bool> = const { Cell::new(false) };
}

/// Gives the search field the keyboard (Android shows its own on a TV).
pub fn edit() {
    // A computer searches from the bar on top.
    if crate::chrome::top_bar::edit() {
        return;
    }
    PENDING.set(true);
    if let Some(field) = FIELD.get() {
        field.request_focus();
    }
}

/// Takes the keyboard away from the search field.
pub fn leave() {
    crate::chrome::top_bar::leave();
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
        // Opened to type (⌘K): the field takes the keyboard once it is laid out.
        use_hook(move || {
            if PENDING.replace(false) {
                spawn(async move { field.request_focus() });
            }
        });
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
        let spot = match state.focus.zone == Zone::Content && !ui::touch() {
            true => Some(state.focus.content),
            false => None,
        };
        drop(state);
        let found = search.read().search.results.clone();
        let filter = search.read().search.filter;
        let playing = now.read().now.song.as_ref().map(|song| song.id.clone());
        let light = now.read().now.light.unwrap_or(color::FOCUS_ON_PRIMARY);
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
                HEAD + TOP + 24. + place as f32 * shelf_height()
            }
            (Some(Spot::Cell(0, row)), Filter::Songs) => HEAD + row as f32 * ROW,
            (Some(Spot::Cell(_, at)), _) if filter != Filter::All => {
                HEAD + (at / columns) as f32 * cards::row()
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
                    let typed = query.peek().clone();
                    run(station, typed);
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
        let compact = ui::compact();

        // A finger scrolls through `TouchScroll`, which leaves sideways swipes to the rows.
        ui::touch_scroll::TouchScroll {
            scroll,
            content: ScrollView::new_controlled(scroll)
                .drag_scrolling(false)
                .width(Size::fill())
                .height(Size::fill())
                .child(
                    rect()
                        .width(Size::fill())
                        .padding(ui::inset())
                        .spacing(18.)
                        // A computer types in the bar on top; here only on a TV or a phone.
                        .maybe(!crate::chrome::top_bar::shown(), |page| {
                            page.child(
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
                                    .on_press(ui::tap(Target::Content(Spot::Action(0))))
                                    .child(ui::icon(Icon::Search, 18., color::MUTED_FOREGROUND))
                                    .child(rect().width(Size::flex(1.)).child(input))
                                    .maybe(has_query, |row| {
                                        row.child(
                                            rect()
                                                .width(Size::px(32.))
                                                .height(Size::px(32.))
                                                .center()
                                                .corner_radius(16.)
                                                .border(ui::focus_border(
                                                    spot == Some(Spot::Action(1)),
                                                ))
                                                .on_press(ui::tap(Target::Content(Spot::Action(1))))
                                                .child(ui::icon(
                                                    Icon::Close,
                                                    18.,
                                                    color::FOREGROUND,
                                                )),
                                        )
                                    }),
                            )
                        })
                        .maybe(results.is_some(), |page| {
                            page.child(chips(filter, spot, compact))
                        })
                        .map(status, |page, status| {
                            page.child(ui::line(status, text::BODY, color::MUTED_FOREGROUND))
                        })
                        .map(results, |page, found| {
                            let view = View {
                                found: &found,
                                spot,
                                playing: playing.as_deref(),
                                columns,
                                compact,
                                light,
                            };
                            match filter {
                                Filter::All => page.children(view.everything(&shelves)),
                                Filter::Songs => page.child(view.songs_list()),
                                other => page.child(view.grid(other.group().unwrap_or(1))),
                            }
                        }),
                )
                .into_element(),
        }
    }
}

/// The filter chips: the chosen one filled white, the D-pad's one ringed.
fn chips(filter: Filter, spot: Option<Spot>, compact: bool) -> impl IntoElement {
    let (height, pad, size) = match compact {
        true => (34., 14., text::LABEL),
        false => (40., 18., text::BODY),
    };
    rect()
        .width(Size::fill())
        .direction(Direction::Horizontal)
        // A phone has no room for every chip on one line: they wrap.
        .content(Content::Wrap {
            wrap_spacing: Some(8.),
        })
        .spacing(8.)
        .children(Filter::ALL.iter().enumerate().map(|(at, chip)| {
            let chosen = *chip == filter;
            rect()
                .key(at)
                .height(Size::px(height))
                .padding((0., pad))
                .center()
                .corner_radius(height / 2.)
                .background(match chosen {
                    true => color::PRIMARY,
                    false => color::MUTED,
                })
                .border(match spot == Some(Spot::Chip(at)) {
                    true => Border::new().fill(color::FOCUS_ON_PRIMARY).width(3.),
                    false => Border::new().fill(Color::TRANSPARENT).width(3.),
                })
                .on_press(ui::tap(Target::Content(Spot::Chip(at))))
                .child(
                    label()
                        .text(chip.name())
                        .font_size(size)
                        .font_weight(FontWeight::SEMI_BOLD)
                        .max_lines(1)
                        .color(match chosen {
                            true => color::PRIMARY_FOREGROUND,
                            false => color::FOREGROUND,
                        }),
                )
                .into_element()
        }))
}

/// What the results are drawn from.
struct View<'a> {
    found: &'a library::Results,
    spot: Option<Spot>,
    playing: Option<&'a str>,
    columns: usize,
    compact: bool,
    /// The light of the song playing, the swipe's colour.
    light: Color,
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
        // A phone stacks the best match over the songs.
        if self.compact {
            if let Some(best) = found.best.clone() {
                parts.push(
                    rect()
                        .width(Size::fill())
                        .child(section("Mejor resultado"))
                        .child(best_card(&best, self.spot == Some(Spot::Best), true))
                        .into(),
                );
            }
            if !songs.is_empty() {
                parts.push(
                    rect()
                        .width(Size::fill())
                        .child(section("Canciones"))
                        .children(songs)
                        .into(),
                );
            }
            for group in shelves {
                let (title, items) = self.group(*group);
                parts.push(self.shelf(title, *group, items).into_element());
            }
            return parts;
        }
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
                            .child(best_card(&best, self.spot == Some(Spot::Best), false)),
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
        let shelf = rect()
            .width(Size::fill())
            .margin((24., 0., 0., 0.))
            .child(section(title));
        let tiles = |skip: usize, take: usize| {
            rect()
                .direction(Direction::Horizontal)
                .spacing(ui::gap())
                .children(
                    items
                        .iter()
                        .enumerate()
                        .skip(skip)
                        .take(take)
                        .map(|(at, item)| {
                            tile(
                                at,
                                item,
                                focused == Some(at),
                                Target::Content(Spot::Cell(group, at)),
                            )
                            .into_element()
                        }),
                )
        };
        // A phone scrolls the whole shelf sideways under a finger.
        if self.compact {
            return shelf.child(
                ScrollView::new()
                    .direction(Direction::Horizontal)
                    .show_scrollbar(false)
                    .width(Size::fill())
                    .height(Size::px(cards::row()))
                    .child(tiles(0, items.len())),
            );
        }
        let start = focused.map_or(0, |at| (at + 1).saturating_sub(self.columns));
        shelf.child(tiles(start, self.columns))
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
                        .height(Size::px(cards::row()))
                        .direction(Direction::Horizontal)
                        .spacing(ui::gap())
                        .children(chunk.iter().enumerate().map(|(column, item)| {
                            let at = row * self.columns + column;
                            tile(
                                at,
                                item,
                                focused == Some(at),
                                Target::Content(Spot::Cell(group, at)),
                            )
                            .into_element()
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
        let detailed = detailed && !self.compact;
        let subtitle = match (detailed, &song.album) {
            (true, Some(album)) => format!("Canción • {} • {album}", song.artist),
            (true, None) => format!("Canción • {}", song.artist),
            (false, _) => song.artist.clone(),
        };
        let line = rect()
            .width(Size::fill())
            .height(Size::px(ROW))
            .padding((0., 10.))
            .direction(Direction::Horizontal)
            .content(Content::Flex)
            .cross_align(Alignment::Center)
            .spacing(14.)
            .corner_radius(metrics::RADIUS)
            .background(match ui::ring(focused) {
                true => color::FOCUS_FILL,
                false => Color::TRANSPARENT,
            })
            .on_press(ui::tap(Target::Content(Spot::Cell(0, row))))
            .child(
                Cover::new(song.cover.clone(), THUMB, 4.)
                    .apple(crate::artwork::Wanted::song(&song)),
            )
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
            ));
        // Swiped to the left, the song joins the queue.
        let queued = song.clone();
        ui::swipe::Swipe {
            content: line.into_element(),
            glyph: Icon::Queue,
            label: "Agregar a la cola",
            tint: self.light,
            surface: color::BACKGROUND,
            height: ROW,
            on_swipe: EventHandler::new(move |_| crate::app::enqueue(queued.clone())),
            key: DiffKey::default(),
        }
        .key(row)
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
fn best_card(best: &Best, focused: bool, compact: bool) -> impl IntoElement {
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
    let picture = match compact {
        true => 88.,
        false => BEST,
    };
    rect()
        .width(Size::fill())
        .map(
            (!compact).then_some(TOP_SONGS as f32 * ROW),
            |card, height| card.height(Size::px(height)),
        )
        .padding(match compact {
            true => 16.,
            false => 20.,
        })
        .spacing(12.)
        .main_align(Alignment::Center)
        .corner_radius(metrics::RADIUS_LG)
        .background(match ui::ring(focused) {
            true => color::RAISED,
            false => color::SECONDARY,
        })
        .on_press(ui::tap(Target::Content(Spot::Best)))
        .child(Cover {
            apple: match best {
                Best::Song(song) => Some(crate::artwork::Wanted::song(song)),
                Best::Artist(_) => None,
            },
            ..Cover::new(
                cover,
                picture,
                match round {
                    true => picture / 2.,
                    false => metrics::RADIUS,
                },
            )
        })
        .child(
            ui::line(
                title,
                match compact {
                    true => text::DISPLAY,
                    false => text::DISPLAY + 6.,
                },
                color::FOREGROUND,
            )
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
pub(crate) fn tile(
    at: usize,
    item: &Collection,
    focused: bool,
    target: Target,
) -> impl IntoElement {
    let width = ui::card();
    let side = width - 6.;
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
        .width(Size::px(width))
        .spacing(8.)
        .on_press(ui::tap(target))
        .child(
            rect()
                .corner_radius(match round {
                    true => side / 2. + 3.,
                    false => metrics::RADIUS + 3.,
                })
                .padding(3.)
                .margin(-3.)
                .border(ui::focus_border(focused))
                .child(Cover {
                    apple: crate::artwork::Wanted::collection(item),
                    ..Cover::new(
                        item.cover.clone(),
                        side,
                        match round {
                            true => side / 2.,
                            false => metrics::RADIUS,
                        },
                    )
                }),
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
    spawn_forever(async move {
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

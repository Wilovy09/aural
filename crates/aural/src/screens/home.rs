//! Home, as YouTube Music's: a greeting, the moods that narrow the feed, then every shelf of
//! the account's feed, each laid out as YouTube Music lays it out: a row of covers, a row of
//! video stills, or a grid of songs. A shelf's header carries its strapline and picture, and
//! "Reproducir todo" when the whole shelf can play. Rows slide with the D-pad or scroll under a
//! finger.

use freya::prelude::*;
use freya::radio::use_radio;

use crate::library::{HomeShelf, Kind, Look, Pick, Song};
use crate::nav::Target;
use crate::screens::cards;
use crate::state::{AppState, Auth, Channel, Load, Spot, Zone};
use crate::ui::{self, Cover, Icon, color, metrics, text};

/// Height of a song in a grid shelf.
const QUICK_HEIGHT: f32 = 64.;
/// Height of a shelf's title.
const TITLE: f32 = 52.;
/// Height of the strapline over a title.
const STRAP: f32 = 30.;
/// How much wider than a cover a video still is.
const WIDE: f32 = 1.6;
/// Height of the greeting and of the moods, over the shelves.
const GREETING: f32 = 76.;
const MOODS: f32 = 56.;

#[derive(PartialEq)]
pub struct Home;

impl Component for Home {
    fn render(&self) -> impl IntoElement {
        let navigation = use_radio::<AppState, Channel>(Channel::Navigation);
        let library = use_radio::<AppState, Channel>(Channel::Library);
        let auth = use_radio::<AppState, Channel>(Channel::Auth);

        let state = navigation.read();
        let spot = match state.focus.zone == Zone::Content && !ui::touch() {
            true => Some(state.focus.content),
            false => None,
        };
        drop(state);
        let name = match &auth.read().auth {
            Auth::SignedIn(Some(account)) => Some(account.name.clone()),
            _ => None,
        };
        let (load, moods, mood) = {
            let state = library.read();
            (state.home.clone(), state.moods.clone(), state.mood)
        };
        let now = use_radio::<AppState, Channel>(Channel::Now);
        let light = now.read().now.light.unwrap_or(color::FOCUS_ON_PRIMARY);
        let compact = ui::compact();
        let columns = ui::columns();

        // Keep the focused shelf in view, a third of the way down.
        let target = match (spot, &load) {
            (Some(Spot::Cell(shelf, _)), Load::Ready(shelves)) => {
                let above: f32 = shelves.iter().take(shelf).map(shelf_height).sum();
                GREETING + MOODS + above
            }
            _ => 0.,
        };
        let (_, height) = ui::viewport();
        let scroll = ui::use_follow((target - height / 4.).max(0.));

        let body = match load {
            Load::Ready(shelves) if !shelves.is_empty() => rect()
                .width(Size::fill())
                .spacing(8.)
                .children(shelves.iter().enumerate().map(|(at, shelf)| {
                    shelf_view(at, shelf, spot, columns, compact, light).into_element()
                }))
                .into_element(),
            Load::Ready(_) => note("Tu inicio está vacío por ahora").into_element(),
            Load::Failed(error) => note(error).into_element(),
            Load::Loading | Load::Idle => note("Cargando tu inicio…").into_element(),
        };

        ScrollView::new_controlled(scroll)
            // A phone scrolls under the finger, with no bar to drag.
            .show_scrollbar(!ui::compact())
            .width(Size::fill())
            .height(Size::fill())
            .child(
                rect()
                    .width(Size::fill())
                    .padding((ui::inset() / 2., ui::inset(), ui::inset(), ui::inset()))
                    .spacing(20.)
                    .child(
                        label()
                            .text(greeting(name.as_deref()))
                            .font_size(match compact {
                                true => text::TITLE,
                                false => text::DISPLAY + 4.,
                            })
                            .font_weight(FontWeight::BOLD)
                            .color(color::FOREGROUND)
                            .width(Size::fill()),
                    )
                    .maybe(!moods.is_empty(), |page| {
                        page.child(chips(&moods, mood, spot, compact))
                    })
                    .child(body),
            )
    }
}

fn note(message: impl Into<String>) -> impl IntoElement {
    ui::line(message, text::BODY, color::MUTED_FOREGROUND)
}

fn quick_columns(compact: bool) -> usize {
    match compact {
        true => 2,
        false => 3,
    }
}

/// How tall a shelf stands, header included, to keep the focused one in view.
fn shelf_height(shelf: &HomeShelf) -> f32 {
    let header = TITLE
        + match shelf.strapline.is_some() {
            true => STRAP,
            false => 0.,
        };
    let body = match shelf.look {
        Look::Cards => cards::row(),
        Look::Videos => video_row(),
        Look::List => {
            let lines = shelf.picks.len().div_ceil(quick_columns(ui::compact()));
            lines as f32 * (QUICK_HEIGHT + 10.)
        }
    };
    header + body + 8.
}

/// The width of a video's still, and the height of a row of them.
fn video_width() -> f32 {
    ui::card() * WIDE
}

fn video_row() -> f32 {
    (video_width() - 6.) * 9. / 16. + 66.
}

/// The moods that narrow the feed, as YouTube Music's chips: the one chosen lit.
fn chips(
    moods: &[crate::library::Mood],
    chosen: Option<usize>,
    spot: Option<Spot>,
    compact: bool,
) -> impl IntoElement {
    let (height, pad, size) = match compact {
        true => (34., 14., text::LABEL),
        false => (38., 16., text::BODY),
    };
    ScrollView::new()
        .direction(Direction::Horizontal)
        .show_scrollbar(false)
        .width(Size::fill())
        .height(Size::px(height + 6.))
        .child(
            rect()
                .direction(Direction::Horizontal)
                .spacing(8.)
                .padding((3., 0.))
                .children(moods.iter().enumerate().map(|(at, mood)| {
                    let on = chosen == Some(at);
                    rect()
                        .key(at)
                        .height(Size::px(height))
                        .padding((0., pad))
                        .center()
                        .corner_radius(8.)
                        .background(match on {
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
                                .text(mood.title.clone())
                                .font_size(size)
                                .font_weight(FontWeight::MEDIUM)
                                .max_lines(1)
                                .color(match on {
                                    true => color::PRIMARY_FOREGROUND,
                                    false => color::FOREGROUND,
                                }),
                        )
                        .into_element()
                })),
        )
}

/// A shelf: its header, then its picks as the shelf's look lays them out.
fn shelf_view(
    at: usize,
    shelf: &HomeShelf,
    spot: Option<Spot>,
    columns: usize,
    compact: bool,
    light: Color,
) -> impl IntoElement {
    let focused = match spot {
        Some(Spot::Cell(row, item)) if row == at => Some(item),
        _ => None,
    };
    let picks = match shelf.look {
        Look::List => grid(at, shelf, focused, compact, light).into_element(),
        Look::Cards | Look::Videos => {
            let wide = shelf.look == Look::Videos;
            let shown = match wide {
                true => ((columns as f32 / WIDE).floor() as usize).max(1),
                false => columns,
            };
            let tiles = |skip: usize, take: usize| {
                rect()
                    .direction(Direction::Horizontal)
                    .spacing(ui::gap())
                    .children(shelf.picks.iter().enumerate().skip(skip).take(take).map(
                        |(index, pick)| {
                            tile(at, index, pick, wide, focused == Some(index)).into_element()
                        },
                    ))
            };
            match compact {
                true => ScrollView::new()
                    .direction(Direction::Horizontal)
                    .show_scrollbar(false)
                    .width(Size::fill())
                    .height(Size::px(match wide {
                        true => video_row(),
                        false => cards::row(),
                    }))
                    .child(tiles(0, shelf.picks.len()))
                    .into_element(),
                false => {
                    let start = focused.map_or(0, |item| (item + 1).saturating_sub(shown));
                    tiles(start, shown).into_element()
                }
            }
        }
    };
    rect()
        .key(at)
        .width(Size::fill())
        .child(header(shelf))
        .child(picks)
}

/// A shelf's header: the strapline with its picture over the title, and "Reproducir todo"
/// across from them when the whole shelf can play.
fn header(shelf: &HomeShelf) -> impl IntoElement {
    let songs: Vec<Song> = shelf
        .picks
        .iter()
        .filter_map(|pick| match pick {
            Pick::Song(song) => Some(song.clone()),
            Pick::Collection(_) => None,
        })
        .collect();
    let playable = shelf.playable && !songs.is_empty();
    rect()
        .width(Size::fill())
        .direction(Direction::Horizontal)
        .content(Content::Flex)
        .cross_align(Alignment::End)
        .child(
            rect()
                .width(Size::flex(1.))
                .map(shelf.strapline.clone(), |titles, strapline| {
                    titles.child(
                        rect()
                            .height(Size::px(STRAP))
                            .direction(Direction::Horizontal)
                            .cross_align(Alignment::Center)
                            .spacing(10.)
                            .maybe_child(
                                shelf
                                    .avatar
                                    .clone()
                                    .map(|avatar| Cover::new(Some(avatar), 26., 13.)),
                            )
                            .child(ui::eyebrow(&strapline)),
                    )
                })
                .child(
                    rect()
                        .height(Size::px(TITLE))
                        .main_align(Alignment::Center)
                        .child(
                            ui::line(shelf.title.clone(), text::LARGE + 3., color::FOREGROUND)
                                .font_weight(FontWeight::BOLD),
                        ),
                ),
        )
        .maybe(playable, |header| {
            header.child(
                rect().height(Size::px(TITLE)).center().child(
                    ui::pill(
                        false,
                        Icon::PlayFilled,
                        Some("Reproducir todo"),
                        false,
                        color::LIGHT,
                    )
                    .on_press(move |event: Event<PressEventData>| {
                        event.stop_propagation();
                        crate::app::play_all(songs.clone());
                    }),
                ),
            )
        })
}

/// A shelf of songs as a grid of short rows, like YouTube Music's "Quick picks".
fn grid(
    at: usize,
    shelf: &HomeShelf,
    focused: Option<usize>,
    compact: bool,
    light: Color,
) -> impl IntoElement {
    let columns = quick_columns(compact);
    let picks: Vec<(usize, &Pick)> = shelf.picks.iter().enumerate().collect();
    rect()
        .width(Size::fill())
        .spacing(10.)
        .children(picks.chunks(columns).enumerate().map(|(line, chunk)| {
            rect()
                .key(line)
                .width(Size::fill())
                .direction(Direction::Horizontal)
                .content(Content::Flex)
                .spacing(10.)
                .children(chunk.iter().map(|(index, pick)| {
                    quick_tile(at, *index, pick, focused == Some(*index), light).into_element()
                }))
                // An unfilled last line keeps its tiles the size of the others.
                .children(
                    (chunk.len()..columns)
                        .map(|gap| rect().key(100 + gap).width(Size::flex(1.)).into_element()),
                )
                .into_element()
        }))
}

fn quick_tile(
    shelf: usize,
    at: usize,
    pick: &Pick,
    focused: bool,
    light: Color,
) -> impl IntoElement {
    let lit = ui::ring(focused);
    let compact = ui::compact();
    let side = match compact {
        true => 56.,
        false => QUICK_HEIGHT,
    };
    rect()
        .key(at)
        .width(Size::flex(1.))
        .height(Size::px(side))
        .direction(Direction::Horizontal)
        .content(Content::Flex)
        .cross_align(Alignment::Center)
        .spacing(12.)
        .corner_radius(metrics::RADIUS_SM)
        .overflow(Overflow::Clip)
        .background(match lit {
            true => color::RAISED,
            false => color::SECONDARY,
        })
        .on_press(ui::tap(Target::Content(Spot::Cell(shelf, at))))
        .map(song_of(pick), |tile, song| {
            tile.on_secondary_down(move |event: Event<PressEventData>| {
                crate::menu::open(song.clone(), crate::menu::location(&event))
            })
        })
        .child(cover(pick, side, 0.))
        .child(
            rect()
                .width(Size::flex(1.))
                .padding((0., 8., 0., 0.))
                .spacing(2.)
                .child(
                    ui::line(pick.title().to_owned(), text::BODY, color::FOREGROUND)
                        .font_weight(FontWeight::SEMI_BOLD)
                        .width(Size::fill()),
                )
                .child(
                    ui::line(subtitle(pick), text::SMALL, color::MUTED_FOREGROUND)
                        .width(Size::fill()),
                ),
        )
        .child(crate::chrome::sidebar::bar(lit, light))
}

/// The song a pick plays, for its menu.
fn song_of(pick: &Pick) -> Option<Song> {
    match pick {
        Pick::Song(song) => Some(song.clone()),
        Pick::Collection(_) => None,
    }
}

/// What a pick's second line says.
fn subtitle(pick: &Pick) -> String {
    match pick {
        Pick::Song(song) => song.artist.clone(),
        Pick::Collection(collection) => match collection.kind {
            Kind::Artist => "Artista".to_owned(),
            _ => collection.subtitle.clone(),
        },
    }
}

/// One pick as a large cover: an artist's round, a video's wide, everything else square.
fn tile(shelf: usize, at: usize, pick: &Pick, wide: bool, focused: bool) -> impl IntoElement {
    let width = match wide {
        true => video_width(),
        false => ui::card(),
    };
    let side = width - 6.;
    let round = matches!(pick, Pick::Collection(collection) if collection.kind == Kind::Artist);
    let radius = match round {
        true => side / 2.,
        false => metrics::RADIUS,
    };
    let art = match wide {
        // A video keeps its still: Apple's cover is square.
        true => Cover::new(pick.cover().map(str::to_owned), side, radius).height(side * 9. / 16.),
        false => cover(pick, side, radius),
    };
    rect()
        .key(at)
        .width(Size::px(width))
        .spacing(8.)
        .on_press(ui::tap(Target::Content(Spot::Cell(shelf, at))))
        .map(song_of(pick), |tile, song| {
            tile.on_secondary_down(move |event: Event<PressEventData>| {
                crate::menu::open(song.clone(), crate::menu::location(&event))
            })
        })
        .child(
            rect()
                .corner_radius(radius + 3.)
                .padding(3.)
                .border(ui::focus_border(focused))
                .child(art),
        )
        .child(
            ui::line(pick.title().to_owned(), text::BODY, color::FOREGROUND)
                .font_weight(FontWeight::SEMI_BOLD)
                .width(Size::fill())
                .margin((0., 3.)),
        )
        .child(
            ui::line(subtitle(pick), text::SMALL, color::MUTED_FOREGROUND)
                .width(Size::fill())
                .margin((0., 3.)),
        )
}

/// "Buenos días, Wilovy09", by the hour where the device is.
fn greeting(name: Option<&str>) -> String {
    let hello = match local_hour() {
        Some(5..=11) => "Buenos días",
        Some(12..=18) => "Buenas tardes",
        Some(_) => "Buenas noches",
        None => "Hola",
    };
    match name {
        Some(name) => format!("{hello}, {name}"),
        None => hello.to_owned(),
    }
}

#[cfg(unix)]
fn local_hour() -> Option<i32> {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .ok()?
        .as_secs() as libc::time_t;
    // SAFETY: `localtime_r` fills the `tm` handed to it and keeps no pointer to it.
    unsafe {
        let mut local: libc::tm = std::mem::zeroed();
        if libc::localtime_r(&now, &mut local).is_null() {
            return None;
        }
        Some(local.tm_hour)
    }
}

#[cfg(not(unix))]
fn local_hour() -> Option<i32> {
    None
}

/// A pick's cover, Apple's when the catalog has it.
fn cover(pick: &Pick, side: f32, radius: f32) -> Cover {
    let apple = match pick {
        Pick::Song(song) => Some(crate::artwork::Wanted::song(song)),
        Pick::Collection(collection) => crate::artwork::Wanted::collection(collection),
    };
    Cover::new(pick.cover().map(str::to_owned), side, radius).maybe_apple(apple)
}

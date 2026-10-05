//! Home: a greeting, the first shelf of the account's feed as a grid of quick tiles (the songs
//! to put on right now), then the feed's other shelves as rows of covers that slide with the
//! D-pad or scroll under a finger.

use freya::prelude::*;
use freya::radio::use_radio;

use crate::library::{HomeShelf, Kind, Pick};
use crate::nav::Target;
use crate::screens::cards;
use crate::state::{AppState, Auth, Channel, Load, Spot, Zone};
use crate::ui::{self, Cover, color, metrics, text};

/// How many quick tiles the first shelf shows.
const QUICK: usize = 6;
/// Height of a quick tile.
const QUICK_HEIGHT: f32 = 64.;
/// Height of a shelf's title.
const TITLE: f32 = 52.;

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
        let load = library.read().home.clone();
        let now = use_radio::<AppState, Channel>(Channel::Now);
        let light = now.read().now.light.unwrap_or(color::FOCUS_ON_PRIMARY);
        let compact = ui::compact();
        let columns = ui::columns();

        // Keep the focused shelf in view, a third of the way down.
        let quick_rows = QUICK.div_ceil(quick_columns(compact)) as f32;
        let target = match spot {
            Some(Spot::Cell(shelf, _)) if shelf > 0 => {
                120. + quick_rows * (QUICK_HEIGHT + 10.) + (shelf - 1) as f32 * shelf_height()
            }
            _ => 0.,
        };
        let (_, height) = ui::viewport();
        let scroll = ui::use_follow((target - height / 4.).max(0.));

        let body = match load {
            Load::Ready(shelves) if !shelves.is_empty() => {
                let first = quick(&shelves[0], spot, compact, light).into_element();
                let rest: Vec<Element> = shelves
                    .iter()
                    .enumerate()
                    .skip(1)
                    .map(|(at, shelf)| row(at, shelf, spot, columns, compact).into_element())
                    .collect();
                rect()
                    .width(Size::fill())
                    .spacing(8.)
                    .child(first)
                    .children(rest)
                    .into_element()
            }
            Load::Ready(_) => note("Tu inicio está vacío por ahora").into_element(),
            Load::Failed(error) => note(error).into_element(),
            Load::Loading | Load::Idle => note("Cargando tu inicio…").into_element(),
        };

        ScrollView::new_controlled(scroll)
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

fn shelf_height() -> f32 {
    TITLE + cards::row() + 8.
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

/// The first shelf as a grid of wide tiles: cover on the left, title beside it, lit when the
/// D-pad is on one.
fn quick(shelf: &HomeShelf, spot: Option<Spot>, compact: bool, light: Color) -> impl IntoElement {
    let columns = quick_columns(compact);
    let picks: Vec<(usize, &Pick)> = shelf.picks.iter().take(QUICK).enumerate().collect();
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
                .children(chunk.iter().map(|(at, pick)| {
                    quick_tile(*at, pick, spot == Some(Spot::Cell(0, *at)), light).into_element()
                }))
                // An unfilled last line keeps its tiles the size of the others.
                .children(
                    (chunk.len()..columns)
                        .map(|gap| rect().key(100 + gap).width(Size::flex(1.)).into_element()),
                )
                .into_element()
        }))
}

fn quick_tile(at: usize, pick: &Pick, focused: bool, light: Color) -> impl IntoElement {
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
        .on_press(ui::tap(Target::Content(Spot::Cell(0, at))))
        .child(cover(pick, side, 0.))
        .child(
            rect()
                .width(Size::flex(1.))
                .padding((0., 8., 0., 10.))
                .child(
                    label()
                        .text(pick.title().to_owned())
                        .font_size(match compact {
                            true => text::LABEL,
                            false => text::BODY,
                        })
                        .font_weight(FontWeight::SEMI_BOLD)
                        .color(color::FOREGROUND)
                        .max_lines(2)
                        .text_overflow(TextOverflow::Ellipsis)
                        .width(Size::fill()),
                ),
        )
        .child(crate::chrome::sidebar::bar(lit, light))
}

/// A shelf after the first: its title and a row of covers.
fn row(
    at: usize,
    shelf: &HomeShelf,
    spot: Option<Spot>,
    columns: usize,
    compact: bool,
) -> impl IntoElement {
    let focused = match spot {
        Some(Spot::Cell(row, item)) if row == at => Some(item),
        _ => None,
    };
    let tiles =
        |skip: usize, take: usize| {
            rect()
                .direction(Direction::Horizontal)
                .spacing(ui::gap())
                .children(shelf.picks.iter().enumerate().skip(skip).take(take).map(
                    |(index, pick)| tile(at, index, pick, focused == Some(index)).into_element(),
                ))
        };
    let shelf_row = rect().key(at).width(Size::fill()).child(
        rect()
            .height(Size::px(TITLE))
            .main_align(Alignment::Center)
            .child(
                ui::line(shelf.title.clone(), text::LARGE + 3., color::FOREGROUND)
                    .font_weight(FontWeight::BOLD),
            ),
    );
    if compact {
        return shelf_row.child(
            ScrollView::new()
                .direction(Direction::Horizontal)
                .show_scrollbar(false)
                .width(Size::fill())
                .height(Size::px(cards::row()))
                .child(tiles(0, shelf.picks.len())),
        );
    }
    let start = focused.map_or(0, |item| (item + 1).saturating_sub(columns));
    shelf_row.child(tiles(start, columns))
}

/// One pick as a large cover: an artist's round, everything else square.
fn tile(shelf: usize, at: usize, pick: &Pick, focused: bool) -> impl IntoElement {
    let width = ui::card();
    let side = width - 6.;
    let round = matches!(pick, Pick::Collection(collection) if collection.kind == Kind::Artist);
    let subtitle = match pick {
        Pick::Song(song) => song.artist.clone(),
        Pick::Collection(collection) => match collection.kind {
            Kind::Artist => "Artista".to_owned(),
            _ => collection.subtitle.clone(),
        },
    };
    rect()
        .key(at)
        .width(Size::px(width))
        .spacing(8.)
        .on_press(ui::tap(Target::Content(Spot::Cell(shelf, at))))
        .child(
            rect()
                .corner_radius(match round {
                    true => side / 2. + 3.,
                    false => metrics::RADIUS + 3.,
                })
                .padding(3.)
                .margin(-3.)
                .border(ui::focus_border(focused))
                .child(cover(
                    pick,
                    side,
                    match round {
                        true => side / 2.,
                        false => metrics::RADIUS,
                    },
                )),
        )
        .child(
            ui::line(pick.title().to_owned(), text::BODY, color::FOREGROUND)
                .font_weight(FontWeight::SEMI_BOLD)
                .width(Size::fill()),
        )
        .child(ui::line(subtitle, text::SMALL, color::MUTED_FOREGROUND).width(Size::fill()))
}

/// A pick's cover, Apple's when the catalog has it.
fn cover(pick: &Pick, side: f32, radius: f32) -> Cover {
    let apple = match pick {
        Pick::Song(song) => Some(crate::artwork::Wanted::song(song)),
        Pick::Collection(collection) => crate::artwork::Wanted::collection(collection),
    };
    Cover::new(pick.cover().map(str::to_owned), side, radius).maybe_apple(apple)
}

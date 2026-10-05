//! A page of songs: the hero (cover, title, Play and Shuffle) over the track table. It shows
//! liked songs and the tracks of a playlist or album.

use freya::prelude::*;
use freya::radio::use_radio;

use crate::library::{self, Kind, Song};
use crate::nav::Target;
use crate::state::{AppState, Channel, Load, Page, Spot, Zone};
use crate::ui::{self, Cover, Icon, color, metrics, text};

/// The hero's cover side on this page.
const HERO_COVER: f32 = 168.;

#[derive(PartialEq)]
pub struct Songs;

impl Component for Songs {
    fn render(&self) -> impl IntoElement {
        let navigation = use_radio::<AppState, Channel>(Channel::Navigation);
        let library = use_radio::<AppState, Channel>(Channel::Library);
        let detail = use_radio::<AppState, Channel>(Channel::Detail);
        let now = use_radio::<AppState, Channel>(Channel::Now);

        let state = navigation.read();
        let page = state.page.clone();
        let focus = state.focus;
        let in_content = focus.zone == Zone::Content;
        drop(state);

        let (eyebrow, title, cover, load): (&str, String, Option<String>, Load<Vec<Song>>) =
            match &page {
                Page::Detail(collection) => (
                    match collection.kind {
                        Kind::Playlist => "Playlist",
                        Kind::Album => "Álbum",
                        Kind::Artist => "Artista",
                    },
                    collection.title.clone(),
                    collection.cover.clone(),
                    detail.read().detail.clone(),
                ),
                _ => (
                    "Biblioteca",
                    "Canciones que te gustan".into(),
                    None,
                    match &library.read().library {
                        Load::Ready(library) => Load::Ready(library.liked.clone()),
                        Load::Failed(error) => Load::Failed(error.clone()),
                        Load::Loading => Load::Loading,
                        Load::Idle => Load::Idle,
                    },
                ),
            };
        let subtitle = match (&page, &load) {
            (Page::Detail(collection), Load::Ready(songs)) if collection.kind == Kind::Album => {
                format!(
                    "{} • {}",
                    collection.subtitle,
                    library::songs_count(songs.len())
                )
            }
            (Page::Detail(collection), Load::Ready(_)) if collection.kind == Kind::Artist => {
                "Canciones populares".into()
            }
            (_, Load::Ready(songs)) => library::songs_count(songs.len()),
            (_, Load::Loading | Load::Idle) => "Cargando…".into(),
            (_, Load::Failed(error)) => error.clone(),
        };

        let hero_radius = match &page {
            Page::Detail(collection) if collection.kind == Kind::Artist => HERO_COVER / 2.,
            _ => metrics::RADIUS,
        };
        let action = |index: usize| in_content && focus.content == Spot::Action(index);
        let row = match focus.content {
            Spot::Row(row) if in_content && !ui::touch() => Some(row),
            _ => None,
        };
        let songs = load.ready().cloned().unwrap_or_default();
        let count = songs.len();
        let compact = ui::compact();
        let cover_side = match compact {
            true => 124.,
            false => HERO_COVER,
        };
        let playing = now.read().now.song.as_ref().map(|song| song.id.clone());

        let (_, height) = ui::viewport();
        let view = height - metrics::PLAYER_BAR - cover_side - metrics::INSET * 3.;
        let scroll = ui::use_follow(ui::follow_offset(row.unwrap_or(0), row_height(), view));

        // The page glows with its own cover; liked songs with the newest one.
        let art = cover
            .clone()
            .or_else(|| songs.first().and_then(|song| song.cover.clone()))
            .map(|url| library::sized(&url, library::THUMB_EDGE));
        let light = ui::tint::use_tint(art);
        let ring_light = light.unwrap_or(color::FOCUS_ON_PRIMARY);
        let inset = ui::inset();
        rect()
            .expanded()
            .child(
                rect()
                    .position(Position::new_absolute().top(0.).left(0.))
                    .layer(Layer::Relative(-1))
                    // Wider than the page: an absolute `fill` stops short of its right edge.
                    .width(Size::px(ui::viewport().0))
                    .height(Size::px(cover_side + inset * 4.))
                    .map(light, |glow, light| glow.child(ui::tint::wash(light, 0.30))),
            )
            .child(
                rect()
                    .expanded()
                    .padding((inset, inset, 0., inset))
                    .spacing(inset)
                    .content(Content::Flex)
                    .child(
                        rect()
                            .direction(Direction::Horizontal)
                            .cross_align(Alignment::End)
                            .spacing(match compact {
                                true => 14.,
                                false => 20.,
                            })
                            .child(match cover {
                                Some(url) => Cover::new(Some(url), cover_side, hero_radius)
                                    .maybe_apple(match &page {
                                        Page::Detail(collection) => {
                                            crate::artwork::Wanted::collection(collection)
                                        }
                                        _ => None,
                                    })
                                    .edge(library::COVER_EDGE)
                                    .into_element(),
                                None => liked_tile(cover_side, &songs).into_element(),
                            })
                            .child(
                                rect()
                                    .width(Size::flex(1.))
                                    .height(Size::px(cover_side))
                                    .main_align(Alignment::End)
                                    .spacing(8.)
                                    .child(ui::eyebrow(eyebrow))
                                    .child(
                                        ui::line(
                                            title,
                                            match compact {
                                                true => text::TITLE,
                                                false => text::DISPLAY * 1.5,
                                            },
                                            color::FOREGROUND,
                                        )
                                        .font_weight(FontWeight::BOLD)
                                        .width(Size::fill()),
                                    )
                                    .child(ui::line(subtitle, text::BODY, color::MUTED_FOREGROUND))
                                    .child(
                                        rect()
                                            .direction(Direction::Horizontal)
                                            .spacing(10.)
                                            .padding((8., 0., 0., 0.))
                                            .child(
                                                ui::pill(
                                                    true,
                                                    Icon::PlayFilled,
                                                    Some("Reproducir"),
                                                    action(0),
                                                    ring_light,
                                                )
                                                .on_press(ui::tap(Target::Content(Spot::Action(
                                                    0,
                                                )))),
                                            )
                                            .child(
                                                ui::pill(
                                                    false,
                                                    Icon::Shuffle,
                                                    None,
                                                    action(1),
                                                    ring_light,
                                                )
                                                .on_press(ui::tap(Target::Content(Spot::Action(
                                                    1,
                                                )))),
                                            ),
                                    ),
                            ),
                    )
                    .child(
                        rect()
                            .width(Size::fill())
                            .height(Size::flex(1.))
                            .content(Content::Flex)
                            .maybe(!compact, |table| table.child(header()))
                            .child(
                                // The data is what the rows depend on: the list compares it, not the
                                // builder, to know when to redraw.
                                VirtualScrollView::new_with_data_controlled(
                                    (songs, row, playing),
                                    |index, (songs, row, playing)| {
                                        let song = &songs[index];
                                        track_row(
                                            index,
                                            song,
                                            *row == Some(index),
                                            playing.as_deref() == Some(song.id.as_str()),
                                        )
                                        .into_element()
                                    },
                                    scroll,
                                )
                                .length(count)
                                .item_size(row_height())
                                .width(Size::fill())
                                .height(Size::flex(1.)),
                            ),
                    ),
            )
    }
}

/// The liked songs hero: the four newest liked covers as a mosaic under a dark veil, the
/// heart over them. Before anything loads, a heart on a raised square.
fn liked_tile(side: f32, songs: &[Song]) -> impl IntoElement {
    let half = side / 2.;
    let quarter = |at: usize| {
        let song = songs.get(at);
        Cover::new(song.and_then(|song| song.cover.clone()), half, 0.)
            .maybe_apple(song.map(crate::artwork::Wanted::song))
    };
    rect()
        .width(Size::px(side))
        .height(Size::px(side))
        .corner_radius(metrics::RADIUS)
        .overflow(Overflow::Clip)
        .background(color::RAISED)
        .maybe(songs.len() >= 4, |tile| {
            tile.child(
                rect()
                    .position(Position::new_absolute().top(0.).left(0.))
                    .child(
                        rect()
                            .direction(Direction::Horizontal)
                            .child(quarter(0))
                            .child(quarter(1)),
                    )
                    .child(
                        rect()
                            .direction(Direction::Horizontal)
                            .child(quarter(2))
                            .child(quarter(3)),
                    ),
            )
            .child(
                rect()
                    .position(Position::new_absolute().top(0.).left(0.))
                    .layer(Layer::Relative(6))
                    .width(Size::px(side))
                    .height(Size::px(side))
                    .background(Color::from_argb(0x99, 0x09, 0x09, 0x09)),
            )
        })
        .child(
            rect()
                .width(Size::px(side))
                .height(Size::px(side))
                .layer(Layer::Relative(10))
                .center()
                .child(ui::icon(Icon::HeartFilled, side * 0.34, color::FOREGROUND)),
        )
}

/// The table's column titles.
fn header() -> impl IntoElement {
    rect()
        .height(Size::px(metrics::HEADER))
        .width(Size::fill())
        .margin((0., 0., 4., 0.))
        .padding((0., 0., 0., 3.))
        .direction(Direction::Horizontal)
        .content(Content::Flex)
        .cross_align(Alignment::Center)
        .border(
            Border::new()
                .fill(color::SIDEBAR_BORDER)
                .width(BorderWidth {
                    bottom: 1.,
                    ..Default::default()
                }),
        )
        .child(rect().width(Size::px(40.)).center().child(column("#")))
        .child(rect().width(Size::px(metrics::THUMB + 14.)))
        .child(
            rect()
                .width(Size::flex(0.6))
                .padding((0., 12., 0., 0.))
                .child(column("Título")),
        )
        .child(
            rect()
                .width(Size::flex(0.4))
                .padding((0., 12., 0., 0.))
                .child(column("Álbum")),
        )
        .child(
            rect()
                .width(Size::px(84.))
                .padding((0., 14., 0., 0.))
                .main_align(Alignment::End)
                .direction(Direction::Horizontal)
                .child(column("Duración")),
        )
}

/// A column title: small, faint, spaced out.
fn column(title: &str) -> Label {
    ui::line(title.to_uppercase(), text::TINY, color::FAINT).font_weight(FontWeight::SEMI_BOLD)
}

/// Height of a track row: taller on a phone, for a finger.
pub(crate) fn row_height() -> f32 {
    match ui::compact() {
        true => 60.,
        false => metrics::ROW,
    }
}

/// One song of the table: number (a ♪ while it plays, ▶ under the pointer), cover, title and
/// artists, album, duration. A phone keeps the cover, title, artists and duration. The row the
/// D-pad is on is lit, with a bar of the cover's light, never outlined.
pub(crate) fn track_row(
    index: usize,
    song: &Song,
    focused: bool,
    playing: bool,
) -> impl IntoElement {
    TrackRow {
        index,
        song: song.clone(),
        focused: ui::ring(focused),
        playing,
    }
}

#[derive(PartialEq)]
struct TrackRow {
    index: usize,
    song: Song,
    focused: bool,
    playing: bool,
}

impl Component for TrackRow {
    fn render(&self) -> impl IntoElement {
        let now = use_radio::<AppState, Channel>(Channel::Now);
        let light = now.read().now.light.unwrap_or(color::LIGHT);
        let now_playing = now.read().now.playing;
        let mut hovered = use_state(|| false);
        let hover = *hovered.read();
        let (song, index, focused, playing) = (&self.song, self.index, self.focused, self.playing);
        let compact = ui::compact();
        let title_ink = match playing {
            true => light,
            false => color::FOREGROUND,
        };
        let fill = match (focused, hover, playing) {
            (true, _, _) => color::FOCUS_FILL,
            (false, true, _) => color::HOVER,
            (false, false, true) => ui::tint::alpha(light, 0.08),
            (false, false, false) => Color::TRANSPARENT,
        };
        let number = match (playing, hover && !compact) {
            (true, _) => ui::spectrum::Spectrum {
                size: 18.,
                tint: light,
                playing: now_playing,
            }
            .into_element(),
            (false, true) => ui::icon(Icon::PlayFilled, 12., color::FOREGROUND).into_element(),
            (false, false) => {
                ui::line(format!("{}", index + 1), text::SMALL, color::FAINT).into_element()
            }
        };
        let duration = ui::line(
            song.duration.map(library::clock).unwrap_or_default(),
            text::SMALL,
            color::MUTED_FOREGROUND,
        );
        let names = |size: f32| {
            rect()
                .width(Size::fill())
                .spacing(2.)
                .child(
                    ui::line(song.title.clone(), size, title_ink)
                        .font_weight(FontWeight::MEDIUM)
                        .width(Size::fill()),
                )
                .child(
                    ui::line(song.artist.clone(), text::SMALL, color::MUTED_FOREGROUND)
                        .width(Size::fill()),
                )
        };

        rect()
            .height(Size::px(row_height()))
            .width(Size::fill())
            .direction(Direction::Horizontal)
            .content(Content::Flex)
            .cross_align(Alignment::Center)
            .corner_radius(metrics::RADIUS_SM)
            .overflow(Overflow::Clip)
            .background(fill)
            .on_pointer_enter(move |_| hovered.set(true))
            .on_pointer_leave(move |_| hovered.set(false))
            .on_press(ui::tap(Target::Content(Spot::Row(index))))
            .child(crate::chrome::sidebar::bar(focused, light))
            .map((!compact).then_some(()), |row, _| {
                row.child(rect().width(Size::px(40.)).center().child(number))
            })
            .child(
                rect()
                    .width(Size::px(match compact {
                        true => 60.,
                        false => metrics::THUMB + 14.,
                    }))
                    .padding((
                        0.,
                        0.,
                        0.,
                        match compact {
                            true => 6.,
                            false => 0.,
                        },
                    ))
                    .child(
                        Cover::new(
                            song.cover.clone(),
                            match compact {
                                true => 48.,
                                false => metrics::THUMB,
                            },
                            metrics::RADIUS_SM / 2.,
                        )
                        .apple(crate::artwork::Wanted::song(song)),
                    ),
            )
            .child(
                rect()
                    .width(Size::flex(match compact {
                        true => 1.,
                        false => 0.6,
                    }))
                    .padding((0., 12., 0., 0.))
                    .child(names(match compact {
                        true => text::BODY + 1.,
                        false => text::BODY,
                    })),
            )
            .maybe(!compact, |row| {
                row.child(
                    rect()
                        .width(Size::flex(0.4))
                        .padding((0., 12., 0., 0.))
                        .child(
                            ui::line(
                                song.album.clone().unwrap_or_default(),
                                text::SMALL,
                                color::MUTED_FOREGROUND,
                            )
                            .width(Size::fill()),
                        ),
                )
            })
            .child(
                rect()
                    .width(Size::px(84.))
                    .padding((0., 14., 0., 0.))
                    .direction(Direction::Horizontal)
                    .main_align(Alignment::End)
                    .child(duration),
            )
    }
}

//! A page of songs: the hero (cover, title, Play and Shuffle) over the track table. It shows
//! liked songs and the tracks of a playlist or album.

use freya::prelude::*;
use freya::radio::use_radio;

use crate::library::{self, Kind, Song};
use crate::nav::Target;
use crate::state::{AppState, Channel, Load, Page, Spot, Zone};
use crate::ui::{self, Cover, Icon, Variant, color, metrics, text};

/// The hero's cover side on this page.
const HERO_COVER: f32 = 112.;

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
            _ => metrics::RADIUS * 1.5,
        };
        let action = |index: usize| in_content && focus.content == Spot::Action(index);
        let row = match focus.content {
            Spot::Row(row) if in_content && !ui::touch() => Some(row),
            _ => None,
        };
        let compact = ui::compact();
        let cover_side = match compact {
            true => 124.,
            false => HERO_COVER,
        };
        let playing = now.read().now.song.as_ref().map(|song| song.id.clone());

        let (_, height) = ui::viewport();
        let view = height - metrics::PLAYER_BAR - cover_side - metrics::INSET * 3.;
        let scroll = ui::use_follow(ui::follow_offset(row.unwrap_or(0), row_height(), view));

        let songs = load.ready().cloned().unwrap_or_default();
        let count = songs.len();

        let inset = ui::inset();
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
                        None => liked_tile(cover_side).into_element(),
                    })
                    .child(
                        rect()
                            .width(Size::flex(1.))
                            .height(Size::px(cover_side))
                            .main_align(Alignment::End)
                            .spacing(6.)
                            .child(ui::eyebrow(eyebrow))
                            .child(
                                ui::line(
                                    title,
                                    match compact {
                                        true => text::TITLE,
                                        false => text::DISPLAY,
                                    },
                                    color::FOREGROUND,
                                )
                                .font_weight(FontWeight::BOLD)
                                .width(Size::fill()),
                            )
                            .child(ui::line(subtitle, text::SMALL, color::MUTED_FOREGROUND))
                            .child(
                                rect()
                                    .direction(Direction::Horizontal)
                                    .spacing(8.)
                                    .padding((4., 0., 0., 0.))
                                    .child(
                                        ui::button(
                                            Variant::Primary,
                                            Some(Icon::PlayFilled),
                                            Some("Reproducir"),
                                            action(0),
                                        )
                                        .on_press(ui::tap(Target::Content(Spot::Action(0)))),
                                    )
                                    .child(
                                        ui::button(
                                            Variant::Outline,
                                            Some(Icon::Shuffle),
                                            None,
                                            action(1),
                                        )
                                        .on_press(ui::tap(Target::Content(Spot::Action(1)))),
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
            )
    }
}

/// The liked songs hero: a heart on a muted square, as Sonora draws it.
fn liked_tile(side: f32) -> impl IntoElement {
    rect()
        .width(Size::px(side))
        .height(Size::px(side))
        .corner_radius(metrics::RADIUS * 1.5)
        .background(color::MUTED)
        .center()
        .child(ui::icon(Icon::HeartFilled, side * 0.4, color::FOREGROUND))
}

/// The table's column titles.
fn header() -> impl IntoElement {
    rect()
        .height(Size::px(metrics::HEADER))
        .width(Size::fill())
        .padding((0., metrics::PAD))
        .direction(Direction::Horizontal)
        .content(Content::Flex)
        .cross_align(Alignment::Center)
        .background(color::TABLE_HEAD)
        .corner_radius(metrics::RADIUS)
        .child(rect().width(Size::px(44.)).child(ui::line(
            "#",
            text::SMALL,
            color::MUTED_FOREGROUND,
        )))
        .child(rect().width(Size::px(metrics::THUMB + 12.)))
        .child(rect().width(Size::flex(0.6)).child(ui::line(
            "Título",
            text::SMALL,
            color::MUTED_FOREGROUND,
        )))
        .child(rect().width(Size::flex(0.4)).child(ui::line(
            "Álbum",
            text::SMALL,
            color::MUTED_FOREGROUND,
        )))
        .child(
            rect()
                .width(Size::px(64.))
                .main_align(Alignment::End)
                .direction(Direction::Horizontal)
                .child(ui::line("Duración", text::SMALL, color::MUTED_FOREGROUND)),
        )
}

/// Height of a track row: taller on a phone, for a finger.
pub(crate) fn row_height() -> f32 {
    match ui::compact() {
        true => 60.,
        false => metrics::ROW,
    }
}

/// One song of the table: index, thumbnail, title and artists, album, duration. A phone keeps
/// the cover, the title and artists and the duration.
pub(crate) fn track_row(
    index: usize,
    song: &Song,
    focused: bool,
    playing: bool,
) -> impl IntoElement {
    let focused = ui::ring(focused);
    let title_ink = match playing {
        true => color::PRIMARY,
        false => color::FOREGROUND,
    };
    if ui::compact() {
        return rect()
            .key(index)
            .height(Size::px(row_height()))
            .width(Size::fill())
            .padding((0., 4.))
            .direction(Direction::Horizontal)
            .content(Content::Flex)
            .cross_align(Alignment::Center)
            .spacing(12.)
            .corner_radius(metrics::RADIUS)
            .background(match (focused, playing) {
                (true, _) => color::FOCUS_FILL,
                (false, true) => color::MUTED,
                (false, false) => Color::TRANSPARENT,
            })
            .border(ui::focus_border(focused))
            .on_press(ui::tap(Target::Content(Spot::Row(index))))
            .child(
                Cover::new(song.cover.clone(), 48., 4.).apple(crate::artwork::Wanted::song(&song)),
            )
            .child(
                rect()
                    .width(Size::flex(1.))
                    .spacing(2.)
                    .child(
                        ui::line(song.title.clone(), text::BODY + 1., title_ink)
                            .width(Size::fill()),
                    )
                    .child(
                        ui::line(song.artist.clone(), text::SMALL, color::MUTED_FOREGROUND)
                            .width(Size::fill()),
                    ),
            )
            .child(ui::line(
                song.duration.map(library::clock).unwrap_or_default(),
                text::SMALL,
                color::MUTED_FOREGROUND,
            ))
            .into_element();
    }
    rect()
        .key(index)
        .height(Size::px(metrics::ROW))
        .width(Size::fill())
        .padding((0., metrics::PAD))
        .direction(Direction::Horizontal)
        .content(Content::Flex)
        .cross_align(Alignment::Center)
        .corner_radius(metrics::RADIUS)
        .background(match (focused, playing) {
            (true, _) => color::FOCUS_FILL,
            (false, true) => color::MUTED,
            (false, false) => Color::TRANSPARENT,
        })
        .border(ui::focus_border(focused))
        .on_press(ui::tap(Target::Content(Spot::Row(index))))
        .child(
            rect().width(Size::px(44.)).child(match playing {
                true => ui::icon(Icon::Playing, 14., color::PRIMARY).into_element(),
                false => ui::line(
                    (index + 1).to_string(),
                    text::SMALL,
                    color::MUTED_FOREGROUND,
                )
                .into_element(),
            }),
        )
        .child(
            rect().width(Size::px(metrics::THUMB + 12.)).child(
                Cover::new(song.cover.clone(), metrics::THUMB, 4.)
                    .apple(crate::artwork::Wanted::song(&song)),
            ),
        )
        .child(
            rect()
                .width(Size::flex(0.6))
                .spacing(1.)
                .child(ui::line(song.title.clone(), text::BODY, title_ink))
                .child(ui::line(
                    song.artist.clone(),
                    text::SMALL,
                    color::MUTED_FOREGROUND,
                )),
        )
        .child(rect().width(Size::flex(0.4)).child(ui::line(
            song.album.clone().unwrap_or_default(),
            text::SMALL,
            color::MUTED_FOREGROUND,
        )))
        .child(
            rect()
                .width(Size::px(64.))
                .direction(Direction::Horizontal)
                .main_align(Alignment::End)
                .child(ui::line(
                    song.duration.map(library::clock).unwrap_or_default(),
                    text::SMALL,
                    color::MUTED_FOREGROUND,
                )),
        )
        .into_element()
}

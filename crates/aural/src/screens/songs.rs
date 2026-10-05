//! A page of songs: the hero (cover, title, Play and Shuffle) over the track table. It shows
//! liked songs and the tracks of a playlist or album.

use freya::prelude::*;
use freya::radio::use_radio;

use crate::library::{self, Kind, Song};
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
            Spot::Row(row) if in_content => Some(row),
            _ => None,
        };
        let playing = now.read().now.song.as_ref().map(|song| song.id.clone());

        let (_, height) = ui::viewport();
        let view = height - metrics::PLAYER_BAR - HERO_COVER - metrics::INSET * 3.;
        let scroll = ui::use_follow(ui::follow_offset(row.unwrap_or(0), metrics::ROW, view));

        let songs = load.ready().cloned().unwrap_or_default();
        let count = songs.len();

        rect()
            .expanded()
            .padding((metrics::INSET, metrics::INSET, 0., metrics::INSET))
            .spacing(metrics::INSET)
            .content(Content::Flex)
            .child(
                rect()
                    .direction(Direction::Horizontal)
                    .cross_align(Alignment::End)
                    .spacing(20.)
                    .child(match cover {
                        Some(url) => Cover::new(Some(url), HERO_COVER, hero_radius)
                            .edge(library::COVER_EDGE)
                            .into_element(),
                        None => liked_tile().into_element(),
                    })
                    .child(
                        rect()
                            .height(Size::px(HERO_COVER))
                            .main_align(Alignment::End)
                            .spacing(6.)
                            .child(ui::eyebrow(eyebrow))
                            .child(
                                ui::line(title, text::DISPLAY, color::FOREGROUND)
                                    .font_weight(FontWeight::BOLD),
                            )
                            .child(ui::line(subtitle, text::SMALL, color::MUTED_FOREGROUND))
                            .child(
                                rect()
                                    .direction(Direction::Horizontal)
                                    .spacing(8.)
                                    .padding((4., 0., 0., 0.))
                                    .child(ui::button(
                                        Variant::Primary,
                                        Some(Icon::PlayFilled),
                                        Some("Reproducir"),
                                        action(0),
                                    ))
                                    .child(ui::button(
                                        Variant::Outline,
                                        Some(Icon::Shuffle),
                                        None,
                                        action(1),
                                    )),
                            ),
                    ),
            )
            .child(
                rect()
                    .width(Size::fill())
                    .height(Size::flex(1.))
                    .content(Content::Flex)
                    .child(header())
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
                        .item_size(metrics::ROW)
                        .width(Size::fill())
                        .height(Size::flex(1.)),
                    ),
            )
    }
}

/// The liked songs hero: a heart on a muted square, as Sonora draws it.
fn liked_tile() -> impl IntoElement {
    rect()
        .width(Size::px(HERO_COVER))
        .height(Size::px(HERO_COVER))
        .corner_radius(metrics::RADIUS * 1.5)
        .background(color::MUTED)
        .center()
        .child(ui::icon(
            Icon::HeartFilled,
            HERO_COVER * 0.4,
            color::FOREGROUND,
        ))
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

/// One song of the table: index, thumbnail, title and artists, album, duration.
pub(crate) fn track_row(
    index: usize,
    song: &Song,
    focused: bool,
    playing: bool,
) -> impl IntoElement {
    let title_ink = match playing {
        true => color::PRIMARY,
        false => color::FOREGROUND,
    };
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
            rect()
                .width(Size::px(metrics::THUMB + 12.))
                .child(Cover::new(song.cover.clone(), metrics::THUMB, 4.)),
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
}

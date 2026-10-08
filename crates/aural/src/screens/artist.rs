//! An artist's page, after YouTube Music's: the artist's picture as a banner fading into the
//! page with the name, audience, description and Play and Shuffle over it; the top songs;
//! then the page's shelves (albums, singles, playlists, related artists), each a row of large
//! covers that slides along with the D-pad.

use bytes::Bytes;
use freya::prelude::*;
use freya::radio::use_radio;

use crate::images;
use crate::library::{self, ArtistPage, Shelf};
use crate::nav::Target;
use crate::screens::{cards, search, songs};
use crate::state::{AppState, Channel, Load, Spot, Zone};
use crate::ui::{self, Icon, color, metrics, text};

/// Height of the banner: shorter on a phone.
fn hero_height() -> f32 {
    match ui::compact() {
        true => 340.,
        false => 460.,
    }
}
/// Height of a section title.
const TITLE: f32 = 52.;
/// One shelf: its title and a row of covers.
fn shelf_height() -> f32 {
    TITLE + cards::row() + 16.
}

#[derive(PartialEq)]
pub struct Artist;

impl Component for Artist {
    fn render(&self) -> impl IntoElement {
        let navigation = use_radio::<AppState, Channel>(Channel::Navigation);
        let detail = use_radio::<AppState, Channel>(Channel::Detail);
        let now = use_radio::<AppState, Channel>(Channel::Now);

        let state = navigation.read();
        let spot = match state.focus.zone == Zone::Content && !ui::touch() {
            true => Some(state.focus.content),
            false => None,
        };
        drop(state);
        let load = detail.read().artist.clone();
        // The photo's light washes the hero, so every artist's page has its own room.
        let light = ui::tint::use_tint(
            load.ready()
                .and_then(|page| page.banner.clone())
                .map(|url| library::sized(&url, library::THUMB_EDGE)),
        );
        let playing = now.read().now.song.as_ref().map(|song| song.id.clone());
        let columns = ui::columns();

        let top_rows = load
            .ready()
            .map_or(0, |page| page.top.len().min(crate::state::ARTIST_TOP));
        let target = match spot {
            Some(Spot::Row(row)) => hero_height() + TITLE + row as f32 * songs::row_height(),
            Some(Spot::Cell(shelf, _)) => {
                hero_height()
                    + TITLE
                    + top_rows as f32 * songs::row_height()
                    + 24.
                    + shelf as f32 * shelf_height()
            }
            _ => 0.,
        };
        let (_, height) = ui::viewport();
        let view = height - metrics::PLAYER_BAR;
        let scroll = ui::use_follow((target - view / 3.).max(0.));

        let page = match load {
            Load::Ready(page) => page,
            Load::Failed(error) => {
                return rect()
                    .expanded()
                    .padding(metrics::INSET)
                    .child(ui::line(error, text::BODY, color::MUTED_FOREGROUND))
                    .into_element();
            }
            Load::Loading | Load::Idle => {
                return rect()
                    .expanded()
                    .padding(metrics::INSET)
                    .child(ui::line("Cargando…", text::BODY, color::MUTED_FOREGROUND))
                    .into_element();
            }
        };

        let top: Vec<Element> = page
            .top
            .iter()
            .take(crate::state::ARTIST_TOP)
            .enumerate()
            .map(|(row, song)| {
                songs::track_row(
                    row,
                    song,
                    spot == Some(Spot::Row(row)),
                    playing.as_deref() == Some(song.id.as_str()),
                )
                .into_element()
            })
            .collect();
        let shelves: Vec<Element> = page
            .shelves
            .iter()
            .enumerate()
            .map(|(at, shelf)| shelf_row(at, shelf, spot, columns).into_element())
            .collect();

        // A finger scrolls through `TouchScroll`, which leaves sideways swipes to the rows.
        ui::touch_scroll::TouchScroll {
            scroll,
            content: ScrollView::new_controlled(scroll)
                // A phone scrolls under the finger, with no bar to drag.
                .show_scrollbar(!ui::compact())
                .drag_scrolling(false)
                .width(Size::fill())
                .height(Size::fill())
                .child(hero(&page, spot, light))
                .child(
                    rect()
                        .width(Size::fill())
                        .padding((0., ui::inset(), ui::inset(), ui::inset()))
                        .maybe(!top.is_empty(), |content| {
                            content
                                .child(section("Canciones más populares"))
                                .children(top)
                        })
                        .child(rect().height(Size::px(24.)))
                        .children(shelves),
                )
                .into_element(),
        }
        .into_element()
    }
}

/// The banner with the name, audience, description and the two buttons over its faded foot.
fn hero(page: &ArtistPage, spot: Option<Spot>, light: Option<Color>) -> impl IntoElement {
    let compact = ui::compact();
    let ring_light = light.unwrap_or(color::FOCUS_ON_PRIMARY);
    rect()
        .width(Size::fill())
        .height(Size::px(hero_height()))
        .map(page.banner.clone(), |hero, url| {
            hero.child(
                rect()
                    .position(Position::new_absolute().top(0.).left(0.))
                    .width(Size::fill())
                    .height(Size::px(hero_height()))
                    .child(Banner { url, light }),
            )
        })
        .child(
            // Freya stacks by depth too (each level adds one), and the fade sits a few levels
            // deep, so the text is lifted well clear of it.
            rect()
                .layer(Layer::Relative(8))
                .width(Size::fill())
                .height(Size::px(hero_height()))
                .padding((0., ui::inset(), 32., ui::inset()))
                .main_align(Alignment::End)
                .spacing(10.)
                .child(ui::eyebrow("Artista").color(color::ON_BACKDROP))
                .child(
                    ui::line(
                        page.name.clone(),
                        match compact {
                            true => text::DISPLAY * 1.3,
                            false => text::DISPLAY * 2.2,
                        },
                        color::FOREGROUND,
                    )
                    .font_weight(FontWeight::BOLD)
                    .width(Size::fill()),
                )
                .map(page.audience.clone(), |hero, audience| {
                    hero.child(
                        ui::line(audience, text::BODY, color::FOREGROUND)
                            .font_weight(FontWeight::MEDIUM),
                    )
                })
                .map(page.description.clone(), |hero, description| {
                    hero.child(
                        label()
                            .text(description)
                            .font_size(text::LABEL)
                            .color(color::ON_BACKDROP)
                            .max_lines(2)
                            .text_overflow(TextOverflow::Ellipsis)
                            .width(Size::percent(match compact {
                                true => 100.,
                                false => 60.,
                            })),
                    )
                })
                .child(
                    rect()
                        .direction(Direction::Horizontal)
                        .spacing(10.)
                        .margin((10., 0., 0., 0.))
                        .child(
                            ui::pill(
                                true,
                                Icon::PlayFilled,
                                Some("Reproducir"),
                                spot == Some(Spot::Action(0)),
                                ring_light,
                            )
                            .on_press(ui::tap(Target::Content(Spot::Action(0)))),
                        )
                        .child(
                            ui::pill(
                                false,
                                Icon::Shuffle,
                                Some("Aleatorio"),
                                spot == Some(Spot::Action(1)),
                                ring_light,
                            )
                            .on_press(ui::tap(Target::Content(Spot::Action(1)))),
                        ),
                ),
        )
}

/// One shelf of the page: its title and the covers around the D-pad's one.
fn shelf_row(at: usize, shelf: &Shelf, spot: Option<Spot>, columns: usize) -> impl IntoElement {
    let focused = match spot {
        Some(Spot::Cell(row, item)) if row == at => Some(item),
        _ => None,
    };
    let tiles =
        |skip: usize, take: usize| {
            rect()
                .direction(Direction::Horizontal)
                .spacing(ui::gap())
                .children(shelf.items.iter().enumerate().skip(skip).take(take).map(
                    |(index, item)| {
                        search::tile(
                            index,
                            item,
                            focused == Some(index),
                            Target::Content(Spot::Cell(at, index)),
                        )
                        .into_element()
                    },
                ))
        };
    let row = rect()
        .key(at)
        .width(Size::fill())
        .margin((0., 0., 16., 0.))
        .child(section(&shelf.title));
    // A phone scrolls the whole shelf sideways under a finger; the D-pad slides it instead.
    if ui::compact() {
        return row.child(
            ScrollView::new()
                .direction(Direction::Horizontal)
                .show_scrollbar(false)
                .width(Size::fill())
                .height(Size::px(cards::row()))
                .child(tiles(0, shelf.items.len())),
        );
    }
    let start = focused.map_or(0, |item| (item + 1).saturating_sub(columns));
    row.child(tiles(start, columns))
}

fn section(title: &str) -> impl IntoElement {
    rect()
        .height(Size::px(TITLE))
        .main_align(Alignment::Center)
        .child(ui::line(title, text::TITLE, color::FOREGROUND).font_weight(FontWeight::BOLD))
}

/// The artist's wide picture, filling the banner and cropped to it.
#[derive(PartialEq)]
struct Banner {
    url: String,
    light: Option<Color>,
}

impl Component for Banner {
    fn render(&self) -> impl IntoElement {
        let url = wide(&self.url);
        let reactive = use_reactive(&url);
        let mut loaded = use_state(|| images::cached(&url).map(|bytes| (url.clone(), bytes)));
        use_side_effect(move || {
            let url = reactive.read().clone();
            if let Some(bytes) = images::cached(&url) {
                loaded.set(Some((url, bytes)));
                return;
            }
            spawn(async move {
                if let Some(bytes) = images::fetch(url.clone()).await {
                    loaded.set(Some((url, bytes)));
                }
            });
        });
        let image: Option<(u64, Bytes)> = loaded
            .read()
            .clone()
            .filter(|(from, _)| *from == url)
            .map(|(from, bytes)| (images::key(&from), bytes));
        rect()
            .width(Size::fill())
            .height(Size::px(hero_height()))
            .map(image, |banner, image| {
                banner.child(
                    ImageViewer::new(ImageSource::from(image))
                        .width(Size::fill())
                        .height(Size::px(hero_height()))
                        .aspect_ratio(AspectRatio::Max)
                        .image_cover(ImageCover::Center),
                )
            })
            .child(
                rect()
                    .position(Position::new_absolute().top(0.).left(0.))
                    .width(Size::fill())
                    .height(Size::px(hero_height()))
                    .child(fade(self.light)),
            )
    }
}

/// `url` asked wide enough for a banner: Google's image host takes the size after the `=`.
fn wide(url: &str) -> String {
    match url.rsplit_once('=') {
        Some((base, _)) if url.contains("googleusercontent.com") || url.contains("ggpht.com") => {
            format!("{base}=w1440-h810-p-l90-rj")
        }
        _ => url.to_owned(),
    }
}

/// The banner's fade: a light tint over the picture, then dark toward the foot where the name
/// and buttons sit, ending in the page's own colour. Drawn with Skia, since Freya's gradient
/// fill draws nothing over an image here.
fn fade(light: Option<Color>) -> impl IntoElement {
    use skia_safe::gradient::{Colors, Gradient, Interpolation, shaders};
    use skia_safe::{Color4f, Paint, Point, Rect, TileMode};

    rect()
        .background(RenderCallback::new(move |context| {
            let (width, height) = (context.size.width, context.size.height);
            let shade = |alpha: f32| Color4f::new(0.035, 0.035, 0.035, alpha);
            // Halfway down, the dark takes on the photo's light before it closes into the page.
            let tinted = |alpha: f32| match light {
                Some(light) => {
                    let mixed = ui::tint::mix(color::BACKGROUND, light, 0.30);
                    Color4f::new(
                        mixed.r() as f32 / 255.,
                        mixed.g() as f32 / 255.,
                        mixed.b() as f32 / 255.,
                        alpha,
                    )
                }
                None => shade(alpha),
            };
            let colors = [shade(0.10), tinted(0.30), tinted(0.80), shade(1.)];
            let stops = [0., 0.45, 0.8, 1.];
            let gradient = Gradient::new(
                Colors::new(&colors, Some(&stops[..]), TileMode::Clamp, None),
                Interpolation::default(),
            );
            let Some(shader) = shaders::linear_gradient(
                (Point::new(0., 0.), Point::new(0., height)),
                &gradient,
                None,
            ) else {
                return;
            };
            let mut paint = Paint::default();
            paint.set_shader(shader);
            context
                .canvas
                .draw_rect(Rect::from_wh(width, height), &paint);
        }))
        .width(Size::fill())
        .height(Size::px(hero_height()))
}

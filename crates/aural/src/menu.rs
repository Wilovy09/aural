//! The menu a song opens on a right click or its ⋮, as in YouTube Music: start a mix from it,
//! play it next or add it to the queue, like it, save it to one of the account's playlists, or
//! copy its link.
//!
//! Drawn by Aural rather than Freya's context menu, so it always fits the window: it opens
//! from the pointer to the right and down, and to the left or up where there is no room, and
//! the playlists replace the options in the same panel, scrolling when there are many.

use std::cell::Cell;

use freya::prelude::*;

use crate::app;
use crate::library::{Collection, Song};
use crate::ui::{self, Icon, color, text};

/// The panel's width.
const WIDTH: f32 = 280.;
/// One option's height.
const ITEM: f32 = 40.;
/// The room around the options, inside the panel.
const PAD: f32 = 6.;
/// The least room kept between the panel and the window's edges.
const MARGIN: f32 = 8.;

/// The menu open: whose it is, where it was opened, and whether it lists the playlists.
#[derive(Clone, PartialEq)]
struct Shown {
    song: Song,
    at: (f32, f32),
    playlists: bool,
}

thread_local! {
    /// The open menu's state, held by [`MenuLayer`] once it is mounted.
    static SHOWN: Cell<Option<State<Option<Shown>>>> = const { Cell::new(None) };
}

/// Opens `song`'s menu at `at`, where the pointer pressed.
pub fn open(song: Song, at: (f32, f32)) {
    if let Some(mut shown) = SHOWN.get() {
        shown.set(Some(Shown {
            song,
            at,
            playlists: false,
        }));
    }
}

fn close() {
    if let Some(mut shown) = SHOWN.get() {
        shown.set(None);
    }
}

/// Where a press happened, in the window.
pub fn location(event: &PressEventData) -> (f32, f32) {
    let point = match event {
        PressEventData::Mouse(mouse) => mouse.global_location,
        PressEventData::Touch(touch) => touch.global_location,
        _ => return (0., 0.),
    };
    (point.x as f32, point.y as f32)
}

/// Draws the menu over everything while one is open. Mounted once, at the app's root.
#[derive(PartialEq)]
pub struct MenuLayer;

impl Component for MenuLayer {
    fn render(&self) -> impl IntoElement {
        let shown = use_state(|| None::<Shown>);
        use_hook(|| SHOWN.set(Some(shown)));
        let Some(open) = shown.read().clone() else {
            return rect().into_element();
        };

        let (liked, playlists) = app::menu_facts(&open.song.id);
        let items: Vec<Item> = match open.playlists {
            false => {
                let mut items = vec![
                    Item::new(Icon::Mix, "Comenzar mix", Act::Mix),
                    Item::new(Icon::PlayNext, "Reproducir a continuación", Act::PlayNext),
                    Item::new(Icon::Enqueue, "Agregar a la fila", Act::Enqueue),
                    match liked {
                        true => Item::new(Icon::HeartFilled, "Quitar de Me gusta", Act::Like),
                        false => Item::new(Icon::Heart, "Agregar a Me gusta", Act::Like),
                    },
                ];
                if !playlists.is_empty() {
                    items.push(Item::new(
                        Icon::AddToPlaylist,
                        "Guardar en una playlist",
                        Act::Playlists,
                    ));
                }
                items.push(Item::new(Icon::Share, "Compartir", Act::Share));
                items
            }
            true => std::iter::once(Item::new(Icon::Back, "Guardar en una playlist", Act::Back))
                .chain(playlists.into_iter().map(|playlist| Item {
                    glyph: None,
                    title: playlist.title.clone(),
                    act: Act::Save(playlist),
                }))
                .collect(),
        };

        // From the pointer to the right and down, or to the left and up where it would not fit.
        let (width, height) = ui::viewport();
        let tall = (items.len() as f32 * ITEM + PAD * 2.).min(height - MARGIN * 2.);
        let (x, y) = open.at;
        let left = match x + WIDTH > width - MARGIN {
            true => (x - WIDTH).max(MARGIN),
            false => x,
        };
        let top = match y + tall > height - MARGIN {
            true => (y - tall).max(MARGIN),
            false => y,
        };
        let song = open.song;

        // A press anywhere else closes the menu, and goes no further.
        rect()
            .layer(Layer::Overlay)
            .position(Position::new_global().left(0.).top(0.))
            .width(Size::px(width))
            .height(Size::px(height))
            .on_pointer_down(|event: Event<PointerEventData>| {
                event.stop_propagation();
                close();
            })
            .on_global_key_down(|event: Event<KeyboardEventData>| {
                if event.key == Key::Named(NamedKey::Escape) {
                    close();
                }
            })
            .child(
                rect()
                    .position(Position::new_global().left(left).top(top))
                    .width(Size::px(WIDTH))
                    .height(Size::px(tall))
                    .padding(PAD)
                    .corner_radius(10.)
                    .background(color::RAISED)
                    .border(Border::new().fill(color::BORDER).width(1.))
                    .shadow((0., 6., 18., 0., Color::from_argb(0x80, 0, 0, 0)))
                    .on_pointer_down(|event: Event<PointerEventData>| event.stop_propagation())
                    .child(
                        ScrollView::new()
                            .width(Size::fill())
                            .height(Size::fill())
                            .children(items.into_iter().map(|item| {
                                Choice {
                                    item,
                                    song: song.clone(),
                                }
                                .into_element()
                            })),
                    ),
            )
            .into_element()
    }
}

/// What an option does.
#[derive(Clone, PartialEq)]
enum Act {
    Mix,
    PlayNext,
    Enqueue,
    Like,
    Share,
    /// Shows the playlists in the panel.
    Playlists,
    /// Back from the playlists to the options.
    Back,
    Save(Collection),
}

#[derive(Clone, PartialEq)]
struct Item {
    glyph: Option<Icon>,
    title: String,
    act: Act,
}

impl Item {
    fn new(glyph: Icon, title: &str, act: Act) -> Self {
        Self {
            glyph: Some(glyph),
            title: title.to_owned(),
            act,
        }
    }
}

/// One option, lit under the pointer.
#[derive(PartialEq)]
struct Choice {
    item: Item,
    song: Song,
}

impl Component for Choice {
    fn render(&self) -> impl IntoElement {
        let mut hovered = use_state(|| false);
        let (act, song) = (self.item.act.clone(), self.song.clone());
        rect()
            .width(Size::fill())
            .height(Size::px(ITEM))
            .padding((0., 12.))
            .corner_radius(6.)
            .direction(Direction::Horizontal)
            .cross_align(Alignment::Center)
            .spacing(12.)
            .background(match *hovered.read() {
                true => color::HOVER,
                false => Color::TRANSPARENT,
            })
            .on_pointer_enter(move |_| hovered.set(true))
            .on_pointer_leave(move |_| hovered.set(false))
            .on_press(move |event: Event<PressEventData>| {
                event.stop_propagation();
                run(act.clone(), song.clone());
            })
            .maybe_child(
                self.item
                    .glyph
                    .map(|glyph| ui::icon(glyph, 18., color::FOREGROUND)),
            )
            .child(
                ui::line(self.item.title.clone(), text::BODY, color::FOREGROUND)
                    .width(Size::flex(1.)),
            )
    }
}

/// Does what `act` says for `song`. Every option closes the menu but the two that change
/// what it lists.
fn run(act: Act, song: Song) {
    let lists = |playlists: bool| {
        if let Some(mut shown) = SHOWN.get()
            && let Some(open) = shown.write().as_mut()
        {
            open.playlists = playlists;
        }
    };
    match act {
        Act::Playlists => return lists(true),
        Act::Back => return lists(false),
        _ => close(),
    }
    match act {
        Act::Mix => app::start_mix(song),
        Act::PlayNext => app::play_next(song),
        Act::Enqueue => app::enqueue(song),
        Act::Like => app::like(song),
        Act::Share => app::share(&song),
        Act::Save(playlist) => app::add_to_playlist(song, playlist),
        Act::Playlists | Act::Back => {}
    }
}

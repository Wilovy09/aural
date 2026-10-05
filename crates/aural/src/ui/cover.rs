//! A cover: Apple Music's when the catalog has it, else the url's (YouTube's), on a muted
//! square with a music glyph until an image arrives.

use bytes::Bytes;
use freya::prelude::*;

use super::{Icon, color, icon};
use crate::{images, library};

/// A square cover of `side` px with `radius` corners, fetched at `edge` px.
#[derive(Clone, PartialEq)]
pub struct Cover {
    pub url: Option<String>,
    pub side: f32,
    pub radius: f32,
    pub edge: u32,
    /// What to ask Apple's catalog for; its cover replaces the url's once found.
    pub apple: Option<crate::artwork::Wanted>,
}

impl Cover {
    pub fn new(url: Option<String>, side: f32, radius: f32) -> Self {
        Self {
            url,
            side,
            radius,
            edge: library::THUMB_EDGE,
            apple: None,
        }
    }

    /// Prefer Apple Music's cover for `wanted`, when there is something to ask for.
    pub fn maybe_apple(mut self, wanted: Option<crate::artwork::Wanted>) -> Self {
        self.apple = wanted;
        self
    }

    /// Prefer Apple Music's cover for `wanted`.
    pub fn apple(mut self, wanted: crate::artwork::Wanted) -> Self {
        self.apple = Some(wanted);
        self
    }

    /// Fetch at `edge` px instead of the thumbnail size.
    pub fn edge(mut self, edge: u32) -> Self {
        self.edge = edge;
        self
    }
}

impl Component for Cover {
    fn render(&self) -> impl IntoElement {
        let edge = self.edge;
        // Apple's cover: known already, found while this shows, or absent.
        let wanted = use_reactive(&self.apple);
        let mut apple = use_state(|| {
            self.apple
                .as_ref()
                .and_then(|wanted| crate::artwork::cached(wanted, edge).flatten())
        });
        use_side_effect(move || {
            let Some(asked) = wanted.read().clone() else {
                apple.set(None);
                return;
            };
            match crate::artwork::cached(&asked, edge) {
                Some(answer) => apple.set(answer),
                None => {
                    apple.set(None);
                    spawn(async move {
                        let found = crate::artwork::find(asked.clone(), edge).await;
                        if wanted.peek().as_ref() == Some(&asked) {
                            apple.set(found);
                        }
                    });
                }
            }
        });
        let url = apple.read().clone().or_else(|| {
            self.url
                .as_deref()
                .map(|url| library::sized(url, self.edge))
        });
        let reactive = use_reactive(&url);
        // The bytes are kept with the url they came from: a render between a url change and
        // the effect below must not pair the new url's cache key with the old image.
        let mut loaded = use_state(|| {
            url.clone()
                .and_then(|url| images::cached(&url).map(|bytes| (url, bytes)))
        });

        use_side_effect(move || {
            let Some(url) = reactive.read().clone() else {
                loaded.set(None);
                return;
            };
            if let Some(bytes) = images::cached(&url) {
                loaded.set(Some((url, bytes)));
                return;
            }
            spawn(async move {
                let fetched = images::fetch(url.clone()).await;
                if reactive.peek().as_deref() == Some(url.as_str()) {
                    loaded.set(fetched.map(|bytes| (url, bytes)));
                }
            });
        });

        let side = self.side;
        let image: Option<(u64, Bytes)> = match (&url, loaded.read().clone()) {
            (Some(url), Some((from, bytes))) if *url == from => Some((images::key(url), bytes)),
            _ => None,
        };
        let waiting = image.is_none();
        rect()
            .width(Size::px(side))
            .height(Size::px(side))
            .corner_radius(self.radius)
            .background(color::MUTED)
            .center()
            .map(image, |frame, image| {
                frame.child(
                    ImageViewer::new(ImageSource::from(image))
                        .width(Size::px(side))
                        .height(Size::px(side))
                        .aspect_ratio(AspectRatio::Max)
                        .image_cover(ImageCover::Center)
                        .corner_radius(self.radius),
                )
            })
            .maybe(waiting, |frame| {
                frame.child(icon(Icon::Music, side * 0.4, color::MUTED_FOREGROUND))
            })
    }
}

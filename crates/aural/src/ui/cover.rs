//! A cover from a url: a muted square with a music glyph until the image arrives.

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
}

impl Cover {
    pub fn new(url: Option<String>, side: f32, radius: f32) -> Self {
        Self {
            url,
            side,
            radius,
            edge: library::THUMB_EDGE,
        }
    }

    /// Fetch at `edge` px instead of the thumbnail size.
    pub fn edge(mut self, edge: u32) -> Self {
        self.edge = edge;
        self
    }
}

impl Component for Cover {
    fn render(&self) -> impl IntoElement {
        let url = self
            .url
            .as_deref()
            .map(|url| library::sized(url, self.edge));
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

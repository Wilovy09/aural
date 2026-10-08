//! The light a cover casts: its dominant colour, lifted to glow on near black the way a TV's
//! light shows on the wall behind it. Pages tint their headers with their own artwork's, and the
//! shell glows with the song playing's.

use std::collections::HashMap;
use std::sync::Mutex;

use freya::engine::prelude::{SkData, SkImage, raster_n32_premul};
use freya::prelude::*;

use crate::images;

/// The side the cover is shrunk to before it is read.
const SAMPLE: i32 = 12;

static KNOWN: Mutex<Option<HashMap<String, Color>>> = Mutex::new(None);

/// The light of the image at `url`, once worked out.
pub fn cached(url: &str) -> Option<Color> {
    KNOWN.lock().ok()?.as_ref()?.get(url).copied()
}

/// Fetches the image at `url` and works out its light.
pub async fn of(url: String) -> Option<Color> {
    if let Some(known) = cached(&url) {
        return Some(known);
    }
    let bytes = images::fetch(url.clone()).await?;
    let light = from_bytes(&bytes)?;
    if let Ok(mut known) = KNOWN.lock() {
        known.get_or_insert_with(HashMap::new).insert(url, light);
    }
    Some(light)
}

/// The light of an encoded image: the average of its pixels weighted towards the vivid ones,
/// then set to one brightness, so a dark cover still glows and a pale one does not glare.
pub fn from_bytes(bytes: &[u8]) -> Option<Color> {
    let image = SkImage::from_encoded(SkData::new_copy(bytes))?;
    let mut surface = raster_n32_premul((SAMPLE, SAMPLE))?;
    surface.canvas().draw_image_rect(
        &image,
        None,
        freya::engine::prelude::SkRect::from_wh(SAMPLE as f32, SAMPLE as f32),
        &freya::engine::prelude::Paint::default(),
    );
    let info = surface.image_info();
    let mut pixels = vec![0u8; (SAMPLE * SAMPLE * 4) as usize];
    if !surface.read_pixels(&info, &mut pixels, (SAMPLE * 4) as usize, (0, 0)) {
        return None;
    }
    let (mut red, mut green, mut blue, mut weight) = (0f32, 0f32, 0f32, 0f32);
    for pixel in pixels.chunks_exact(4) {
        // Skia's N32 is BGRA on the platforms Aural runs on.
        let (b, g, r) = (pixel[0] as f32, pixel[1] as f32, pixel[2] as f32);
        let high = r.max(g).max(b);
        let low = r.min(g).min(b);
        let saturation = if high == 0. { 0. } else { (high - low) / high };
        let vividness = 0.05 + saturation * (high / 255.);
        red += r * vividness;
        green += g * vividness;
        blue += b * vividness;
        weight += vividness;
    }
    if weight == 0. {
        return None;
    }
    Some(glow(red / weight, green / weight, blue / weight))
}

/// `(r, g, b)` at one brightness and with a floor of colour, so every light reads as light.
fn glow(r: f32, g: f32, b: f32) -> Color {
    let high = r.max(g).max(b).max(1.);
    let low = r.min(g).min(b);
    let saturation = (high - low) / high;
    // A grey cover gives a neutral, dim light rather than a muddy tint.
    if saturation < 0.12 {
        return super::color::LIGHT;
    }
    let scale = 200. / high;
    let lift = |channel: f32| {
        // Push away from grey a little, then to the common brightness.
        let mean = (r + g + b) / 3.;
        ((mean + (channel - mean) * 1.25) * scale).clamp(0., 255.) as u8
    };
    Color::from_rgb(lift(r), lift(g), lift(b))
}

/// `base` with `share` (0 to 1) of `light` mixed in, opaque.
pub fn mix(base: Color, light: Color, share: f32) -> Color {
    let blend = |a: u8, b: u8| (a as f32 + (b as f32 - a as f32) * share).round() as u8;
    Color::from_rgb(
        blend(base.r(), light.r()),
        blend(base.g(), light.g()),
        blend(base.b(), light.b()),
    )
}

/// `light` at `alpha` (0 to 1).
pub fn alpha(light: Color, alpha: f32) -> Color {
    Color::from_argb(
        (alpha.clamp(0., 1.) * 255.) as u8,
        light.r(),
        light.g(),
        light.b(),
    )
}

/// The light of the image at `url`, worked out in the background. `None` until it is.
pub fn use_tint(url: Option<String>) -> Option<Color> {
    let wanted = use_reactive(&url);
    let mut light = use_state(|| url.as_deref().and_then(cached));
    use_side_effect(move || {
        let Some(url) = wanted.read().clone() else {
            light.set(None);
            return;
        };
        if let Some(known) = cached(&url) {
            light.set(Some(known));
            return;
        }
        spawn(async move {
            let found = of(url.clone()).await;
            if wanted.peek().as_deref() == Some(url.as_str()) {
                light.set(found);
            }
        });
    });
    *light.read()
}

/// A gradient from `light` at `strength` down to nothing over the element's height: the glow a
/// page's header or the shell's top edge carries.
pub fn wash(light: Color, strength: f32) -> impl IntoElement {
    // A colourless cover lights the room only faintly: grey on black reads as a slab.
    let strength = match light == super::color::LIGHT {
        true => strength * 0.35,
        false => strength,
    };
    use skia_safe::gradient::{Colors, Gradient, Interpolation, shaders};
    use skia_safe::{Color4f, Paint, Point, Rect, TileMode};

    rect()
        .background(RenderCallback::new(move |context| {
            let (width, height) = (context.size.width, context.size.height);
            let tone = |alpha: f32| {
                Color4f::new(
                    light.r() as f32 / 255.,
                    light.g() as f32 / 255.,
                    light.b() as f32 / 255.,
                    alpha,
                )
            };
            let colors = [tone(strength), tone(strength * 0.45), tone(0.)];
            let stops = [0., 0.45, 1.];
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
        .height(Size::fill())
}

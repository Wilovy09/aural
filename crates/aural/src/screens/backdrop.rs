//! The fullscreen player's backdrop, after kawarp (better-lyrics/kawarp): the cover blurred
//! into soft colour, slowly warped with simplex noise, with kawarp's vignette and saturation.
//!
//! kawarp runs its warp per pixel on the GPU. A TV's GPU (and a 32-bit TV's CPU) cannot afford
//! that 30 times a second, and does not need to: the noise moves slowly and the image is a
//! blur, so the warp is sampled on a coarse grid instead. Every frame:
//!
//! - kawarp's warp (the same two octaves of simplex noise) is evaluated at the corners of a
//!   16×9 mesh on the CPU, a few hundred noise calls;
//! - the GPU draws the blurred cover through that mesh, each corner pulling the texture to its
//!   warped spot, with the vignette and darkening carried as corner colours and kawarp's
//!   saturation as a colour filter.
//!
//! The cover is shrunk and blurred once per song, standing in for kawarp's Kawase passes, and
//! a new cover fades in over the old one for a second.

use std::time::{Duration, Instant};

use freya::animation::{AnimNum, OnCreation, OnFinish, use_animation};
use freya::engine::prelude::{
    BlendMode, FilterMode, MipmapMode, Paint, SamplingOptions, SkData, SkImage, SkRect, TileMode,
    blur, raster_n32_premul,
};
use freya::prelude::*;
use freya::radio::use_radio;
use skia_safe::vertices::VertexMode;
use skia_safe::{ColorMatrix, Point, Vertices, color_filters};

use crate::library;
use crate::state::{AppState, Channel};
use crate::{images, ui};

/// The side the cover is blurred at, kawarp's `BLUR_SIZE`.
const BLUR_SIZE: i32 = 128;
/// How blurred, standing in for kawarp's eight Kawase passes at this size.
const BLUR_SIGMA: f32 = 14.;
/// Cells of the warp mesh across and down.
const GRID: (usize, usize) = (16, 9);
/// How often the backdrop moves. The warp is slow, so this reads as smooth motion.
const FPS: f32 = 30.;
/// How long a new cover takes to fade in.
const FADE: Duration = Duration::from_millis(1000);
/// kawarp's defaults.
const INTENSITY: f32 = 1.0;
const SATURATION: f32 = 1.5;
/// How much the backdrop is darkened under the player's text.
const SHADE: f32 = 0.5;

/// The animated backdrop of the song playing now, filling the window.
#[derive(PartialEq)]
pub struct Backdrop;

impl Component for Backdrop {
    fn render(&self) -> impl IntoElement {
        let now = use_radio::<AppState, Channel>(Channel::Now);
        let cover = now
            .read()
            .now
            .song
            .as_ref()
            .and_then(|song| song.cover.clone())
            .map(|url| library::sized(&url, library::THUMB_EDGE));

        let started = use_hook(Instant::now);
        // The blurred cover now, the one fading out, and when the fade started.
        let mut bases = use_state(|| (None::<SkImage>, None::<SkImage>, Instant::now()));
        let wanted = use_reactive(&cover);
        use_side_effect(move || {
            let Some(url) = wanted.read().clone() else {
                return;
            };
            spawn(async move {
                let Some(bytes) = images::fetch(url.clone()).await else {
                    return;
                };
                if wanted.peek().as_deref() != Some(url.as_str()) {
                    return;
                }
                if let Some(blurred) = blurred(&bytes) {
                    let (current, _, _) = bases.peek().clone();
                    bases.set((Some(blurred), current, Instant::now()));
                }
            });
        });

        // A looping animation used only as a frame clock; the canvas changes FPS times a second.
        let clock = use_animation(|config| {
            config.on_creation(OnCreation::Run);
            config.on_finish(OnFinish::restart());
            AnimNum::new(0., 1.).time(1000)
        });
        let _ = clock.get();
        let frame = (started.elapsed().as_secs_f32() * FPS) as u64;
        let time = frame as f32 / FPS;

        let (current, previous, since) = bases.read().clone();
        let fade = (since.elapsed().as_secs_f32() / FADE.as_secs_f32()).min(1.);
        let (width, height) = ui::viewport();

        rect()
            .width(Size::px(width))
            .height(Size::px(height))
            .background(ui::color::BACKGROUND)
            .child(
                canvas(RenderCallback::new(move |context: &mut CanvasContext| {
                    let size = (context.size.width, context.size.height);
                    let layers = [(previous.as_ref(), 1.), (current.as_ref(), fade)];
                    for (base, alpha) in layers {
                        if let Some(base) = base
                            && alpha > 0.
                        {
                            draw(context.canvas, base, size, time, alpha);
                        }
                    }
                }))
                .key(frame)
                .expanded(),
            )
    }
}

/// The cover shrunk to `BLUR_SIZE` and blurred, ready to be warped every frame.
fn blurred(bytes: &[u8]) -> Option<SkImage> {
    let cover = SkImage::from_encoded(SkData::new_copy(bytes))?;
    let mut surface = raster_n32_premul((BLUR_SIZE, BLUR_SIZE))?;
    let mut paint = Paint::default();
    paint.set_image_filter(blur((BLUR_SIGMA, BLUR_SIGMA), TileMode::Clamp, None, None));
    let side = BLUR_SIZE as f32;
    surface.canvas().draw_image_rect_with_sampling_options(
        &cover,
        None,
        SkRect::from_wh(side, side),
        SamplingOptions::new(FilterMode::Linear, MipmapMode::Linear),
        &paint,
    );
    let image = surface.image_snapshot();
    image.make_raster_image(None, None).or(Some(image))
}

/// Draws `base` warped over `size` at `time`, at `alpha` opacity.
fn draw(canvas: &skia_safe::Canvas, base: &SkImage, size: (f32, f32), time: f32, alpha: f32) {
    let (columns, rows) = GRID;
    let image = (base.width() as f32, base.height() as f32);
    let t = time * 0.05;
    let corners = (columns + 1) * (rows + 1);
    let mut positions = Vec::with_capacity(corners);
    let mut textures = Vec::with_capacity(corners);
    let mut colors = Vec::with_capacity(corners);
    for row in 0..=rows {
        for column in 0..=columns {
            let (u, v) = (column as f32 / columns as f32, row as f32 / rows as f32);
            let (dx, dy) = warp(u, v, t);
            let (wu, wv) = (
                (u + dx * INTENSITY).clamp(0., 1.),
                (v + dy * INTENSITY).clamp(0., 1.),
            );
            positions.push(Point::new(u * size.0, v * size.1));
            textures.push(Point::new(wu * image.0, wv * image.1));
            // kawarp's vignette and the darkening, as a grey the texture is multiplied by.
            let (cu, cv) = (u - 0.5, v - 0.5);
            let light = SHADE * (1. - (cu * cu + cv * cv) * 0.3);
            let level = (light.clamp(0., 1.) * 255.) as u8;
            colors.push(skia_safe::Color::from_argb(
                (alpha * 255.) as u8,
                level,
                level,
                level,
            ));
        }
    }
    let mut indices = Vec::with_capacity(columns * rows * 6);
    for row in 0..rows {
        for column in 0..columns {
            let corner = (row * (columns + 1) + column) as u16;
            let below = corner + (columns + 1) as u16;
            indices.extend_from_slice(&[corner, corner + 1, below, corner + 1, below + 1, below]);
        }
    }
    let vertices = Vertices::new_copy(
        VertexMode::Triangles,
        &positions,
        &textures,
        &colors,
        Some(&indices),
    );
    let Some(shader) = base.to_shader(
        (TileMode::Clamp, TileMode::Clamp),
        SamplingOptions::new(FilterMode::Linear, MipmapMode::None),
        None,
    ) else {
        return;
    };
    let mut saturation = ColorMatrix::default();
    saturation.set_saturation(SATURATION);
    let mut paint = Paint::default();
    paint.set_shader(shader);
    paint.set_color_filter(color_filters::matrix(&saturation, None));
    canvas.draw_vertices(&vertices, BlendMode::Modulate, &paint);
}

/// kawarp's domain warp at `(u, v)`: two octaves of simplex noise, strongest in the middle.
fn warp(u: f32, v: f32, t: f32) -> (f32, f32) {
    let (cu, cv) = (u - 0.5, v - 0.5);
    let center = 1. - smoothstep(0., 0.7, (cu * cu + cv * cv).sqrt());
    let n1 = simplex(u * 0.35 + t, v * 0.35 + t * 0.7);
    let n2 = simplex(u * 0.35 - t * 0.8 + 50., v * 0.35 + t * 0.5 + 50.);
    let n3 = simplex(u * 0.9 + t * 1.2 + 100., v * 0.9 - t);
    let n4 = simplex(u * 0.9 - t, v * 0.9 + t * 1.1 + 100.);
    (
        (n1 * 0.65 + n3 * 0.35) * center,
        (n2 * 0.65 + n4 * 0.35) * center,
    )
}

fn smoothstep(edge0: f32, edge1: f32, x: f32) -> f32 {
    let t = ((x - edge0) / (edge1 - edge0)).clamp(0., 1.);
    t * t * (3. - 2. * t)
}

/// 2D simplex noise in [-1, 1], the same Ashima/McEwan formula kawarp's shader uses.
fn simplex(x: f32, y: f32) -> f32 {
    const C: [f32; 4] = [0.211_324_87, 0.366_025_4, -0.577_350_26, 0.024_390_243];
    let fract = |value: f32| value - value.floor();
    let mod289 = |value: f32| value - (value / 289.).floor() * 289.;
    let permute = |value: f32| mod289((value * 34. + 1.) * value);

    let skew = (x + y) * C[1];
    let (ix, iy) = ((x + skew).floor(), (y + skew).floor());
    let unskew = (ix + iy) * C[0];
    let (x0, y0) = (x - ix + unskew, y - iy + unskew);
    let (i1x, i1y) = match x0 > y0 {
        true => (1., 0.),
        false => (0., 1.),
    };
    let (x1, y1) = (x0 + C[0] - i1x, y0 + C[0] - i1y);
    let (x2, y2) = (x0 + C[2], y0 + C[2]);
    let (ix, iy) = (mod289(ix), mod289(iy));
    let hashes = [
        permute(permute(iy) + ix),
        permute(permute(iy + i1y) + ix + i1x),
        permute(permute(iy + 1.) + ix + 1.),
    ];
    let offsets = [(x0, y0), (x1, y1), (x2, y2)];
    let mut total = 0.;
    for (hash, (cx, cy)) in hashes.into_iter().zip(offsets) {
        let mut m = (0.5 - (cx * cx + cy * cy)).max(0.);
        m *= m;
        m *= m;
        let gx = 2. * fract(hash * C[3]) - 1.;
        let h = gx.abs() - 0.5;
        let a0 = gx - (gx + 0.5).floor();
        m *= 1.792_842_9 - 0.853_734_7 * (a0 * a0 + h * h);
        total += m * (a0 * cx + h * cy);
    }
    130. * total
}

//! The bars that dance beside the song playing, from `assets/icons/music_spectrum.svg`. That
//! file animates with SMIL, which Skia draws as a still frame, so its keyframes are played here
//! instead: five bars that rise and fall, each with a cap that drops onto it, every second.

use freya::animation::{AnimNum, OnCreation, OnFinish, use_animation};
use freya::prelude::*;

/// One bar: its x, its full height, and how tall it is over the second, as scale keyframes.
struct Bar {
    x: f32,
    height: f32,
    scale: &'static [(f32, f32)],
    /// Where its cap sits over the second, as y keyframes.
    cap: &'static [(f32, f32)],
}

/// The SVG's keyframes, as `(time, value)` over one second, its coordinates unchanged.
const BARS: [Bar; 5] = [
    Bar {
        x: 180.,
        height: 43.,
        scale: &[(0., 1.), (0.52, 0.023), (0.96, 1.), (1., 1.)],
        cap: &[(0., 129.), (0.16, 129.), (0.56, 165.), (0.96, 129.), (1., 129.)],
    },
    Bar {
        x: 190.,
        height: 26.,
        scale: &[(0., 1.), (0.28, 1.65), (0.8, 0.05), (0.96, 0.86), (1., 0.86)],
        cap: &[
            (0., 146.),
            (0.28, 129.),
            (0.44, 129.),
            (0.8, 171.),
            (0.96, 146.),
            (1., 146.),
        ],
    },
    Bar {
        x: 200.,
        height: 34.,
        scale: &[(0., 1.), (0.32, 0.033), (0.8, 1.263), (0.96, 1.013), (1., 1.013)],
        cap: &[(0., 129.), (0.36, 165.), (0.8, 129.), (1., 129.)],
    },
    Bar {
        x: 210.,
        height: 21.,
        scale: &[(0., 0.759), (0.28, 2.069), (0.8, 0.039), (0.96, 0.603), (1., 0.603)],
        cap: &[
            (0., 156.),
            (0.24, 129.),
            (0.4, 129.),
            (0.8, 170.),
            (0.96, 160.),
            (1., 160.),
        ],
    },
    Bar {
        x: 220.,
        height: 9.,
        scale: &[(0., 2.556), (0.2, 0.096), (0.72, 4.776), (0.96, 2.58), (1., 2.58)],
        cap: &[
            (0., 149.),
            (0.2, 165.),
            (0.72, 129.),
            (0.84, 129.),
            (0.96, 140.),
            (1., 140.),
        ],
    },
];

/// The bars stand on this line of the SVG.
const FLOOR: f32 = 174.;
/// The part of the SVG the bars move in.
const LEFT: f32 = 178.;
const TOP: f32 = 125.;
const WIDTH: f32 = 44.;
const HEIGHT: f32 = 50.;
/// A bar's and a cap's size in the SVG.
const BAR: f32 = 4.;
const CAP: f32 = 3.;

/// `keys` at `time`, between the two keyframes around it (the SVG's splines are linear).
fn at(keys: &[(f32, f32)], time: f32) -> f32 {
    for pair in keys.windows(2) {
        let ((from, a), (to, b)) = (pair[0], pair[1]);
        if time <= to {
            let share = if to > from { (time - from) / (to - from) } else { 1. };
            return a + (b - a) * share.clamp(0., 1.);
        }
    }
    keys.last().map_or(0., |key| key.1)
}

/// The spectrum `size` px square in `tint`, moving while `playing`, still otherwise.
#[derive(PartialEq)]
pub struct Spectrum {
    pub size: f32,
    pub tint: Color,
    pub playing: bool,
}

impl Component for Spectrum {
    fn render(&self) -> impl IntoElement {
        let clock = use_animation(|config| {
            config.on_creation(OnCreation::Run);
            config.on_finish(OnFinish::restart());
            AnimNum::new(0., 1.).time(1000)
        });
        let time = match self.playing {
            true => clock.get().value(),
            false => 0.,
        };
        let tint = self.tint;
        canvas(RenderCallback::new(move |context: &mut CanvasContext| {
            use skia_safe::{Paint, Rect};
            let side = context.size.width.min(context.size.height);
            let scale = side / WIDTH.max(HEIGHT);
            // The canvas draws from its own top-left corner.
            let (left, top) = (0., 0.);
            let place = |x: f32, y: f32| {
                (
                    left + (x - LEFT) * scale + (side - WIDTH * scale) / 2.,
                    top + (y - TOP) * scale,
                )
            };
            let mut paint = Paint::default();
            paint.set_anti_alias(true);
            paint.set_color(skia_safe::Color::from_argb(
                tint.a(),
                tint.r(),
                tint.g(),
                tint.b(),
            ));
            for bar in &BARS {
                let tall = bar.height * at(bar.scale, time);
                let (x0, y0) = place(bar.x - BAR / 2., FLOOR - tall);
                let (x1, y1) = place(bar.x + BAR / 2., FLOOR);
                context
                    .canvas
                    .draw_rect(Rect::new(x0, y0, x1, y1), &paint);
                let cap = at(bar.cap, time);
                let (x0, y0) = place(bar.x - BAR / 2., cap - CAP);
                let (x1, y1) = place(bar.x + BAR / 2., cap);
                context
                    .canvas
                    .draw_rect(Rect::new(x0, y0, x1, y1), &paint);
            }
        }))
        // A new key every frame, so the canvas is drawn again as the clock moves.
        .key((time * 1000.) as u64)
        .width(Size::px(self.size))
        .height(Size::px(self.size))
    }
}

#[cfg(test)]
mod tests {
    use super::{BARS, at};

    #[test]
    fn keyframes_interpolate() {
        let keys = [(0., 1.), (0.5, 0.), (1., 1.)];
        assert_eq!(at(&keys, 0.), 1.);
        assert_eq!(at(&keys, 0.25), 0.5);
        assert_eq!(at(&keys, 0.5), 0.);
        assert_eq!(at(&keys, 1.), 1.);
    }

    /// Every bar starts and ends the second at the same height, so the loop does not jump.
    #[test]
    fn the_loop_is_seamless_where_the_svg_is() {
        for bar in &BARS {
            assert!(at(bar.scale, 0.96) == at(bar.scale, 1.));
            assert!(at(bar.cap, 0.96) == at(bar.cap, 1.));
        }
    }
}

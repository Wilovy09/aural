//! A row that can be swiped to the left to act on it, as Spotify's rows add a song to the
//! queue. The row slides under the finger (or the mouse) and uncovers the action on its right;
//! let go far enough and the action happens, otherwise the row springs back. Only a gesture
//! that starts out sideways is a swipe, so the list still scrolls under a finger moving up or
//! down.

use std::cell::Cell;
use std::time::{Duration, Instant};

use freya::animation::{AnimNum, Ease, Function, use_animation_transition};
use freya::prelude::*;

use super::{Icon, color, icon, line, metrics, text};

/// How far the row must slide for the action to happen.
const TRIGGER: f32 = 80.;
/// How wide the action beside the row is.
const ACTION: f32 = 240.;
/// How far the pointer moves before the gesture counts as sideways or as scrolling.
const SLOP: f64 = 10.;
/// How long after a swipe a tap is still taken as part of it.
const AFTER: Duration = Duration::from_millis(350);

thread_local! {
    /// When the last swipe moved or ended.
    static SWIPED: Cell<Option<Instant>> = const { Cell::new(None) };
}

/// Whether a swipe just happened, so the tap that ends it does not also press the row.
pub fn just_swiped() -> bool {
    SWIPED.get().is_some_and(|at| at.elapsed() < AFTER)
}

pub(crate) fn mark() {
    SWIPED.set(Some(Instant::now()));
}

/// Where a gesture on the row stands.
#[derive(Clone, Copy, PartialEq)]
enum Gesture {
    Idle,
    /// Pressed at this point; not yet known to be a swipe.
    Pressing(f64, f64),
    /// Sliding sideways, this far.
    Swiping(f64, f32),
}

/// `content`, swipeable to the left to do `on_swipe`, which `glyph` and `label` show in `tint`.
#[derive(PartialEq)]
pub struct Swipe {
    pub content: Element,
    pub glyph: Icon,
    pub label: &'static str,
    pub tint: Color,
    /// What the row sits on while it slides, so the action does not show through it.
    pub surface: Color,
    pub height: f32,
    pub on_swipe: EventHandler<()>,
    pub key: DiffKey,
}

impl KeyExt for Swipe {
    fn write_key(&mut self) -> &mut DiffKey {
        &mut self.key
    }
}

impl Component for Swipe {
    fn render(&self) -> impl IntoElement {
        let mut gesture = use_state(|| Gesture::Idle);
        let on_swipe = self.on_swipe.clone();
        let pulled = match *gesture.read() {
            Gesture::Swiping(_, pulled) => pulled,
            _ => 0.,
        };
        // Letting go springs the row back instead of snapping it.
        let shown = use_animation_transition(use_reactive(&pulled), |from: f32, to: f32| {
            AnimNum::new(from, to)
                .time(match to == 0. {
                    true => 220,
                    false => 0,
                })
                .function(Function::Expo)
                .ease(Ease::Out)
        });
        let offset = shown.get().value().clamp(0., ACTION);
        let ready = offset >= TRIGGER;

        rect()
            .width(Size::fill())
            .height(Size::px(self.height))
            .overflow(Overflow::Clip)
            .corner_radius(metrics::RADIUS_SM)
            .on_pointer_down(move |event: Event<PointerEventData>| {
                let at = event.global_location();
                gesture.set(Gesture::Pressing(at.x, at.y));
            })
            .on_global_pointer_move(move |event: Event<PointerEventData>| {
                let at = event.global_location();
                // Read first: the borrow must end before the gesture is written.
                let now = *gesture.peek();
                match now {
                    Gesture::Pressing(x, y) => {
                        let (dx, dy) = (at.x - x, at.y - y);
                        if dx < -SLOP && dx.abs() > dy.abs() {
                            gesture.set(Gesture::Swiping(x, 0.));
                            mark();
                        } else if dy.abs() > SLOP || dx > SLOP {
                            // Scrolling, or a swipe the other way: not this row's gesture.
                            gesture.set(Gesture::Idle);
                        }
                    }
                    Gesture::Swiping(x, _) => {
                        gesture.set(Gesture::Swiping(x, (x - at.x - SLOP).max(0.) as f32));
                        mark();
                    }
                    Gesture::Idle => {}
                }
            })
            .on_global_pointer_up(move |_: Event<PointerEventData>| {
                let ended = *gesture.peek();
                if let Gesture::Swiping(_, pulled) = ended {
                    mark();
                    if pulled >= TRIGGER {
                        on_swipe.call(());
                    }
                }
                if ended != Gesture::Idle {
                    gesture.set(Gesture::Idle);
                }
            })
            // The row and the action beside it slide left together, so the action shows as
            // the row uncovers it; it lights up once letting go would do it.
            .child(
                // As wide as the row plus the action, laid out by Freya from the room it has:
                // nothing is measured, so a recycled row never shows a stale width.
                rect()
                    .width(Size::func(|context| Some(context.parent + ACTION)))
                    .height(Size::px(self.height))
                    .direction(Direction::Horizontal)
                    .offset_x(-offset)
                    .child(
                        rect()
                            .width(Size::func(|context| Some(context.parent - ACTION)))
                            .height(Size::px(self.height))
                            .background(match offset > 0. {
                                true => self.surface,
                                false => Color::TRANSPARENT,
                            })
                            .child(self.content.clone()),
                    )
                    // The action exists only while the row slides.
                    .maybe_child((offset > 0.).then(|| {
                        rect()
                            .width(Size::px(ACTION))
                            .height(Size::px(self.height))
                            .background(match ready {
                                true => self.tint,
                                false => super::tint::alpha(self.tint, 0.35),
                            })
                            .padding((0., 16.))
                            .direction(Direction::Horizontal)
                            .cross_align(Alignment::Center)
                            .spacing(10.)
                            .child(icon(self.glyph, 18., color::FOREGROUND))
                            .child(
                                line(self.label, text::LABEL, color::FOREGROUND)
                                    .font_weight(FontWeight::SEMI_BOLD),
                            )
                    })),
            )
    }

    fn render_key(&self) -> DiffKey {
        self.key.clone().or(self.default_key())
    }
}

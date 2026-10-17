//! A line of text that scrolls when it does not fit: it shows its start for a moment, then
//! slides from right to left until its start comes round again, and loops. Text that fits
//! stays still. Its edges fade while it moves, so the letters melt in and out.

use std::time::Duration;

use freya::animation::{AnimNum, Function, OnCreation, OnFinish, use_animation_with_dependencies};
use freya::prelude::*;

/// How fast the text slides, in logical pixels a second.
const SPEED: f32 = 36.;
/// How long the start shows before each pass.
const PAUSE: Duration = Duration::from_millis(2500);
/// Room enough to lay a line out at its natural width.
const ROOM: f32 = 10_000.;
/// How wide the fade at each edge is.
const EDGE: f32 = 18.;

#[derive(PartialEq)]
pub struct Marquee {
    pub text: String,
    pub size: f32,
    pub color: Color,
    pub weight: FontWeight,
    /// Centred when it fits, rather than at the start.
    pub centered: bool,
}

impl Marquee {
    pub fn new(text: impl Into<String>, size: f32, color: Color) -> Self {
        Self {
            text: text.into(),
            size: super::sized(size),
            color,
            weight: FontWeight::NORMAL,
            centered: false,
        }
    }

    pub fn weight(mut self, weight: FontWeight) -> Self {
        self.weight = weight;
        self
    }

    pub fn centered(mut self) -> Self {
        self.centered = true;
        self
    }
}

impl Component for Marquee {
    fn render(&self) -> impl IntoElement {
        let mut room = use_state(|| 0_f32);
        let mut natural = use_state(|| 0_f32);
        let gap = self.size * 2.5;
        let (room_now, natural_now) = (*room.read(), *natural.read());
        // How far one pass slides: the text and the gap, so the copy after it lands where it
        // started. Nothing when it fits.
        let travel = match natural_now > room_now + 1. && room_now > 0. {
            true => natural_now + gap,
            false => 0.,
        };
        let slide = use_animation_with_dependencies(&travel, |conf, travel| {
            conf.on_creation(OnCreation::Run);
            if *travel > 0. {
                conf.on_finish(OnFinish::restart_with_delay(PAUSE));
            }
            AnimNum::new(0., *travel)
                .time((*travel / SPEED * 1000.) as u64)
                .function(Function::Linear)
        });
        let moving = travel > 0.;
        let offset = match moving {
            true => -slide.get().value(),
            false if self.centered => ((room_now - natural_now) / 2.).max(0.),
            false => 0.,
        };
        let line = |key: usize| {
            label()
                .key(key)
                .text(self.text.clone())
                .font_size(self.size)
                .font_weight(self.weight)
                .color(self.color)
                .max_lines(1)
        };

        rect()
            .width(Size::fill())
            .overflow(Overflow::Clip)
            .on_sized(move |event: Event<SizedEventData>| room.set(event.area.width()))
            .fade(moving.then(|| Fade::horizontal(if offset < 0. { EDGE } else { 0. }, EDGE)))
            .child(
                rect()
                    .width(Size::px(ROOM))
                    .direction(Direction::Horizontal)
                    .offset_x(offset)
                    .spacing(gap)
                    .child(
                        rect()
                            .on_sized(move |event: Event<SizedEventData>| {
                                natural.set(event.area.width())
                            })
                            .child(line(0)),
                    )
                    // The copy that follows it round.
                    .maybe_child(moving.then(|| line(1))),
            )
    }
}

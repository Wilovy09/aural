//! Scrolling a list with a finger, done here rather than by Freya's scroll views: theirs takes
//! the whole gesture after two pixels, so a row never hears a sideways swipe. Here the first
//! ten pixels decide: up or down scrolls the list, sideways is left to the row under the
//! finger.

use freya::prelude::*;

/// How far the finger moves before the gesture is known to be a scroll or a swipe.
const SLOP: f64 = 10.;

#[derive(Clone, Copy, PartialEq)]
enum Axis {
    Undecided,
    Vertical,
    Sideways,
}

#[derive(Clone, Copy, PartialEq)]
struct Touch {
    x: f64,
    y: f64,
    /// Where the list was scrolled when the finger came down.
    scrolled: i32,
    axis: Axis,
}

/// `content`, a list scrolled by `scroll`, that a finger scrolls up and down. Give the list
/// `.drag_scrolling(false)` so the two do not fight.
#[derive(PartialEq)]
pub struct TouchScroll {
    pub scroll: ScrollController,
    pub content: Element,
}

impl Component for TouchScroll {
    fn render(&self) -> impl IntoElement {
        let mut touch = use_state(|| None::<Touch>);
        let mut scroll = self.scroll;
        rect()
            .width(Size::fill())
            .height(Size::fill())
            .on_pointer_down(move |event: Event<PointerEventData>| {
                if !matches!(event.data(), PointerEventData::Touch(_)) {
                    return;
                }
                let at = event.global_location();
                let (_, scrolled): (i32, i32) = scroll.into();
                touch.set(Some(Touch {
                    x: at.x,
                    y: at.y,
                    scrolled,
                    axis: Axis::Undecided,
                }));
            })
            .on_global_pointer_move(move |event: Event<PointerEventData>| {
                // Read first: the borrow must end before the state is written.
                let Some(mut now) = *touch.peek() else {
                    return;
                };
                let at = event.global_location();
                let (dx, dy) = (at.x - now.x, at.y - now.y);
                if now.axis == Axis::Undecided {
                    if dy.abs() > SLOP && dy.abs() >= dx.abs() {
                        now.axis = Axis::Vertical;
                    } else if dx.abs() > SLOP {
                        now.axis = Axis::Sideways;
                    } else {
                        return;
                    }
                    touch.set(Some(now));
                }
                if now.axis == Axis::Vertical {
                    scroll.scroll_to_y(now.scrolled + dy as i32);
                    // The finger lifting after a scroll is not a tap on a row.
                    super::swipe::mark();
                }
            })
            .on_global_pointer_press(move |_: Event<PointerEventData>| {
                let held = touch.peek().is_some();
                if held {
                    touch.set(None);
                }
            })
            .child(self.content.clone())
    }
}

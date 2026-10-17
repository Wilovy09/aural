//! A panel over whatever shows, for the devices and the queue: on a computer or a TV it slides
//! in from the right and grows to size, on a phone it rises from the bottom as a sheet that a
//! drag down closes. Over the fullscreen player it is frosted glass, elsewhere the app's gray.
//! A press outside it closes it.

use freya::animation::{AnimNum, Ease, Function, use_animation_transition};
use freya::prelude::*;

use super::color;

/// The panel's width on a computer or a TV.
pub const WIDTH: f32 = 510.;
/// How far down a drag must pull a phone's sheet for it to close.
const PULL: f32 = 120.;
/// How far from a phone sheet's top a drag can start.
const GRIP: f64 = 96.;

/// The panel's colours: the app's grays, or glass over the fullscreen player's backdrop.
#[derive(Clone, Copy)]
pub struct Palette {
    pub panel: Color,
    pub raised: Color,
    pub muted: Color,
    pub edge: Color,
}

impl Palette {
    pub fn new(glass: bool) -> Self {
        match glass {
            true => Self {
                panel: Color::from_argb(0x59, 0x10, 0x10, 0x10),
                raised: color::GLASS,
                muted: color::GLASS_SELECTED,
                edge: color::GLASS_EDGE,
            },
            false => Self {
                panel: color::MUTED,
                raised: color::RAISED,
                muted: color::MUTED,
                edge: color::BORDER,
            },
        }
    }
}

#[derive(PartialEq)]
pub struct Sheet {
    pub open: bool,
    /// Frosted glass, over the fullscreen player.
    pub glass: bool,
    /// How tall a phone's sheet is; on a computer the panel takes the window's height.
    pub tall: f32,
    /// Closes it: a press outside, or a phone's drag down.
    pub close: fn(),
    pub content: Element,
}

impl Component for Sheet {
    fn render(&self) -> impl IntoElement {
        // How far in it is, 0 to 1: it eases in when opened and out when closed, and stays
        // drawn until it is all the way out.
        let shown = use_animation_transition(use_reactive(&self.open), |from: bool, to: bool| {
            let at = |open: bool| if open { 1. } else { 0. };
            AnimNum::new(at(from), at(to))
                .time(if to { 380 } else { 240 })
                .function(Function::Expo)
                .ease(if to { Ease::Out } else { Ease::In })
        });
        // On a phone, where a drag started and how far down it has gone.
        let mut drag = use_state(|| None::<f64>);
        let mut lowered = use_state(|| 0_f32);

        let shown = shown.get().value().clamp(0., 1.);
        if !self.open && shown <= 0. {
            return rect().into_element();
        }
        let (width, height) = super::viewport();
        let compact = super::compact();
        let panel_width = match compact {
            true => width,
            false => WIDTH.min(width),
        };
        let tall = self.tall.min(height * 0.92);
        let offset = *lowered.read();
        let glass = self.glass;
        let palette = Palette::new(glass);
        let close = self.close;

        rect()
            .layer(Layer::Overlay)
            .position(Position::new_global().left(0.).top(0.))
            .width(Size::px(width))
            .height(Size::px(height))
            .background(Color::from_argb(
                ((if glass { 0x33 } else { 0x80 }) as f32 * shown) as u8,
                0,
                0,
                0,
            ))
            .on_press(move |event: Event<PressEventData>| {
                event.stop_propagation();
                close();
            })
            .child(
                rect()
                    .position(match compact {
                        // A phone's sheet rises from under the screen's bottom edge.
                        true => Position::new_global()
                            .left(0.)
                            .bottom(-offset - (1. - shown) * tall),
                        // A computer's panel comes in from the right as it grows to size.
                        false => Position::new_global()
                            .left(width - panel_width + (1. - shown) * 120.)
                            .top(0.),
                    })
                    .maybe(!compact, |panel| {
                        panel
                            .scale(0.92 + 0.08 * shown)
                            .transform_origin(TransformOrigin::right())
                            .opacity(shown)
                    })
                    .width(Size::px(panel_width))
                    .height(Size::px(match compact {
                        true => tall,
                        false => height,
                    }))
                    .content(Content::Flex)
                    .corner_radius(match compact {
                        true => (24., 24., 0., 0.),
                        false => (24., 0., 0., 24.),
                    })
                    .overflow(Overflow::Clip)
                    .background(palette.panel)
                    .maybe(glass, |panel| panel.backdrop_blur(48.))
                    .border(Border::new().fill(palette.edge).width(1.))
                    .on_press(|event: Event<PressEventData>| event.stop_propagation())
                    // On a phone, a drag from the sheet's top pulls it down; far enough, it
                    // closes, else it springs back.
                    .on_pointer_down(move |event: Event<PointerEventData>| {
                        if compact && event.element_location().y < GRIP {
                            drag.set(Some(event.global_location().y));
                        }
                    })
                    .on_global_pointer_move(move |event: Event<PointerEventData>| {
                        let Some(start) = *drag.peek() else {
                            return;
                        };
                        lowered.set((event.global_location().y - start).max(0.) as f32);
                    })
                    .on_global_pointer_up(move |_: Event<PointerEventData>| {
                        if drag.peek().is_none() {
                            return;
                        }
                        drag.set(None);
                        let far = *lowered.peek() > PULL;
                        lowered.set(0.);
                        if far {
                            close();
                        }
                    })
                    .maybe(compact, |sheet| {
                        sheet.child(
                            rect()
                                .width(Size::fill())
                                .padding((10., 0., 0., 0.))
                                .center()
                                .child(
                                    rect()
                                        .width(Size::px(40.))
                                        .height(Size::px(4.))
                                        .corner_radius(2.)
                                        .background(color::FAINT),
                                ),
                        )
                    })
                    .child(
                        rect()
                            .width(Size::fill())
                            .height(Size::flex(1.))
                            .child(self.content.clone()),
                    ),
            )
            .into_element()
    }
}

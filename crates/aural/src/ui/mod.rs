//! Aural's design system, taken from Sonora's dark theme: colour tokens, the text scale,
//! metrics, icons, covers and the focus treatment the D-pad needs. Screens read every colour
//! and size from here.

mod cover;
pub mod form_input;
pub mod spectrum;
pub mod swipe;
pub mod tint;
pub mod touch_scroll;

pub use cover::Cover;

use freya::prelude::*;

/// Colour tokens: a dark room lit by a screen. Surfaces step up by a few points of lightness;
/// edges are translucent white, never grey lines; text has three tiers; the one colour is the
/// light of the cover that plays (see [`tint`]).
pub mod color {
    use freya::prelude::Color;

    /// The room: the canvas behind everything, sidebar included.
    pub const BACKGROUND: Color = Color::from_rgb(0x09, 0x09, 0x09);
    /// One step up: the player island, cards, the search field.
    pub const SECONDARY: Color = Color::from_rgb(0x11, 0x11, 0x11);
    /// Two steps up: chips, tiles waiting for art, raised controls.
    pub const MUTED: Color = Color::from_rgb(0x18, 0x18, 0x18);
    /// Three steps up: what is pressed or open.
    pub const RAISED: Color = Color::from_rgb(0x22, 0x22, 0x22);
    pub const FOREGROUND: Color = Color::from_rgb(0xf5, 0xf5, 0xf5);
    /// Supporting text: artists, counts, labels.
    pub const MUTED_FOREGROUND: Color = Color::from_rgb(0x99, 0x99, 0x99);
    /// Metadata that should almost disappear: track numbers, column titles.
    pub const FAINT: Color = Color::from_rgb(0x66, 0x66, 0x66);
    pub const PRIMARY: Color = Color::from_rgb(0xf5, 0xf5, 0xf5);
    pub const PRIMARY_FOREGROUND: Color = Color::from_rgb(0x0b, 0x0b, 0x0b);
    pub const BORDER: Color = Color::from_argb(0x1f, 0xff, 0xff, 0xff);
    pub const SIDEBAR: Color = BACKGROUND;
    /// The open section in the sidebar.
    pub const SIDEBAR_ACCENT: Color = Color::from_argb(0x14, 0xff, 0xff, 0xff);
    pub const SIDEBAR_BORDER: Color = Color::from_argb(0x0f, 0xff, 0xff, 0xff);
    pub const TABLE_HEAD: Color = Color::TRANSPARENT;
    pub const PROGRESS: Color = Color::from_rgb(0xf5, 0xf5, 0xf5);
    /// A row under the pointer.
    pub const HOVER: Color = Color::from_argb(0x0f, 0xff, 0xff, 0xff);
    /// The ring around a cover or button the D-pad is on.
    pub const FOCUS: Color = Color::from_rgb(0xf5, 0xf5, 0xf5);
    /// The fill of a row the D-pad is on: lit, not outlined.
    pub const FOCUS_FILL: Color = Color::from_argb(0x1f, 0xff, 0xff, 0xff);
    /// The light when nothing plays, or a cover has no colour of its own.
    pub const LIGHT: Color = Color::from_rgb(0x8a, 0x8a, 0x8a);
    /// Secondary text over the fullscreen backdrop, whose colour depends on the cover.
    pub const ON_BACKDROP: Color = Color::from_argb(0xbf, 0xff, 0xff, 0xff);
    /// Frosted glass over the backdrop: its fill, its hairline edge, and a selected part.
    pub const GLASS: Color = Color::from_argb(0x24, 0xff, 0xff, 0xff);
    pub const GLASS_EDGE: Color = Color::from_argb(0x38, 0xff, 0xff, 0xff);
    pub const GLASS_SELECTED: Color = Color::from_argb(0x33, 0xff, 0xff, 0xff);
    /// A row highlighted over the backdrop.
    pub const BACKDROP_ROW: Color = Color::from_argb(0x47, 0x00, 0x00, 0x00);
    /// The ring on a focused white button, where a white ring would not show.
    pub const FOCUS_ON_PRIMARY: Color = Color::from_rgb(0x3b, 0x82, 0xf6);
    pub const DANGER: Color = Color::from_rgb(0xf8, 0x71, 0x71);
}

/// The text scale: Sonora's ratios over a 14 px body.
pub mod text {
    pub const TINY: f32 = 11.;
    pub const SMALL: f32 = 12.;
    pub const LABEL: f32 = 13.;
    pub const BODY: f32 = 14.;
    pub const LARGE: f32 = 19.;
    pub const TITLE: f32 = 24.;
    pub const DISPLAY: f32 = 30.;
}

/// Sizes of the shell and its rows.
pub mod metrics {
    /// The radius scale: controls, rows and covers, cards and the player, sheets.
    pub const RADIUS_SM: f32 = 8.;
    /// The fullscreen player's corners, kept as they were designed.
    pub const PLAYER_RADIUS: f32 = 10.;
    pub const RADIUS: f32 = 12.;
    pub const RADIUS_LG: f32 = 16.;
    pub const RADIUS_XL: f32 = 24.;
    pub const PAD: f32 = 8.;
    pub const INSET: f32 = 24.;
    pub const CONTROL: f32 = 32.;
    pub const ROW: f32 = 42.;
    pub const HEADER: f32 = 32.;
    pub const THUMB: f32 = 34.;
    /// The player island and the margin under it.
    pub const PLAYER_BAR: f32 = 80.;
    pub const SIDEBAR: f32 = 232.;
    pub const ICON: f32 = 16.;
    pub const CARD: f32 = 150.;
    pub const CARD_GAP: f32 = 24.;
}

/// The font every label uses, embedded at launch.
pub const FONT: &str = "Inter";

/// Registers Inter with `config` and makes it the default face.
pub fn fonts(config: freya::prelude::LaunchConfig) -> freya::prelude::LaunchConfig {
    config
        .with_font(
            FONT,
            bytes::Bytes::from_static(include_bytes!("../../../../assets/fonts/Inter-Regular.ttf")),
        )
        .with_font(
            FONT,
            bytes::Bytes::from_static(include_bytes!(
                "../../../../assets/fonts/Inter-SemiBold.ttf"
            )),
        )
        .with_font(
            FONT,
            bytes::Bytes::from_static(include_bytes!("../../../../assets/fonts/Inter-Bold.ttf")),
        )
        .with_default_font(FONT)
}

/// The Lucide icons the app draws, embedded from `assets/icons`.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Icon {
    HeartFilled,
    PlayFilled,
    PauseFilled,
    Previous,
    Next,
    Shuffle,
    Queue,
    Maximize,
    Album,
    Cast,
    Home,
    Back,
    Library,
    Translate,
    Heart,
    Grip,
    Motion,
    Playing,
    Music,
    Settings,
    LogOut,
    User,
    Repeat,
    RepeatOne,
    Lyrics,
    Clear,
    Night,
    Search,
    Close,
}

impl Icon {
    /// The icon's SVG, for components that take raw bytes.
    pub fn bytes(self) -> Bytes {
        Bytes::from_static(self.source().1)
    }

    fn source(self) -> (&'static str, &'static [u8]) {
        macro_rules! svg {
            ($name:literal) => {
                (
                    $name,
                    include_bytes!(concat!("../../../../assets/icons/", $name, ".svg")),
                )
            };
        }
        match self {
            Icon::HeartFilled => svg!("heart-filled"),
            Icon::PlayFilled => svg!("play-filled"),
            Icon::PauseFilled => svg!("pause-filled"),
            Icon::Previous => svg!("skip-back"),
            Icon::Next => svg!("skip-forward"),
            Icon::Shuffle => svg!("shuffle"),
            Icon::Queue => svg!("list-music"),
            Icon::Maximize => svg!("maximize"),
            Icon::Album => svg!("disc-album"),
            Icon::Cast => svg!("cast"),
            Icon::Home => svg!("house"),
            Icon::Back => svg!("chevron-left"),
            Icon::Library => svg!("library"),
            Icon::Translate => svg!("languages"),
            Icon::Heart => svg!("heart"),
            Icon::Grip => svg!("grip-vertical"),
            Icon::Motion => svg!("image-play"),
            Icon::Playing => svg!("music-2"),
            Icon::Music => svg!("music"),
            Icon::Settings => svg!("settings"),
            Icon::LogOut => svg!("log-out"),
            Icon::User => svg!("user"),
            Icon::Repeat => svg!("repeat"),
            Icon::RepeatOne => svg!("repeat-one"),
            Icon::Lyrics => svg!("mic-vocal"),
            Icon::Clear => svg!("eye-dashed"),
            Icon::Night => svg!("moon"),
            Icon::Search => svg!("search"),
            Icon::Close => svg!("x"),
        }
    }
}

/// `icon` at `size` px in `tint`.
pub fn icon(icon: Icon, size: f32, tint: Color) -> impl IntoElement {
    SvgViewer::new(ImageSource::from(icon.source()))
        .width(Size::px(size))
        .height(Size::px(size))
        .color(tint)
}

/// Aural's logo, the light tile from `assets/logos`, `size` px square.
pub fn logo(size: f32) -> impl IntoElement {
    const LOGO: (&str, &[u8]) = (
        "aural-light",
        include_bytes!("../../../../assets/logos/aural_light.svg"),
    );
    SvgViewer::new(ImageSource::from(LOGO))
        .width(Size::px(size))
        .height(Size::px(size))
}

/// Windows narrower than this get the phone layout: a bottom bar instead of the sidebar.
pub const COMPACT: f32 = 640.;

/// Whether the window is narrow enough for the phone layout.
pub fn compact() -> bool {
    viewport().0 < COMPACT
}

thread_local! {
    /// Whether the last input was a touch or a click rather than a key.
    static TOUCH: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// Whether the last input was a touch or a click. Focus rings are for the D-pad only, and a
/// list a finger scrolls should not glide back to the focus on its own.
pub fn touch() -> bool {
    TOUCH.get()
}

/// Notes whether the last input was a touch (or click) or a key.
pub fn set_touch(touch: bool) {
    TOUCH.set(touch);
}

/// What the D-pad is on, as long as the last input was a key.
pub fn ring(focused: bool) -> bool {
    focused && !touch()
}

/// A press handler that moves the focus onto `target` and presses OK there, the way a tap or a
/// click does what the D-pad would.
pub fn tap(target: crate::nav::Target) -> impl FnMut(Event<PressEventData>) + 'static {
    move |event: Event<PressEventData>| {
        event.stop_propagation();
        // The release that ends a swipe is not a tap on the row.
        if swipe::just_swiped() {
            return;
        }
        crate::app::tap(target);
    }
}

/// The border and fill that mark the element the D-pad is on.
pub fn focus_border(focused: bool) -> Border {
    match ring(focused) {
        true => Border::new().fill(color::FOCUS).width(2.),
        false => Border::new().fill(Color::TRANSPARENT).width(2.),
    }
}

thread_local! {
    /// The text size setting's factor.
    static TEXT: std::cell::Cell<f32> = const { std::cell::Cell::new(1.) };
}

/// Sets the factor every [`line`] is drawn at.
pub fn set_text_scale(factor: f32) {
    TEXT.set(factor);
}

/// `size` at the text size setting.
pub fn sized(size: f32) -> f32 {
    (size * TEXT.get()).round()
}

/// `text` as a one-line label in `size` and `tint`, cut with an ellipsis when it overflows.
pub fn line(text: impl Into<String>, size: f32, tint: Color) -> Label {
    label()
        .text(text.into())
        .font_size(sized(size))
        .color(tint)
        .max_lines(1)
        .text_overflow(TextOverflow::Ellipsis)
}

/// The uppercase, small, semibold label above a heading.
pub fn eyebrow(text: &str) -> Label {
    line(text.to_uppercase(), text::SMALL, color::MUTED_FOREGROUND)
        .font_weight(FontWeight::SEMI_BOLD)
}

/// The window's size in logical pixels.
pub fn viewport() -> (f32, f32) {
    let platform = Platform::get();
    let size = *platform.root_size.read();
    let scale = (*platform.scale_factor.read() as f32).max(0.5);
    (size.width / scale, size.height / scale)
}

/// The room the system bars take at the top and bottom of the window, in logical pixels.
pub fn safe() -> (f32, f32) {
    #[cfg(target_os = "android")]
    {
        let scale = (*Platform::get().scale_factor.read() as f32).max(0.5);
        let (top, bottom) = crate::login::insets();
        (top / scale, bottom / scale)
    }
    #[cfg(not(target_os = "android"))]
    (0., 0.)
}

/// How many cards fit a row of the content area: two on a phone.
pub fn columns() -> usize {
    if compact() {
        return 2;
    }
    let (width, _) = viewport();
    let room = width - metrics::SIDEBAR - metrics::INSET * 2.;
    (((room + metrics::CARD_GAP) / (metrics::CARD + metrics::CARD_GAP)).floor() as usize).max(1)
}

/// The side of a card: fixed on a wide window, half the width on a phone.
pub fn card() -> f32 {
    match compact() {
        true => ((viewport().0 - inset() * 2. - gap()) / 2.).min(240.),
        false => metrics::CARD,
    }
}

/// The margin around a page's content: narrower on a phone.
pub fn inset() -> f32 {
    match compact() {
        true => 16.,
        false => metrics::INSET,
    }
}

/// The gap between two cards.
pub fn gap() -> f32 {
    match compact() {
        true => 14.,
        false => metrics::CARD_GAP,
    }
}

/// How a [`button`] looks.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Variant {
    /// White fill, dark text: the page's main action.
    Primary,
    /// A border and no fill.
    Outline,
    /// No border, no fill.
    Ghost,
}

/// A button of `CONTROL` height with an optional icon and label, ringed when `focused`.
pub fn button(variant: Variant, glyph: Option<Icon>, text: Option<&str>, focused: bool) -> Rect {
    let focused = ring(focused);
    let (fill, ink) = match variant {
        Variant::Primary => (color::PRIMARY, color::PRIMARY_FOREGROUND),
        Variant::Outline | Variant::Ghost => (Color::TRANSPARENT, color::FOREGROUND),
    };
    let fill = match (focused, variant) {
        (true, Variant::Primary) => fill,
        (true, _) => color::FOCUS_FILL,
        (false, _) => fill,
    };
    rect()
        .height(Size::px(metrics::CONTROL))
        .min_width(Size::px(metrics::CONTROL))
        .padding((0., if text.is_some() { 12. } else { 8. }))
        .direction(Direction::Horizontal)
        .cross_align(Alignment::Center)
        .main_align(Alignment::Center)
        .spacing(6.)
        .corner_radius(metrics::PLAYER_RADIUS)
        .background(fill)
        .border(match (focused, variant) {
            (true, Variant::Primary) => Border::new().fill(color::FOCUS_ON_PRIMARY).width(3.),
            (true, _) => Border::new().fill(color::FOCUS).width(2.),
            (false, Variant::Outline) => Border::new().fill(color::BORDER).width(1.),
            (false, _) => Border::new().fill(Color::TRANSPARENT).width(2.),
        })
        .map(glyph, |button, glyph| {
            button.child(icon(glyph, metrics::ICON, ink))
        })
        .map(text, |button, text| {
            button.child(line(text, text::LABEL, ink).font_weight(FontWeight::SEMI_BOLD))
        })
}

/// The like heart: filled in the cover's `light` when liked, an outline in `ink` when not.
pub fn heart(liked: bool, size: f32, light: Color, ink: Color) -> impl IntoElement {
    match liked {
        true => icon(Icon::HeartFilled, size, light),
        false => icon(Icon::Heart, size, ink),
    }
}

/// A page's main action as a pill: Play white and solid, the others a raised dark pill. The
/// D-pad's one wears a ring of `light`, the cover's, rather than a stark outline.
pub fn pill(primary: bool, glyph: Icon, text: Option<&str>, focused: bool, light: Color) -> Rect {
    let focused = ring(focused);
    let (fill, ink) = match (primary, focused) {
        (true, _) => (color::PRIMARY, color::PRIMARY_FOREGROUND),
        (false, true) => (color::RAISED, color::FOREGROUND),
        (false, false) => (color::MUTED, color::FOREGROUND),
    };
    rect()
        .height(Size::px(42.))
        .min_width(Size::px(42.))
        .padding((0., if text.is_some() { 20. } else { 0. }))
        .direction(Direction::Horizontal)
        .cross_align(Alignment::Center)
        .main_align(Alignment::Center)
        .spacing(8.)
        .corner_radius(21.)
        .background(fill)
        .border(match focused {
            true => Border::new()
                .fill(light)
                .width(3.)
                .alignment(BorderAlignment::Outer),
            false => Border::new().fill(Color::TRANSPARENT).width(0.),
        })
        .child(icon(glyph, 18., ink))
        .map(text, |button, text| {
            button.child(line(text, text::BODY, ink).font_weight(FontWeight::SEMI_BOLD))
        })
}

/// A scroll controller that glides to `target` (pixels from the top) whenever it changes, so a
/// list keeps the D-pad's row in view.
pub fn use_follow(target: f32) -> ScrollController {
    use_glide(target, true)
}

/// A scroll controller that glides to `target` whenever it changes, even under a finger: the
/// lyrics keep the sung line in place however the last input came.
pub fn use_follow_always(target: f32) -> ScrollController {
    use_glide(target, false)
}

fn use_glide(target: f32, yield_to_touch: bool) -> ScrollController {
    use freya::animation::{AnimNum, Ease, Function, use_animation_transition};

    let mut scroll = use_scroll_controller(ScrollConfig::default);
    let wanted = use_reactive(&target);
    let glide = use_animation_transition(wanted, |from: f32, to: f32| {
        AnimNum::new(from, to)
            .time(200)
            .function(Function::Expo)
            .ease(Ease::Out)
    });
    // Writing the scroll reads it as well, which reruns the effect; only a new position is
    // applied, so the rerun stops there instead of looping.
    let applied = use_hook(|| std::rc::Rc::new(std::cell::Cell::new(i32::MIN)));
    use_side_effect(move || {
        let y = -(glide.get().value() as i32);
        // A finger scrolls the list itself; following the focus would yank it back.
        if yield_to_touch && touch() {
            applied.set(y);
            return;
        }
        if applied.replace(y) != y {
            scroll.scroll_to_y(y);
        }
    });
    scroll
}

/// The scroll offset that keeps item `index` of `size` px a third of the way down a view of
/// `view` px.
pub fn follow_offset(index: usize, size: f32, view: f32) -> f32 {
    (index as f32 * size - view / 3.).max(0.)
}

/// An icon button for a mode that is on or off (shuffle, repeat): white when on, grey when
/// off, ringed when `focused`.
pub fn toggle(glyph: Icon, on: bool, focused: bool) -> Rect {
    let focused = ring(focused);
    rect()
        .width(Size::px(metrics::CONTROL + 8.))
        .height(Size::px(metrics::CONTROL + 8.))
        .center()
        .corner_radius(metrics::PLAYER_RADIUS)
        .background(match focused {
            true => color::FOCUS_FILL,
            false => Color::TRANSPARENT,
        })
        .border(focus_border(focused))
        .child(icon(
            glyph,
            metrics::ICON + 4.,
            match on {
                true => color::FOREGROUND,
                false => color::MUTED_FOREGROUND,
            },
        ))
}

/// A pill of tabs in frosted glass for the fullscreen backdrop: a translucent white fill and
/// hairline border over the blurred cover, the selected tab a little brighter, the D-pad's one
/// ringed.
pub fn tabs(entries: &[(Icon, &str)], selected: usize, focused: Option<usize>) -> Rect {
    rect()
        .direction(Direction::Horizontal)
        .padding(4.)
        .spacing(4.)
        .corner_radius(metrics::PLAYER_RADIUS + 4.)
        .background(color::GLASS)
        .border(Border::new().fill(color::GLASS_EDGE).width(1.))
        .shadow((0., 8., 24., 0., Color::from_argb(0x33, 0, 0, 0)))
        .children(entries.iter().enumerate().map(|(index, (glyph, name))| {
            let ink = match index == selected {
                true => color::FOREGROUND,
                false => color::ON_BACKDROP,
            };
            rect()
                .key(index)
                .height(Size::px(metrics::CONTROL))
                .padding((0., 14.))
                .direction(Direction::Horizontal)
                .cross_align(Alignment::Center)
                .spacing(6.)
                .corner_radius(metrics::PLAYER_RADIUS)
                .background(match index == selected {
                    true => color::GLASS_SELECTED,
                    false => Color::TRANSPARENT,
                })
                .border(focus_border(focused == Some(index)))
                .on_press(tap(crate::nav::Target::Tab(index)))
                .child(icon(*glyph, metrics::ICON, ink))
                .child(line(*name, text::LABEL, ink).font_weight(FontWeight::SEMI_BOLD))
                .into()
        }))
}

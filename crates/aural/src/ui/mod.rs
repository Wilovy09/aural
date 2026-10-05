//! Aural's design system, taken from Sonora's dark theme: colour tokens, the text scale,
//! metrics, icons, covers and the focus treatment the D-pad needs. Screens read every colour
//! and size from here.

mod cover;

pub use cover::Cover;

use freya::prelude::*;

/// Colour tokens of the dark theme (Sonora's `Theme::dark`).
pub mod color {
    use freya::prelude::Color;

    pub const BACKGROUND: Color = Color::from_rgb(0x0a, 0x0a, 0x0a);
    pub const FOREGROUND: Color = Color::from_rgb(0xfa, 0xfa, 0xfa);
    pub const BORDER: Color = Color::from_argb(0x66, 0x50, 0x50, 0x50);
    pub const MUTED: Color = Color::from_rgb(0x26, 0x26, 0x26);
    pub const MUTED_FOREGROUND: Color = Color::from_rgb(0x90, 0x90, 0x90);
    pub const SECONDARY: Color = Color::from_rgb(0x17, 0x17, 0x17);
    pub const PRIMARY: Color = Color::from_rgb(0xfa, 0xfa, 0xfa);
    pub const PRIMARY_FOREGROUND: Color = Color::from_rgb(0x17, 0x17, 0x17);
    pub const SIDEBAR: Color = Color::from_rgb(0x0a, 0x0a, 0x0a);
    pub const SIDEBAR_ACCENT: Color = Color::from_argb(0x66, 0x50, 0x50, 0x50);
    pub const SIDEBAR_BORDER: Color = Color::from_rgb(0x26, 0x26, 0x26);
    pub const TABLE_HEAD: Color = Color::from_argb(0xcc, 0x17, 0x17, 0x17);
    pub const PROGRESS: Color = Color::from_rgb(0xf5, 0xf5, 0xf5);
    /// The ring drawn around whatever the D-pad is on.
    pub const FOCUS: Color = Color::from_rgb(0xfa, 0xfa, 0xfa);
    pub const FOCUS_FILL: Color = Color::from_argb(0x33, 0xfa, 0xfa, 0xfa);
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
    pub const RADIUS: f32 = 10.;
    pub const PAD: f32 = 8.;
    pub const INSET: f32 = 24.;
    pub const CONTROL: f32 = 32.;
    pub const ROW: f32 = 42.;
    pub const HEADER: f32 = 32.;
    pub const THUMB: f32 = 34.;
    pub const PLAYER_BAR: f32 = 76.;
    pub const SIDEBAR: f32 = 195.;
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

/// The border and fill that mark the element the D-pad is on.
pub fn focus_border(focused: bool) -> Border {
    match focused {
        true => Border::new().fill(color::FOCUS).width(2.),
        false => Border::new().fill(Color::TRANSPARENT).width(2.),
    }
}

/// `text` as a one-line label in `size` and `tint`, cut with an ellipsis when it overflows.
pub fn line(text: impl Into<String>, size: f32, tint: Color) -> Label {
    label()
        .text(text.into())
        .font_size(size)
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

/// How many cards fit a row of the content area.
pub fn columns() -> usize {
    let (width, _) = viewport();
    let room = width - metrics::SIDEBAR - metrics::INSET * 2.;
    (((room + metrics::CARD_GAP) / (metrics::CARD + metrics::CARD_GAP)).floor() as usize).max(1)
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
        .corner_radius(metrics::RADIUS)
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

/// A scroll controller that glides to `target` (pixels from the top) whenever it changes, so a
/// list keeps the D-pad's row in view.
pub fn use_follow(target: f32) -> ScrollController {
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
    rect()
        .width(Size::px(metrics::CONTROL + 8.))
        .height(Size::px(metrics::CONTROL + 8.))
        .center()
        .corner_radius(metrics::RADIUS)
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
        .corner_radius(metrics::RADIUS + 4.)
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
                .corner_radius(metrics::RADIUS)
                .background(match index == selected {
                    true => color::GLASS_SELECTED,
                    false => Color::TRANSPARENT,
                })
                .border(focus_border(focused == Some(index)))
                .child(icon(*glyph, metrics::ICON, ink))
                .child(line(*name, text::LABEL, ink).font_weight(FontWeight::SEMI_BOLD))
                .into()
        }))
}

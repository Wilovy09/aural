//! The lyrics panel of the fullscreen player, styled after Apple Music's: generous spacing,
//! every line not being sung in one soft tone, the sung line bright and pinned near the top,
//! a ♪ for the intro and long instrumental stretches, and on word-synced sheets each word
//! lighting up as it is sung.

use std::time::{Duration, Instant};

use freya::animation::{AnimNum, OnCreation, OnFinish, use_animation};
use freya::prelude::*;
use freya::radio::use_radio;
use lyrics::{Lyrics, LyricsLine, LyricsWord};

use crate::state::{AppState, Channel, Load};
use crate::ui::{self, Icon, color, text};

/// Size of a lyric line on the widest windows; narrower ones scale it down.
const SIZE_MAX: f32 = 30.;
/// Where the sung line sits, as a share of the panel's height from the top.
const PIN: f32 = 0.25;
/// The gap between two lines, as a share of the text size (Sonora's 16 px at its 27).
const GAP: f32 = 0.6;
/// How bright lines already sung and lines still to come are, Sonora's 0.4 and 0.6 of its
/// muted grey.
const PAST: f32 = 0.24;
const AHEAD: f32 = 0.36;
/// How much bigger the sung line is, Sonora's fullscreen growth.
const GROWTH: f32 = 3.;
/// How bright a word not yet reached is inside the sung line.
const UNSUNG: f32 = 0.45;
/// How many brightness steps a word passes through while it is sung.
const STEPS: f32 = 8.;
/// A silence between lines at least this long gets Sonora's ♪ ♪ ♪ of its own.
const BREAK: Duration = Duration::from_secs(5);

/// What the panel lists: a ♪ for an instrumental stretch, or a line of the sheet.
#[derive(Clone, Copy, PartialEq)]
enum Item {
    /// An instrumental stretch from one time to another.
    Notes(Duration, Duration),
    Line(usize),
}

#[derive(PartialEq)]
pub struct LyricsPanel;

impl Component for LyricsPanel {
    fn render(&self) -> impl IntoElement {
        let sheet = use_radio::<AppState, Channel>(Channel::Lyrics);
        let position = use_radio::<AppState, Channel>(Channel::Position);
        let now = use_radio::<AppState, Channel>(Channel::Now);
        let navigation = use_radio::<AppState, Channel>(Channel::Navigation);
        let translating = navigation.read().translate;
        let elapsed = position.read().position.elapsed;
        let playing = now.read().now.playing;
        let state = sheet.read().sheet.clone();

        // Item heights as laid out, to pin the sung one exactly. Reset for every song.
        let mut heights = use_state(Vec::<f32>::new);
        let song = use_reactive(&state.song);
        use_side_effect(move || {
            let _ = song.read();
            heights.set(Vec::new());
        });

        let (width, view) = ui::viewport();
        let size = match ui::compact() {
            true => (width / 15.).clamp(20., 28.),
            false => (width / 46.).clamp(18., SIZE_MAX),
        };
        let gap = size * GAP;
        let lines: Option<std::sync::Arc<[LyricsLine]>> = match &state.lyrics {
            Load::Ready(Lyrics::Synced { lines }) => Some(lines.clone()),
            _ => None,
        };
        // Each line carries its translation while translating is on, and none otherwise (a
        // sheet may come with its own).
        let translation = match (&state.translation, translating) {
            (Load::Ready(Some(found)), true) => Some(found.clone()),
            _ => None,
        };
        let lines = lines.map(|lines| -> std::sync::Arc<[LyricsLine]> {
            lines
                .iter()
                .enumerate()
                .map(|(at, line)| LyricsLine {
                    translation: translation
                        .as_ref()
                        .and_then(|found| found.lines.get(at).cloned().flatten()),
                    ..line.clone()
                })
                .collect()
        });
        let items = lines.as_deref().map(items).unwrap_or_default();
        let current = lines
            .as_deref()
            .and_then(|lines| current(&items, lines, elapsed));
        let target = match current {
            Some(index) => {
                let known = heights.read();
                // The top padding already puts the first item at the pin.
                (0..index)
                    .map(|item| known.get(item).copied().unwrap_or(size * 1.3) + gap)
                    .sum()
            }
            None => 0.,
        };
        let scroll = ui::use_follow_always(target);

        let body = match (&state.lyrics, lines) {
            (_, Some(lines)) => synced(
                SyncedSheet {
                    lines,
                    items,
                    current,
                    elapsed,
                    playing,
                    size,
                    gap,
                },
                heights,
            )
            .into_element(),
            (Load::Ready(Lyrics::Plain { text, .. }), None) => rect()
                .spacing(gap)
                .child(note("Letra sin sincronizar"))
                .child(
                    label()
                        .text(text.clone())
                        .font_size(size)
                        .font_weight(FontWeight::SEMI_BOLD)
                        .line_height(1.6)
                        .color(dim(AHEAD))
                        .width(Size::fill()),
                )
                .into_element(),
            (Load::Failed(message), _) => note(message.clone()).into_element(),
            _ => note("Buscando la letra…").into_element(),
        };

        ScrollView::new_controlled(scroll)
            .show_scrollbar(false)
            .width(Size::fill())
            .height(Size::fill())
            .child(
                rect()
                    .width(Size::fill())
                    .padding((view * PIN, 24., view * 0.7, 0.))
                    .child(body)
                    .map(credits(&state, translation.as_ref()), |page, credits| {
                        page.child(rect().margin((size * 2., 0., 0., 0.)).spacing(4.).children(
                            credits.into_iter().map(|line| {
                                label()
                                    .text(line)
                                    .font_size(text::BODY)
                                    .color(dim(AHEAD))
                                    .width(Size::fill())
                                    .into()
                            }),
                        ))
                    }),
            )
    }
}

/// The sheet with ♪ ♪ ♪ for a long intro and every long silence between lines.
fn items(lines: &[LyricsLine]) -> Vec<Item> {
    let mut items = Vec::new();
    if let Some(first) = lines.first()
        && first.start >= BREAK
    {
        items.push(Item::Notes(Duration::ZERO, first.start));
    }
    for (index, line) in lines.iter().enumerate() {
        items.push(Item::Line(index));
        let end = line.sung_end().unwrap_or(line.start);
        if let Some(next) = lines.get(index + 1)
            && next.start.saturating_sub(end) >= BREAK
        {
            items.push(Item::Notes(end, next.start));
        }
    }
    items
}

/// Which item is current at `elapsed`: an instrumental stretch being played, otherwise the
/// line being sung.
fn current(items: &[Item], lines: &[LyricsLine], elapsed: Duration) -> Option<usize> {
    if let Some(at) = items
        .iter()
        .position(|item| matches!(item, Item::Notes(from, to) if *from <= elapsed && elapsed < *to))
    {
        return Some(at);
    }
    let line = lyrics::active(lines, elapsed)?;
    items.iter().position(|item| *item == Item::Line(line))
}

/// What the timed sheet is drawn from.
struct SyncedSheet {
    lines: std::sync::Arc<[LyricsLine]>,
    items: Vec<Item>,
    current: Option<usize>,
    elapsed: Duration,
    playing: bool,
    size: f32,
    gap: f32,
}

/// Every item of a timed sheet.
fn synced(sheet: SyncedSheet, mut heights: State<Vec<f32>>) -> impl IntoElement {
    let SyncedSheet {
        lines,
        items,
        current,
        elapsed,
        playing,
        size,
        gap,
    } = sheet;
    rect()
        .width(Size::fill())
        .spacing(gap)
        .children(items.into_iter().enumerate().map(|(index, item)| {
            let lit = current == Some(index);
            let past = current.is_some_and(|current| index < current);
            let content = match item {
                Item::Notes(from, to) => notes(from, to, elapsed, lit, past, size).into_element(),
                Item::Line(line) if lit => SungLine {
                    line: lines[line].clone(),
                    elapsed,
                    playing,
                    size,
                }
                .into_element(),
                Item::Line(line) => still(
                    &lines[line],
                    match past {
                        true => PAST,
                        false => AHEAD,
                    },
                    size,
                )
                .into_element(),
            };
            rect()
                .key(index)
                .width(Size::fill())
                .on_sized(move |event: Event<SizedEventData>| {
                    let height = event.area.height();
                    let known = heights.peek().get(index).copied();
                    if known.is_none_or(|known| (known - height).abs() > 0.5) {
                        let mut heights = heights.write();
                        if heights.len() <= index {
                            heights.resize(index + 1, size * 1.3);
                        }
                        heights[index] = height;
                    }
                })
                .child(content)
                .into()
        }))
}

/// A line that is not being sung, at `shade` of full brightness.
fn still(line: &LyricsLine, shade: f32, size: f32) -> impl IntoElement {
    rect()
        .width(Size::fill())
        .spacing(4.)
        .child(
            label()
                .text(line.text.clone())
                .font_size(size)
                .font_weight(FontWeight::SEMI_BOLD)
                .color(dim(shade))
                .text_align(align(line))
                .width(Size::fill()),
        )
        .children(extras(line, shade * 0.8))
}

/// A duet's second voice sings from the right, as Sonora and Apple Music show it.
fn align(line: &LyricsLine) -> TextAlign {
    match line.voice.lead() {
        true => TextAlign::Left,
        false => TextAlign::Right,
    }
}

/// Sonora's instrumental row: three ♪ that light up in turn as the stretch plays.
fn notes(
    from: Duration,
    to: Duration,
    elapsed: Duration,
    lit: bool,
    past: bool,
    size: f32,
) -> impl IntoElement {
    let length = to.saturating_sub(from).as_secs_f32().max(0.1);
    let progress = match (lit, past) {
        (true, _) => (elapsed.saturating_sub(from).as_secs_f32() / length).clamp(0., 1.),
        (false, true) => 1.,
        (false, false) => 0.,
    };
    rect()
        .direction(Direction::Horizontal)
        .spacing(size * 0.4)
        .padding((size * 0.3, 0.))
        .children((0..3).map(|index| {
            let lit_note = (progress * 3. - index as f32).clamp(0., 1.);
            let shade = match past {
                true => PAST,
                false => AHEAD + (1. - AHEAD) * lit_note,
            };
            ui::icon(Icon::Playing, size, dim(shade)).into_element()
        }))
}

/// Romanization and background vocals under a line.
fn extras(line: &LyricsLine, shade: f32) -> Vec<Element> {
    let side = align(line);
    let mut extra = Vec::new();
    if let Some(translated) = &line.translation {
        extra.push(
            label()
                .text(translated.clone())
                .font_size(text::LARGE)
                .font_weight(FontWeight::MEDIUM)
                .color(dim(shade))
                .text_align(side)
                .width(Size::fill())
                .into(),
        );
    }
    if let Some(romanized) = &line.romanized {
        extra.push(
            label()
                .text(romanized.text.clone())
                .font_size(text::LARGE)
                .color(dim(shade))
                .text_align(side)
                .width(Size::fill())
                .into(),
        );
    }
    for lane in &line.secondary {
        extra.push(
            label()
                .text(format!("({})", lane.text.trim()))
                .font_size(text::LARGE)
                .color(dim(shade * 0.8))
                .text_align(side)
                .width(Size::fill())
                .into(),
        );
    }
    extra
}

/// The line being sung. On a word-synced sheet it keeps its own clock between position ticks
/// and repaints every frame, so each word lights up smoothly as it is sung.
#[derive(PartialEq)]
struct SungLine {
    line: LyricsLine,
    elapsed: Duration,
    playing: bool,
    size: f32,
}

impl Component for SungLine {
    fn render(&self) -> impl IntoElement {
        let worded = self
            .line
            .words
            .as_ref()
            .is_some_and(|words| !words.is_empty());
        let reported = use_reactive(&self.elapsed);
        let mut anchor = use_state(|| (self.elapsed, Instant::now()));
        if anchor.peek().0 != *reported.peek() {
            anchor.set((*reported.peek(), Instant::now()));
        }
        // A looping animation used only as a frame clock while words are being swept.
        let clock = use_animation(|config| {
            config.on_creation(OnCreation::Run);
            config.on_finish(OnFinish::restart());
            AnimNum::new(0., 1.).time(1000)
        });
        if worded && self.playing {
            let _ = clock.get();
        }
        let (base, at) = *anchor.read();
        let now = match self.playing {
            true => base + at.elapsed().min(Duration::from_millis(400)),
            false => base,
        };

        let words = self.line.words.clone().unwrap_or_default();
        rect()
            .width(Size::fill())
            .spacing(4.)
            .child(match worded {
                true => paragraph()
                    .font_size(self.size + GROWTH)
                    .font_weight(FontWeight::SEMI_BOLD)
                    .text_align(align(&self.line))
                    .width(Size::fill())
                    .spans_iter(words.into_iter().map(move |word| sung(word, now)))
                    .into_element(),
                false => label()
                    .text(self.line.text.clone())
                    .font_size(self.size + GROWTH)
                    .font_weight(FontWeight::SEMI_BOLD)
                    .color(color::FOREGROUND)
                    .text_align(align(&self.line))
                    .width(Size::fill())
                    .into_element(),
            })
            .children(extras(&self.line, 0.8))
    }
}

/// A word of the sung line, brighter the further it has been sung.
fn sung(word: LyricsWord, now: Duration) -> Span<'static> {
    let length = word.end.saturating_sub(word.start).as_secs_f32().max(0.05);
    let into = now.saturating_sub(word.start).as_secs_f32();
    let done = match now >= word.start {
        true => (into / length).clamp(0., 1.),
        false => 0.,
    };
    // A few brightness steps per word: most frames then build the very same spans, which
    // Freya sees as unchanged and does not lay out again.
    let done = (done * STEPS).round() / STEPS;
    Span::new(word.text).color(dim(UNSUNG + (1. - UNSUNG) * done))
}

/// The foreground colour at `shade` of its brightness.
fn dim(shade: f32) -> Color {
    let alpha = (shade.clamp(0., 1.) * 255.) as u8;
    Color::from_argb(alpha, 0xfa, 0xfa, 0xfa)
}

fn note(message: impl Into<String>) -> impl IntoElement {
    ui::line(message, text::LARGE, color::MUTED_FOREGROUND)
}

/// Sonora's footer under a sheet: where it came from and who wrote the song.
fn credits(
    sheet: &crate::state::Sheet,
    translation: Option<&lyrics::Translation>,
) -> Option<Vec<String>> {
    let source = sheet.source?;
    let mut lines = vec![format!("Letra de {source}")];
    if !sheet.writers.is_empty() {
        lines.push(format!("Escrita por {}", sheet.writers.join(", ")));
    }
    if let Some(found) = translation {
        lines.push(match found.machine {
            true => format!("Traducción automática de {}", found.source),
            false => format!("Traducción de {}", found.source),
        });
    }
    Some(lines)
}

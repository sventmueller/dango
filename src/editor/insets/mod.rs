//! Text shown in the editor that is not in the file (E81): inlay hints, blame and edit prediction's ghost text, later
//! the debugger's values. None of it enters the text, its undo or the saved file. At the end of a line it is only drawn, in the
//! overlay. In the middle of a line the last code point before it gets a kern attribute as wide as the text, an
//! attribute change without a text change or an undo step, and the overlay draws into the gap. Where no character
//! precedes it on its line, it goes to the line's end.

mod draw;
mod measure;
mod rotor;

use std::ops::Range;
use std::rc::Rc;

use ide_document::Change;
use objc2::rc::Retained;
use objc2::runtime::AnyObject;
use objc2::{AnyThread, DefinedClass, define_class, msg_send, sel};
use objc2_app_kit::{
    NSColor, NSFontAttributeName, NSForegroundColorAttributeName, NSKernAttributeName, NSTextInputClient, NSTextStorage,
};
use objc2_foundation::{NSDictionary, NSNumber, NSObjectProtocol, NSRange, NSRect, NSString};

use super::view::EditorTextView;
use crate::util::constant;
#[cfg(feature = "selftest")]
use draw::fill;
use rotor::Rotor;

pub(in crate::editor) use draw::line_fragments;
pub use draw::{click, click_gap, draw, draw_fragment, hits, offers, retarget, shown, tooltip, width_after_text};
pub use rotor::attach;
#[cfg(feature = "selftest")]
mod selftest_probe;
#[cfg(feature = "selftest")]
pub use selftest_probe::*;

/// The part of the app an inset belongs to; each sets and clears its own.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Layer {
    /// What edit prediction proposes to write at the caret, and where a proposal elsewhere waits (E92); first in a gap
    /// and at a line's end, right where the caret is.
    Prediction,
    /// The language server's inlay hints (E83).
    Inlay,
    /// The values of the paused frame's variables at the ends of the lines that use them (E94).
    Debug,
    /// The most serious diagnostic of a line (E130).
    Diagnostics,
    /// Who changed the cursor's line last (E86); at a line's end the first to give way when room runs out.
    Blame,
}

/// Where an inset wants to show.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Place {
    /// Before the character at its offset, in a gap of the line; at the line's end where no character precedes it.
    Inline,
    /// After the end of its offset's line.
    LineEnd,
}

/// What a click on an inset does.
pub type Action = Rc<dyn Fn(&EditorTextView)>;

/// One text that is not in the file.
#[derive(Clone)]
pub struct Inset {
    /// The UTF-16 offset it belongs to.
    pub at: usize,
    pub text: String,
    pub place: Place,
    pub tooltip: Option<String>,
    /// What a click on it does; without one a click goes on to the text.
    pub action: Option<Action>,
    /// Room for a space before or after it, as the language server asks.
    pub pad_left: bool,
    pub pad_right: bool,
    /// Ghost text: drawn as code would be, italic in the secondary colour and without a box, with the caret before it;
    /// at a line's end it follows the text without a space.
    pub ghost: bool,
    /// How serious it is, shown with a symbol before the text in the severity's colour.
    pub severity: Option<ide_ui_lsp::Severity>,
    /// Its width on screen, with its margins, measured when it is set.
    width: f64,
}

impl Inset {
    pub fn new(at: usize, text: impl Into<String>, place: Place) -> Inset {
        Inset {
            at,
            text: text.into(),
            place,
            tooltip: None,
            action: None,
            pad_left: false,
            pad_right: false,
            ghost: false,
            severity: None,
            width: 0.0,
        }
    }
}

impl std::fmt::Debug for Inset {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Inset")
            .field("at", &self.at)
            .field("text", &self.text)
            .field("place", &self.place)
            .field("clickable", &self.action.is_some())
            .finish_non_exhaustive()
    }
}

/// A kern attribute this module put on a code point, with its value.
#[derive(Debug, Clone, PartialEq)]
struct Kerned {
    range: Range<usize>,
    width: f64,
    /// Whether the caret after the kerned code point stands at the gap's left edge, before ghost text.
    left: bool,
}

/// Where an inset was drawn last, in the editor's coordinates.
#[derive(Debug, Clone)]
pub struct Shown {
    pub rect: NSRect,
    pub layer: Layer,
    pub index: usize,
    /// Whether it sits in a gap of its line rather than after the line's end.
    pub in_gap: bool,
    /// For a gap, the offset after the character it follows, where the caret stands right of the gap.
    pub gap_end: Option<usize>,
}

/// What an editor shows that is not in its text.
#[derive(Default)]
pub struct Insets {
    layers: Vec<(Layer, Vec<Inset>)>,
    kerned: Vec<Kerned>,
    /// Text an edit put in since the last settling, which may have taken a kern attribute from its neighbour.
    sweep: Option<Range<usize>>,
    /// A layer changed since the last settling, so every gap is looked at again, not only those near an edit.
    full: bool,
    shown: Vec<Shown>,
    rotor: Option<Retained<Rotor>>,
    /// The text layout's delegate that makes the fragments that draw the insets; the layout holds it weakly.
    layout: Option<Retained<super::inset_fragment::InsetLayout>>,
    widths: measure::Widths,
}

/// Room around an inset's text inside its box.
const MARGIN: f64 = 4.0;
/// The room a server's padding adds on one side.
const PADDING: f64 = 3.0;
/// Between the end of a line and the first inset after it, and between two insets there.
const END_GAP: f64 = 14.0;

/// The size of an inset's text for an editor whose text has `font_size`.
fn text_size(font_size: f64) -> f64 {
    (font_size * 0.85).round()
}

/// The colour of an inset's text in `view`: the secondary label colour, the label colour itself with increased
/// contrast, where the secondary one stays below E66's 4.5:1 on the box.
fn text_color(view: &EditorTextView) -> Retained<NSColor> {
    use objc2_app_kit::NSAppearanceCustomization;
    let system = objc2_app_kit::NSWorkspace::sharedWorkspace().accessibilityDisplayShouldIncreaseContrast();
    if system || forced_contrast() || super::style::increased_contrast(&view.effectiveAppearance()) {
        NSColor::labelColor()
    } else {
        NSColor::secondaryLabelColor()
    }
}

#[cfg(not(feature = "selftest"))]
fn forced_contrast() -> bool {
    false
}

fn attributes(font_size: f64, color: &NSColor) -> Retained<NSDictionary<NSString, AnyObject>> {
    NSDictionary::<NSString, AnyObject>::from_retained_objects(
        &[
            constant!(NSFontAttributeName),
            constant!(NSForegroundColorAttributeName),
        ],
        &[
            super::style::code_font(text_size(font_size)).into(),
            objc2::Message::retain(color).into(),
        ],
    )
}

/// The attributes of ghost text in an editor whose text has `font_size`.
fn ghost_attributes(font_size: f64) -> Retained<NSDictionary<NSString, AnyObject>> {
    NSDictionary::<NSString, AnyObject>::from_retained_objects(
        &[
            constant!(NSFontAttributeName),
            constant!(NSForegroundColorAttributeName),
        ],
        &[
            super::style::styled_code_font(font_size, false, true).into(),
            NSColor::secondaryLabelColor().into(),
        ],
    )
}

/// The text an inset shows, its severity's symbol first.
fn shown_text(inset: &Inset) -> String {
    match inset.severity {
        Some(severity) => format!("{} {}", severity.symbol(), inset.text),
        None => inset.text.clone(),
    }
}

/// Shows `insets` for `layer` in `view`, in place of the ones it had.
pub fn set(view: &EditorTextView, layer: Layer, mut insets: Vec<Inset>) {
    let font_size = view.ivars().font_size.get();
    let attributes = attributes(font_size, &NSColor::secondaryLabelColor());
    {
        let widths = &mut view.ivars().insets.borrow_mut().widths;
        for inset in &mut insets {
            inset.width = widths.of(inset, &attributes, font_size);
        }
    }
    insets.sort_by_key(|inset| inset.at);
    let changed = {
        let state = view.ivars().insets.borrow();
        measure::changed_span(layer_insets(&state, layer).map_or(&[], Vec::as_slice), &insets)
    };
    {
        let mut state = view.ivars().insets.borrow_mut();
        state.layers.retain(|(l, _)| *l != layer);
        if !insets.is_empty() {
            state.layers.push((layer, insets));
            state.layers.sort_by_key(|(l, _)| *l);
        }
        // Only the gaps of the insets that changed need a look; the others keep their kern.
        match &changed {
            Some(span) => {
                let swept = state.sweep.take();
                state.sweep =
                    Some(swept.map_or_else(|| span.clone(), |s| s.start.min(span.start)..s.end.max(span.end)));
            }
            None => state.full = true,
        }
    }
    settle(view);
    if let Some(span) = changed {
        redraw_text(view, span);
    }
    super::overlay::redraw(view);
}

/// Has TextKit lay out and draw the paragraphs of `span` again, which draw their insets.
fn redraw_text(view: &EditorTextView, span: Range<usize>) {
    let (Some(layout), Some(content)) = (view.textLayoutManager(), view.textContentStorage()) else {
        return;
    };
    let length = view.storage().map_or(0, |s| s.length());
    let start = objc2_app_kit::NSTextElementProvider::documentRange(&*content).location();
    let at = |offset: usize| content.locationFromLocation_withOffset(&start, isize::try_from(offset.min(length)).ok()?);
    let (Some(from), Some(to)) = (at(span.start), at(span.end)) else {
        return;
    };
    if let Some(range) =
        objc2_app_kit::NSTextRange::initWithLocation_endLocation(objc2_app_kit::NSTextRange::alloc(), &from, Some(&to))
    {
        layout.invalidateLayoutForRange(&range);
    }
}

/// Takes away everything `layer` shows in `view`.
pub fn clear(view: &EditorTextView, layer: Layer) {
    let had = view.ivars().insets.borrow().layers.iter().any(|(l, _)| *l == layer);
    if had {
        set(view, layer, Vec::new());
    }
}

/// What `layer` shows in `view` now.
pub fn of(view: &EditorTextView, layer: Layer) -> Vec<Inset> {
    view.ivars()
        .insets
        .borrow()
        .layers
        .iter()
        .find(|(l, _)| *l == layer)
        .map(|(_, insets)| insets.clone())
        .unwrap_or_default()
}

/// Moves an offset with an edit; false when the edit replaced the character before it, which the inset belonged to.
fn follow_point(at: &mut usize, change: &Change) -> bool {
    let old = &change.utf16;
    if *at <= old.start {
        return true;
    }
    if *at <= old.end {
        return false;
    }
    *at = *at - (old.end - old.start) + change.text_utf16;
    true
}

/// Moves a kerned code point with an edit; false when the edit replaced it.
fn follow_range(range: &mut Range<usize>, change: &Change) -> bool {
    let old = &change.utf16;
    if range.end <= old.start {
        return true;
    }
    if range.start >= old.end {
        let shift = |x: usize| x - (old.end - old.start) + change.text_utf16;
        *range = shift(range.start)..shift(range.end);
        return true;
    }
    false
}

/// Keeps the insets on their text while it changes; an inset whose character went goes too, until its layer sets it
/// again.
pub fn shift(view: &EditorTextView, change: &Change) {
    let Ok(mut state) = view.ivars().insets.try_borrow_mut() else {
        return;
    };
    if state.layers.is_empty() && state.kerned.is_empty() {
        return;
    }
    for (_, insets) in &mut state.layers {
        insets.retain_mut(|inset| follow_point(&mut inset.at, change));
    }
    state.layers.retain(|(_, insets)| !insets.is_empty());
    state
        .kerned
        .retain_mut(|kerned| follow_range(&mut kerned.range, change));
    publish(view, &state.kerned);
    let added = change.utf16.start..change.utf16.start + change.text_utf16;
    state.sweep = Some(match state.sweep.take() {
        Some(sweep) => sweep.start.min(added.start)..sweep.end.max(added.end),
        None => added,
    });
    drop(state);
    view.later(sel!(ideInsets), 0.0);
    super::overlay::redraw(view);
}

/// The code point before UTF-16 offset `at` that a gap there kerns, and `at` moved to the end of the grapheme it lies
/// in; `None` where no character precedes it on its line.
/// A grapheme of several code points, such as a letter with a combining mark or an emoji with a modifier, falls back
/// too: TextKit 2 ignores a kern on its last code point (R14), and a kern anywhere else would sit inside it.
fn anchor(
    units: impl Fn(usize) -> u16,
    grapheme: impl Fn(usize) -> Range<usize>,
    at: usize,
    length: usize,
) -> Option<(Range<usize>, usize)> {
    if at == 0 || at > length {
        return None;
    }
    let at = if at < length {
        let around = grapheme(at);
        if around.start < at { around.end } else { at }
    } else {
        at
    };
    let before = units(at - 1);
    if before == u16::from(b'\n') || before == u16::from(b'\r') {
        return None;
    }
    let low = (0xDC00..=0xDFFF).contains(&before);
    let start = if low && at >= 2 && (0xD800..=0xDBFF).contains(&units(at - 2)) {
        at - 2
    } else {
        at - 1
    };
    (grapheme(start).start == start).then_some((start..at, at))
}

/// Whether an inline inset at UTF-16 offset `at` of `view` gets a gap of its line, rather than going to the line's end.
pub fn gap_possible(view: &EditorTextView, at: usize) -> bool {
    let Some(storage) = view.storage() else {
        return false;
    };
    let string = storage.string();
    let units = |i: usize| string.characterAtIndex(i);
    let grapheme = |i: usize| {
        let range = string.rangeOfComposedCharacterSequenceAtIndex(i);
        range.location..range.location + range.length
    };
    anchor(units, grapheme, at, string.length()).is_some()
}

/// Where an inset shows, before drawing.
#[derive(Debug, Clone, PartialEq)]
enum Spot {
    /// In a gap after the code point `anchor`, which is kerned.
    Gap { anchor: Range<usize> },
    /// After the end of a line, whose content ends at this UTF-16 offset.
    End { line_end: usize },
}

#[derive(Debug, Clone)]
struct Placed {
    layer: Layer,
    index: usize,
    spot: Spot,
    width: f64,
    ghost: bool,
}

/// The UTF-16 offset where the content of the line holding `at` ends, before its line break.
fn line_end(view: &EditorTextView, at: usize) -> usize {
    let document = view.document();
    let text = document.text();
    let Ok(byte) = text.utf16_to_byte(at.min(text.len_utf16())) else {
        return text.len_utf16();
    };
    text.line_content(text.line_of(byte))
        .and_then(|content| text.byte_to_utf16(content.end).ok())
        .unwrap_or_else(|| text.len_utf16())
}

/// Where each inset of `state` whose offset lies in `window` shows in the text `string`.
fn resolve(view: &EditorTextView, state: &Insets, string: &NSString, window: &Range<usize>) -> Vec<Placed> {
    let length = string.length();
    let units = |i: usize| string.characterAtIndex(i);
    let grapheme = |i: usize| {
        let range = string.rangeOfComposedCharacterSequenceAtIndex(i);
        range.location..range.location + range.length
    };
    let mut placed = Vec::new();
    for (layer, insets) in &state.layers {
        let first = insets.partition_point(|inset| inset.at < window.start);
        let last = insets.partition_point(|inset| inset.at <= window.end);
        for (index, inset) in insets.iter().enumerate().take(last).skip(first) {
            let gap = match inset.place {
                Place::Inline => anchor(units, grapheme, inset.at, length),
                Place::LineEnd => None,
            };
            let spot = match gap {
                Some((anchor, _)) => Spot::Gap { anchor },
                None => Spot::End {
                    line_end: line_end(view, inset.at),
                },
            };
            placed.push(Placed {
                layer: *layer,
                index,
                spot,
                width: inset.width,
                ghost: inset.ghost,
            });
        }
    }
    placed
}

/// The kern each gap needs, ordered by place: the widths of its insets together.
fn wanted(placed: &[Placed]) -> Vec<Kerned> {
    let mut gaps: Vec<(Range<usize>, f64, bool)> = placed
        .iter()
        .filter_map(|place| match &place.spot {
            Spot::Gap { anchor } => Some((anchor.clone(), place.width, place.ghost)),
            Spot::End { .. } => None,
        })
        .collect();
    gaps.sort_by_key(|(range, _, _)| (range.start, range.end));
    let mut kerns: Vec<Kerned> = Vec::with_capacity(gaps.len());
    for (range, width, ghost) in gaps {
        match kerns.last_mut() {
            Some(last) if last.range == range => {
                last.width += width;
                last.left |= ghost;
            }
            _ => kerns.push(Kerned {
                range,
                width,
                left: ghost,
            }),
        }
    }
    kerns
}

/// The kern on `range` in `kerns`, which is ordered by place.
fn find_kern<'a>(kerns: &'a [Kerned], range: &Range<usize>) -> Option<&'a Kerned> {
    let index = kerns.partition_point(|k| (k.range.start, k.range.end) < (range.start, range.end));
    kerns.get(index).filter(|k| k.range == *range)
}

/// The stretches inside `range` that carry a kern attribute.
fn kerned_in(storage: &NSTextStorage, range: Range<usize>) -> Vec<Range<usize>> {
    let end = range.end.min(storage.length());
    let key = constant!(NSKernAttributeName);
    let mut found = Vec::new();
    let mut at = range.start;
    while at < end {
        let mut effective = NSRange::new(at, 0);
        let limit = NSRange::new(at, end - at);
        // SAFETY: The index lies inside the limit, which lies inside the text; `effective` is a valid range to fill.
        let value =
            unsafe { storage.attribute_atIndex_longestEffectiveRange_inRange(key, at, &raw mut effective, limit) };
        let next = (effective.location + effective.length).max(at + 1);
        if value.is_some() {
            found.push(effective.location..next);
        }
        at = next;
    }
    found
}

/// Puts the kern attributes where the gaps are now, and takes them from text an edit gave them to by inheritance.
/// After an edit only the gaps near it are looked at; the others moved with the text. Waits while an input method has
/// marked text, which an attribute change would disturb.
pub fn settle(view: &EditorTextView) {
    if view.hasMarkedText() {
        view.later(sel!(ideInsets), 0.1);
        return;
    }
    let Some(storage) = view.storage() else { return };
    let Ok(mut state) = view.ivars().insets.try_borrow_mut() else {
        return;
    };
    let length = storage.length();
    let sweep = state.sweep.take();
    let window = if std::mem::take(&mut state.full) || state.kerned.is_empty() {
        0..length
    } else {
        match &sweep {
            Some(sweep) => sweep.start.saturating_sub(2)..(sweep.end + 2).min(length),
            None => return,
        }
    };
    let wanted: Vec<Kerned> = wanted(&resolve(view, &state, &storage.string(), &window))
        .into_iter()
        .filter(|k| k.range.end <= length)
        .collect();
    let inside = |k: &Kerned| k.range.start >= window.start && k.range.start <= window.end;
    let mut kerned = std::mem::take(&mut state.kerned);
    kerned.sort_by_key(|k| (k.range.start, k.range.end));
    let old: Vec<Kerned> = kerned.iter().filter(|k| inside(k)).cloned().collect();
    kerned.retain(|k| !inside(k));
    let mut remove: Vec<Range<usize>> = old
        .iter()
        .filter(|k| find_kern(&wanted, &k.range) != Some(k) && k.range.end <= length)
        .map(|k| k.range.clone())
        .collect();
    if let Some(sweep) = sweep {
        let widened = sweep.start.saturating_sub(1)..(sweep.end + 1).min(length);
        remove.extend(
            kerned_in(&storage, widened)
                .into_iter()
                .filter(|r| find_kern(&wanted, r).is_none()),
        );
    }
    remove.sort_by_key(|r| r.start);
    apply(&storage, &remove, &wanted, &old);
    kerned.extend(wanted);
    kerned.sort_by_key(|k| (k.range.start, k.range.end));
    state.kerned = kerned;
    publish(view, &state.kerned);
    drop(state);
    strip_typing(view);
    super::overlay::redraw(view);
}

/// Takes the kern from `remove` and puts it on what `wanted` has that `old` lacks or `remove` touched.
fn apply(storage: &NSTextStorage, remove: &[Range<usize>], wanted: &[Kerned], old: &[Kerned]) {
    let touched = |k: &Kerned| {
        let index = remove.partition_point(|r| r.end <= k.range.start);
        remove.get(index).is_some_and(|r| r.start < k.range.end)
    };
    let add: Vec<&Kerned> = wanted
        .iter()
        .filter(|k| find_kern(old, &k.range) != Some(k) || touched(k))
        .collect();
    if remove.is_empty() && add.is_empty() {
        return;
    }
    let key = constant!(NSKernAttributeName);
    storage.beginEditing();
    for range in remove {
        storage.removeAttribute_range(key, NSRange::new(range.start, range.end - range.start));
    }
    for kern in add {
        let range = NSRange::new(kern.range.start, kern.range.end - kern.range.start);
        // SAFETY: The kern key takes a number.
        unsafe { storage.addAttribute_value_range(key, &NSNumber::new_f64(kern.width), range) };
    }
    storage.endEditing();
}

/// The text of `view` was put in whole, without its attributes: every gap needs its kern again.
pub fn reset(view: &EditorTextView) {
    if let Ok(mut state) = view.ivars().insets.try_borrow_mut() {
        state.kerned.clear();
        state.full = true;
    }
    settle(view);
}

/// `attributes` without a kern, when they have one: text typed after a gap must not open a gap of its own.
pub fn without_kern(
    attributes: &NSDictionary<NSString, AnyObject>,
) -> Option<Retained<NSDictionary<NSString, AnyObject>>> {
    let key = constant!(NSKernAttributeName);
    attributes.objectForKey(key)?;
    let copy = objc2_foundation::NSMutableDictionary::dictionaryWithDictionary(attributes);
    copy.removeObjectForKey(key);
    Some(copy.into_super())
}

/// Takes a kern out of the typing attributes of `view`, which AppKit copies from the character before the caret.
pub fn strip_typing(view: &EditorTextView) {
    if let Some(clean) = without_kern(&view.typingAttributes()) {
        // SAFETY: The dictionary holds the attributes the view had, which are valid typing attributes.
        unsafe { view.setTypingAttributes(&clean) };
    }
}

fn layer_insets(state: &Insets, layer: Layer) -> Option<&Vec<Inset>> {
    state.layers.iter().find(|(l, _)| *l == layer).map(|(_, insets)| insets)
}

/// Tells the layout manager of `view` where the gaps are, by the offset after each and its width.
fn publish(view: &EditorTextView, kerned: &[Kerned]) {
    let Some(layout) = view.textLayoutManager() else {
        return;
    };
    if let Ok(gaps) = layout.downcast::<GapLayoutManager>() {
        *gaps.ivars().gaps.borrow_mut() = kerned
            .iter()
            .map(|k| (k.range.end, k.width, k.left))
            .collect::<Vec<_>>()
            .into();
    }
}

pub struct GapIvars {
    /// The offset after each gap's character, the gap's width and whether the caret stands at its left edge, ordered
    /// by offset.
    gaps: std::cell::RefCell<Rc<[(usize, f64, bool)]>>,
}

define_class!(
    // SAFETY: NSTextLayoutManager may be subclassed; `GapLayoutManager` overrides one method, calls the superclass's
    // implementation from it, and does not implement Drop.
    #[unsafe(super(objc2_app_kit::NSTextLayoutManager))]
    #[name = "IDEGapLayoutManager"]
    #[ivars = GapIvars]
    pub struct GapLayoutManager;

    unsafe impl NSObjectProtocol for GapLayoutManager {}

    impl GapLayoutManager {
        #[unsafe(method(enumerateTextSegmentsInRange:type:options:usingBlock:))]
        fn enumerate_segments(
            &self,
            range: &objc2_app_kit::NSTextRange,
            kind: objc2_app_kit::NSTextLayoutManagerSegmentType,
            options: objc2_app_kit::NSTextLayoutManagerSegmentOptions,
            block: &block2::DynBlock<SegmentFn<'_>>,
        ) {
            let moved = crate::util::guarded(None, || self.moved_segments(block));
            let block = moved.as_deref().unwrap_or(block);
            // SAFETY: The superclass's implementation of this method, passed the caller's arguments and a block of the
            // caller's block type.
            let _: () = unsafe {
                msg_send![super(self), enumerateTextSegmentsInRange: range, type: kind, options: options, usingBlock: block]
            };
        }
    }
);

/// TextKit 2 puts the caret after a kerned character in the middle of the kern: a segment that starts there starts at
/// the gap's right edge instead, and one that ends there ends at its left edge. A caret before ghost text stands at the
/// gap's left edge, so the proposal follows it.
fn caret_right_of_gap(gaps: &[(usize, f64, bool)], mut rect: NSRect, from: isize, to: isize) -> NSRect {
    let gap_at = |offset: isize| {
        let offset = usize::try_from(offset).ok()?;
        let index = gaps.partition_point(|(end, _, _)| *end < offset);
        gaps.get(index)
            .filter(|(end, _, _)| *end == offset)
            .map(|(_, w, left)| (*w / 2.0, *left))
    };
    match gap_at(from) {
        Some((half, true)) if to == from => rect.origin.x -= half,
        Some((half, _)) => {
            rect.origin.x += half;
            rect.size.width = (rect.size.width - half).max(0.0);
        }
        None => {}
    }
    if to > from
        && let Some((half, _)) = gap_at(to)
    {
        rect.size.width = (rect.size.width - half).max(0.0);
    }
    rect
}

/// The block TextKit hands each text segment to: the segment's range, its rect, baseline and text container.
type SegmentFn<'a> = dyn Fn(
        *mut objc2_app_kit::NSTextRange,
        NSRect,
        f64,
        std::ptr::NonNull<objc2_app_kit::NSTextContainer>,
    ) -> objc2::runtime::Bool
    + 'a;

impl GapLayoutManager {
    /// `block` behind a block that moves each segment off the gaps' kerns, or `None` while there are no gaps to move it
    /// off; the gaps are only read on the main thread, which is the one that writes them.
    fn moved_segments<'a>(&self, block: &'a block2::DynBlock<SegmentFn<'a>>) -> Option<block2::RcBlock<SegmentFn<'a>>> {
        objc2::MainThreadMarker::new()?;
        let gaps = self.ivars().gaps.try_borrow().ok()?.clone();
        if gaps.is_empty() {
            return None;
        }
        let content = self.textContentManager()?;
        let start = objc2_app_kit::NSTextElementProvider::documentRange(&*content).location();
        Some(block2::RcBlock::new(
            move |segment: *mut objc2_app_kit::NSTextRange,
                  rect: NSRect,
                  baseline: f64,
                  container: std::ptr::NonNull<objc2_app_kit::NSTextContainer>| {
                crate::util::guarded(objc2::runtime::Bool::NO, || {
                    // SAFETY: TextKit passes a valid text range or none, alive for the call.
                    let segment = unsafe { segment.as_ref() };
                    let rect = match segment {
                        Some(segment) => {
                            let offset =
                                |location: &objc2::runtime::ProtocolObject<dyn objc2_app_kit::NSTextLocation>| {
                                    objc2_app_kit::NSTextElementProvider::offsetFromLocation_toLocation(
                                        &*content, &start, location,
                                    )
                                };
                            let (from, to) = (offset(&segment.location()), offset(&segment.endLocation()));
                            caret_right_of_gap(&gaps, rect, from, to)
                        }
                        None => rect,
                    };
                    let segment = segment.map_or(std::ptr::null_mut(), |s| std::ptr::from_ref(s).cast_mut());
                    block.call((segment, rect, baseline, container))
                })
            },
        ))
    }

    /// A layout manager that knows no gaps yet.
    pub fn new() -> Retained<GapLayoutManager> {
        let this = Self::alloc().set_ivars(GapIvars {
            gaps: std::cell::RefCell::new(Rc::new([])),
        });
        // SAFETY: NSTextLayoutManager's initializer, called once on the newly allocated object.
        unsafe { msg_send![super(this), init] }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn units(text: &str) -> Vec<u16> {
        text.encode_utf16().collect()
    }

    /// Grapheme bounds as NSString gives them for the texts below: a base with its combining mark, and an emoji with
    /// its skin tone modifier, each count as one.
    fn graphemes(text: &str) -> impl Fn(usize) -> Range<usize> {
        let clusters: Vec<Range<usize>> = match text {
            "e\u{301}x" => vec![0..2, 2..3],
            "👍🏽y" => vec![0..4, 4..5],
            "😀y" => vec![0..2, 2..3],
            _ => vec![],
        };
        move |i| clusters.iter().find(|r| r.contains(&i)).cloned().unwrap_or(i..i + 1)
    }

    fn anchor_in(text: &str, at: usize) -> Option<(Range<usize>, usize)> {
        let u = units(text);
        anchor(|i| u[i], graphemes(text), at, u.len())
    }

    #[test]
    fn a_gap_kerns_the_code_point_before_it() {
        assert_eq!(anchor_in("let x = 1;", 5), Some((4..5, 5)));
    }

    #[test]
    fn nothing_precedes_the_start_of_a_line_or_an_empty_text() {
        assert_eq!(anchor_in("", 0), None);
        assert_eq!(anchor_in("abc", 0), None);
        assert_eq!(anchor_in("ab\ncd", 3), None);
        assert_eq!(anchor_in("ab\r\ncd", 4), None);
    }

    #[test]
    fn the_kern_sits_on_a_grapheme_of_one_code_point_only() {
        let text = "e\u{301}x";
        assert_eq!(
            anchor_in(text, 2),
            None,
            "a letter with a combining mark goes to the line's end"
        );
        assert_eq!(
            anchor_in(text, 1),
            None,
            "inside a grapheme moves to its end, which falls back too"
        );
        let emoji = "👍🏽y";
        assert_eq!(
            anchor_in(emoji, 4),
            None,
            "an emoji with its modifier goes to the line's end"
        );
        assert_eq!(anchor_in(emoji, 1), None, "never between the halves of a pair");
        assert_eq!(
            anchor_in("😀y", 2),
            Some((0..2, 2)),
            "a lone emoji is one code point, both halves kerned"
        );
        assert_eq!(anchor_in("😀y", 1), Some((0..2, 2)));
    }

    fn change(utf16: Range<usize>, text: &str) -> Change {
        let point = ide_document::Point {
            line: 0,
            utf8: 0,
            utf16: 0,
            utf32: 0,
        };
        Change {
            revision: 1,
            bytes: utf16.clone(),
            utf16,
            start: point,
            old_end: point,
            new_end: point,
            text_utf16: text.encode_utf16().count(),
            text: text.into(),
        }
    }

    #[test]
    fn an_offset_follows_edits_and_goes_with_its_character() {
        let mut at = 5;
        assert!(
            follow_point(&mut at, &change(5..5, "yz")),
            "typing at the gap keeps it before the typing"
        );
        assert_eq!(at, 5);
        assert!(follow_point(&mut at, &change(0..0, "ab")));
        assert_eq!(at, 7);
        assert!(
            !follow_point(&mut at, &change(6..7, "")),
            "the character before it went"
        );
        let mut at = 5;
        assert!(
            follow_point(&mut at, &change(5..9, "")),
            "text after it does not matter"
        );
        assert_eq!(at, 5);
    }

    #[test]
    fn the_caret_stands_right_of_an_inlay_gap_and_left_of_ghost_text() {
        let rect = |x: f64, width: f64| {
            NSRect::new(
                objc2_foundation::NSPoint::new(x, 0.0),
                objc2_foundation::NSSize::new(width, 10.0),
            )
        };
        let hint = [(5, 20.0, false)];
        assert_eq!(caret_right_of_gap(&hint, rect(50.0, 0.0), 5, 5).origin.x, 60.0);
        let ghost = [(5, 20.0, true)];
        assert_eq!(caret_right_of_gap(&ghost, rect(50.0, 0.0), 5, 5).origin.x, 40.0);
        let selected = caret_right_of_gap(&ghost, rect(50.0, 30.0), 5, 8);
        assert_eq!(
            (selected.origin.x, selected.size.width),
            (60.0, 20.0),
            "a selection after it starts past the gap"
        );
    }

    #[test]
    fn a_kerned_code_point_follows_edits_and_goes_when_replaced() {
        let mut range = 4..5;
        assert!(follow_range(&mut range, &change(5..5, "x")));
        assert_eq!(range, 4..5);
        assert!(
            follow_range(&mut range, &change(4..4, "ab")),
            "typing before it moves it"
        );
        assert_eq!(range, 6..7);
        assert!(!follow_range(&mut range, &change(6..7, "q")));
    }
}

//! Text shown in the editor that is not in the file (E81): inlay hints and blame, later ghost text and the debugger's
//! values. None of it enters the text, its undo or the saved file. At the end of a line it is only drawn, in the
//! overlay. In the middle of a line the last code point before it gets a kern attribute as wide as the text, an
//! attribute change without a text change or an undo step, and the overlay draws into the gap. Where no character
//! precedes it on its line, it goes to the line's end.

use std::ops::Range;
use std::rc::Rc;

use ide_document::Change;
use objc2::rc::{Retained, Weak};
use objc2::runtime::{AnyObject, ProtocolObject};
use objc2::{AnyThread, DefinedClass, MainThreadOnly, define_class, msg_send, sel};
use objc2_app_kit::{
    NSAccessibility, NSAccessibilityCustomRotor, NSAccessibilityCustomRotorItemResult,
    NSAccessibilityCustomRotorItemSearchDelegate, NSAccessibilityCustomRotorSearchDirection,
    NSAccessibilityCustomRotorSearchParameters, NSBezierPath, NSColor, NSEvent, NSFontAttributeName,
    NSForegroundColorAttributeName, NSKernAttributeName, NSStringDrawing, NSTextInputClient, NSTextStorage, NSView,
};
use objc2_foundation::{
    NSArray, NSDictionary, NSNumber, NSObject, NSObjectProtocol, NSPoint, NSRange, NSRect, NSSize, NSString,
};
use splat_i18n::{t, tf};

use super::view::EditorTextView;
use crate::util::{constant, guarded, ns};

/// The part of the app an inset belongs to; each sets and clears its own.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Layer {
    /// The language server's inlay hints (E83).
    Inlay,
    /// Who changed the cursor's line last (E86).
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

/// Whether editors draw as with increased contrast, for the self-test.
#[cfg(feature = "selftest")]
#[derive(Default)]
pub struct ForcedContrast {
    forced_contrast: std::cell::Cell<bool>,
}

#[cfg(feature = "selftest")]
fn with_forced_contrast<R>(f: impl FnOnce(&std::cell::Cell<bool>) -> R) -> R {
    f(&crate::context::app().editor.forced_contrast.forced_contrast)
}

/// Draws as with increased contrast, which AppKit gives a window only through the system setting, for the self-test.
#[cfg(feature = "selftest")]
pub fn force_increased_contrast(on: bool) {
    with_forced_contrast(|forced| forced.set(on));
}

#[cfg(feature = "selftest")]
fn forced_contrast() -> bool {
    with_forced_contrast(std::cell::Cell::get)
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

fn measure(inset: &Inset, attributes: &NSDictionary<NSString, AnyObject>) -> f64 {
    // SAFETY: The attributes hold a font and a colour, as the method expects.
    let text = unsafe { ns(&inset.text).sizeWithAttributes(Some(attributes)) }.width;
    let pads = f64::from(u8::from(inset.pad_left) + u8::from(inset.pad_right)) * PADDING;
    (text + 2.0 * MARGIN + pads).ceil()
}

/// Shows `insets` for `layer` in `view`, in place of the ones it had.
pub fn set(view: &EditorTextView, layer: Layer, mut insets: Vec<Inset>) {
    let attributes = attributes(view.ivars().font_size.get(), &NSColor::secondaryLabelColor());
    for inset in &mut insets {
        inset.width = measure(inset, &attributes);
    }
    insets.sort_by_key(|inset| inset.at);
    let changed = {
        let state = view.ivars().insets.borrow();
        let old = layer_insets(&state, layer).cloned().unwrap_or_default();
        changed_span(&old, &insets)
    };
    {
        let mut state = view.ivars().insets.borrow_mut();
        state.layers.retain(|(l, _)| *l != layer);
        if !insets.is_empty() {
            state.layers.push((layer, insets));
            state.layers.sort_by_key(|(l, _)| *l);
        }
        state.full = true;
    }
    settle(view);
    if let Some(span) = changed {
        redraw_text(view, span);
    }
    super::overlay::redraw(view);
}

/// The offsets from the first to the last inset that differs between `old` and `new`, both ordered by offset.
fn changed_span(old: &[Inset], new: &[Inset]) -> Option<Range<usize>> {
    let same = |a: &Inset, b: &Inset| a.at == b.at && a.text == b.text && a.place == b.place;
    let differs = old
        .iter()
        .filter(|a| !new.iter().any(|b| same(a, b)))
        .chain(new.iter().filter(|b| !old.iter().any(|a| same(a, b))))
        .map(|inset| inset.at);
    differs.fold(None, |span: Option<Range<usize>>, at| {
        Some(span.map_or(at..at + 1, |s| s.start.min(at)..s.end.max(at + 1)))
    })
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
            });
        }
    }
    placed
}

/// The kern each gap needs, ordered by place: the widths of its insets together.
fn wanted(placed: &[Placed]) -> Vec<Kerned> {
    let mut gaps: Vec<(Range<usize>, f64)> = placed
        .iter()
        .filter_map(|place| match &place.spot {
            Spot::Gap { anchor } => Some((anchor.clone(), place.width)),
            Spot::End { .. } => None,
        })
        .collect();
    gaps.sort_by_key(|(range, _)| (range.start, range.end));
    let mut kerns: Vec<Kerned> = Vec::with_capacity(gaps.len());
    for (range, width) in gaps {
        match kerns.last_mut() {
            Some(last) if last.range == range => last.width += width,
            _ => kerns.push(Kerned { range, width }),
        }
    }
    kerns
}

/// The kern on `range` in `kerns`, which is ordered by place.
fn find_kern<'a>(kerns: &'a [Kerned], range: &Range<usize>) -> Option<&'a Kerned> {
    let index = kerns.partition_point(|k| (k.range.start, k.range.end) < (range.start, range.end));
    kerns.get(index).filter(|k| k.range == *range)
}

#[cfg(feature = "selftest")]
fn kern_at(storage: &NSTextStorage, at: usize) -> Option<f64> {
    if at >= storage.length() {
        return None;
    }
    let key = constant!(NSKernAttributeName);
    // SAFETY: The index lies inside the text; no effective range is asked for.
    let value = unsafe { storage.attribute_atIndex_effectiveRange(key, at, std::ptr::null_mut()) }?;
    value.downcast::<NSNumber>().ok().map(|n| n.doubleValue())
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

/// Draws the insets of `view` in `dirty`, as the overlay asks. While scrolling, AppKit draws ahead of the visible
/// part, and what it drew there is shown later without drawing again, so `dirty` decides, not the visible part.
pub fn draw(view: &EditorTextView, dirty: NSRect) {
    let seen = view.visibleRect();
    let top = dirty.origin.y.min(seen.origin.y);
    let bottom = (dirty.origin.y + dirty.size.height).max(seen.origin.y + seen.size.height);
    let Some(shown) = shown_in(
        view,
        NSRect::new(NSPoint::new(0.0, top), NSSize::new(1.0, bottom - top)),
    ) else {
        return;
    };
    if let Ok(mut state) = view.ivars().insets.try_borrow_mut() {
        state.shown = shown;
    }
}

/// The UTF-16 range of the paragraph `fragment` lays out.
fn fragment_range(view: &EditorTextView, fragment: &objc2_app_kit::NSTextLayoutFragment) -> Option<Range<usize>> {
    let content = view.textContentStorage()?;
    let start = objc2_app_kit::NSTextElementProvider::documentRange(&*content).location();
    let range = fragment.rangeInElement();
    let from = usize::try_from(content.offsetFromLocation_toLocation(&start, &range.location())).ok()?;
    let to = usize::try_from(content.offsetFromLocation_toLocation(&start, &range.endLocation())).ok()?;
    Some(from..to)
}

/// Where the insets of the paragraph `fragment` lays out go, in the editor's coordinates.
fn fragment_insets(
    view: &EditorTextView,
    state: &Insets,
    fragment: &objc2_app_kit::NSTextLayoutFragment,
) -> Vec<Shown> {
    let (Some(range), Some(storage)) = (fragment_range(view, fragment), view.storage()) else {
        return Vec::new();
    };
    if state.layers.is_empty() {
        return Vec::new();
    }
    let inside =
        |at: usize| at >= range.start && (at < range.end || (at == range.end && range.end == storage.length()));
    let placed: Vec<Placed> = resolve(view, state, &storage.string(), &range)
        .into_iter()
        .filter(|place| match &place.spot {
            Spot::Gap { anchor } => inside(anchor.start),
            Spot::End { line_end } => inside(*line_end),
        })
        .collect();
    if placed.is_empty() {
        return Vec::new();
    }
    lay_out(view, &storage, state, placed, &range)
}

/// Draws the insets of the paragraph `fragment` lays out after its text, at `point` in `context`, as the fragment
/// draws itself: with the text, so that they scroll and show with it.
pub fn draw_fragment(
    view: &EditorTextView,
    fragment: &objc2_app_kit::NSTextLayoutFragment,
    point: NSPoint,
    context: &objc2_core_graphics::CGContext,
) {
    let Ok(state) = view.ivars().insets.try_borrow() else {
        return;
    };
    let shown = fragment_insets(view, &state, fragment);
    if shown.is_empty() {
        return;
    }
    let frame = fragment.layoutFragmentFrame();
    let origin = view.textContainerOrigin();
    let (dx, dy) = (point.x - frame.origin.x - origin.x, point.y - frame.origin.y - origin.y);
    let graphics = objc2_app_kit::NSGraphicsContext::graphicsContextWithCGContext_flipped(context, true);
    objc2_app_kit::NSGraphicsContext::saveGraphicsState_class();
    objc2_app_kit::NSGraphicsContext::setCurrentContext(Some(&graphics));
    let font_size = view.ivars().font_size.get();
    let attributes = attributes(font_size, &text_color(view));
    for place in &shown {
        if let Some(inset) = layer_insets(&state, place.layer).and_then(|insets| insets.get(place.index)) {
            let rect = NSRect::new(
                NSPoint::new(place.rect.origin.x + dx, place.rect.origin.y + dy),
                place.rect.size,
            );
            draw_box(rect, inset, &attributes, font_size);
        }
    }
    objc2_app_kit::NSGraphicsContext::restoreGraphicsState_class();
}

/// How much wider than its text the paragraph `fragment` lays out draws, for the insets at the end of its lines. TextKit
/// asks for every fragment on screen at every keystroke, so this adds up widths instead of laying the insets out.
pub fn width_after_text(view: &EditorTextView, fragment: &objc2_app_kit::NSTextLayoutFragment) -> f64 {
    let (Some(range), Ok(state)) = (fragment_range(view, fragment), view.ivars().insets.try_borrow()) else {
        return 0.0;
    };
    let ends: f64 = state
        .layers
        .iter()
        .flat_map(|(_, insets)| {
            let first = insets.partition_point(|inset| inset.at < range.start);
            let last = insets.partition_point(|inset| inset.at <= range.end);
            insets[first..last].iter()
        })
        .map(|inset| inset.width + END_GAP)
        .sum();
    if ends > 0.0 {
        // Room for a fold's placeholder, which an inset at the line's end goes after.
        ends + END_GAP + 40.0
    } else {
        0.0
    }
}

/// Where the insets of the lines in `rect` go on screen.
fn shown_in(view: &EditorTextView, rect: NSRect) -> Option<Vec<Shown>> {
    let storage = view.storage()?;
    let lines = super::utf16_in(view, rect);
    let state = view.ivars().insets.try_borrow().ok()?;
    if state.layers.is_empty() {
        return Some(Vec::new());
    }
    let placed = resolve(view, &state, &storage.string(), &lines);
    Some(lay_out(view, &storage, &state, placed, &lines))
}

/// How many insets the overlay draws when AppKit asks it for `rect`, for the self-test.
#[cfg(feature = "selftest")]
pub fn drawn_in(view: &EditorTextView, rect: NSRect) -> usize {
    shown_in(view, rect).map_or(0, |shown| shown.len())
}

fn layer_insets(state: &Insets, layer: Layer) -> Option<&Vec<Inset>> {
    state.layers.iter().find(|(l, _)| *l == layer).map(|(_, insets)| insets)
}

/// The rectangles of the insets on screen: gaps from the kerned code point's box, line ends after the line's last
/// character and any fold placeholder there, one after the other.
fn lay_out(
    view: &EditorTextView,
    storage: &NSTextStorage,
    state: &Insets,
    placed: Vec<Placed>,
    visible: &Range<usize>,
) -> Vec<Shown> {
    let kerned = &state.kerned;
    let placeholders: Vec<NSRect> = super::fold::placeholders(view)
        .into_iter()
        .map(|(rect, _)| rect)
        .collect();
    let mut gap_used: Vec<(Range<usize>, f64)> = Vec::new();
    let mut end_used: Vec<(usize, f64)> = Vec::new();
    let mut shown = Vec::new();
    for place in placed {
        let Some(inset) = layer_insets(state, place.layer).and_then(|insets| insets.get(place.index)) else {
            continue;
        };
        if inset.at < visible.start || inset.at > visible.end || super::fold::hidden_by_fold(view, inset.at) {
            continue;
        }
        let rect = match &place.spot {
            Spot::Gap { anchor } => {
                if find_kern(kerned, anchor).is_none() {
                    continue;
                }
                let Some(glyph) = caret(view, anchor.start) else {
                    continue;
                };
                let before = gap_used.iter().find(|(r, _)| r == anchor).map_or(0.0, |(_, w)| *w);
                let left = glyph.origin.x + advance(storage, anchor) + before;
                match gap_used.iter_mut().find(|(r, _)| r == anchor) {
                    Some(used) => used.1 += place.width,
                    None => gap_used.push((anchor.clone(), place.width)),
                }
                NSRect::new(
                    NSPoint::new(left, glyph.origin.y),
                    NSSize::new(place.width, glyph.size.height),
                )
            }
            Spot::End { line_end } => {
                let Some(caret) = caret(view, *line_end).or_else(|| crate::lsp::rect_of(view, *line_end..*line_end))
                else {
                    continue;
                };
                let mut x = caret.origin.x + END_GAP;
                if let Some(folded) = placeholders
                    .iter()
                    .find(|p| (p.origin.y - 1.0 - caret.origin.y).abs() < 2.0 && p.origin.x + 1.0 >= caret.origin.x)
                {
                    x = x.max(folded.origin.x + folded.size.width + END_GAP);
                }
                let before = end_used.iter().find(|(l, _)| l == line_end).map_or(0.0, |(_, w)| *w);
                match end_used.iter_mut().find(|(l, _)| l == line_end) {
                    Some(used) => used.1 += place.width + END_GAP / 2.0,
                    None => end_used.push((*line_end, place.width + END_GAP / 2.0)),
                }
                NSRect::new(
                    NSPoint::new(x + before, caret.origin.y),
                    NSSize::new(place.width, caret.size.height),
                )
            }
        };
        shown.push(Shown {
            rect,
            layer: place.layer,
            index: place.index,
            in_gap: matches!(place.spot, Spot::Gap { .. }),
            gap_end: match &place.spot {
                Spot::Gap { anchor } => Some(anchor.end),
                Spot::End { .. } => None,
            },
        });
    }
    shown
}

/// The caret before UTF-16 offset `at`, as TextKit 2 lays out the line fragment that holds it: its left edge, the
/// fragment's top and height, in the editor's coordinates; without the window conversions of an input method's rect.
fn caret(view: &EditorTextView, at: usize) -> Option<NSRect> {
    let (layout, content) = (view.textLayoutManager()?, view.textContentStorage()?);
    let start = objc2_app_kit::NSTextElementProvider::documentRange(&*content).location();
    let location = content.locationFromLocation_withOffset(&start, isize::try_from(at).ok()?)?;
    let fragment = layout.textLayoutFragmentForLocation(&location)?;
    let paragraph = content.offsetFromLocation_toLocation(&start, &fragment.rangeInElement().location());
    let local = isize::try_from(at).ok()? - paragraph;
    let frame = fragment.layoutFragmentFrame();
    let origin = view.textContainerOrigin();
    let lines = fragment.textLineFragments();
    let count = lines.count();
    lines.iter().enumerate().find_map(|(index, line)| {
        let range = line.characterRange();
        let (from, to) = (
            range.location.cast_signed(),
            (range.location + range.length).cast_signed(),
        );
        let holds = (from..to).contains(&local) || (index + 1 == count && local == to);
        holds.then(|| {
            let bounds = line.typographicBounds();
            NSRect::new(
                NSPoint::new(
                    frame.origin.x + origin.x + bounds.origin.x + line.locationForCharacterAtIndex(local).x,
                    frame.origin.y + origin.y + bounds.origin.y,
                ),
                NSSize::new(0.0, bounds.size.height),
            )
        })
    })
}

/// How wide the characters of `range` are without their kern: TextKit 2 reports the caret after a kerned character in
/// the middle of the kern, so the gap is measured from the character's start instead.
fn advance(storage: &NSTextStorage, range: &Range<usize>) -> f64 {
    let piece = storage.attributedSubstringFromRange(NSRange::new(range.start, range.end - range.start));
    let plain = objc2_foundation::NSMutableAttributedString::initWithAttributedString(
        objc2_foundation::NSMutableAttributedString::alloc(),
        &piece,
    );
    plain.removeAttribute_range(constant!(NSKernAttributeName), NSRange::new(0, plain.length()));
    objc2_app_kit::NSAttributedStringNSStringDrawing::size(&**plain).width
}

/// The ground of an inset's box: the system's faintest fill, which follows light, dark and increased contrast.
fn fill() -> Retained<NSColor> {
    NSColor::quaternarySystemFillColor()
}

/// A tinted box in `rect` with the inset's text, smaller than the code and in the secondary colour.
fn draw_box(rect: NSRect, inset: &Inset, attributes: &NSDictionary<NSString, AnyObject>, font_size: f64) {
    let left = rect.origin.x + if inset.pad_left { PADDING } else { 0.0 };
    let right = rect.origin.x + rect.size.width - if inset.pad_right { PADDING } else { 0.0 };
    let height = (text_size(font_size) * 1.35).min(rect.size.height);
    let top = rect.origin.y + (rect.size.height - height) / 2.0;
    let frame = NSRect::new(NSPoint::new(left, top), NSSize::new((right - left).max(0.0), height));
    fill().setFill();
    NSBezierPath::bezierPathWithRoundedRect_xRadius_yRadius(frame, 3.0, 3.0).fill();
    let text = ns(&inset.text);
    // SAFETY: The attributes hold a font and a colour, as the method expects.
    let size = unsafe { text.sizeWithAttributes(Some(attributes)) };
    let at = NSPoint::new(left + MARGIN, top + (height - size.height) / 2.0);
    // SAFETY: As above.
    unsafe { text.drawAtPoint_withAttributes(at, Some(attributes)) };
}

/// Where the insets of `view` were drawn last.
pub fn shown(view: &EditorTextView) -> Vec<Shown> {
    view.ivars()
        .insets
        .try_borrow()
        .map(|state| state.shown.clone())
        .unwrap_or_default()
}

fn contains(rect: NSRect, point: NSPoint) -> bool {
    point.x >= rect.origin.x
        && point.x < rect.origin.x + rect.size.width
        && point.y >= rect.origin.y
        && point.y < rect.origin.y + rect.size.height
}

/// The inset under `point` of the editor, with whether a click on it does something.
fn under(view: &EditorTextView, point: NSPoint) -> Option<(Shown, Inset)> {
    let state = view.ivars().insets.try_borrow().ok()?;
    let shown = state.shown.iter().find(|s| contains(s.rect, point))?.clone();
    let inset = layer_insets(&state, shown.layer)?.get(shown.index)?.clone();
    Some((shown, inset))
}

/// Whether a click at `point` of the editor lands on an inset that does something; everything else goes to the text.
pub fn hits(view: &EditorTextView, point: NSPoint) -> bool {
    under(view, point).is_some_and(|(_, inset)| inset.action.is_some())
}

/// Runs what the inset under `point` does; whether there was one.
pub fn click(view: &EditorTextView, point: NSPoint) -> bool {
    let Some(action) = under(view, point).and_then(|(_, inset)| inset.action) else {
        return false;
    };
    action(view);
    true
}

/// The tooltip of the inset under `point`.
pub fn tooltip(view: &EditorTextView, point: NSPoint) -> Option<String> {
    under(view, point).and_then(|(_, inset)| inset.tooltip)
}

/// The gap a click at `point` belongs to, with where TextKit's own hit test puts its caret: the gap, and the left half
/// of the character after it, whose caret position TextKit counts from the middle of the gap.
fn gap_under(view: &EditorTextView, point: NSPoint) -> Option<(usize, f64)> {
    let storage = view.storage()?;
    let state = view.ivars().insets.try_borrow().ok()?;
    let mut gaps: Vec<(usize, NSRect)> = Vec::new();
    for shown in state.shown.iter().filter(|s| s.in_gap) {
        let end = shown.gap_end?;
        match gaps.iter_mut().find(|(e, _)| *e == end) {
            Some((_, rect)) => rect.size.width = shown.rect.origin.x + shown.rect.size.width - rect.origin.x,
            None => gaps.push((end, shown.rect)),
        }
    }
    gaps.into_iter().find_map(|(end, rect)| {
        let next = if end < storage.length() {
            let grapheme = storage.string().rangeOfComposedCharacterSequenceAtIndex(end);
            advance(&storage, &(grapheme.location..grapheme.location + grapheme.length)) / 2.0
        } else {
            0.0
        };
        let inside = point.y >= rect.origin.y
            && point.y < rect.origin.y + rect.size.height
            && point.x >= rect.origin.x
            && point.x < rect.origin.x + rect.size.width + next;
        inside.then_some((end, rect.origin.x + rect.size.width / 2.0))
    })
}

/// A plain click in a gap, or on the left half of the character after it, puts the caret right of the gap; whether
/// it was one.
pub fn click_gap(view: &EditorTextView, event: &NSEvent) -> bool {
    let plain = (event.modifierFlags() & objc2_app_kit::NSEventModifierFlags::DeviceIndependentFlagsMask).is_empty();
    if !plain || event.clickCount() != 1 {
        return false;
    }
    let point = view.convertPoint_fromView(event.locationInWindow(), None);
    let Some((end, _)) = gap_under(view, point) else {
        return false;
    };
    view.setSelectedRange(NSRange::new(end, 0));
    if let Some(window) = view.window() {
        window.makeFirstResponder(Some(view));
    }
    true
}

/// Any other click in a gap goes to the text where TextKit's hit test finds the caret right of the gap, so a drag or a
/// ⇧-click starts or ends there.
pub fn retarget(view: &EditorTextView, event: &NSEvent) -> Option<Retained<NSEvent>> {
    let point = view.convertPoint_fromView(event.locationInWindow(), None);
    let (_, middle) = gap_under(view, point)?;
    let target = view.convertPoint_toView(NSPoint::new(middle, point.y), None);
    NSEvent::mouseEventWithType_location_modifierFlags_timestamp_windowNumber_context_eventNumber_clickCount_pressure(
        event.r#type(),
        target,
        event.modifierFlags(),
        event.timestamp(),
        event.windowNumber(),
        None,
        event.eventNumber(),
        event.clickCount(),
        event.pressure(),
    )
}

/// Tells the layout manager of `view` where the gaps are, by the offset after each and its width.
fn publish(view: &EditorTextView, kerned: &[Kerned]) {
    let Some(layout) = view.textLayoutManager() else {
        return;
    };
    if let Ok(gaps) = layout.downcast::<GapLayoutManager>() {
        *gaps.ivars().gaps.borrow_mut() = kerned.iter().map(|k| (k.range.end, k.width)).collect::<Vec<_>>().into();
    }
}

pub struct GapIvars {
    /// The offset after each gap's character and the gap's width, ordered by offset.
    gaps: std::cell::RefCell<Rc<[(usize, f64)]>>,
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
            block: &block2::DynBlock<
                dyn Fn(*mut objc2_app_kit::NSTextRange, NSRect, f64, std::ptr::NonNull<objc2_app_kit::NSTextContainer>) -> objc2::runtime::Bool,
            >,
        ) {
            let gaps = self.ivars().gaps.borrow().clone();
            let start = self
                .textContentManager()
                .map(|content| (content.clone(), objc2_app_kit::NSTextElementProvider::documentRange(&*content).location()));
            let moved = block2::RcBlock::new(
                move |segment: *mut objc2_app_kit::NSTextRange,
                      rect: NSRect,
                      baseline: f64,
                      container: std::ptr::NonNull<objc2_app_kit::NSTextContainer>| {
                    // SAFETY: TextKit passes a valid text range or none, alive for the call.
                    let segment = unsafe { segment.as_ref() };
                    let rect = match (&start, segment) {
                        (Some((content, start)), Some(segment)) if !gaps.is_empty() => {
                            let from = objc2_app_kit::NSTextElementProvider::offsetFromLocation_toLocation(
                                &**content,
                                start,
                                &segment.location(),
                            );
                            let to = objc2_app_kit::NSTextElementProvider::offsetFromLocation_toLocation(
                                &**content,
                                start,
                                &segment.endLocation(),
                            );
                            caret_right_of_gap(&gaps, rect, from, to)
                        }
                        _ => rect,
                    };
                    block.call((segment.map_or(std::ptr::null_mut(), |s| std::ptr::from_ref(s).cast_mut()), rect, baseline, container))
                },
            );
            // SAFETY: The superclass's implementation, with this method's signature and a block of the same type.
            let _: () = unsafe {
                msg_send![super(self), enumerateTextSegmentsInRange: range, type: kind, options: options, usingBlock: &*moved]
            };
        }
    }
);

/// TextKit 2 puts the caret after a kerned character in the middle of the kern: a segment that starts there starts at
/// the gap's right edge instead, and one that ends there ends at its left edge.
fn caret_right_of_gap(gaps: &[(usize, f64)], mut rect: NSRect, from: isize, to: isize) -> NSRect {
    let width_at = |offset: isize| {
        let offset = usize::try_from(offset).ok()?;
        let index = gaps.partition_point(|(end, _)| *end < offset);
        gaps.get(index).filter(|(end, _)| *end == offset).map(|(_, w)| *w / 2.0)
    };
    if let Some(half) = width_at(from) {
        rect.origin.x += half;
        rect.size.width = (rect.size.width - half).max(0.0);
    }
    if to > from
        && let Some(half) = width_at(to)
    {
        rect.size.width = (rect.size.width - half).max(0.0);
    }
    rect
}

impl GapLayoutManager {
    /// A layout manager that knows no gaps yet.
    pub fn new() -> Retained<GapLayoutManager> {
        let this = Self::alloc().set_ivars(GapIvars {
            gaps: std::cell::RefCell::new(Rc::new([])),
        });
        // SAFETY: NSTextLayoutManager's initializer, called once on the newly allocated object.
        unsafe { msg_send![super(this), init] }
    }
}

pub struct RotorIvars {
    editor: Weak<EditorTextView>,
}

define_class!(
    // SAFETY: NSObject has no subclassing requirements and `Rotor` does not implement Drop.
    #[unsafe(super(NSObject))]
    #[thread_kind = MainThreadOnly]
    #[name = "IDEInsetRotor"]
    #[ivars = RotorIvars]
    pub struct Rotor;

    unsafe impl NSObjectProtocol for Rotor {}

    unsafe impl NSAccessibilityCustomRotorItemSearchDelegate for Rotor {
        #[unsafe(method_id(rotor:resultForSearchParameters:))]
        fn result(
            &self,
            _rotor: &NSAccessibilityCustomRotor,
            parameters: &NSAccessibilityCustomRotorSearchParameters,
        ) -> Option<Retained<NSAccessibilityCustomRotorItemResult>> {
            guarded(None, || self.search(parameters))
        }
    }
);

/// One entry of the rotor: where it is, and what VoiceOver reads.
#[derive(Debug, Clone, PartialEq)]
pub struct RotorItem {
    pub at: usize,
    pub label: String,
}

/// The rotor's entries for `view`: every inset, those at one place together, with their line.
pub fn rotor_items(view: &EditorTextView) -> Vec<RotorItem> {
    let Ok(state) = view.ivars().insets.try_borrow() else {
        return Vec::new();
    };
    let mut all: Vec<(usize, &str)> = state
        .layers
        .iter()
        .flat_map(|(_, insets)| insets.iter().map(|i| (i.at, i.text.as_str())))
        .collect();
    all.sort_by_key(|(at, _)| *at);
    let document = view.document();
    let text = document.text();
    let mut items: Vec<RotorItem> = Vec::new();
    let mut texts: Vec<(usize, Vec<&str>)> = Vec::new();
    for (at, label) in all {
        match texts.last_mut() {
            Some((last, labels)) if *last == at => labels.push(label),
            _ => texts.push((at, vec![label])),
        }
    }
    for (at, labels) in texts {
        let line = text
            .utf16_to_byte(at.min(text.len_utf16()))
            .map_or(0, |byte| text.line_of(byte));
        items.push(RotorItem {
            at,
            label: tf("{}, Zeile {}", &[&labels.join(" "), &(line + 1).to_string()]),
        });
    }
    items
}

/// The entry after or before `current` in `items`, among those whose label holds `filter`.
fn next_item(items: &[RotorItem], current: Option<usize>, forward: bool, filter: &str) -> Option<RotorItem> {
    let filter = filter.to_lowercase();
    let matching = items
        .iter()
        .filter(|item| filter.is_empty() || item.label.to_lowercase().contains(&filter));
    let found = if forward {
        matching.into_iter().find(|item| current.is_none_or(|c| item.at > c))
    } else {
        matching
            .into_iter()
            .rev()
            .find(|item| current.is_none_or(|c| item.at < c))
    };
    found.cloned()
}

impl Rotor {
    fn search(
        &self,
        parameters: &NSAccessibilityCustomRotorSearchParameters,
    ) -> Option<Retained<NSAccessibilityCustomRotorItemResult>> {
        let editor = self.ivars().editor.load()?;
        let current = parameters
            .currentItem()
            .map(|item| item.targetRange().location)
            .filter(|&at| at != objc2_foundation::NSNotFound.cast_unsigned());
        let forward = parameters.searchDirection() == NSAccessibilityCustomRotorSearchDirection::Next;
        // SAFETY: The getter takes no arguments; AppKit answers nil without a filter, which the generated binding does
        // not allow for.
        let filter: Option<Retained<NSString>> = unsafe { msg_send![parameters, filterString] };
        let filter = filter.map(|f| f.to_string()).unwrap_or_default();
        let item = next_item(&rotor_items(&editor), current, forward, &filter)?;
        let element: &NSView = &editor;
        let result = NSAccessibilityCustomRotorItemResult::initWithTargetElement(
            NSAccessibilityCustomRotorItemResult::alloc(),
            ProtocolObject::from_ref(element),
        );
        result.setTargetRange(NSRange::new(item.at, 0));
        result.setCustomLabel(Some(&ns(&item.label)));
        Some(result)
    }
}

/// Gives `view` the rotor "Hinweise", beside the rotors it has.
pub fn attach(view: &EditorTextView) {
    let delegate = Rotor::alloc(view.mtm()).set_ivars(RotorIvars {
        editor: Weak::from(view),
    });
    // SAFETY: NSObject's initializer, called once on the newly allocated object.
    let delegate: Retained<Rotor> = unsafe { msg_send![super(delegate), init] };
    let rotor = NSAccessibilityCustomRotor::initWithLabel_itemSearchDelegate(
        NSAccessibilityCustomRotor::alloc(),
        &ns(t("Hinweise")),
        ProtocolObject::from_ref(&*delegate),
    );
    // SAFETY: The getter takes no arguments; NSTextView answers nil where it has no rotors, which the generated
    // binding does not allow for.
    let own: Option<Retained<NSArray<NSAccessibilityCustomRotor>>> =
        unsafe { msg_send![view, accessibilityCustomRotors] };
    let mut rotors: Vec<Retained<NSAccessibilityCustomRotor>> = own.map(|r| r.to_vec()).unwrap_or_default();
    rotors.push(rotor);
    view.setAccessibilityCustomRotors(&NSArray::from_retained_slice(&rotors));
    view.ivars().insets.borrow_mut().rotor = Some(delegate);
    let layout = super::inset_fragment::attach(view);
    view.ivars().insets.borrow_mut().layout = layout;
    if let Some(manager) = view.textLayoutManager() {
        // Fragments laid out before the delegate came are plain ones.
        manager.invalidateLayoutForRange(&objc2_app_kit::NSTextSelectionDataSource::documentRange(&*manager));
    }
}

/// The rotor "Hinweise" of `view` and the entry it finds from `current` onward, as VoiceOver asks, for the self-test.
#[cfg(feature = "selftest")]
pub fn rotor_step(view: &EditorTextView, current: Option<usize>, forward: bool) -> (Option<String>, Option<RotorItem>) {
    // SAFETY: As in `attach`.
    let rotors: Option<Retained<NSArray<NSAccessibilityCustomRotor>>> =
        unsafe { msg_send![view, accessibilityCustomRotors] };
    let ours = rotors
        .iter()
        .flat_map(|r| r.to_vec())
        .find(|r| r.label().to_string() == t("Hinweise"));
    let label = ours.as_ref().map(|r| r.label().to_string());
    let Some(rotor) = ours else {
        return (None, None);
    };
    let parameters = NSAccessibilityCustomRotorSearchParameters::new();
    parameters.setSearchDirection(if forward {
        NSAccessibilityCustomRotorSearchDirection::Next
    } else {
        NSAccessibilityCustomRotorSearchDirection::Previous
    });
    if let Some(at) = current {
        let element: &NSView = view;
        let item = NSAccessibilityCustomRotorItemResult::initWithTargetElement(
            NSAccessibilityCustomRotorItemResult::alloc(),
            ProtocolObject::from_ref(element),
        );
        item.setTargetRange(NSRange::new(at, 0));
        parameters.setCurrentItem(Some(&item));
    }
    let found = rotor
        .itemSearchDelegate()
        .and_then(|delegate| delegate.rotor_resultForSearchParameters(&rotor, &parameters));
    let item = found.map(|result| RotorItem {
        at: result.targetRange().location,
        label: result.customLabel().map(|l| l.to_string()).unwrap_or_default(),
    });
    (label, item)
}

/// The contrast of an inset's text against its box on the editor's ground, in the appearance `view` has now, for the
/// self-test.
#[cfg(feature = "selftest")]
pub fn contrast(view: &EditorTextView) -> Option<f64> {
    use objc2_app_kit::{NSAppearanceCustomization, NSColorSpace};
    let rgba = |color: &NSColor| {
        let found = std::cell::Cell::new(None);
        view.effectiveAppearance()
            .performAsCurrentDrawingAppearance(&block2::RcBlock::new(|| {
                found.set(color.colorUsingColorSpace(&NSColorSpace::sRGBColorSpace()).map(|c| {
                    [
                        c.redComponent(),
                        c.greenComponent(),
                        c.blueComponent(),
                        c.alphaComponent(),
                    ]
                }));
            }));
        found.get()
    };
    let over =
        |top: [f64; 4], below: [f64; 3]| -> [f64; 3] { [0, 1, 2].map(|i| top[i] * top[3] + below[i] * (1.0 - top[3])) };
    let ground = rgba(&NSColor::textBackgroundColor())?;
    let ground = [ground[0], ground[1], ground[2]];
    let back = over(rgba(&fill())?, ground);
    let front = over(rgba(&text_color(view))?, back);
    let luminance = |c: [f64; 3]| {
        let linear = |v: f64| {
            if v <= 0.040_45 {
                v / 12.92
            } else {
                ((v + 0.055) / 1.055).powf(2.4)
            }
        };
        0.2126 * linear(c[0]) + 0.7152 * linear(c[1]) + 0.0722 * linear(c[2])
    };
    let (a, b) = (luminance(back), luminance(front));
    Some((a.max(b) + 0.05) / (a.min(b) + 0.05))
}

/// The kern attributes in the text of `view`, as ranges with their values, for the self-test.
#[cfg(feature = "selftest")]
pub fn kerns(view: &EditorTextView) -> Vec<(Range<usize>, f64)> {
    let Some(storage) = view.storage() else {
        return Vec::new();
    };
    kerned_in(&storage, 0..storage.length())
        .into_iter()
        .map(|range| {
            let width = kern_at(&storage, range.start).unwrap_or(0.0);
            (range, width)
        })
        .collect()
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

    #[test]
    fn the_rotor_steps_through_insets_both_ways_and_filters() {
        let items: Vec<RotorItem> = [
            (3, "a: i32, Zeile 1"),
            (9, "b: u8, Zeile 2"),
            (20, "c: String, Zeile 4"),
        ]
        .iter()
        .map(|(at, label)| RotorItem {
            at: *at,
            label: (*label).into(),
        })
        .collect();
        assert_eq!(next_item(&items, None, true, "").map(|i| i.at), Some(3));
        assert_eq!(next_item(&items, Some(3), true, "").map(|i| i.at), Some(9));
        assert_eq!(next_item(&items, Some(20), true, ""), None);
        assert_eq!(next_item(&items, None, false, "").map(|i| i.at), Some(20));
        assert_eq!(next_item(&items, Some(9), false, "").map(|i| i.at), Some(3));
        assert_eq!(next_item(&items, None, true, "STRING").map(|i| i.at), Some(20));
    }
}

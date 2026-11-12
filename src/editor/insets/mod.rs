//! Text shown in the editor that is not in the file (E81): inlay hints and blame, later ghost text and the debugger's
//! values. None of it enters the text, its undo or the saved file. At the end of a line it is only drawn, in the
//! overlay. In the middle of a line the last code point before it gets a kern attribute as wide as the text, an
//! attribute change without a text change or an undo step, and the overlay draws into the gap. Where no character
//! precedes it on its line, it goes to the line's end.

mod draw;
mod rotor;

use std::ops::Range;
use std::rc::Rc;

use ide_document::Change;
use objc2::rc::Retained;
use objc2::runtime::AnyObject;
use objc2::{AnyThread, DefinedClass, define_class, msg_send, sel};
use objc2_app_kit::{
    NSColor, NSFontAttributeName, NSForegroundColorAttributeName, NSKernAttributeName, NSStringDrawing,
    NSTextInputClient, NSTextStorage,
};
use objc2_foundation::{NSDictionary, NSNumber, NSObjectProtocol, NSRange, NSRect, NSString};

use super::view::EditorTextView;
use crate::util::{constant, ns};
#[cfg(feature = "selftest")]
use draw::fill;
use rotor::Rotor;

#[cfg(feature = "selftest")]
pub use draw::drawn_in;
pub use draw::{click, click_gap, draw, draw_fragment, hits, retarget, shown, tooltip, width_after_text};
pub use rotor::attach;
#[cfg(feature = "selftest")]
pub use rotor::rotor_items;
#[cfg(feature = "selftest")]
pub use rotor::rotor_step;

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

fn layer_insets(state: &Insets, layer: Layer) -> Option<&Vec<Inset>> {
    state.layers.iter().find(|(l, _)| *l == layer).map(|(_, insets)| insets)
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
}

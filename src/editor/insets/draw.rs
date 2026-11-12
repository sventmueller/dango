//! Drawing the insets: at the end of a line or into the gap their kern left, from the overlay or from TextKit's layout
//! fragments, and what lies under the pointer for clicks and tooltips.

use std::ops::Range;

use objc2::rc::Retained;
use objc2::runtime::AnyObject;
use objc2::{AnyThread, DefinedClass};
use objc2_app_kit::{NSBezierPath, NSColor, NSEvent, NSKernAttributeName, NSStringDrawing, NSTextStorage};
use objc2_foundation::{NSDictionary, NSPoint, NSRange, NSRect, NSSize, NSString};

use super::{
    END_GAP, Inset, Insets, MARGIN, PADDING, Placed, Shown, Spot, attributes, find_kern, layer_insets, resolve,
    text_color, text_size,
};
use crate::editor::EditorTextView;
use crate::util::{constant, ns};

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
    let lines = crate::editor::utf16_in(view, rect);
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
    let placeholders: Vec<NSRect> = crate::editor::fold::placeholders(view)
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
        if inset.at < visible.start || inset.at > visible.end || crate::editor::fold::hidden_by_fold(view, inset.at) {
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
pub(super) fn fill() -> Retained<NSColor> {
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

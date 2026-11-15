//! Whitespace made visible on request: spaces as faint dots, tabs as arrows and spaces and tabs at the end of a line
//! on a tinted ground. Drawn in a view over the text from TextKit 2's line geometry, so the text storage, its undo
//! and its colours stay untouched.

use std::cell::Cell;
use std::ptr::NonNull;

use block2::RcBlock;
use ide_core::settings::{Settings, keys};
use objc2::rc::{Retained, Weak};
use objc2::runtime::Bool;
use objc2::{DefinedClass, MainThreadOnly, define_class, msg_send};
use objc2_app_kit::{
    NSAccessibility, NSAutoresizingMaskOptions, NSBezierPath, NSColor, NSScrollView, NSTextLayoutFragment,
    NSTextLayoutFragmentEnumerationOptions, NSTextLineFragment, NSView, NSViewBoundsDidChangeNotification,
    NSWindowOrderingMode,
};
use objc2_foundation::{NSObjectProtocol, NSPoint, NSRect, NSSize};

use super::view::EditorTextView;
use crate::util::{constant, guarded};

thread_local! {
    static SHOWN: Cell<Option<bool>> = const { Cell::new(None) };
}

/// Whether editors show their whitespace, as the user last chose.
pub(crate) fn shown() -> bool {
    SHOWN.with(|shown| {
        shown.get().unwrap_or_else(|| {
            let value = Settings::load_user().get(&keys::SHOW_WHITESPACE);
            shown.set(Some(value));
            value
        })
    })
}

/// Reads the setting again on next use, after it changed elsewhere, and redraws every editor.
pub(crate) fn forget() {
    SHOWN.with(|shown| shown.set(None));
    for view in super::open_editors() {
        redraw(&view);
    }
}

/// Shows whitespace in every editor, or hides it again, and remembers the choice.
pub(crate) fn toggle() {
    let value = !shown();
    SHOWN.with(|shown| shown.set(Some(value)));
    let mut settings = Settings::load_user();
    if let Err(error) = settings.set(&keys::SHOW_WHITESPACE, &value) {
        crate::diag::report("saving the whitespace setting", error);
    }
    for view in super::open_editors() {
        redraw(&view);
    }
}

pub(crate) struct OverlayIvars {
    editor: Weak<EditorTextView>,
    /// The scrolling it redraws for, which ends with the overlay.
    scrolled: std::cell::OnceCell<crate::util::Observation>,
}

define_class!(
    // SAFETY: NSView has no subclassing requirements and `Overlay` does not implement Drop.
    #[unsafe(super(NSView))]
    #[thread_kind = MainThreadOnly]
    #[name = "IDEWhitespace"]
    #[ivars = OverlayIvars]
    pub(crate) struct Overlay;

    unsafe impl NSObjectProtocol for Overlay {}

    impl Overlay {
        #[unsafe(method(isFlipped))]
        fn is_flipped(&self) -> bool {
            true
        }

        #[unsafe(method_id(hitTest:))]
        fn hit_test(&self, _point: NSPoint) -> Option<Retained<NSView>> {
            None
        }

        #[unsafe(method(drawRect:))]
        fn draw_rect(&self, dirty: NSRect) {
            guarded((), || self.draw(dirty));
        }
    }
);

/// Lays the overlay over `editor`, redrawn as `scroll` scrolls.
pub(crate) fn attach(editor: &EditorTextView, scroll: &NSScrollView) {
    let this = Overlay::alloc(editor.mtm()).set_ivars(OverlayIvars {
        editor: Weak::from(editor),
        scrolled: std::cell::OnceCell::new(),
    });
    // SAFETY: NSView's initializer, called once on the newly allocated object.
    let this: Retained<Overlay> = unsafe { msg_send![super(this), initWithFrame: editor.bounds()] };
    this.setAutoresizingMask(
        NSAutoresizingMaskOptions::ViewWidthSizable | NSAutoresizingMaskOptions::ViewHeightSizable,
    );
    this.setAccessibilityElement(false);
    this.setHidden(!shown());
    editor.addSubview_positioned_relativeTo(&this, NSWindowOrderingMode::Above, None);
    *editor.ivars().around.whitespace.borrow_mut() = Some(Weak::from(&*this));
    let clip = scroll.contentView();
    clip.setPostsBoundsChangedNotifications(true);
    let mtm = editor.mtm();
    let editor = Weak::from(editor);
    let name = constant!(NSViewBoundsDidChangeNotification);
    let watch = crate::util::observe(mtm, name, Some(&clip), move |_| {
        if let Some(editor) = editor.load()
            && shown()
        {
            redraw(&editor);
        }
    });
    let _ = this.ivars().scrolled.set(watch);
}

/// Whether `item` is the menu item that shows whitespace, which then gets its check mark.
pub(crate) fn validate(item: &objc2_app_kit::NSMenuItem) -> Option<bool> {
    if item.action() != Some(objc2::sel!(ideToggleWhitespace:)) {
        return None;
    }
    item.setState(if shown() {
        objc2_app_kit::NSControlStateValueOn
    } else {
        objc2_app_kit::NSControlStateValueOff
    });
    Some(true)
}

/// Draws the whitespace of `view`'s lines on screen again, or hides it.
pub(crate) fn redraw(view: &EditorTextView) {
    let Some(overlay) = view.ivars().around.whitespace.borrow().as_ref().and_then(Weak::load) else {
        return;
    };
    let show = shown();
    overlay.setHidden(!show);
    if show {
        overlay.setNeedsDisplayInRect(view.visibleRect());
    }
}

/// Where a line fragment lies in the editor, and its text.
struct Line<'a> {
    fragment: &'a NSTextLineFragment,
    /// The fragment's top left corner in the editor.
    origin: NSPoint,
    height: f64,
    /// The UTF-16 units of the paragraph the fragment belongs to.
    units: &'a [u16],
}

impl Line<'_> {
    fn x(&self, index: usize) -> f64 {
        let index = isize::try_from(index).unwrap_or(isize::MAX);
        self.origin.x + self.fragment.locationForCharacterAtIndex(index).x
    }
}

impl Overlay {
    fn draw(&self, dirty: NSRect) {
        let Some(editor) = self.ivars().editor.load() else {
            return;
        };
        let Some(layout) = editor.textLayoutManager() else {
            return;
        };
        let origin = editor.textContainerOrigin();
        let visible = editor.visibleRect();
        let top = visible.origin.y.max(dirty.origin.y);
        let bottom = (visible.origin.y + visible.size.height).min(dirty.origin.y + dirty.size.height);
        let start = layout
            .textLayoutFragmentForPosition(NSPoint::new(0.0, top - origin.y))
            .map(|fragment| fragment.rangeInElement().location());
        let block = RcBlock::new(|fragment: NonNull<NSTextLayoutFragment>| -> Bool {
            crate::util::guarded(Bool::NO, || {
                // SAFETY: TextKit keeps the fragment alive for the call.
                let fragment = unsafe { fragment.as_ref() };
                let frame = fragment.layoutFragmentFrame();
                if frame.origin.y + origin.y > bottom {
                    return Bool::NO;
                }
                draw_fragment(
                    fragment,
                    NSPoint::new(frame.origin.x + origin.x, frame.origin.y + origin.y),
                );
                Bool::YES
            })
        });
        let _ = layout.enumerateTextLayoutFragmentsFromLocation_options_usingBlock(
            start.as_deref(),
            NSTextLayoutFragmentEnumerationOptions::EnsuresLayout,
            &block,
        );
    }
}

/// Draws the whitespace of one paragraph, whose layout fragment's top left corner is at `corner` in the editor.
fn draw_fragment(fragment: &NSTextLayoutFragment, corner: NSPoint) {
    let Some(lines) = super::insets::line_fragments(fragment) else {
        return;
    };
    let Some(first) = lines.firstObject() else { return };
    let units: Vec<u16> = first.attributedString().string().to_string().encode_utf16().collect();
    let end = units
        .iter()
        .rposition(|&u| u != u16::from(b'\n') && u != u16::from(b'\r'))
        .map_or(0, |i| i + 1);
    let trailing = units[..end]
        .iter()
        .rposition(|&u| u != u16::from(b' ') && u != u16::from(b'\t'))
        .map_or(0, |i| i + 1);
    for fragment in &lines {
        let bounds = fragment.typographicBounds();
        let line = Line {
            fragment: &fragment,
            origin: NSPoint::new(corner.x + bounds.origin.x, corner.y + bounds.origin.y),
            height: bounds.size.height,
            units: &units,
        };
        let range = fragment.characterRange();
        let span = range.location..(range.location + range.length).min(end);
        draw_line(&line, span, trailing);
    }
}

fn draw_line(line: &Line<'_>, span: std::ops::Range<usize>, trailing: usize) {
    let from = span.start.max(trailing);
    if from < span.end {
        let left = line.x(from);
        let right = line.x(span.end);
        NSColor::systemRedColor().colorWithAlphaComponent(0.16).setFill();
        objc2_app_kit::NSRectFill(NSRect::new(
            NSPoint::new(left, line.origin.y),
            NSSize::new(right - left, line.height),
        ));
    }
    NSColor::tertiaryLabelColor().set();
    let middle = line.origin.y + line.height / 2.0;
    let mut next_x = None;
    for index in span {
        let unit = line.units[index];
        let is_space = unit == u16::from(b' ');
        let is_tab = unit == u16::from(b'\t');
        if !is_space && !is_tab {
            next_x = None;
            continue;
        }
        let left = next_x.unwrap_or_else(|| line.x(index));
        let right = line.x(index + 1);
        next_x = Some(right);
        if is_space {
            let size = 2.0;
            let dot = NSRect::new(
                NSPoint::new((left + right - size) / 2.0, middle - size / 2.0),
                NSSize::new(size, size),
            );
            NSBezierPath::bezierPathWithOvalInRect(dot).fill();
        } else if right - left > 6.0 {
            arrow(left + 2.0, right - 2.0, middle);
        }
    }
}

/// An arrow from `left` to `right` at height `y`.
fn arrow(left: f64, right: f64, y: f64) {
    let path = NSBezierPath::bezierPath();
    path.moveToPoint(NSPoint::new(left, y));
    path.lineToPoint(NSPoint::new(right, y));
    path.moveToPoint(NSPoint::new(right - 3.0, y - 3.0));
    path.lineToPoint(NSPoint::new(right, y));
    path.lineToPoint(NSPoint::new(right - 3.0, y + 3.0));
    path.setLineWidth(1.0);
    path.stroke();
}

#[cfg(feature = "selftest")]
mod selftest_probe;
#[cfg(feature = "selftest")]
pub(crate) use selftest_probe::*;

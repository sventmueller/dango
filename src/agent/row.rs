//! A row of the timeline: the vertical line on the leading side with the entry's dot, and the keyboard selection.

use std::cell::{Cell, RefCell};

use objc2::rc::Retained;
use objc2::{DefinedClass, MainThreadMarker, MainThreadOnly, define_class, msg_send};
use objc2_app_kit::{NSBezierPath, NSColor, NSTableRowView, NSUserInterfaceLayoutDirection};
use objc2_foundation::{NSObjectProtocol, NSPoint, NSRect, NSSize};

use crate::util::guarded;

/// Where the line runs, counted from the leading edge.
pub const LINE_X: f64 = 13.0;
/// How far below the row's top the dot sits: in the middle of the first line of text.
pub const DOT_Y: f64 = 13.0;
const DOT_RADIUS: f64 = 3.5;

/// How a row is marked.
#[derive(Clone, Default)]
pub struct Mark {
    /// The dot's colour; no dot without one.
    pub dot: Option<Retained<NSColor>>,
    /// Whether the line comes from the row above.
    pub line_above: bool,
    /// Whether the line goes on to the row below.
    pub line_below: bool,
}

#[derive(Default)]
pub struct RowIvars {
    mark: RefCell<Mark>,
    rtl: Cell<bool>,
}

define_class!(
    // SAFETY: NSTableRowView has no subclassing requirements and `TimelineRow` does not implement Drop.
    #[unsafe(super(NSTableRowView))]
    #[thread_kind = MainThreadOnly]
    #[name = "IDEAgentTimelineRow"]
    #[ivars = RowIvars]
    pub struct TimelineRow;

    unsafe impl NSObjectProtocol for TimelineRow {}

    impl TimelineRow {
        #[unsafe(method(isFlipped))]
        fn is_flipped(&self) -> bool {
            true
        }

        #[unsafe(method(drawBackgroundInRect:))]
        fn draw_background(&self, _dirty: NSRect) {
            guarded((), || self.draw_mark());
        }

        #[unsafe(method(drawSelectionInRect:))]
        fn draw_selection(&self, _dirty: NSRect) {
            guarded((), || {
                let bounds = self.bounds();
                let rect = NSRect::new(
                    NSPoint::new(bounds.origin.x + 2.0, bounds.origin.y + 1.0),
                    NSSize::new(bounds.size.width - 4.0, bounds.size.height - 2.0),
                );
                let color = if self.isEmphasized() {
                    NSColor::selectedContentBackgroundColor().colorWithAlphaComponent(0.12)
                } else {
                    NSColor::unemphasizedSelectedContentBackgroundColor().colorWithAlphaComponent(0.5)
                };
                color.setFill();
                NSBezierPath::bezierPathWithRoundedRect_xRadius_yRadius(rect, 6.0, 6.0).fill();
            });
        }
    }
);

impl TimelineRow {
    pub fn new(mtm: MainThreadMarker, mark: Mark) -> Retained<TimelineRow> {
        let this = Self::alloc(mtm).set_ivars(RowIvars::default());
        // SAFETY: NSTableRowView's initializer, called once on the newly allocated object.
        let row: Retained<TimelineRow> = unsafe { msg_send![super(this), init] };
        *row.ivars().mark.borrow_mut() = mark;
        row.ivars()
            .rtl
            .set(row.userInterfaceLayoutDirection() == NSUserInterfaceLayoutDirection::RightToLeft);
        row
    }

    pub fn set_mark(&self, mark: Mark) {
        *self.ivars().mark.borrow_mut() = mark;
        self.setNeedsDisplay(true);
    }

    fn draw_mark(&self) {
        let mark = self.ivars().mark.borrow();
        let bounds = self.bounds();
        let x = if self.ivars().rtl.get() {
            bounds.size.width - LINE_X
        } else {
            LINE_X
        };
        if mark.line_above || mark.line_below {
            let top = if mark.line_above { 0.0 } else { DOT_Y };
            let bottom = if mark.line_below { bounds.size.height } else { DOT_Y };
            if bottom > top {
                NSColor::separatorColor().setFill();
                NSBezierPath::fillRect(NSRect::new(NSPoint::new(x - 0.5, top), NSSize::new(1.0, bottom - top)));
            }
        }
        if let Some(color) = &mark.dot {
            let rect = NSRect::new(
                NSPoint::new(x - DOT_RADIUS, DOT_Y - DOT_RADIUS),
                NSSize::new(DOT_RADIUS * 2.0, DOT_RADIUS * 2.0),
            );
            color.setFill();
            NSBezierPath::bezierPathWithOvalInRect(rect).fill();
        }
    }
}

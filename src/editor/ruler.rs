//! Line numbers beside the text, drawn for the lines on screen only. A view of its own beside the scroll view:
//! NSScrollView's ruler stops TextKit 2 text from drawing on macOS 27.

use std::ptr::NonNull;

use block2::RcBlock;
use objc2::rc::{Retained, Weak};
use objc2::runtime::Bool;
use objc2::{DefinedClass, MainThreadMarker, MainThreadOnly, define_class, msg_send};
use objc2_app_kit::{
    NSColor, NSEvent, NSFont, NSFontAttributeName, NSForegroundColorAttributeName, NSScrollView, NSStringDrawing,
    NSTextElementProvider, NSTextLayoutFragment, NSTextLayoutFragmentEnumerationOptions, NSView,
    NSViewBoundsDidChangeNotification, NSViewFrameDidChangeNotification,
};
use objc2_foundation::{
    NSDictionary, NSNotification, NSNotificationCenter, NSObjectProtocol, NSPoint, NSRect, NSSize, NSString,
};

use super::view::EditorTextView;
use crate::util::{constant, guarded, ns};

/// Where the numbers begin, level with the left edge of the tab bar; diagnostic dots sit in the margin before.
const NUMBERS_LEFT: f64 = 6.0;

/// Between the numbers and the strip of fold triangles.
const NUMBERS_GAP: f64 = 3.0;

/// The strip at the column's right edge that shows and answers change marks.
const CHANGE_STRIP: f64 = 8.0;

/// The width of the strip of fold triangles, between the numbers and the change marks.
const FOLD_WIDTH: f64 = 12.0;

/// The size of the numbers for an editor whose text has `font_size`.
fn number_font(font_size: f64) -> Retained<NSFont> {
    NSFont::monospacedDigitSystemFontOfSize_weight(font_size * 0.85, constant!(objc2_app_kit::NSFontWeightRegular))
}

fn number_attributes(font_size: f64) -> Retained<NSDictionary<NSString, objc2::runtime::AnyObject>> {
    NSDictionary::<NSString, objc2::runtime::AnyObject>::from_retained_objects(
        &[
            constant!(NSFontAttributeName),
            constant!(NSForegroundColorAttributeName),
        ],
        &[number_font(font_size).into(), NSColor::tertiaryLabelColor().into()],
    )
}

/// The column's width for a text of `lines` lines in `font_size`: as wide as its highest number needs.
pub fn width_for(lines: usize, font_size: f64) -> f64 {
    let digits = lines.max(1).to_string();
    let widest = ns(&"0".repeat(digits.len()));
    // SAFETY: The attributes hold a font and a colour, as the method expects.
    let numbers = unsafe { widest.sizeWithAttributes(Some(&number_attributes(font_size))) }.width;
    (NUMBERS_LEFT + numbers + NUMBERS_GAP + FOLD_WIDTH + CHANGE_STRIP).ceil()
}

pub struct GutterIvars {
    editor: Weak<EditorTextView>,
    /// The texts of the diagnostic markers' tooltips; a view holds the owners of its tooltips only weakly.
    tips: std::cell::RefCell<Vec<Retained<NSString>>>,
    /// Whether the mouse is over the column, which then shows every fold's triangle.
    hovered: std::cell::Cell<bool>,
    /// The constraint that holds the column's width, which follows the number of lines.
    width: std::cell::RefCell<Option<Retained<objc2_app_kit::NSLayoutConstraint>>>,
}

define_class!(
    // SAFETY: NSView has no subclassing requirements and `LineNumbers` does not implement Drop.
    #[unsafe(super(NSView))]
    #[thread_kind = MainThreadOnly]
    #[name = "IDELineNumbers"]
    #[ivars = GutterIvars]
    pub struct LineNumbers;

    unsafe impl NSObjectProtocol for LineNumbers {}

    impl LineNumbers {

        #[unsafe(method(isFlipped))]
        fn is_flipped(&self) -> bool {
            true
        }

        #[unsafe(method(drawRect:))]
        fn draw_rect(&self, rect: NSRect) {
            guarded((), || self.draw_numbers(rect));
        }

        #[unsafe(method(mouseDown:))]
        fn mouse_down(&self, event: &NSEvent) {
            guarded((), || self.clicked(event));
        }

        #[unsafe(method(mouseEntered:))]
        fn mouse_entered(&self, _event: &NSEvent) {
            guarded((), || self.set_hovered(true));
        }

        #[unsafe(method(mouseExited:))]
        fn mouse_exited(&self, _event: &NSEvent) {
            guarded((), || self.set_hovered(false));
        }

        #[unsafe(method(updateTrackingAreas))]
        fn update_tracking_areas(&self) {
            // SAFETY: The superclass's implementation, with this method's signature.
            let _: () = unsafe { msg_send![super(self), updateTrackingAreas] };
            guarded((), || self.track_hover());
        }
    }
);

impl LineNumbers {
    /// A click on the change strip opens a change mark's committed lines, one on the fold strip folds or unfolds.
    fn clicked(&self, event: &NSEvent) {
        let x = self.convertPoint_fromView(event.locationInWindow(), None).x;
        let fold = self.fold_left();
        if x >= self.bounds().size.width - CHANGE_STRIP {
            let _ = self.click_change(event);
        } else if (fold - 3.0..fold + FOLD_WIDTH).contains(&x) {
            self.fold_click(event);
        }
    }

    /// Where the strip of fold triangles begins, right of the numbers.
    fn fold_left(&self) -> f64 {
        self.bounds().size.width - CHANGE_STRIP - FOLD_WIDTH
    }

    /// Holds the column at `constraint`'s width, which `fit` sets.
    pub fn set_width_constraint(&self, constraint: Retained<objc2_app_kit::NSLayoutConstraint>) {
        *self.ivars().width.borrow_mut() = Some(constraint);
        self.fit();
    }

    /// Makes the column as wide as the editor's highest line number needs.
    pub fn fit(&self) {
        let Some(editor) = self.ivars().editor.load() else {
            return;
        };
        let width = width_for(editor.document().text().line_count(), editor.ivars().font_size.get());
        if let Some(constraint) = self.ivars().width.borrow().as_ref()
            && (constraint.constant() - width).abs() > 0.5
        {
            constraint.setConstant(width);
            self.setNeedsDisplay(true);
        }
    }

    /// Draws the triangle of a fold that starts in `row`: pointing right when folded, always shown; pointing down when
    /// open, shown while the mouse is over the column.
    fn fold_mark(&self, row: NSRect, folded: bool) {
        if !folded && !self.ivars().hovered.get() {
            return;
        }
        let side = 7.0;
        let x = row.origin.x + (row.size.width - side) / 2.0;
        let y = row.origin.y + (row.size.height - side) / 2.0;
        let path = objc2_app_kit::NSBezierPath::bezierPath();
        if folded {
            path.moveToPoint(NSPoint::new(x + 1.0, y));
            path.lineToPoint(NSPoint::new(x + side, y + side / 2.0));
            path.lineToPoint(NSPoint::new(x + 1.0, y + side));
        } else {
            path.moveToPoint(NSPoint::new(x, y + 1.0));
            path.lineToPoint(NSPoint::new(x + side, y + 1.0));
            path.lineToPoint(NSPoint::new(x + side / 2.0, y + side));
        }
        path.closePath();
        NSColor::secondaryLabelColor().setFill();
        path.fill();
    }

    /// Folds or unfolds the range that starts on the clicked line.
    fn fold_click(&self, event: &NSEvent) {
        let Some(editor) = self.ivars().editor.load() else {
            return;
        };
        let in_editor = editor.convertPoint_fromView(event.locationInWindow(), None);
        if let Some(line) = super::highlight::line_at(&editor, NSPoint::new(0.0, in_editor.y)) {
            super::fold::toggle(&editor, line);
        }
    }

    fn set_hovered(&self, hovered: bool) {
        self.ivars().hovered.set(hovered);
        self.setNeedsDisplay(true);
    }

    /// Hears the mouse enter and leave the column.
    fn track_hover(&self) {
        use objc2::AnyThread;
        use objc2_app_kit::{NSTrackingArea, NSTrackingAreaOptions};
        let options = NSTrackingAreaOptions::MouseEnteredAndExited
            | NSTrackingAreaOptions::ActiveInKeyWindow
            | NSTrackingAreaOptions::InVisibleRect;
        if self.trackingAreas().iter().any(|area| area.options().contains(options)) {
            return;
        }
        // SAFETY: The column owns the area and outlives it; AppKit holds the owner weakly.
        let area = unsafe {
            NSTrackingArea::initWithRect_options_owner_userInfo(
                NSTrackingArea::alloc(),
                NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(0.0, 0.0)),
                options,
                Some(self),
                None,
            )
        };
        self.addTrackingArea(&area);
    }

    /// Draws a diagnostic's dot in `row` and gives it the message as its tooltip.
    fn mark(&self, row: NSRect, severity: crate::lsp::Severity, message: &str) {
        let size = 5.0;
        let dot = NSRect::new(
            NSPoint::new(
                (NUMBERS_LEFT - size) / 2.0,
                row.origin.y + (row.size.height - size) / 2.0,
            ),
            NSSize::new(size, size),
        );
        severity.color().setFill();
        objc2_app_kit::NSBezierPath::bezierPathWithOvalInRect(dot).fill();
        let tip = ns(message);
        // SAFETY: The owner is a string, whose description is the tooltip; `tips` keeps it alive while it is shown.
        unsafe { self.addToolTipRect_owner_userData(row, &tip, std::ptr::null_mut()) };
        self.ivars().tips.borrow_mut().push(tip);
    }

    /// Draws how `line` differs from the last commit at the right edge of `row`, with its description as the tooltip.
    fn change_mark(&self, editor: &EditorTextView, line: usize, row: NSRect) {
        let Some(mark) = super::changes::mark_at(editor, line) else {
            return;
        };
        super::changes::draw(mark, row, row.size.width);
        let area = NSRect::new(
            NSPoint::new(row.size.width - CHANGE_STRIP, row.origin.y),
            NSSize::new(CHANGE_STRIP, row.size.height),
        );
        let tip = ns(mark.description());
        // SAFETY: The owner is a string, whose description is the tooltip; `tips` keeps it alive while it is shown.
        unsafe { self.addToolTipRect_owner_userData(area, &tip, std::ptr::null_mut()) };
        self.ivars().tips.borrow_mut().push(tip);
    }

    /// Opens the committed lines of the change mark under a click near the column's right edge.
    fn click_change(&self, event: &NSEvent) -> bool {
        let Some(editor) = self.ivars().editor.load() else {
            return false;
        };
        let point = self.convertPoint_fromView(event.locationInWindow(), None);
        let width = self.bounds().size.width;
        if point.x < width - CHANGE_STRIP {
            return false;
        }
        let in_editor = editor.convertPoint_fromView(event.locationInWindow(), None);
        let Some(line) = super::highlight::line_at(&editor, NSPoint::new(0.0, in_editor.y)) else {
            return false;
        };
        let at = NSRect::new(
            NSPoint::new(width - CHANGE_STRIP, point.y - 1.0),
            NSSize::new(CHANGE_STRIP, 2.0),
        );
        super::changes::clicked(&editor, line, self, at)
    }

    /// The column for `editor`, redrawn whenever the text scrolls or changes size.
    pub fn new(mtm: MainThreadMarker, scroll: &NSScrollView, editor: &EditorTextView) -> Retained<LineNumbers> {
        let this = Self::alloc(mtm).set_ivars(GutterIvars {
            editor: Weak::from(editor),
            tips: std::cell::RefCell::new(vec![]),
            hovered: std::cell::Cell::new(false),
            width: std::cell::RefCell::new(None),
        });
        // SAFETY: NSView's initializer, called once on the newly allocated object.
        let this: Retained<Self> = unsafe { msg_send![super(this), init] };
        // Since macOS 14 a view may draw past its bounds, which let a number spill onto the status bar below.
        this.setClipsToBounds(true);
        let clip = scroll.contentView();
        clip.setPostsBoundsChangedNotifications(true);
        editor.setPostsFrameChangedNotifications(true);
        let center = NSNotificationCenter::defaultCenter();
        for (name, object) in [
            (constant!(NSViewBoundsDidChangeNotification), &*clip as &NSView),
            (constant!(NSViewFrameDidChangeNotification), editor as &NSView),
        ] {
            let gutter = Weak::from(&*this);
            let block = RcBlock::new(move |_: NonNull<NSNotification>| {
                if let Some(gutter) = gutter.load() {
                    gutter.setNeedsDisplay(true);
                }
            });
            // SAFETY: The block only marks the view for redrawing; the weak reference makes it do nothing once the
            // column is gone.
            let observer =
                unsafe { center.addObserverForName_object_queue_usingBlock(Some(name), Some(object), None, &block) };
            std::mem::forget(observer);
        }
        this
    }

    fn draw_numbers(&self, rect: NSRect) {
        NSColor::textBackgroundColor().setFill();
        objc2_app_kit::NSRectFill(rect);
        let Some(editor) = self.ivars().editor.load() else {
            return;
        };
        let (Some(layout), Some(content)) = (editor.textLayoutManager(), editor.textContentStorage()) else {
            return;
        };
        let attributes = number_attributes(editor.ivars().font_size.get());
        let origin = editor.textContainerOrigin();
        let visible = editor.visibleRect();
        let start = layout
            .textLayoutFragmentForPosition(NSPoint::new(0.0, visible.origin.y - origin.y))
            .map(|fragment| fragment.rangeInElement().location());
        let document_start = content.documentRange().location();
        let width = self.bounds().size.width;
        self.removeAllToolTips();
        self.ivars().tips.borrow_mut().clear();
        let marks = crate::lsp::line_marks(&editor);
        let folds = super::fold::gutter(&editor);
        let bulb = crate::lsp::bulb_line(&editor);
        let bulb_row = std::cell::Cell::new(None);
        let bottom = visible.origin.y + visible.size.height;
        let block = RcBlock::new(|fragment: NonNull<NSTextLayoutFragment>| -> Bool {
            // SAFETY: TextKit keeps the fragment alive for the call.
            let fragment = unsafe { fragment.as_ref() };
            let y = fragment.layoutFragmentFrame().origin.y + origin.y;
            if y > bottom {
                return Bool::NO;
            }
            let offset = content.offsetFromLocation_toLocation(&document_start, &fragment.rangeInElement().location());
            let document = editor.document();
            let line = usize::try_from(offset)
                .ok()
                .and_then(|o| document.text().utf16_to_byte(o).ok())
                .map(|byte| document.text().line_of(byte) + 1);
            if let Some(line) = line {
                let label = ns(&line.to_string());
                // SAFETY: The attributes hold a font and a colour, as the method expects.
                let label_width = unsafe { label.sizeWithAttributes(Some(&attributes)) }.width;
                let point = self.convertPoint_fromView(NSPoint::new(0.0, y), Some(&editor));
                let at = NSPoint::new(self.fold_left() - NUMBERS_GAP - label_width, point.y + 1.0);
                let height = fragment.layoutFragmentFrame().size.height;
                if bulb == Some(line - 1) && bulb_row.get().is_none() {
                    let numbers = self.fold_left() - NUMBERS_GAP - NUMBERS_LEFT;
                    bulb_row.set(Some(NSRect::new(
                        NSPoint::new(NUMBERS_LEFT, point.y),
                        NSSize::new(numbers, height),
                    )));
                } else {
                    // SAFETY: As above.
                    unsafe { label.drawAtPoint_withAttributes(at, Some(&attributes)) };
                }
                if let Some(folded) = folds.get(&(line - 1)) {
                    self.fold_mark(
                        NSRect::new(NSPoint::new(self.fold_left(), point.y), NSSize::new(FOLD_WIDTH, height)),
                        *folded,
                    );
                }
                if let Some((severity, message)) = marks.get(&(line - 1)) {
                    self.mark(
                        NSRect::new(NSPoint::new(0.0, point.y), NSSize::new(NUMBERS_LEFT, height)),
                        *severity,
                        message,
                    );
                }
                self.change_mark(
                    &editor,
                    line - 1,
                    NSRect::new(NSPoint::new(0.0, point.y), NSSize::new(width, height)),
                );
            }
            Bool::YES
        });
        let _ = layout.enumerateTextLayoutFragmentsFromLocation_options_usingBlock(
            start.as_deref(),
            NSTextLayoutFragmentEnumerationOptions::EnsuresLayout,
            &block,
        );
        crate::lsp::place_bulb(&editor, self, bulb_row.get());
    }
}

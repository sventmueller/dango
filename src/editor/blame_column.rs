//! The blame column (E86, "Blame zeigen"): before the line numbers, one entry per block of lines that one commit
//! changed last, author and age; a click opens the block's popover. Zero wide while hidden.

use std::ptr::NonNull;

use block2::RcBlock;
use objc2::rc::{Retained, Weak};
use objc2::runtime::Bool;
use objc2::{DefinedClass, MainThreadMarker, MainThreadOnly, define_class, msg_send};
use objc2_app_kit::{
    NSAccessibility, NSBezierPath, NSColor, NSEvent, NSFontAttributeName, NSForegroundColorAttributeName,
    NSLayoutConstraint, NSLineBreakMode, NSMutableParagraphStyle, NSParagraphStyleAttributeName, NSScrollView,
    NSStringDrawing, NSTextElementProvider, NSTextLayoutFragment, NSTextLayoutFragmentEnumerationOptions, NSView,
    NSViewBoundsDidChangeNotification,
};
use objc2_foundation::{NSDictionary, NSObjectProtocol, NSPoint, NSRect, NSSize, NSString};
use splat_i18n::t;

use super::blame::{self, Line};
use super::view::EditorTextView;
use crate::util::{constant, guarded, ns};

/// The column's width while it shows.
pub(crate) const WIDTH: f64 = 190.0;
const INSET: f64 = 8.0;

pub(crate) struct ColumnIvars {
    editor: Weak<EditorTextView>,
    width: std::cell::RefCell<Option<Retained<NSLayoutConstraint>>>,
    /// The scrolling it redraws for, which ends with the column.
    scrolled: std::cell::OnceCell<crate::util::Observation>,
}

define_class!(
    // SAFETY: NSView has no subclassing requirements and `BlameColumn` does not implement Drop.
    #[unsafe(super(NSView))]
    #[thread_kind = MainThreadOnly]
    #[name = "IDEBlameColumn"]
    #[ivars = ColumnIvars]
    pub(crate) struct BlameColumn;

    unsafe impl NSObjectProtocol for BlameColumn {}

    impl BlameColumn {
        #[unsafe(method(isFlipped))]
        fn is_flipped(&self) -> bool {
            true
        }

        #[unsafe(method(drawRect:))]
        fn draw_rect(&self, rect: NSRect) {
            guarded((), || self.draw(rect));
        }

        #[unsafe(method(mouseDown:))]
        fn mouse_down(&self, event: &NSEvent) {
            guarded((), || self.clicked(event));
        }
    }
);

/// One entry of the column: the block's first line, counted from 0, and its top and height in the column.
struct Entry {
    line: usize,
    top: f64,
    height: f64,
    text: String,
}

impl BlameColumn {
    /// The column for `editor`, hidden, redrawn when `scroll` scrolls.
    pub(crate) fn new(mtm: MainThreadMarker, scroll: &NSScrollView, editor: &EditorTextView) -> Retained<BlameColumn> {
        let this = Self::alloc(mtm).set_ivars(ColumnIvars {
            editor: Weak::from(editor),
            width: std::cell::RefCell::new(None),
            scrolled: std::cell::OnceCell::new(),
        });
        // SAFETY: NSView's initializer, called once on the newly allocated object.
        let this: Retained<Self> = unsafe { msg_send![super(this), init] };
        this.setClipsToBounds(true);
        this.setAccessibilityElement(true);
        this.setAccessibilityRole(Some(constant!(objc2_app_kit::NSAccessibilityGroupRole)));
        this.setAccessibilityLabel(Some(&ns(t("Blame"))));
        let clip = scroll.contentView();
        clip.setPostsBoundsChangedNotifications(true);
        let column = Weak::from(&*this);
        let name = constant!(NSViewBoundsDidChangeNotification);
        let watch = crate::util::observe(mtm, name, Some(&clip), move |_| {
            if let Some(column) = column.load()
                && column.frame().size.width > 0.0
            {
                column.setNeedsDisplay(true);
            }
        });
        let _ = this.ivars().scrolled.set(watch);
        editor.ivars().blame.borrow_mut().view = Some(Weak::from(&*this));
        this
    }

    /// Holds the column at `constraint`'s width, zero while hidden.
    pub(crate) fn set_width_constraint(&self, constraint: Retained<NSLayoutConstraint>) {
        *self.ivars().width.borrow_mut() = Some(constraint);
    }

    fn attributes(color: &NSColor) -> Retained<NSDictionary<NSString, objc2::runtime::AnyObject>> {
        let style = NSMutableParagraphStyle::new();
        style.setLineBreakMode(NSLineBreakMode::ByTruncatingTail);
        NSDictionary::<NSString, objc2::runtime::AnyObject>::from_retained_objects(
            &[
                constant!(NSFontAttributeName),
                constant!(NSForegroundColorAttributeName),
                constant!(NSParagraphStyleAttributeName),
            ],
            &[
                ide_ui_common::style::small_system().into(),
                objc2::Message::retain(color).into(),
                style.into_super().into(),
            ],
        )
    }

    /// The entries on screen: a block starts where a line's commit differs from the line before.
    fn entries(&self, editor: &EditorTextView) -> Vec<Entry> {
        let Some(lines) = blame::all_lines(editor) else {
            return Vec::new();
        };
        let (Some(layout), Some(content)) = (editor.textLayoutManager(), editor.textContentStorage()) else {
            return Vec::new();
        };
        let origin = editor.textContainerOrigin();
        let visible = editor.visibleRect();
        let start = layout
            .textLayoutFragmentForPosition(NSPoint::new(0.0, visible.origin.y - origin.y))
            .map(|fragment| fragment.rangeInElement().location());
        let document_start = content.documentRange().location();
        let bottom = visible.origin.y + visible.size.height;
        let entries = std::cell::RefCell::new(Vec::new());
        let block = RcBlock::new(|fragment: NonNull<NSTextLayoutFragment>| -> Bool {
            crate::util::guarded(Bool::NO, || {
                // SAFETY: TextKit keeps the fragment alive for the call.
                let fragment = unsafe { fragment.as_ref() };
                let frame = fragment.layoutFragmentFrame();
                let y = frame.origin.y + origin.y;
                if y > bottom {
                    return Bool::NO;
                }
                let offset =
                    content.offsetFromLocation_toLocation(&document_start, &fragment.rangeInElement().location());
                let document = editor.document();
                let Some(line) = usize::try_from(offset)
                    .ok()
                    .and_then(|o| document.text().utf16_to_byte(o).ok())
                    .map(|byte| document.text().line_of(byte))
                else {
                    return Bool::YES;
                };
                let here = lines.at(line);
                let starts = line == 0
                    || entries.borrow().is_empty()
                    || lines.at(line - 1) != here
                    || super::fold::hidden_by_fold(editor, editor.line_start(line.saturating_sub(1)).unwrap_or(0));
                if let Some(here) = here.filter(|_| starts) {
                    let top = self.convertPoint_fromView(NSPoint::new(0.0, y), Some(editor)).y;
                    entries.borrow_mut().push(Entry {
                        line,
                        top,
                        height: frame.size.height,
                        text: entry_text(&here),
                    });
                }
                Bool::YES
            })
        });
        let _ = layout.enumerateTextLayoutFragmentsFromLocation_options_usingBlock(
            start.as_deref(),
            NSTextLayoutFragmentEnumerationOptions::EnsuresLayout,
            &block,
        );
        drop(block);
        entries.into_inner()
    }

    fn draw(&self, rect: NSRect) {
        NSColor::textBackgroundColor().setFill();
        objc2_app_kit::NSRectFill(rect);
        let Some(editor) = self.ivars().editor.load() else {
            return;
        };
        let width = self.bounds().size.width;
        NSColor::separatorColor().setFill();
        objc2_app_kit::NSRectFill(NSRect::new(
            NSPoint::new(width - 1.0, rect.origin.y),
            NSSize::new(1.0, rect.size.height),
        ));
        let normal = Self::attributes(&NSColor::secondaryLabelColor());
        let uncommitted = Self::attributes(&NSColor::labelColor());
        let entries = self.entries(&editor);
        for (index, entry) in entries.iter().enumerate() {
            if index > 0 {
                NSColor::separatorColor().setStroke();
                let path = NSBezierPath::bezierPath();
                path.moveToPoint(NSPoint::new(INSET, entry.top + 0.5));
                path.lineToPoint(NSPoint::new(width - INSET, entry.top + 0.5));
                path.setLineWidth(0.5);
                path.stroke();
            }
            let attributes = if entry.text == t("Nicht committet") {
                &uncommitted
            } else {
                &normal
            };
            let text = ns(&entry.text);
            let box_ = NSRect::new(
                NSPoint::new(INSET, entry.top + 1.0),
                NSSize::new(width - 2.0 * INSET, entry.height),
            );
            // SAFETY: The attributes hold a font, a colour and a paragraph style, as the method expects.
            unsafe { text.drawInRect_withAttributes(box_, Some(attributes)) };
        }
    }

    /// A click on an entry opens the popover of its block.
    fn clicked(&self, event: &NSEvent) {
        let Some(editor) = self.ivars().editor.load() else {
            return;
        };
        let y = self.convertPoint_fromView(event.locationInWindow(), None).y;
        let entries = self.entries(&editor);
        let hit = entries.iter().rev().find(|entry| entry.top <= y);
        if let Some(entry) = hit {
            let rect = NSRect::new(
                NSPoint::new(0.0, entry.top),
                NSSize::new(self.bounds().size.width, entry.height),
            );
            blame::show_popover(&editor, entry.line, self, rect);
        }
    }
}

/// An entry's text: the author and the age, or that the lines are not committed.
fn entry_text(line: &Line) -> String {
    match line {
        Line::Uncommitted => t("Nicht committet").to_string(),
        Line::Commit(commit) => splat_i18n::tf("{} · {}", &[&commit.author, &blame::age(commit.author_time)]),
    }
}

fn column_of(view: &EditorTextView) -> Option<Retained<BlameColumn>> {
    view.ivars().blame.borrow().view.as_ref().and_then(Weak::load)
}

/// Shows or hides the column of `view`.
pub(crate) fn show(view: &EditorTextView, on: bool) {
    let Some(column) = column_of(view) else { return };
    if let Some(constraint) = column.ivars().width.borrow().as_ref() {
        constraint.setConstant(if on { WIDTH } else { 0.0 });
    }
    column.setAccessibilityElement(on);
    column.setNeedsDisplay(true);
}

/// Draws the column of `view` again, if it shows.
pub(crate) fn redraw(view: &EditorTextView) {
    if let Some(column) = column_of(view)
        && column.frame().size.width > 0.0
    {
        column.setNeedsDisplay(true);
    }
}

#[cfg(feature = "selftest")]
mod selftest_probe;
#[cfg(feature = "selftest")]
pub(crate) use selftest_probe::*;

//! The outline view of the sidebar's lists, the folder tree and the AI panel's, as VS Code lays them out: a row that
//! opens shows a chevron where another row shows its icon, so names line up whatever a row is, and the outline view's
//! own disclosure triangle is gone.

use std::cell::Cell;

use objc2::rc::Retained;
use objc2::runtime::NSObjectProtocol;
use objc2::{DefinedClass, MainThreadMarker, MainThreadOnly, define_class, msg_send};
use objc2_app_kit::{NSColor, NSImage, NSImageSymbolConfiguration, NSOutlineView, NSUserInterfaceLayoutDirection};
use objc2_foundation::{NSRect, NSSize};

/// Where a top-level row of the folder tree begins.
const LEADING: f64 = 12.0;

pub struct OutlineIvars {
    /// Where a top-level row begins.
    leading: Cell<f64>,
    /// How far a row's cell ends from the far edge; AppKit's source list inset when `None`.
    trailing: Option<f64>,
}
/// The size of a folder's chevron.
const CHEVRON_POINTS: f64 = 10.0;

define_class!(
    // SAFETY: NSOutlineView has no subclassing requirements and `FileOutline` does not implement Drop.
    #[unsafe(super(NSOutlineView))]
    #[thread_kind = MainThreadOnly]
    #[name = "IDEFileOutline"]
    #[ivars = OutlineIvars]
    pub struct FileOutline;

    unsafe impl NSObjectProtocol for FileOutline {}

    impl FileOutline {
        /// No disclosure triangle: a folder's cell shows its chevron as its icon.
        #[unsafe(method(frameOfOutlineCellAtRow:))]
        fn outline_cell_frame(&self, _row: isize) -> NSRect {
            NSRect::ZERO
        }

        /// The cell begins at the row's indentation, without room for a triangle; from the right in a right-to-left
        /// interface.
        #[unsafe(method(frameOfCellAtColumn:row:))]
        fn cell_frame(&self, column: isize, row: isize) -> NSRect {
            // SAFETY: The superclass's implementation, with this method's signature.
            let frame: NSRect = unsafe { msg_send![super(self), frameOfCellAtColumn: column, row: row] };
            if column != 0 {
                return frame;
            }
            let indent = self.ivars().leading.get() + self.levelForRow(row).max(0) as f64 * self.indentationPerLevel();
            let (left, right) = if self.userInterfaceLayoutDirection() == NSUserInterfaceLayoutDirection::RightToLeft {
                let left = self.ivars().trailing.unwrap_or(frame.origin.x);
                (left, self.bounds().size.width - indent)
            } else {
                let right = self
                    .ivars()
                    .trailing
                    .map_or(frame.origin.x + frame.size.width, |trailing| self.bounds().size.width - trailing);
                (indent, right)
            };
            NSRect::new(
                objc2_foundation::NSPoint::new(left, frame.origin.y),
                NSSize::new((right - left).max(0.0), frame.size.height),
            )
        }
    }
);

impl FileOutline {
    /// The folder tree's.
    pub fn new(mtm: MainThreadMarker, frame: NSRect) -> Retained<FileOutline> {
        FileOutline::with_insets(mtm, frame, LEADING, None)
    }

    /// One whose top-level rows begin `leading` from its edge and end `trailing` from the far one.
    pub fn with_insets(
        mtm: MainThreadMarker,
        frame: NSRect,
        leading: f64,
        trailing: Option<f64>,
    ) -> Retained<FileOutline> {
        let this = Self::alloc(mtm).set_ivars(OutlineIvars {
            leading: Cell::new(leading),
            trailing,
        });
        // SAFETY: NSOutlineView's designated initializer, called once on the newly allocated object.
        unsafe { msg_send![super(this), initWithFrame: frame] }
    }
}

/// `frame` of a list's disclosure triangle, or of its first column's cell when `cell`, moved `by` toward the leading
/// edge; the cell keeps its far edge. For lists that keep AppKit's triangle but line up with the panel's title.
pub fn outdent(view: &NSOutlineView, frame: NSRect, by: f64, cell: bool) -> NSRect {
    if frame.size.width <= 0.0 {
        return frame;
    }
    let rtl = view.userInterfaceLayoutDirection() == NSUserInterfaceLayoutDirection::RightToLeft;
    let x = match (rtl, cell) {
        (false, _) => (frame.origin.x - by).max(0.0),
        (true, false) => frame.origin.x + by,
        (true, true) => frame.origin.x,
    };
    let width = if cell { frame.size.width + by } else { frame.size.width };
    NSRect::new(
        objc2_foundation::NSPoint::new(x, frame.origin.y),
        NSSize::new(width, frame.size.height),
    )
}

/// A folder's chevron, pointing down when it is open.
pub fn chevron(open: bool) -> Option<Retained<NSImage>> {
    let name = if open { "chevron.down" } else { "chevron.right" };
    let configuration = NSImageSymbolConfiguration::configurationWithPointSize_weight(
        CHEVRON_POINTS,
        // SAFETY: AppKit sets its font weight constants when it loads.
        unsafe { objc2_app_kit::NSFontWeightSemibold },
    )
    .configurationByApplyingConfiguration(&NSImageSymbolConfiguration::configurationWithHierarchicalColor(
        &NSColor::secondaryLabelColor(),
    ));
    crate::util::symbol(name, "")?.imageWithSymbolConfiguration(&configuration)
}

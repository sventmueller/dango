//! The folder tree's outline view, as VS Code's explorer lays it out: a folder's chevron stands where a file's icon
//! does, so names line up whatever a row is, and the outline view's own disclosure triangle is gone.

use objc2::rc::Retained;
use objc2::runtime::NSObjectProtocol;
use objc2::{MainThreadMarker, MainThreadOnly, define_class, msg_send};
use objc2_app_kit::{NSColor, NSImage, NSImageSymbolConfiguration, NSOutlineView, NSUserInterfaceLayoutDirection};
use objc2_foundation::{NSRect, NSSize};

/// Where a top-level row's icon begins.
const LEADING: f64 = 12.0;
/// The size of a folder's chevron.
const CHEVRON_POINTS: f64 = 10.0;

define_class!(
    // SAFETY: NSOutlineView has no subclassing requirements and `FileOutline` has no ivars.
    #[unsafe(super(NSOutlineView))]
    #[thread_kind = MainThreadOnly]
    #[name = "IDEFileOutline"]
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
            let indent = LEADING + self.levelForRow(row).max(0) as f64 * self.indentationPerLevel();
            let (left, right) = if self.userInterfaceLayoutDirection() == NSUserInterfaceLayoutDirection::RightToLeft {
                (frame.origin.x, self.bounds().size.width - indent)
            } else {
                (indent, frame.origin.x + frame.size.width)
            };
            NSRect::new(
                objc2_foundation::NSPoint::new(left, frame.origin.y),
                NSSize::new((right - left).max(0.0), frame.size.height),
            )
        }
    }
);

impl FileOutline {
    pub fn new(mtm: MainThreadMarker, frame: NSRect) -> Retained<FileOutline> {
        // SAFETY: NSOutlineView's designated initializer, called once on the newly allocated object.
        unsafe { msg_send![Self::alloc(mtm), initWithFrame: frame] }
    }
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

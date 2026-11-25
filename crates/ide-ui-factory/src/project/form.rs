//! The one grid every section of "Software Factory für <Ordner>" sits in, so titles line up on the right and controls
//! on the left across all sections: a section's heading and note span both columns, a titled row puts its title on
//! the control's first baseline, and a wide row spans both columns.

use ide_ui_common::style::space;
use ide_ui_common::util::label;
use objc2::MainThreadMarker;
use objc2::rc::Retained;
use objc2_app_kit::{NSGridCell, NSGridCellPlacement, NSGridRowAlignment, NSGridView, NSTextField, NSView};
use objc2_foundation::{NSArray, NSRange};

use crate::ui;

/// The width of the content, both columns and the gap between them.
pub(crate) const CONTENT: f64 = 560.0;

/// The room above a section's heading.
const BEFORE_HEADING: f64 = space::GROUP;

/// The room above the first row of a group within a section.
const BEFORE_GROUP: f64 = space::XXL;

/// The grid of the sheet's sections.
pub(crate) struct Form {
    pub(crate) grid: Retained<NSGridView>,
    mtm: MainThreadMarker,
}

impl Form {
    pub(crate) fn new(mtm: MainThreadMarker) -> Form {
        let grid = NSGridView::gridViewWithNumberOfColumns_rows(2, 0, mtm);
        grid.setRowSpacing(space::L);
        grid.setColumnSpacing(space::M);
        grid.setRowAlignment(NSGridRowAlignment::FirstBaseline);
        grid.columnAtIndex(0).setXPlacement(NSGridCellPlacement::Trailing);
        grid.columnAtIndex(1).setXPlacement(NSGridCellPlacement::Leading);
        Form { grid, mtm }
    }

    fn add(&self, views: &[&NSView]) -> isize {
        self.grid.addRowWithViews(&NSArray::from_slice(views));
        self.grid.numberOfRows() - 1
    }

    /// A section's heading, with the room of a new section above it unless it is the first.
    pub(crate) fn heading(&self, words: &str, first: bool) {
        let heading = ui::heading(self.mtm, words);
        let row = self.wide(&heading);
        if !first {
            self.grid.rowAtIndex(row).setTopPadding(BEFORE_HEADING);
        }
    }

    /// The note under a heading, wrapped at the content's width.
    pub(crate) fn note(&self, words: &str) -> Retained<NSTextField> {
        let note = ui::note(self.mtm, words, CONTENT);
        self.wide(&note);
        note
    }

    /// `view` across both columns; its row.
    pub(crate) fn wide(&self, view: &NSView) -> isize {
        let row = self.add(&[view, &NSGridCell::emptyContentView(self.mtm)]);
        self.grid
            .mergeCellsInHorizontalRange_verticalRange(NSRange::new(0, 2), NSRange::new(row.unsigned_abs(), 1));
        self.grid
            .cellAtColumnIndex_rowIndex(0, row)
            .setXPlacement(NSGridCellPlacement::Leading);
        row
    }

    /// `control` with the title `words` before it; the title.
    pub(crate) fn row(&self, words: &str, control: &NSView) -> Retained<NSTextField> {
        let title = label(self.mtm, words);
        self.add(&[&title, control]);
        title
    }

    /// Room above the row added last, which starts a group within its section.
    pub(crate) fn group(&self) {
        let last = self.grid.numberOfRows() - 1;
        self.grid.rowAtIndex(last).setTopPadding(BEFORE_GROUP);
    }
}

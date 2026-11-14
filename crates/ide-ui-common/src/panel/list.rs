//! The lists of the panels beside the activity bar and the page around them: a source-list table or outline of one
//! column in a scroll view, the page's margins, and rows of a symbol and a name that VoiceOver reads as one.

use objc2::rc::Retained;
use objc2::{MainThreadMarker, MainThreadOnly};
use objc2_app_kit::{
    NSAccessibility, NSColor, NSImageView, NSLineBreakMode, NSOutlineView, NSScrollView, NSStackView,
    NSStackViewGravity, NSTableColumn, NSTableColumnResizingOptions, NSTableView, NSTableViewColumnAutoresizingStyle,
    NSTableViewStyle, NSTextField, NSUserInterfaceLayoutOrientation, NSView,
};
use objc2_foundation::{NSEdgeInsets, NSPoint, NSRect, NSSize};

use crate::style::space;
use crate::util::{label, ns, symbol};

/// The frame a panel's list starts with, before the sidebar lays it out.
pub const LIST_FRAME: NSRect = NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(240.0, 400.0));
/// How much further in a row sits than its parent, in every panel's list.
pub const INDENT: f64 = 12.0;
/// How far a top-level row built as a stack keeps from the list's edges.
pub const ROW_INSET: f64 = 4.0;
/// The margins of a panel's page around its list.
pub const PAGE_INSETS: NSEdgeInsets = NSEdgeInsets {
    top: 4.0,
    left: 8.0,
    bottom: 6.0,
    right: 8.0,
};

/// A source-list table of one column `id` without a header, named `name` for VoiceOver, in a scroll view.
pub fn source_list_table(
    mtm: MainThreadMarker,
    id: &str,
    name: &str,
) -> (Retained<NSScrollView>, Retained<NSTableView>) {
    let table = NSTableView::initWithFrame(NSTableView::alloc(mtm), LIST_FRAME);
    one_column(mtm, &table, id, name);
    (scroll(mtm, &table), table)
}

/// Makes `view`, an outline view made with [`LIST_FRAME`] or another frame, a source list of one column `id`
/// without a header, named `name` for VoiceOver, whose one column takes the width; the scroll view that shows it.
pub fn source_list_outline(
    mtm: MainThreadMarker,
    view: &NSOutlineView,
    id: &str,
    name: &str,
) -> Retained<NSScrollView> {
    let column = one_column(mtm, view, id, name);
    // SAFETY: The column was just added to this outline view, which keeps it.
    unsafe { view.setOutlineTableColumn(Some(&column)) };
    view.setIndentationPerLevel(INDENT);
    view.setAutoresizesOutlineColumn(false);
    view.setColumnAutoresizingStyle(NSTableViewColumnAutoresizingStyle::FirstColumnOnlyAutoresizingStyle);
    let scroll = scroll(mtm, view);
    view.sizeLastColumnToFit();
    scroll
}

fn one_column(mtm: MainThreadMarker, table: &NSTableView, id: &str, name: &str) -> Retained<NSTableColumn> {
    let column = NSTableColumn::initWithIdentifier(NSTableColumn::alloc(mtm), &ns(id));
    column.setResizingMask(NSTableColumnResizingOptions::AutoresizingMask);
    // The table scales its columns by their share of its width, so the one column starts as wide as the table.
    column.setWidth(table.frame().size.width);
    table.addTableColumn(&column);
    table.setHeaderView(None);
    table.setStyle(NSTableViewStyle::SourceList);
    table.setAccessibilityLabel(Some(&ns(name)));
    column
}

fn scroll(mtm: MainThreadMarker, list: &NSView) -> Retained<NSScrollView> {
    let scroll = NSScrollView::initWithFrame(NSScrollView::alloc(mtm), list.frame());
    scroll.setHasVerticalScroller(true);
    scroll.setAutohidesScrollers(true);
    scroll.setDrawsBackground(false);
    scroll.setDocumentView(Some(list));
    scroll
}

/// A panel's page: a vertical stack with the margins of [`PAGE_INSETS`].
pub fn panel_page(mtm: MainThreadMarker) -> Retained<NSStackView> {
    let page = NSStackView::new(mtm);
    page.setOrientation(NSUserInterfaceLayoutOrientation::Vertical);
    page.setEdgeInsets(PAGE_INSETS);
    page
}

/// A row of a panel's list, `level` deep: the symbol `name`, described as `description` and tinted `tint`, then
/// `text` cut at its end; VoiceOver reads the row as one element named `spoken`. Views added in the trailing gravity
/// go after the text.
pub fn icon_row(
    mtm: MainThreadMarker,
    (name, description, tint): (&str, &str, &NSColor),
    text: &str,
    level: usize,
    spoken: &str,
) -> (Retained<NSStackView>, Retained<NSTextField>) {
    let icon = NSImageView::new(mtm);
    icon.setImage(symbol(name, description).as_deref());
    icon.setContentTintColor(Some(tint));
    let text = label(mtm, text);
    text.setLineBreakMode(NSLineBreakMode::ByTruncatingTail);
    let row = NSStackView::new(mtm);
    row.setOrientation(NSUserInterfaceLayoutOrientation::Horizontal);
    row.setSpacing(space::S);
    row.setEdgeInsets(NSEdgeInsets {
        top: 0.0,
        left: row_indent(level),
        bottom: 0.0,
        right: ROW_INSET,
    });
    row.addView_inGravity(&icon, NSStackViewGravity::Leading);
    row.addView_inGravity(&text, NSStackViewGravity::Leading);
    row.setAccessibilityElement(true);
    row.setAccessibilityRole(Some(crate::constant!(objc2_app_kit::NSAccessibilityGroupRole)));
    row.setAccessibilityLabel(Some(&ns(spoken)));
    (row, text)
}

/// Where a row `level` deep begins.
#[expect(clippy::cast_precision_loss, reason = "a nesting level of a few at most")]
fn row_indent(level: usize) -> f64 {
    ROW_INSET + level as f64 * INDENT
}

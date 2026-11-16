//! The parts every settings tab is built from, so all tabs share one margin, one width, one type scale and one way of
//! lining up a title with its control: sections of a heading, a note and their content; forms of titled rows; lists.

use objc2::rc::Retained;
use objc2::runtime::ProtocolObject;
use objc2::{MainThreadMarker, MainThreadOnly, Message};
use objc2_app_kit::{
    NSAccessibility, NSColor, NSFont, NSGridCellPlacement, NSGridView, NSLayoutAttribute, NSLineBreakMode,
    NSScrollView, NSStackView, NSTableCellView, NSTableColumn, NSTableView, NSTableViewColumnAutoresizingStyle,
    NSTableViewDataSource, NSTableViewDelegate, NSTextField, NSUserInterfaceItemIdentification,
    NSUserInterfaceLayoutOrientation, NSView,
};
use objc2_foundation::{NSArray, NSPoint, NSRect, NSSize};

use crate::util::{activate, fixed_size, label, ns};

/// The room between a tab's content and the window's edges.
pub(crate) const MARGIN: f64 = 20.0;
/// The width of every tab's content, so the window keeps its width from tab to tab.
pub(crate) const WIDTH: f64 = 520.0;
/// The narrowest the column of a form's titles gets, so the forms of all tabs line up their controls.
const TITLES: f64 = 140.0;
const COLUMN_GAP: f64 = 8.0;
/// The width left for a form's controls next to its titles.
pub(crate) const CONTROLS: f64 = WIDTH - TITLES - COLUMN_GAP;
const ROW_GAP: f64 = 10.0;
const GAP: f64 = 8.0;
const AFTER_HEADING: f64 = 4.0;
const BEFORE_HEADING: f64 = 22.0;
const HEADING: &str = "settings.heading";

/// A section's heading.
pub(crate) fn heading(mtm: MainThreadMarker, text: &str) -> Retained<NSView> {
    let heading = label(mtm, text);
    heading.setFont(Some(&NSFont::boldSystemFontOfSize(NSFont::systemFontSize())));
    heading.setIdentifier(Some(&ns(HEADING)));
    heading.into_super().into_super()
}

/// The small grey text that explains a section or a control, wrapped at `width`.
pub(crate) fn note_field(mtm: MainThreadMarker, text: &str, width: f64) -> Retained<NSTextField> {
    let note = NSTextField::wrappingLabelWithString(&ns(text), mtm);
    note.setFont(Some(&ide_ui_common::style::small_system()));
    note.setTextColor(Some(&NSColor::secondaryLabelColor()));
    note.setPreferredMaxLayoutWidth(width);
    note
}

/// The note under a section's heading, as wide as the tab.
pub(crate) fn note(mtm: MainThreadMarker, text: &str) -> Retained<NSView> {
    note_field(mtm, text, WIDTH).into_super().into_super()
}

/// A value a form shows rather than lets the user change, wrapped at the controls' width.
pub(crate) fn value(mtm: MainThreadMarker, text: &str) -> Retained<NSTextField> {
    let value = NSTextField::wrappingLabelWithString(&ns(text), mtm);
    value.setPreferredMaxLayoutWidth(CONTROLS);
    value
}

/// The red line that says what went wrong in a section, hidden until there is something to say.
pub(crate) fn problem(mtm: MainThreadMarker) -> Retained<NSTextField> {
    let problem = NSTextField::wrappingLabelWithString(&ns(""), mtm);
    problem.setTextColor(Some(&NSColor::systemRedColor()));
    problem.setPreferredMaxLayoutWidth(WIDTH);
    problem.setHidden(true);
    problem
}

/// A row of a form: its title, what stands next to it, and the view in it whose first line of text the title sits
/// on. A row without a title continues the one above it.
pub(crate) struct Row {
    title: Option<Retained<NSTextField>>,
    content: Retained<NSView>,
    baseline: Retained<NSView>,
}

impl Row {
    /// `control` titled `title`, which VoiceOver also reads as the control's name when it is a control.
    pub(crate) fn new(mtm: MainThreadMarker, title: &str, control: &NSView) -> Row {
        let caption = label(mtm, title);
        if let Some(control) = control.downcast_ref::<objc2_app_kit::NSControl>()
            && control
                .downcast_ref::<NSTextField>()
                .is_none_or(NSTextField::isEditable)
        {
            crate::util::set_title_element(control, &caption);
        }
        Row {
            title: Some(caption),
            content: control.retain(),
            baseline: control.retain(),
        }
    }

    /// `content`, several views side by side, titled `title` on the line of `baseline`, one of them.
    pub(crate) fn group(mtm: MainThreadMarker, title: &str, content: &NSView, baseline: &NSView) -> Row {
        Row {
            content: content.retain(),
            ..Row::new(mtm, title, baseline)
        }
    }

    /// The row's title, if it has one.
    pub(crate) fn caption(&self) -> Option<&NSTextField> {
        self.title.as_deref()
    }

    /// `view` in the controls' column, under the row before it.
    pub(crate) fn under(view: &NSView) -> Row {
        Row {
            title: None,
            content: view.retain(),
            baseline: view.retain(),
        }
    }
}

/// Views side by side, for a row of a form.
pub(crate) fn line(mtm: MainThreadMarker, views: &[&NSView]) -> Retained<NSView> {
    let line = NSStackView::new(mtm);
    line.setOrientation(NSUserInterfaceLayoutOrientation::Horizontal);
    line.setAlignment(NSLayoutAttribute::FirstBaseline);
    line.setSpacing(GAP);
    for view in views {
        line.addArrangedSubview(view);
    }
    line.into_super()
}

/// The rows as a form: titles right-aligned in a column of their own, each on the first line of text of its control.
pub(crate) fn grid(mtm: MainThreadMarker, rows: &[Row]) -> Retained<NSGridView> {
    let grid = NSGridView::new(mtm);
    fill(&grid, rows);
    grid
}

/// Lays `rows` out in `grid`, an empty one, as [`grid`] does.
pub(crate) fn fill(grid: &NSGridView, rows: &[Row]) {
    let mtm = grid.mtm();
    for row in rows {
        let title = row
            .title
            .clone()
            .unwrap_or_else(|| label(mtm, ""))
            .into_super()
            .into_super();
        grid.addRowWithViews(&NSArray::from_retained_slice(&[title, row.content.clone()]));
    }
    grid.setRowSpacing(ROW_GAP);
    grid.setColumnSpacing(COLUMN_GAP);
    grid.setYPlacement(NSGridCellPlacement::Top);
    let titles = grid.columnAtIndex(0);
    titles.setXPlacement(NSGridCellPlacement::Trailing);
    for (n, row) in rows.iter().enumerate() {
        let Some(title) = &row.title else { continue };
        let index = isize::try_from(n).unwrap_or(0);
        // Placed by the grid, a title would sit at the row's top, above the text of a taller control.
        grid.cellAtColumnIndex_rowIndex(0, index)
            .setYPlacement(NSGridCellPlacement::None);
        title
            .firstBaselineAnchor()
            .constraintEqualToAnchor(&row.baseline.firstBaselineAnchor())
            .setActive(true);
    }
    if let Some(first) = rows.first() {
        first
            .content
            .leadingAnchor()
            .constraintGreaterThanOrEqualToAnchor_constant(&grid.leadingAnchor(), TITLES + COLUMN_GAP)
            .setActive(true);
    }
}

/// Shows or hides `view` and, when it stands in a form, the row it stands in; in a [`stack`], the room before the
/// next heading stays the same.
pub(crate) fn show(view: &NSView, shown: bool) {
    view.setHidden(!shown);
    // SAFETY: Asking a view for its superview on the main thread, where views live.
    let parent = unsafe { view.superview() };
    let Some(parent) = parent else { return };
    if let Some(grid) = parent.downcast_ref::<NSGridView>() {
        let cell = grid.cellForView(view);
        if let Some(row) = cell.and_then(|cell| cell.row()) {
            row.setHidden(!shown);
        }
    } else if let Some(stack) = parent.downcast_ref::<NSStackView>() {
        space(stack);
    }
}

/// The views one under another as a tab's sections: close together, with room before each heading.
pub(crate) fn stack(mtm: MainThreadMarker, views: &[Retained<NSView>]) -> Retained<NSStackView> {
    let stack = NSStackView::new(mtm);
    stack.setOrientation(NSUserInterfaceLayoutOrientation::Vertical);
    stack.setAlignment(NSLayoutAttribute::Leading);
    stack.setSpacing(GAP);
    for view in views {
        stack.addArrangedSubview(view);
    }
    space(&stack);
    stack
}

/// Sets the room between the shown views of `stack`, whose hidden ones take no room and keep none after them.
fn space(stack: &NSStackView) {
    let shown: Vec<Retained<NSView>> = stack
        .arrangedSubviews()
        .iter()
        .filter(|view| !view.isHidden())
        .collect();
    for (before, view) in shown.iter().zip(shown.iter().skip(1)) {
        let room = if is_heading(before) {
            AFTER_HEADING
        } else if is_heading(view) {
            BEFORE_HEADING
        } else {
            GAP
        };
        stack.setCustomSpacing_afterView(room, before);
    }
    super::list_bar::attach(stack);
}

fn is_heading(view: &NSView) -> bool {
    view.identifier().is_some_and(|id| id.to_string() == HEADING)
}

/// `content` with the tab's margins around it, as wide as every tab.
pub(crate) fn page(mtm: MainThreadMarker, content: &NSView) -> Retained<NSView> {
    let view = NSView::new(mtm);
    view.addSubview(content);
    content.setTranslatesAutoresizingMaskIntoConstraints(false);
    activate([
        content
            .leadingAnchor()
            .constraintEqualToAnchor_constant(&view.leadingAnchor(), MARGIN),
        content
            .trailingAnchor()
            .constraintLessThanOrEqualToAnchor_constant(&view.trailingAnchor(), -MARGIN),
        content
            .topAnchor()
            .constraintEqualToAnchor_constant(&view.topAnchor(), MARGIN),
        content
            .bottomAnchor()
            .constraintEqualToAnchor_constant(&view.bottomAnchor(), -MARGIN),
        view.widthAnchor()
            .constraintGreaterThanOrEqualToConstant(WIDTH + 2.0 * MARGIN),
    ]);
    view
}

/// `content`, a stack of [`stack`], in a page `height` tall that scrolls, for a tab taller than a screen leaves room
/// for; the stack is the scroll view's document, and a stack view draws from its top.
pub(crate) fn scrolling_page(mtm: MainThreadMarker, content: &NSStackView, height: f64) -> Retained<NSView> {
    content.setEdgeInsets(objc2_foundation::NSEdgeInsets {
        top: MARGIN,
        left: MARGIN,
        bottom: MARGIN,
        right: MARGIN,
    });
    let width = WIDTH + 2.0 * MARGIN;
    let frame = NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(width, height));
    let scroll = NSScrollView::initWithFrame(NSScrollView::alloc(mtm), frame);
    scroll.setHasVerticalScroller(true);
    scroll.setAutohidesScrollers(true);
    scroll.setDrawsBackground(false);
    content.setTranslatesAutoresizingMaskIntoConstraints(false);
    scroll.setDocumentView(Some(content));
    let clip = scroll.contentView();
    activate([
        content.topAnchor().constraintEqualToAnchor(&clip.topAnchor()),
        content.leadingAnchor().constraintEqualToAnchor(&clip.leadingAnchor()),
        content.widthAnchor().constraintEqualToAnchor(&clip.widthAnchor()),
    ]);
    fixed_size(&scroll, width, height);
    scroll.into_super()
}

/// A list `height` tall with the columns `columns`, titles and widths, the last one taking the room left; `owner`
/// gives its rows.
pub(crate) fn table<T>(
    mtm: MainThreadMarker,
    owner: &T,
    columns: &[(&str, f64)],
    height: f64,
) -> (Retained<NSTableView>, Retained<NSView>)
where
    T: NSTableViewDataSource + NSTableViewDelegate + Message,
{
    let frame = NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(WIDTH, height));
    let table = NSTableView::initWithFrame(NSTableView::alloc(mtm), frame);
    table.setColumnAutoresizingStyle(NSTableViewColumnAutoresizingStyle::LastColumnOnlyAutoresizingStyle);
    for (n, (title, width)) in columns.iter().enumerate() {
        let column = NSTableColumn::initWithIdentifier(NSTableColumn::alloc(mtm), &ns(&n.to_string()));
        column.setTitle(&ns(title));
        column.setWidth(*width);
        table.addTableColumn(&column);
    }
    table.setUsesAlternatingRowBackgroundColors(true);
    table.setAllowsEmptySelection(true);
    table.setAccessibilityLabel(columns.first().map(|(title, _)| ns(title)).as_deref());
    // SAFETY: The table holds its data source and delegate weakly; the settings window keeps `owner` alive.
    unsafe {
        table.setDataSource(Some(ProtocolObject::from_ref(owner)));
        table.setDelegate(Some(ProtocolObject::from_ref(owner)));
    }
    let scroll = NSScrollView::initWithFrame(NSScrollView::alloc(mtm), frame);
    scroll.setHasVerticalScroller(true);
    scroll.setAutohidesScrollers(true);
    scroll.setBorderType(objc2_app_kit::NSBorderType::BezelBorder);
    scroll.setDocumentView(Some(&table));
    table.sizeLastColumnToFit();
    fixed_size(&scroll, WIDTH, height);
    (table, scroll.into_super())
}

/// A list's cell showing `text` in the middle of its row's height, cut in the middle when it does not fit.
pub(crate) fn cell(mtm: MainThreadMarker, text: &str) -> Retained<NSView> {
    let cell = NSTableCellView::new(mtm);
    let field = label(mtm, text);
    field.setLineBreakMode(NSLineBreakMode::ByTruncatingMiddle);
    field.setToolTip(Some(&ns(text)));
    field.setContentCompressionResistancePriority_forOrientation(
        250.0,
        objc2_app_kit::NSLayoutConstraintOrientation::Horizontal,
    );
    cell.addSubview(&field);
    // SAFETY: The label is a subview of the cell, which keeps it.
    unsafe { cell.setTextField(Some(&field)) };
    field.setTranslatesAutoresizingMaskIntoConstraints(false);
    activate([
        field
            .leadingAnchor()
            .constraintEqualToAnchor_constant(&cell.leadingAnchor(), 2.0),
        field
            .trailingAnchor()
            .constraintLessThanOrEqualToAnchor_constant(&cell.trailingAnchor(), -2.0),
        field.centerYAnchor().constraintEqualToAnchor(&cell.centerYAnchor()),
    ]);
    cell.into_super()
}

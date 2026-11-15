//! What the self-test reads from and does with a search panel, through the code its controls run.

use objc2::DefinedClass;

use super::model::Spot;
use super::panel::spot_of;
use super::{SearchPanel, Toggle};
use crate::util::ns;

/// What the panel shows.
pub(crate) struct Seen {
    /// Whether the matches shown are those of the field and options as they are now, found to the end.
    pub(crate) settled: bool,
    /// The files with their match counts, in the order shown.
    pub(crate) files: Vec<(String, usize)>,
    /// The rows of the outline.
    pub(crate) rows: isize,
    pub(crate) status: String,
    pub(crate) results_shown: bool,
    pub(crate) field_focused: bool,
    /// Whether "Alle ersetzen" can be pressed.
    pub(crate) can_replace_all: bool,
}

pub(crate) fn seen(panel: &SearchPanel) -> Seen {
    let options = panel.options();
    let model = panel.read();
    let views = panel.ivars().views.get();
    let focused = views.is_some_and(|views| {
        let editor = views.page.window().and_then(|w| w.firstResponder());
        let editor = editor.and_then(|r| r.downcast::<objc2_app_kit::NSTextView>().ok());
        let owner = editor.and_then(|e| e.delegate());
        owner.is_some_and(|owner| {
            objc2::rc::Retained::as_ptr(&owner).cast::<()>() == objc2::rc::Retained::as_ptr(&views.field).cast()
        })
    });
    Seen {
        settled: model.shows(&options) && !model.running(),
        files: model.groups().iter().map(|g| (g.relative.clone(), g.matches)).collect(),
        rows: views.map_or(0, |v| v.outline.numberOfRows()),
        status: views.map(|v| v.status.stringValue().to_string()).unwrap_or_default(),
        results_shown: views.is_some_and(|v| v.results_shown()),
        field_focused: focused,
        can_replace_all: views.is_some_and(|v| v.replace_all.isEnabled()),
    }
}

/// The distance from one result row to the next.
pub(crate) fn row_pitch(panel: &SearchPanel) -> Option<f64> {
    let outline = &panel.ivars().views.get()?.outline;
    (outline.numberOfRows() > 1).then(|| outline.rectOfRow(1).origin.y - outline.rectOfRow(0).origin.y)
}

/// The chevron that opens the replace field.
pub(crate) fn replace_chevron(panel: &SearchPanel) -> Option<objc2::rc::Retained<objc2_app_kit::NSButton>> {
    panel.ivars().views.get().map(|views| views.replace_toggle.clone())
}

/// Opens the replace field, as its chevron does, and types `with` into it.
pub(crate) fn replace_with(panel: &SearchPanel, with: &str) {
    if let Some(views) = panel.ivars().views.get() {
        if !views.replace_shown() {
            views.set_replace_shown(true);
        }
        views.replace.setStringValue(&ns(with));
    }
    panel.show();
}

/// The row of the file `relative`, or of its match on line `line`.
fn row(panel: &SearchPanel, relative: &str, line: Option<u64>) -> Option<isize> {
    let views = panel.ivars().views.get()?;
    let model = panel.read();
    (0..views.outline.numberOfRows()).find(|row| {
        let spot = views.outline.itemAtRow(*row).and_then(|i| spot_of(&i));
        match (spot, line) {
            (Some(Spot::File(path)), None) => model.group(&path).is_some_and(|g| g.relative == relative),
            (Some(Spot::Line(path, index)), Some(line)) => model.group(&path).is_some_and(|group| {
                group.relative == relative && group.lines.get(index).is_some_and(|l| l.line == line)
            }),
            _ => false,
        }
    })
}

/// Clicks the replace button of the row of `relative`, or of its line `line`, which shows while the pointer is on
/// the row; whether the button was there and hidden before.
pub(crate) fn click_replace(panel: &SearchPanel, relative: &str, line: Option<u64>) -> bool {
    let Some(views) = panel.ivars().views.get() else {
        return false;
    };
    let Some(row) = row(panel, relative, line) else {
        return false;
    };
    let view = views.outline.viewAtColumn_row_makeIfNecessary(0, row, true);
    let Some(button) = view.as_deref().and_then(super::rows::button_of) else {
        return false;
    };
    let hidden = button.isHidden();
    button.setHidden(false);
    // SAFETY: A click with no sender, which the panel's action takes.
    unsafe { button.performClick(None) };
    hidden
}

/// Presses "Alle ersetzen", which asks first in a sheet.
pub(crate) fn replace_all(panel: &SearchPanel) {
    panel.ask_replace_all();
}

/// Types `query` into the field as the keyboard does.
pub(crate) fn type_query(panel: &SearchPanel, query: &str) {
    if let Some(views) = panel.ivars().views.get() {
        views.field.setStringValue(&ns(query));
    }
    panel.schedule();
}

/// Switches `toggle` as a click on its button does.
pub(crate) fn set(panel: &SearchPanel, toggle: Toggle, on: bool) {
    if let Some(views) = panel.ivars().views.get() {
        views.set_toggled(toggle, on);
    }
    panel.schedule();
}

/// Opens the file filters and types `include` and `exclude` into them.
pub(crate) fn set_globs(panel: &SearchPanel, include: &str, exclude: &str) {
    if let Some(views) = panel.ivars().views.get() {
        views.set_globs_shown(true);
        views.include.setStringValue(&ns(include));
        views.exclude.setStringValue(&ns(exclude));
    }
    panel.schedule();
}

/// Opens the match on line `line` of the file `relative` as Return on its row does.
pub(crate) fn open(panel: &SearchPanel, relative: &str, line: u64) -> bool {
    let Some(views) = panel.ivars().views.get() else {
        return false;
    };
    let row = (0..views.outline.numberOfRows()).find(|row| {
        let Some(Spot::Line(path, index)) = views.outline.itemAtRow(*row).and_then(|i| spot_of(&i)) else {
            return false;
        };
        panel
            .read()
            .group(&path)
            .is_some_and(|group| group.relative == relative && group.lines.get(index).is_some_and(|l| l.line == line))
    });
    let Some(row) = row else { return false };
    views.outline.selectRowIndexes_byExtendingSelection(
        &objc2_foundation::NSIndexSet::indexSetWithIndex(row.unsigned_abs()),
        false,
    );
    panel.open_row(row, true);
    true
}

/// Presses Esc in the field, as the field editor reports it.
pub(crate) fn escape(panel: &SearchPanel) {
    panel.focus();
    if let Some(views) = panel.ivars().views.get() {
        let field: &objc2_app_kit::NSControl = &views.field;
        panel.field_command(field, objc2::sel!(cancelOperation:));
    }
}

/// The rows of the hierarchy on show, each with its level and name; empty while the text search shows.
pub(crate) fn hierarchy_rows(panel: &SearchPanel) -> Vec<(isize, String)> {
    match panel.ivars().hierarchy.get() {
        Some(h) if h.showing() => super::hierarchy::selftest::rows(h),
        _ => vec![],
    }
}

/// Opens row `row` of the hierarchy as its disclosure triangle does.
pub(crate) fn hierarchy_expand(panel: &SearchPanel, row: isize) {
    if let Some(h) = panel.ivars().hierarchy.get() {
        super::hierarchy::selftest::expand(h, row);
    }
}

/// Clicks row `row` of the hierarchy.
pub(crate) fn hierarchy_click(panel: &SearchPanel, row: isize) {
    if let Some(h) = panel.ivars().hierarchy.get() {
        super::hierarchy::selftest::click(h, row);
    }
}

/// Clicks segment `segment` of the hierarchy's direction switch.
pub(crate) fn hierarchy_direction(panel: &SearchPanel, segment: isize) {
    if let Some(h) = panel.ivars().hierarchy.get() {
        super::hierarchy::selftest::choose_direction(h, segment);
    }
}

/// The titles of the hierarchy's menu of earlier searches.
pub(crate) fn hierarchy_history(panel: &SearchPanel) -> Vec<String> {
    panel
        .ivars()
        .hierarchy
        .get()
        .map(|h| super::hierarchy::selftest::history(h))
        .unwrap_or_default()
}

/// Chooses item `index` of the hierarchy's menu of earlier searches; 0 is the text search.
pub(crate) fn hierarchy_choose(panel: &SearchPanel, index: isize) {
    if let Some(h) = panel.ivars().hierarchy.get() {
        super::hierarchy::selftest::choose_history(h, index);
    }
}

/// What VoiceOver reads for the row `row`.
pub(crate) fn row_label(panel: &SearchPanel, row: isize) -> Option<String> {
    let views = panel.ivars().views.get()?;
    let view = views.outline.viewAtColumn_row_makeIfNecessary(0, row, true)?;
    super::rows::spoken(&view)
}

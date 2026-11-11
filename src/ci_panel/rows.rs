//! The rows of the CI list: runs with their number and age, jobs with how long they took, steps; the context menu
//! and what a row's action acts on.

use ide_ci::{Job, Run, State};
use objc2::rc::Retained;
use objc2::runtime::{AnyObject, Sel};
use objc2::{MainThreadMarker, MainThreadOnly, define_class, msg_send, sel};
use objc2_app_kit::{
    NSAccessibility, NSColor, NSEvent, NSFont, NSImageSymbolConfiguration, NSImageView, NSLayoutAttribute,
    NSLineBreakMode, NSMenu, NSMenuItem, NSOutlineView, NSStackView, NSTableCellView, NSUserInterfaceLayoutOrientation,
    NSView,
};
use objc2_foundation::{NSObjectProtocol, NSRect, NSString};
use splat_i18n::{t, tf};

use super::panel::CiPanel;
use super::text;
use crate::util::{item, label, ns, separator, set_target, symbol};

/// Return and the keypad's Enter.
const RETURN_KEYS: [u16; 2] = [36, 76];

const RUN_HEIGHT: f64 = 36.0;
const JOB_HEIGHT: f64 = 24.0;
const STEP_HEIGHT: f64 = 22.0;

/// How far AppKit's source list sets its disclosure triangles in past the panel's title.
const OUTDENT: f64 = 6.0;

define_class!(
    // SAFETY: NSOutlineView has no subclassing requirements and `CiOutlineView` does not implement Drop.
    /// Return opens a job's log or folds a run open, as a double click does.
    #[unsafe(super(NSOutlineView))]
    #[thread_kind = MainThreadOnly]
    #[name = "IDECiOutlineView"]
    pub struct CiOutlineView;

    unsafe impl NSObjectProtocol for CiOutlineView {}

    impl CiOutlineView {
        /// The disclosure triangle where the panel's title begins, not in the source list's margin.
        #[unsafe(method(frameOfOutlineCellAtRow:))]
        fn outline_cell_frame(&self, row: isize) -> NSRect {
            // SAFETY: The superclass's implementation, with this method's signature.
            let frame: NSRect = unsafe { msg_send![super(self), frameOfOutlineCellAtRow: row] };
            crate::window::file_outline::outdent(self, frame, OUTDENT, false)
        }

        /// The rows moved along with the triangle.
        #[unsafe(method(frameOfCellAtColumn:row:))]
        fn cell_frame(&self, column: isize, row: isize) -> NSRect {
            // SAFETY: The superclass's implementation, with this method's signature.
            let frame: NSRect = unsafe { msg_send![super(self), frameOfCellAtColumn: column, row: row] };
            if column != 0 {
                return frame;
            }
            crate::window::file_outline::outdent(self, frame, OUTDENT, true)
        }

        #[unsafe(method(keyDown:))]
        fn key_down(&self, event: &NSEvent) {
            let handled = crate::util::guarded(false, || {
                let Some(panel) = self.panel().filter(|_| RETURN_KEYS.contains(&event.keyCode())) else {
                    return false;
                };
                panel.activate_row(self.selectedRow());
                true
            });
            if !handled {
                // SAFETY: The superclass's implementation, with this method's signature.
                let _: () = unsafe { msg_send![super(self), keyDown: event] };
            }
        }
    }
);

impl CiOutlineView {
    pub fn new(mtm: MainThreadMarker, frame: NSRect) -> Retained<CiOutlineView> {
        // SAFETY: NSOutlineView's designated initializer, called once on the newly allocated object.
        unsafe { msg_send![CiOutlineView::alloc(mtm), initWithFrame: frame] }
    }

    fn panel(&self) -> Option<Retained<CiPanel>> {
        // SAFETY: Only `ui::outline` makes this view, and it sets the panel as the data source.
        let source = unsafe { self.dataSource() }?;
        let object: Retained<AnyObject> = source.into();
        object.downcast::<CiPanel>().ok()
    }
}

/// Names a row across reads.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Key {
    Run(u64),
    Job(u64, u64),
    Step(u64, u64, u32),
}

impl Key {
    pub fn encode(&self) -> String {
        match self {
            Key::Run(run) => format!("run:{run}"),
            Key::Job(run, job) => format!("job:{run}:{job}"),
            Key::Step(run, job, step) => format!("step:{run}:{job}:{step}"),
        }
    }

    fn decode(text: &str) -> Option<Key> {
        let mut parts = text.split(':');
        let kind = parts.next()?;
        let numbers: Vec<u64> = parts.map(|p| p.parse().ok()).collect::<Option<_>>()?;
        match (kind, numbers.as_slice()) {
            ("run", [run]) => Some(Key::Run(*run)),
            ("job", [run, job]) => Some(Key::Job(*run, *job)),
            ("step", [run, job, step]) => Some(Key::Step(*run, *job, u32::try_from(*step).ok()?)),
            _ => None,
        }
    }

    /// The key `item` of the outline view stands for.
    pub fn of(item: &AnyObject) -> Option<Key> {
        item.downcast_ref::<NSString>()
            .and_then(|text| Key::decode(&text.to_string()))
    }
}

pub fn height(key: Option<&Key>) -> f64 {
    match key {
        Some(Key::Run(_)) => RUN_HEIGHT,
        Some(Key::Step(..)) => STEP_HEIGHT,
        _ => JOB_HEIGHT,
    }
}

impl CiPanel {
    pub fn find_run(&self, id: u64) -> Option<Run> {
        self.read().runs.iter().find(|r| r.run.id == id).map(|r| r.run.clone())
    }

    pub fn find_job(&self, run: u64, job: u64) -> Option<Job> {
        let state = self.read();
        let row = state.runs.iter().find(|r| r.run.id == run)?;
        row.jobs.iter().find(|j| j.id == job).cloned()
    }

    pub fn row_view(&self, key: &Key) -> Option<Retained<NSView>> {
        let mtm = self.mtm();
        match *key {
            Key::Run(id) => {
                let run = self.find_run(id)?;
                let detail = [run.event.clone(), run.updated.or(run.created).map(text::ago)]
                    .into_iter()
                    .flatten()
                    .collect::<Vec<_>>()
                    .join(" · ");
                let title = format!("{} #{}", run.name, run.number);
                Some(cell(mtm, run.state, &title, Some(&detail), None))
            }
            Key::Job(run, id) => {
                let job = self.find_job(run, id)?;
                let took = job.seconds(ide_ci::now()).map(text::took);
                let name = match &job.stage {
                    Some(stage) if *stage != job.name => format!("{stage} › {}", job.name),
                    _ => job.name.clone(),
                };
                Some(cell(mtm, job.state, &name, None, took.as_deref()))
            }
            Key::Step(run, job, number) => {
                let step = self
                    .find_job(run, job)?
                    .steps
                    .into_iter()
                    .find(|s| s.number == number)?;
                Some(cell(
                    mtm,
                    step.state,
                    &format!("{}  {}", step.number, step.name),
                    None,
                    None,
                ))
            }
        }
    }

    /// The keys of the selected rows.
    pub fn selected_keys(&self) -> Vec<Key> {
        let outline = &self.ui().outline;
        (0..outline.numberOfRows())
            .filter(|row| outline.isRowSelected(*row))
            .filter_map(|row| outline.itemAtRow(row))
            .filter_map(|item| Key::of(&item))
            .collect()
    }

    /// What an action acts on: the context menu's row, else the selection.
    fn keys_for(&self, sender: Option<&AnyObject>) -> Vec<Key> {
        if sender.is_some_and(|s| s.downcast_ref::<NSMenuItem>().is_some()) {
            return self.read().menu_keys.clone();
        }
        self.selected_keys()
    }

    /// The run an action means: the one of the row it came from, else the first run in `state` or still running.
    pub fn run_for(&self, sender: Option<&AnyObject>, state: State) -> Option<Run> {
        let picked = self.keys_for(sender).into_iter().find_map(|key| match key {
            Key::Run(run) | Key::Job(run, _) | Key::Step(run, _, _) => self.find_run(run),
        });
        picked.or_else(|| {
            self.read()
                .runs
                .iter()
                .map(|r| &r.run)
                .find(|r| {
                    if state == State::Failed {
                        r.state == State::Failed
                    } else {
                        r.state.is_active()
                    }
                })
                .cloned()
        })
    }

    /// The job an action means: the one of the row it came from.
    pub fn job_for(&self, sender: Option<&AnyObject>) -> Option<Job> {
        self.keys_for(sender).into_iter().find_map(|key| match key {
            Key::Job(run, job) | Key::Step(run, job, _) => self.find_job(run, job),
            Key::Run(_) => None,
        })
    }

    /// Return or a double click: a job or step opens its log, a run opens or closes.
    pub fn activate_row(&self, row: isize) {
        let outline = &self.ui().outline;
        let Some(item) = (row >= 0).then(|| outline.itemAtRow(row)).flatten() else {
            return;
        };
        match Key::of(&item) {
            Some(Key::Job(run, job) | Key::Step(run, job, _)) => self.open_log(self.find_job(run, job)),
            Some(Key::Run(_)) => {
                // SAFETY: The item came from this outline view, which knows it.
                unsafe {
                    if outline.isItemExpanded(Some(&item)) {
                        outline.collapseItem(Some(&item));
                    } else {
                        outline.expandItem(Some(&item));
                    }
                }
            }
            None => {}
        }
    }

    /// Reloads the list, keeping the selection; runs show their jobs.
    pub fn reload_outline(&self) {
        let outline = &self.ui().outline;
        let selected = self.selected_keys();
        outline.reloadData();
        let runs: Vec<Key> = self.read().runs.iter().map(|r| Key::Run(r.run.id)).collect();
        for key in runs {
            // SAFETY: The item is one the data source hands to this view, kept alive by the panel.
            unsafe { outline.expandItem(Some(&self.item_for(&key))) };
        }
        let rows = objc2_foundation::NSMutableIndexSet::new();
        for key in &selected {
            // SAFETY: The item is one the data source hands to this view, kept alive by the panel.
            let row = unsafe { outline.rowForItem(Some(&self.item_for(key))) };
            if let Ok(row) = usize::try_from(row) {
                rows.addIndex(row);
            }
        }
        outline.selectRowIndexes_byExtendingSelection(&rows, false);
        self.ui().show_empty_list(self.read().runs.is_empty());
    }

    /// The context menu of the clicked row.
    pub fn fill_menu(&self, menu: &NSMenu) {
        menu.removeAllItems();
        menu.setAutoenablesItems(false);
        let outline = &self.ui().outline;
        let clicked = outline.clickedRow();
        let key = (clicked >= 0)
            .then(|| outline.itemAtRow(clicked))
            .flatten()
            .and_then(|item| Key::of(&item));
        let Some(key) = key else { return };
        self.state().menu_keys = vec![key];
        let mtm = self.mtm();
        for (title, action, enabled) in self.menu_entries(key) {
            let Some(action) = action else {
                menu.addItem(&separator(mtm));
                continue;
            };
            let entry = item(mtm, title, Some(action), "");
            set_target(&entry, self);
            entry.setEnabled(enabled);
            menu.addItem(&entry);
        }
    }

    fn menu_entries(&self, key: Key) -> Vec<(&'static str, Option<Sel>, bool)> {
        let busy = self.read().busy;
        match key {
            Key::Run(id) => {
                let state = self.find_run(id).map_or(State::Neutral, |r| r.state);
                vec![
                    (
                        t("Fehlgeschlagene Jobs neu starten"),
                        Some(sel!(ideCiRerun:)),
                        state == State::Failed && !busy,
                    ),
                    (
                        t("Lauf abbrechen …"),
                        Some(sel!(ideCiCancel:)),
                        state.is_active() && !busy,
                    ),
                    ("", None, false),
                    (t("Im Browser öffnen"), Some(sel!(ideCiOpenWeb:)), true),
                ]
            }
            Key::Job(run, job) | Key::Step(run, job, _) => {
                let failed = self.find_job(run, job).is_some_and(|j| j.state == State::Failed);
                vec![
                    (t("Log öffnen"), Some(sel!(ideCiOpenLog:)), true),
                    (t("An Agent senden"), Some(sel!(ideCiSendToAgent:)), failed),
                    ("", None, false),
                    (t("Im Browser öffnen"), Some(sel!(ideCiOpenWeb:)), true),
                ]
            }
        }
    }
}

/// A row: the state's symbol in its colour, the title, maybe a second line, and maybe a value at the end.
fn cell(
    mtm: MainThreadMarker,
    state: State,
    title: &str,
    detail: Option<&str>,
    value: Option<&str>,
) -> Retained<NSView> {
    let cell = NSTableCellView::new(mtm);
    let icon = NSImageView::new(mtm);
    if let Some(image) = symbol(text::symbol(state), text::word(state)) {
        icon.setImage(Some(&image));
    }
    icon.setSymbolConfiguration(Some(&NSImageSymbolConfiguration::configurationWithPointSize_weight(
        12.0,
        // SAFETY: AppKit sets its font weight constants when it loads.
        unsafe { objc2_app_kit::NSFontWeightMedium },
    )));
    icon.setContentTintColor(Some(&text::color(state)));
    let name = label(mtm, title);
    name.setLineBreakMode(NSLineBreakMode::ByTruncatingTail);
    let lines = NSStackView::new(mtm);
    lines.setOrientation(NSUserInterfaceLayoutOrientation::Vertical);
    lines.setAlignment(NSLayoutAttribute::Leading);
    lines.setSpacing(1.0);
    lines.addArrangedSubview(&name);
    if let Some(detail) = detail {
        let second = label(mtm, detail);
        second.setFont(Some(&NSFont::systemFontOfSize(NSFont::smallSystemFontSize())));
        second.setTextColor(Some(&NSColor::secondaryLabelColor()));
        second.setLineBreakMode(NSLineBreakMode::ByTruncatingTail);
        lines.addArrangedSubview(&second);
    }
    let row = NSStackView::new(mtm);
    row.setOrientation(NSUserInterfaceLayoutOrientation::Horizontal);
    row.setSpacing(6.0);
    row.addArrangedSubview(&icon);
    row.addArrangedSubview(&lines);
    cell.addSubview(&row);
    row.setTranslatesAutoresizingMaskIntoConstraints(false);
    row.leadingAnchor()
        .constraintEqualToAnchor(&cell.leadingAnchor())
        .setActive(true);
    row.centerYAnchor()
        .constraintEqualToAnchor(&cell.centerYAnchor())
        .setActive(true);
    let mut spoken = tf("{}: {}", &[title, text::word(state)]);
    if let Some(value) = value {
        let end = label(mtm, value);
        end.setFont(Some(&NSFont::monospacedDigitSystemFontOfSize_weight(
            NSFont::smallSystemFontSize(),
            // SAFETY: AppKit sets its font weight constants when it loads.
            unsafe { objc2_app_kit::NSFontWeightRegular },
        )));
        end.setTextColor(Some(&NSColor::secondaryLabelColor()));
        cell.addSubview(&end);
        end.setTranslatesAutoresizingMaskIntoConstraints(false);
        end.trailingAnchor()
            .constraintEqualToAnchor_constant(&cell.trailingAnchor(), -6.0)
            .setActive(true);
        end.centerYAnchor()
            .constraintEqualToAnchor(&cell.centerYAnchor())
            .setActive(true);
        row.trailingAnchor()
            .constraintLessThanOrEqualToAnchor_constant(&end.leadingAnchor(), -6.0)
            .setActive(true);
        spoken = tf("{}, {}", &[&spoken, value]);
    } else {
        row.trailingAnchor()
            .constraintLessThanOrEqualToAnchor_constant(&cell.trailingAnchor(), -6.0)
            .setActive(true);
    }
    if let Some(detail) = detail {
        spoken = tf("{}, {}", &[&spoken, detail]);
    }
    cell.setAccessibilityLabel(Some(&ns(&spoken)));
    cell.setToolTip(Some(&ns(&spoken)));
    // SAFETY: The label is a subview of the cell, which keeps it.
    unsafe { cell.setTextField(Some(&name)) };
    cell.into_super()
}

//! The rows of the CI list: runs with their number and age, GitLab's stages, jobs with how long they took, steps;
//! which of them stand open, the context menu and what a row's action acts on.

use ide_ci::{Item, Job, Kind, Run, State};
use objc2::rc::Retained;
use objc2::runtime::{AnyObject, Sel};
use objc2::{DefinedClass, MainThreadMarker, MainThreadOnly, define_class, msg_send, sel};
use objc2_app_kit::{
    NSAccessibility, NSColor, NSEvent, NSFont, NSImageSymbolConfiguration, NSImageView, NSLayoutAttribute,
    NSLayoutConstraintOrientation, NSLineBreakMode, NSMenu, NSMenuItem, NSOutlineView, NSStackView, NSTableCellView,
    NSUserInterfaceLayoutOrientation, NSView,
};
use objc2_foundation::{NSNotification, NSObjectProtocol, NSRect, NSString};
use splat_i18n::{t, tf};

use super::model::{Key, Tab};
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

impl Key {
    /// The key `item` of the outline view stands for.
    pub fn of(item: &AnyObject) -> Option<Key> {
        item.downcast_ref::<NSString>()
            .and_then(|text| Key::decode(&text.to_string()))
    }
}

pub fn height(key: Option<&Key>) -> f64 {
    match key {
        Some(Key::Run(_) | Key::Item(_)) => RUN_HEIGHT,
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

    pub fn find_item(&self, number: u64) -> Option<Item> {
        self.read().items.iter().find(|i| i.number == number).cloned()
    }

    /// Whether the panel talks to GitHub.
    pub fn is_github(&self) -> bool {
        self.read().forge.as_ref().is_some_and(|f| f.kind == Kind::GitHub)
    }

    /// The name of the run's stage `index` and how far its jobs got.
    pub fn find_stage(&self, run: u64, index: u32) -> Option<(String, State)> {
        let state = self.read();
        let row = state.runs.iter().find(|r| r.run.id == run)?;
        let stages = ide_ci::stages(&row.jobs);
        let stage = stages.get(usize::try_from(index).ok()?)?;
        Some((stage.name.to_string(), stage.state()))
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
            Key::Stage(run, index) => {
                let (name, state) = self.find_stage(run, index)?;
                Some(cell(mtm, state, &name, None, None))
            }
            Key::Job(run, id) => {
                let job = self.find_job(run, id)?;
                let took = job.seconds(ide_ci::now()).map(text::took);
                Some(cell(mtm, job.state, &job.name, None, took.as_deref()))
            }
            Key::Item(number) => {
                let item = self.find_item(number)?;
                let changes = self.read().tab == Tab::Changes;
                Some(item_cell(mtm, &item, changes, self.is_github()))
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
            Key::Run(run) | Key::Stage(run, _) | Key::Job(run, _) | Key::Step(run, _, _) => self.find_run(run),
            Key::Item(_) => None,
        });
        picked.or_else(|| {
            self.read()
                .runs
                .iter()
                .map(|r| &r.run)
                .find(|r| match state {
                    State::Failed => r.state == State::Failed,
                    State::Running => r.state.is_active(),
                    _ => !r.state.is_active(),
                })
                .cloned()
        })
    }

    /// The job an action means: the one of the row it came from.
    pub fn job_for(&self, sender: Option<&AnyObject>) -> Option<Job> {
        self.keys_for(sender).into_iter().find_map(|key| match key {
            Key::Job(run, job) | Key::Step(run, job, _) => self.find_job(run, job),
            Key::Run(_) | Key::Stage(..) | Key::Item(_) => None,
        })
    }

    /// The merge request or issue an action means: the one of the row it came from.
    pub fn item_for_action(&self, sender: Option<&AnyObject>) -> Option<Item> {
        self.keys_for(sender).into_iter().find_map(|key| match key {
            Key::Item(number) => self.find_item(number),
            _ => None,
        })
    }

    /// Return or a double click: a job or step opens its log, a run or stage opens or closes.
    pub fn activate_row(&self, row: isize) {
        let outline = &self.ui().outline;
        let Some(item) = (row >= 0).then(|| outline.itemAtRow(row)).flatten() else {
            return;
        };
        match Key::of(&item) {
            Some(Key::Job(run, job) | Key::Step(run, job, _)) => self.open_log(self.find_job(run, job)),
            Some(Key::Item(number)) => {
                if let Some(item) = self.find_item(number) {
                    self.open_url(&item.url);
                }
            }
            Some(Key::Run(_) | Key::Stage(..)) => {
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

    /// Reloads the list, keeping the selection and what the user opened or closed.
    pub fn reload_outline(&self) {
        let outline = &self.ui().outline;
        let selected = self.selected_keys();
        outline.reloadData();
        self.unfold(None);
        let rows = objc2_foundation::NSMutableIndexSet::new();
        for key in &selected {
            // SAFETY: The item is one the data source hands to this view, kept alive by the panel.
            let row = unsafe { outline.rowForItem(Some(&self.item_for(key))) };
            if let Ok(row) = usize::try_from(row) {
                rows.addIndex(row);
            }
        }
        outline.selectRowIndexes_byExtendingSelection(&rows, false);
        self.ui().show_empty_list(self);
    }

    /// Opens and closes the rows below `parent` as the user left them, else as their state asks for.
    fn unfold(&self, parent: Option<&Key>) {
        let outline = &self.ui().outline;
        let parent = parent.map(|key| self.item_for(key));
        let children = self.children(parent.as_deref().map(|item| item as &AnyObject));
        for (index, key) in children.into_iter().enumerate() {
            let item = self.item_for(&key);
            if self.children(Some(&item)).is_empty() {
                continue;
            }
            let chosen = self.read().chosen.get(&key).copied();
            let open = chosen.unwrap_or_else(|| self.opens_by_itself(key, index));
            self.ivars().unfolding.set(true);
            // SAFETY: The item is one the data source hands to this view, kept alive by the panel.
            unsafe {
                if open {
                    outline.expandItem(Some(&item));
                } else {
                    outline.collapseItem(Some(&item));
                }
            }
            self.ivars().unfolding.set(false);
            if open {
                self.unfold(Some(&key));
            }
        }
    }

    /// Whether a row the user has not opened or closed stands open: the newest run and the ones that failed or still
    /// run, and the stages that failed or run.
    fn opens_by_itself(&self, key: Key, index: usize) -> bool {
        match key {
            Key::Run(run) => {
                index == 0
                    || self
                        .find_run(run)
                        .is_some_and(|r| r.state == State::Failed || r.state.is_active())
            }
            Key::Stage(run, stage) => self
                .find_stage(run, stage)
                .is_some_and(|(_, state)| matches!(state, State::Failed | State::Running)),
            Key::Job(..) | Key::Step(..) | Key::Item(_) => false,
        }
    }

    /// The user opened or closed a row, which it stays from now on; what it shows opens as its states ask for.
    pub fn note_expansion(&self, notification: &NSNotification, open: bool) {
        if self.ivars().unfolding.get() {
            return;
        }
        let Some(key) = notification
            .userInfo()
            .and_then(|info| info.objectForKey(&ns("NSObject")))
            .and_then(|item| Key::of(&item))
        else {
            return;
        };
        self.state().chosen.insert(key, open);
        if open {
            self.unfold(Some(&key));
        }
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

    /// The titles of `key`'s context menu and whether each can be chosen, separators left out.
    #[cfg(feature = "selftest")]
    pub fn menu_titles(&self, key: Key) -> Vec<(String, bool)> {
        self.menu_entries(key)
            .into_iter()
            .filter(|(_, action, _)| action.is_some())
            .map(|(title, _, enabled)| (title.to_string(), enabled))
            .collect()
    }

    fn menu_entries(&self, key: Key) -> Vec<(&'static str, Option<Sel>, bool)> {
        let busy = self.read().busy;
        match key {
            Key::Run(id) | Key::Stage(id, _) => {
                let state = self.find_run(id).map_or(State::Neutral, |r| r.state);
                let mut entries = vec![(
                    t("Fehlgeschlagene Jobs neu starten"),
                    Some(sel!(ideCiRerun:)),
                    state == State::Failed && !busy,
                )];
                if self.is_github() {
                    entries.push((
                        t("Alle Jobs neu starten"),
                        Some(sel!(ideCiRerunAll:)),
                        !state.is_active() && !busy,
                    ));
                }
                entries.extend([
                    (
                        t("Lauf abbrechen …"),
                        Some(sel!(ideCiCancel:)),
                        state.is_active() && !busy,
                    ),
                    ("", None, false),
                    (t("Im Browser öffnen"), Some(sel!(ideCiOpenWeb:)), true),
                ]);
                entries
            }
            Key::Job(run, job) | Key::Step(run, job, _) => self.job_entries(run, job, busy),
            Key::Item(_) => vec![
                (t("Im Browser öffnen"), Some(sel!(ideCiOpenWeb:)), true),
                (t("Link kopieren"), Some(sel!(ideCiCopyLink:)), true),
            ],
        }
    }

    fn job_entries(&self, run: u64, job: u64, busy: bool) -> Vec<(&'static str, Option<Sel>, bool)> {
        let state = self.find_job(run, job).map_or(State::Neutral, |j| j.state);
        let run_active = self.find_run(run).is_some_and(|r| r.state.is_active());
        // GitHub re-runs a job only once its whole run has ended.
        let finished =
            matches!(state, State::Failed | State::Success | State::Cancelled) && !(self.is_github() && run_active);
        let start = if self.is_github() {
            t("Deployment freigeben")
        } else {
            t("Job starten")
        };
        vec![
            (t("Log öffnen"), Some(sel!(ideCiOpenLog:)), true),
            (
                t("An Agent senden"),
                Some(sel!(ideCiSendToAgent:)),
                state == State::Failed,
            ),
            ("", None, false),
            (t("Job neu starten"), Some(sel!(ideCiRetryJob:)), finished && !busy),
            (start, Some(sel!(ideCiStartJob:)), state == State::Manual && !busy),
            ("", None, false),
            (t("Im Browser öffnen"), Some(sel!(ideCiOpenWeb:)), true),
        ]
    }
}

/// A merge request's or issue's row: its number and title, then who opened it, its branch and when it last changed.
fn item_cell(mtm: MainThreadMarker, item: &Item, change: bool, github: bool) -> Retained<NSView> {
    let sign = if change && !github { "!" } else { "#" };
    let title = format!("{sign}{} {}", item.number, item.title);
    let mut detail: Vec<String> = Vec::new();
    if item.draft {
        detail.push(t("Entwurf").to_string());
    }
    detail.extend(item.author.clone());
    detail.extend(item.branch.clone());
    detail.extend(item.updated.map(text::ago));
    let (symbol_name, word) = if change {
        ("arrow.triangle.pull", t("offen"))
    } else {
        ("smallcircle.filled.circle", t("offen"))
    };
    let color = if item.draft {
        NSColor::secondaryLabelColor()
    } else {
        NSColor::systemGreenColor()
    };
    styled_cell(
        mtm,
        (symbol_name, word, &color),
        &title,
        Some(&detail.join(" · ")),
        None,
    )
}

/// A row: the state's symbol in its colour, the title, maybe a second line, and maybe a value at the end.
fn cell(
    mtm: MainThreadMarker,
    state: State,
    title: &str,
    detail: Option<&str>,
    value: Option<&str>,
) -> Retained<NSView> {
    let color = text::color(state);
    styled_cell(
        mtm,
        (text::symbol(state), text::word(state), &color),
        title,
        detail,
        value,
    )
}

/// A row whose symbol, the word it stands for and its colour are given.
fn styled_cell(
    mtm: MainThreadMarker,
    (symbol_name, word, color): (&str, &str, &NSColor),
    title: &str,
    detail: Option<&str>,
    value: Option<&str>,
) -> Retained<NSView> {
    let cell = NSTableCellView::new(mtm);
    let icon = NSImageView::new(mtm);
    if let Some(image) = symbol(symbol_name, word) {
        icon.setImage(Some(&image));
    }
    icon.setSymbolConfiguration(Some(&NSImageSymbolConfiguration::configurationWithPointSize_weight(
        12.0,
        // SAFETY: AppKit sets its font weight constants when it loads.
        unsafe { objc2_app_kit::NSFontWeightMedium },
    )));
    icon.setContentTintColor(Some(color));
    icon.setContentCompressionResistancePriority_forOrientation(1000.0, NSLayoutConstraintOrientation::Horizontal);
    let name = label(mtm, title);
    name.setLineBreakMode(NSLineBreakMode::ByTruncatingTail);
    name.setContentCompressionResistancePriority_forOrientation(250.0, NSLayoutConstraintOrientation::Horizontal);
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
        second.setContentCompressionResistancePriority_forOrientation(250.0, NSLayoutConstraintOrientation::Horizontal);
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
    let mut spoken = tf("{}: {}", &[title, word]);
    if let Some(value) = value {
        let end = label(mtm, value);
        end.setFont(Some(&NSFont::monospacedDigitSystemFontOfSize_weight(
            NSFont::smallSystemFontSize(),
            // SAFETY: AppKit sets its font weight constants when it loads.
            unsafe { objc2_app_kit::NSFontWeightRegular },
        )));
        end.setTextColor(Some(&NSColor::secondaryLabelColor()));
        end.setContentCompressionResistancePriority_forOrientation(1000.0, NSLayoutConstraintOrientation::Horizontal);
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

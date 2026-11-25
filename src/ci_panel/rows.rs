//! The rows of the CI list: runs with their number and age, GitLab's stages, jobs with how long they took, steps;
//! which of them stand open, the context menu and what a row's action acts on.

use std::hash::{DefaultHasher, Hash, Hasher};

use ide_ci::{Item, Job, Kind, Run, State};
use ide_ui_common::style::{BODY, small_system, space};
use ide_ui_common::ui_size::{self, Aspect};
use objc2::rc::Retained;
use objc2::runtime::{AnyObject, Sel};
use objc2::{DefinedClass, MainThreadMarker, MainThreadOnly, define_class, msg_send, sel};
use objc2_app_kit::{
    NSAccessibility, NSColor, NSEvent, NSFont, NSImageSymbolConfiguration, NSImageView, NSLayoutAttribute,
    NSLayoutConstraintOrientation, NSLineBreakMode, NSMenu, NSMenuItem, NSOutlineView, NSStackView, NSTableCellView,
    NSUserInterfaceItemIdentification, NSUserInterfaceLayoutOrientation, NSView,
};
use objc2_foundation::{NSNotification, NSObjectProtocol, NSRect};
use splat_i18n::{t, tf};

use super::model::{Key, Tab};
use super::panel::CiPanel;
use super::text;
use crate::util::{FlagGuard, activate, item, label, ns, separator, set_target, symbol};

/// Return and the keypad's Enter.
const RETURN_KEYS: [u16; 2] = [36, 76];

const RUN_HEIGHT: f64 = 36.0;
const JOB_HEIGHT: f64 = 24.0;
const STEP_HEIGHT: f64 = 22.0;
/// The size of a row's state symbol.
const SYMBOL_POINTS: f64 = 12.0;
/// The room between a row's text and the value at its end.
const VALUE_GAP: f64 = 6.0;

/// How far AppKit's source list sets its disclosure triangles in past the panel's title.
const OUTDENT: f64 = 6.0;

define_class!(
    // SAFETY: NSOutlineView has no subclassing requirements and `CiOutlineView` does not implement Drop.
    /// Return opens a job's log or folds a run open, as a double click does.
    #[unsafe(super(NSOutlineView))]
    #[thread_kind = MainThreadOnly]
    #[name = "IDECiOutlineView"]
    pub(crate) struct CiOutlineView;

    unsafe impl NSObjectProtocol for CiOutlineView {}

    impl CiOutlineView {
        /// The disclosure triangle where the panel's title begins, not in the source list's margin.
        /// AppKit makes no disclosure button, whose observers would keep it for good; the row's own chevron stands
        /// where it would be.
        #[unsafe(method(frameOfOutlineCellAtRow:))]
        fn no_outline_cell(&self, _row: isize) -> NSRect {
            NSRect::ZERO
        }

        #[unsafe(method(didAddRowView:forRow:))]
        fn did_add_row_view(&self, row_view: &objc2_app_kit::NSTableRowView, row: isize) {
            // SAFETY: The overridden method with arguments of its types, which objc2 checks in debug builds.
            let _: () = unsafe { msg_send![super(self), didAddRowView: row_view, forRow: row] };
            ide_ui_common::util::guarded((), || {
                // SAFETY: This class's own method, with its signature.
                let frame: NSRect = unsafe { msg_send![self, ideDisclosureFrameAtRow: row] };
                ide_ui_common::panel::chevron::place(self, row_view, frame);
            });
        }

        #[unsafe(method(ideDisclosureFrameAtRow:))]
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
    pub(crate) fn new(mtm: MainThreadMarker, frame: NSRect) -> Retained<CiOutlineView> {
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

/// The key `item` of the outline view stands for.
pub(crate) fn key_of(item: &AnyObject) -> Option<Key> {
    ide_ui_common::panel::RowItems::<Key>::key_of(item)
}

/// The height of the row of `key` at the interface size.
pub(crate) fn height(key: Option<&Key>) -> f64 {
    ui_size::scaled(match key {
        Some(Key::Run(_) | Key::Item(_)) => RUN_HEIGHT,
        Some(Key::Step(..)) => STEP_HEIGHT,
        _ => JOB_HEIGHT,
    })
}

impl CiPanel {
    pub(crate) fn find_run(&self, id: u64) -> Option<Run> {
        self.read().runs.iter().find(|r| r.run.id == id).map(|r| r.run.clone())
    }

    pub(crate) fn find_job(&self, run: u64, job: u64) -> Option<Job> {
        let state = self.read();
        let row = state.runs.iter().find(|r| r.run.id == run)?;
        row.jobs.iter().find(|j| j.id == job).cloned()
    }

    pub(crate) fn find_item(&self, number: u64) -> Option<Item> {
        self.read().items.iter().find(|i| i.number == number).cloned()
    }

    /// Whether the panel talks to GitHub.
    pub(crate) fn is_github(&self) -> bool {
        self.read().forge.as_ref().is_some_and(|f| f.kind == Kind::GitHub)
    }

    /// A hash of what the row of `key` shows apart from the times, which [`CiPanel::retime`] keeps current in place;
    /// the cell stays while this does.
    pub(crate) fn look(&self, key: &Key) -> Option<u64> {
        let state = self.read();
        let mut hasher = DefaultHasher::new();
        match *key {
            Key::Run(id) => {
                let run = &state.runs.iter().find(|r| r.run.id == id)?.run;
                let dated = run.updated.or(run.created).is_some();
                (run.state, &run.name, run.number, &run.event, dated).hash(&mut hasher);
            }
            Key::Stage(run, index) => state.stage(run, index)?.hash(&mut hasher),
            Key::Job(run, id) => {
                let job = job_in(&state, run, id)?;
                (job.state, &job.name, job.started.is_some()).hash(&mut hasher);
            }
            Key::Item(number) => {
                let item = state.items.iter().find(|i| i.number == number)?;
                let github = state.forge.as_ref().is_some_and(|f| f.kind == Kind::GitHub);
                let seen = (
                    &item.title,
                    &item.author,
                    &item.branch,
                    item.draft,
                    item.updated.is_some(),
                );
                (seen, state.tab, github).hash(&mut hasher);
            }
            Key::Step(run, job, number) => {
                let step = job_in(&state, run, job)?.steps.iter().find(|s| s.number == number)?;
                (step.number, &step.name, step.state).hash(&mut hasher);
            }
        }
        Some(hasher.finish())
    }

    /// The cell of `key`'s row, the one built before while the row shows the same, with its times brought up to now.
    pub(crate) fn kept_row_view(&self, key: &Key) -> Option<Retained<NSView>> {
        let look = self.look(key)?;
        let mut built = false;
        let cell = self.ivars().cells.cell(key, &look, || {
            built = true;
            self.row_view(key)
        })?;
        if built {
            self.note_timed(key);
        } else if self.is_stale(key) {
            self.retime(key, &cell);
        }
        Some(cell)
    }

    pub(crate) fn row_view(&self, key: &Key) -> Option<Retained<NSView>> {
        Some(styled_cell(self.mtm(), &self.texts(key)?))
    }

    /// What the row of `key` shows.
    pub(crate) fn texts(&self, key: &Key) -> Option<Texts> {
        let state = self.read();
        Some(match *key {
            Key::Run(id) => {
                let run = &state.runs.iter().find(|r| r.run.id == id)?.run;
                let detail = [run.event.clone(), run.updated.or(run.created).map(text::ago)]
                    .into_iter()
                    .flatten()
                    .collect::<Vec<_>>()
                    .join(" · ");
                let title = format!("{} #{}", run.name, run.number);
                Texts::of(run.state, title, Some(detail), None)
            }
            Key::Stage(run, index) => {
                let (name, stage) = state.stage(run, index)?;
                Texts::of(stage, name.to_string(), None, None)
            }
            Key::Job(run, id) => {
                let job = job_in(&state, run, id)?;
                let took = job.seconds(ide_ci::now()).map(text::took);
                Texts::of(job.state, job.name.clone(), None, took)
            }
            Key::Item(number) => {
                let item = state.items.iter().find(|i| i.number == number)?;
                let github = state.forge.as_ref().is_some_and(|f| f.kind == Kind::GitHub);
                item_texts(item, state.tab == Tab::Changes, github)
            }
            Key::Step(run, job, number) => {
                let step = job_in(&state, run, job)?.steps.iter().find(|s| s.number == number)?;
                Texts::of(step.state, format!("{}  {}", step.number, step.name), None, None)
            }
        })
    }

    /// The keys of the selected rows.
    pub(crate) fn selected_keys(&self) -> Vec<Key> {
        let outline = &self.ui().outline;
        (0..outline.numberOfRows())
            .filter(|row| outline.isRowSelected(*row))
            .filter_map(|row| outline.itemAtRow(row))
            .filter_map(|item| key_of(&item))
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
    pub(crate) fn run_for(&self, sender: Option<&AnyObject>, state: State) -> Option<Run> {
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
    pub(crate) fn job_for(&self, sender: Option<&AnyObject>) -> Option<Job> {
        self.keys_for(sender).into_iter().find_map(|key| match key {
            Key::Job(run, job) | Key::Step(run, job, _) => self.find_job(run, job),
            Key::Run(_) | Key::Stage(..) | Key::Item(_) => None,
        })
    }

    /// The merge request or issue an action means: the one of the row it came from.
    pub(crate) fn item_for_action(&self, sender: Option<&AnyObject>) -> Option<Item> {
        self.keys_for(sender).into_iter().find_map(|key| match key {
            Key::Item(number) => self.find_item(number),
            _ => None,
        })
    }

    /// Return or a double click: a job or step opens its log, a run or stage opens or closes.
    pub(crate) fn activate_row(&self, row: isize) {
        let outline = &self.ui().outline;
        let Some(item) = (row >= 0).then(|| outline.itemAtRow(row)).flatten() else {
            return;
        };
        match key_of(&item) {
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

    /// The page came into view: the list catches up with what changed while it was away.
    pub(crate) fn came_into_view(&self) {
        if self.ivars().held.catch_up(&self.ui().page) {
            self.reload_outline();
        }
        self.retime_shown();
        self.start_clock();
    }

    /// Reloads the list, keeping the selection and what the user opened or closed; out of view it waits.
    pub(crate) fn reload_outline(&self) {
        if self.ivars().held.holds(&self.ui().page) {
            return;
        }
        let outline = &self.ui().outline;
        let selected = self.selected_keys();
        self.ivars().cells.keep_shown();
        self.ivars().shown_revision.set(Some(self.read().revision()));
        self.forget_times();
        ide_ui_common::panel::reload(outline);
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
            if self.child_count(Some(&item)) == 0 {
                continue;
            }
            let chosen = self.read().chosen.get(&key).copied();
            let open = chosen.unwrap_or_else(|| self.opens_by_itself(key, index));
            {
                let _unfolding = FlagGuard::raise(&self.ivars().unfolding);
                // SAFETY: The item is one the data source hands to this view, kept alive by the panel.
                unsafe {
                    if open {
                        outline.expandItem(Some(&item));
                    } else {
                        outline.collapseItem(Some(&item));
                    }
                }
            }
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
                .read()
                .stage(run, stage)
                .is_some_and(|(_, state)| matches!(state, State::Failed | State::Running)),
            Key::Job(..) | Key::Step(..) | Key::Item(_) => false,
        }
    }

    /// The user opened or closed a row, which it stays from now on; what it shows opens as its states ask for.
    pub(crate) fn note_expansion(&self, notification: &NSNotification, open: bool) {
        if self.ivars().unfolding.get() {
            return;
        }
        let Some(key) = notification
            .userInfo()
            .and_then(|info| info.objectForKey(&ns("NSObject")))
            .and_then(|item| key_of(&item))
        else {
            return;
        };
        self.state().chosen.insert(key, open);
        if open {
            self.unfold(Some(&key));
        }
    }

    /// The context menu of the clicked row.
    pub(crate) fn fill_menu(&self, menu: &NSMenu) {
        menu.removeAllItems();
        menu.setAutoenablesItems(false);
        let outline = &self.ui().outline;
        let clicked = outline.clickedRow();
        let key = (clicked >= 0)
            .then(|| outline.itemAtRow(clicked))
            .flatten()
            .and_then(|item| key_of(&item));
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
            Key::Item(_) => {
                let mut entries = vec![
                    (t("Im Browser öffnen"), Some(sel!(ideCiOpenWeb:)), true),
                    (t("Link kopieren"), Some(sel!(ideCiCopyLink:)), true),
                ];
                if self.read().tab == Tab::Issues && self.has_leitstand() {
                    entries.extend([
                        ("", None, false),
                        (t("Als Auftrag übernehmen"), Some(sel!(ideCiTakeAsOrder:)), !busy),
                    ]);
                }
                entries
            }
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

/// Finds job `id` of `run` without copying it.
fn job_in(state: &super::model::State, run: u64, id: u64) -> Option<&Job> {
    let row = state.runs.iter().find(|r| r.run.id == run)?;
    row.jobs.iter().find(|j| j.id == id)
}

/// What a row shows: its symbol, the word it stands for and its colour, the title, maybe a second line, and maybe a
/// value at the end.
pub(crate) struct Texts {
    pub(crate) symbol: &'static str,
    pub(crate) word: &'static str,
    pub(crate) color: Retained<NSColor>,
    pub(crate) title: String,
    pub(crate) detail: Option<String>,
    pub(crate) value: Option<String>,
}

impl Texts {
    fn of(state: State, title: String, detail: Option<String>, value: Option<String>) -> Texts {
        Texts {
            symbol: text::symbol(state),
            word: text::word(state),
            color: text::color(state),
            title,
            detail,
            value,
        }
    }

    /// What VoiceOver reads and the tooltip says.
    pub(crate) fn spoken(&self) -> String {
        let mut spoken = tf("{}: {}", &[&self.title, self.word]);
        if let Some(value) = &self.value {
            spoken = tf("{}, {}", &[&spoken, value]);
        }
        if let Some(detail) = &self.detail {
            spoken = tf("{}, {}", &[&spoken, detail]);
        }
        spoken
    }
}

/// A merge request's or issue's row: its number and title, then who opened it, its branch and when it last changed.
fn item_texts(item: &Item, change: bool, github: bool) -> Texts {
    let sign = if change && !github { "!" } else { "#" };
    let title = format!("{sign}{} {}", item.number, item.title);
    let mut detail: Vec<String> = Vec::new();
    if item.draft {
        detail.push(t("Entwurf").to_string());
    }
    detail.extend(item.author.clone());
    detail.extend(item.branch.clone());
    detail.extend(item.updated.map(text::ago));
    let symbol = if change {
        "arrow.triangle.pull"
    } else {
        "smallcircle.filled.circle"
    };
    let color = if item.draft {
        NSColor::secondaryLabelColor()
    } else {
        NSColor::systemGreenColor()
    };
    Texts {
        symbol,
        word: t("offen"),
        color,
        title,
        detail: Some(detail.join(" · ")),
        value: None,
    }
}

/// A row as `texts` has it.
fn styled_cell(mtm: MainThreadMarker, texts: &Texts) -> Retained<NSView> {
    let cell = NSTableCellView::new(mtm);
    let icon = NSImageView::new(mtm);
    if let Some(image) = symbol(texts.symbol, texts.word) {
        icon.setImage(Some(&image));
    }
    ui_size::keep(&*icon, Aspect::Symbol, |icon| {
        icon.setSymbolConfiguration(Some(&NSImageSymbolConfiguration::configurationWithPointSize_weight(
            ui_size::scaled(SYMBOL_POINTS),
            crate::util::constant!(objc2_app_kit::NSFontWeightMedium),
        )));
    });
    icon.setContentTintColor(Some(&texts.color));
    icon.setContentCompressionResistancePriority_forOrientation(1000.0, NSLayoutConstraintOrientation::Horizontal);
    let name = label(mtm, &texts.title);
    ui_size::set_system_font(&name, BODY);
    name.setLineBreakMode(NSLineBreakMode::ByTruncatingTail);
    name.setContentCompressionResistancePriority_forOrientation(250.0, NSLayoutConstraintOrientation::Horizontal);
    let lines = NSStackView::new(mtm);
    lines.setOrientation(NSUserInterfaceLayoutOrientation::Vertical);
    lines.setAlignment(NSLayoutAttribute::Leading);
    ui_size::follow_spacing(&lines, space::HAIR);
    lines.addArrangedSubview(&name);
    if let Some(detail) = &texts.detail {
        let second = label(mtm, detail);
        ui_size::set_font(&second, &small_system());
        second.setTextColor(Some(&NSColor::secondaryLabelColor()));
        second.setLineBreakMode(NSLineBreakMode::ByTruncatingTail);
        second.setContentCompressionResistancePriority_forOrientation(250.0, NSLayoutConstraintOrientation::Horizontal);
        second.setIdentifier(Some(&ns(super::clock::DETAIL)));
        lines.addArrangedSubview(&second);
    }
    let row = NSStackView::new(mtm);
    row.setOrientation(NSUserInterfaceLayoutOrientation::Horizontal);
    ui_size::follow_spacing(&row, space::S);
    row.addArrangedSubview(&icon);
    row.addArrangedSubview(&lines);
    cell.addSubview(&row);
    row.setTranslatesAutoresizingMaskIntoConstraints(false);
    activate([
        row.leadingAnchor().constraintEqualToAnchor(&cell.leadingAnchor()),
        row.centerYAnchor().constraintEqualToAnchor(&cell.centerYAnchor()),
    ]);
    if let Some(value) = &texts.value {
        let end = label(mtm, value);
        ui_size::set_font(
            &end,
            &NSFont::monospacedDigitSystemFontOfSize_weight(
                NSFont::smallSystemFontSize(),
                crate::util::constant!(objc2_app_kit::NSFontWeightRegular),
            ),
        );
        end.setTextColor(Some(&NSColor::secondaryLabelColor()));
        end.setContentCompressionResistancePriority_forOrientation(1000.0, NSLayoutConstraintOrientation::Horizontal);
        end.setIdentifier(Some(&ns(super::clock::VALUE)));
        cell.addSubview(&end);
        end.setTranslatesAutoresizingMaskIntoConstraints(false);
        activate([
            end.trailingAnchor()
                .constraintEqualToAnchor_constant(&cell.trailingAnchor(), -6.0),
            end.centerYAnchor().constraintEqualToAnchor(&cell.centerYAnchor()),
            row.trailingAnchor()
                .constraintLessThanOrEqualToAnchor_constant(&end.leadingAnchor(), -ui_size::scaled(VALUE_GAP)),
        ]);
    } else {
        row.trailingAnchor()
            .constraintLessThanOrEqualToAnchor_constant(&cell.trailingAnchor(), -6.0)
            .setActive(true);
    }
    let spoken = ns(&texts.spoken());
    cell.setAccessibilityLabel(Some(&spoken));
    cell.setToolTip(Some(&spoken));
    // SAFETY: The label is a subview of the cell, which keeps it.
    unsafe { cell.setTextField(Some(&name)) };
    cell.into_super()
}

#[cfg(feature = "selftest")]
mod selftest_probe;

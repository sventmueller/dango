//! The search panel (⌘⇧F): a field whose matches are listed grouped per file, searched off the main thread and shown as
//! they are found, and a replace field that replaces all of them, those of a file or those of a line. Open editors
//! with unsaved changes are searched in place of their files, and the matches on show follow changes on disk and in
//! those editors.

use std::cell::{OnceCell, RefCell};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use ide_project::{Cancel, FileResult, LineResult, SearchError, SearchOptions, Summary};
use objc2::rc::{Retained, Weak};
use objc2::runtime::{AnyObject, NSObject, Sel};
use objc2::{DefinedClass, MainThreadMarker, MainThreadOnly, define_class, msg_send, sel};
use objc2_app_kit::{
    NSControl, NSControlTextEditingDelegate, NSEvent, NSMenu, NSMenuDelegate, NSOutlineView, NSOutlineViewDataSource,
    NSOutlineViewDelegate, NSSearchFieldDelegate, NSTableColumn, NSTextField, NSTextFieldDelegate, NSTextView, NSView,
};
use objc2_foundation::{NSIndexSet, NSNotification, NSObjectProtocol, NSRect, NSString};
use splat_i18n::{t, tf};

use super::WindowController;
use crate::util::{guarded, main_after, main_soon, ns};

mod replace;
mod rows;
mod views;

/// Matches one search shows at most; beyond them the user is asked to narrow it.
pub const LIMIT: usize = 5_000;

/// How long typing must pause before a search starts.
const DEBOUNCE: Duration = Duration::from_millis(150);

/// How often a running search hands what it found to the main thread.
const FLUSH: Duration = Duration::from_millis(80);

/// How long files and unsaved texts must rest before the matches on show are searched again.
const REFRESH_DELAY: Duration = Duration::from_millis(500);

const RETURN_KEYS: [u16; 2] = [36, 76];
const ESCAPE_KEY: u16 = 53;

thread_local! {
    static PANELS: RefCell<Vec<Weak<SearchPanel>>> = const { RefCell::new(Vec::new()) };
    static NEXT_ID: std::cell::Cell<u64> = const { std::cell::Cell::new(1) };
}

/// Which option a toggle button stands for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Toggle {
    Case,
    Word,
    Regex,
}

impl Toggle {
    const ALL: [Toggle; 3] = [Toggle::Case, Toggle::Word, Toggle::Regex];
}

/// How a search ended.
enum Outcome {
    Done(Summary),
    Failed(SearchError),
}

/// The matches of one file as the outline shows them.
struct Group {
    path: PathBuf,
    relative: String,
    lines: Vec<LineResult>,
    matches: usize,
    item: Retained<NSString>,
    line_items: Vec<Retained<NSString>>,
}

#[derive(Default)]
struct State {
    /// Counts changes of the field and options; a timer set for an older one does nothing.
    pending: u64,
    /// Counts the searches started; what an older one finds is dropped.
    run: u64,
    cancel: Option<Cancel>,
    /// The options of the search on show, to leave it be when a change gives the same options again.
    started: Option<SearchOptions>,
    groups: Vec<Group>,
    /// Each group's position in `groups` by path.
    index: HashMap<PathBuf, usize>,
    matches: usize,
    running: bool,
    outcome: Option<Outcome>,
    /// Files the user folded, which stay folded while more results come in.
    collapsed: HashSet<PathBuf>,
    /// Counts requests to search again after a change; a timer set for an older one does nothing.
    refresh: u64,
    /// The running search replaces the matches on show when it ends, instead of showing its own as they come.
    quiet: bool,
    /// The row the context menu was opened on.
    menu_row: isize,
}

/// Files below `root` changed on disk: the searches of its folder that show matches run again.
pub fn files_changed(root: &Path) {
    for panel in panels() {
        if panel.ivars().root == root {
            panel.refresh_soon();
        }
    }
}

/// The unsaved text of `path` changed: the searches that show matches below its folder run again.
pub fn buffer_edited(path: &Path) {
    for panel in panels() {
        if path.starts_with(&panel.ivars().root) {
            panel.refresh_soon();
        }
    }
}

fn panels() -> Vec<Retained<SearchPanel>> {
    PANELS.with(|panels| panels.borrow().iter().filter_map(Weak::load).collect())
}

/// Where an outline item points.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Spot {
    File(PathBuf),
    Line(PathBuf, usize),
}

pub struct SearchIvars {
    id: u64,
    controller: Weak<WindowController>,
    root: PathBuf,
    views: OnceCell<views::Views>,
    state: RefCell<State>,
}

define_class!(
    // SAFETY: NSObject has no subclassing requirements and `SearchPanel` does not implement Drop.
    #[unsafe(super(NSObject))]
    #[thread_kind = MainThreadOnly]
    #[name = "IDESearchPanel"]
    #[ivars = SearchIvars]
    pub struct SearchPanel;

    unsafe impl NSObjectProtocol for SearchPanel {}

    unsafe impl NSControlTextEditingDelegate for SearchPanel {
        #[unsafe(method(control:textView:doCommandBySelector:))]
        fn do_command(&self, control: &NSControl, _view: &NSTextView, command: Sel) -> bool {
            guarded(false, || self.field_command(control, command))
        }

        #[unsafe(method(controlTextDidChange:))]
        fn text_changed(&self, notification: &NSNotification) {
            guarded((), || {
                if self.is_replace_field(notification) {
                    self.show();
                } else {
                    self.schedule();
                }
            });
        }
    }

    unsafe impl NSMenuDelegate for SearchPanel {
        #[unsafe(method(menuNeedsUpdate:))]
        fn menu_needs_update(&self, menu: &NSMenu) {
            guarded((), || self.fill_menu(menu));
        }
    }

    unsafe impl NSTextFieldDelegate for SearchPanel {}

    unsafe impl NSSearchFieldDelegate for SearchPanel {}

    unsafe impl NSOutlineViewDataSource for SearchPanel {
        #[unsafe(method(outlineView:numberOfChildrenOfItem:))]
        fn children_count(&self, _view: &NSOutlineView, item: Option<&AnyObject>) -> isize {
            guarded(0, || isize::try_from(self.children(item)).unwrap_or(0))
        }

        #[unsafe(method_id(outlineView:child:ofItem:))]
        fn child(&self, _view: &NSOutlineView, index: isize, item: Option<&AnyObject>) -> Retained<AnyObject> {
            let found = guarded(None, || self.child_item(usize::try_from(index).ok()?, item));
            found.map_or_else(|| ns("").into(), Into::into)
        }

        #[unsafe(method(outlineView:isItemExpandable:))]
        fn expandable(&self, _view: &NSOutlineView, item: &AnyObject) -> bool {
            guarded(false, || matches!(spot_of(item), Some(Spot::File(_))))
        }
    }

    unsafe impl NSOutlineViewDelegate for SearchPanel {
        #[unsafe(method_id(outlineView:viewForTableColumn:item:))]
        fn view_for(
            &self,
            _view: &NSOutlineView,
            _column: Option<&NSTableColumn>,
            item: &AnyObject,
        ) -> Option<Retained<NSView>> {
            guarded(None, || self.row_view(item))
        }

        #[unsafe(method(outlineViewItemDidCollapse:))]
        fn did_collapse(&self, notification: &NSNotification) {
            guarded((), || self.folded(notification, true));
        }

        #[unsafe(method(outlineViewItemDidExpand:))]
        fn did_expand(&self, notification: &NSNotification) {
            guarded((), || self.folded(notification, false));
        }
    }

    impl SearchPanel {
        #[unsafe(method(ideSearchOptionChanged:))]
        fn option_changed(&self, _sender: Option<&AnyObject>) {
            guarded((), || {
                if let Some(views) = self.ivars().views.get() {
                    views.paint_toggles();
                }
                self.schedule();
            });
        }

        #[unsafe(method(ideSearchDisclose:))]
        fn disclose(&self, _sender: Option<&AnyObject>) {
            guarded((), || {
                if let Some(views) = self.ivars().views.get() {
                    views.set_globs_shown(views.globs.isHidden());
                }
            });
        }

        #[unsafe(method(ideSearchRowClicked:))]
        fn row_clicked(&self, _sender: Option<&AnyObject>) {
            guarded((), || {
                let Some(views) = self.ivars().views.get() else { return };
                let row = views.outline.clickedRow();
                let is_line = views.outline.itemAtRow(row).is_some_and(|i| matches!(spot_of(&i), Some(Spot::Line(..))));
                if is_line {
                    self.open_row(row, false);
                }
            });
        }

        #[unsafe(method(ideSearchRowDoubleClicked:))]
        fn row_double_clicked(&self, _sender: Option<&AnyObject>) {
            guarded((), || {
                if let Some(views) = self.ivars().views.get() {
                    self.open_row(views.outline.clickedRow(), true);
                }
            });
        }

        #[unsafe(method(ideSearchToggleReplace:))]
        fn toggle_replace(&self, _sender: Option<&AnyObject>) {
            guarded((), || {
                if let Some(views) = self.ivars().views.get() {
                    views.set_replace_shown(!views.replace_shown());
                    if views.replace_shown()
                        && let Some(window) = views.page.window()
                    {
                        window.makeFirstResponder(Some(&views.replace));
                    }
                }
                self.show();
            });
        }

        #[unsafe(method(ideSearchReplaceAll:))]
        fn replace_all_action(&self, _sender: Option<&AnyObject>) {
            guarded((), || self.ask_replace_all());
        }

        #[unsafe(method(ideSearchReplaceRow:))]
        fn replace_row_action(&self, sender: Option<&AnyObject>) {
            guarded((), || {
                let Some(views) = self.ivars().views.get() else { return };
                let Some(view) = sender.and_then(|s| s.downcast_ref::<NSView>()) else { return };
                self.replace_at(views.outline.rowForView(view));
            });
        }

        #[unsafe(method(ideSearchMenuReplace:))]
        fn menu_replace(&self, _sender: Option<&AnyObject>) {
            guarded((), || {
                let row = self.ivars().state.borrow().menu_row;
                self.replace_at(row);
            });
        }

        #[unsafe(method(ideSearchMenuOpen:))]
        fn menu_open(&self, _sender: Option<&AnyObject>) {
            guarded((), || {
                let row = self.ivars().state.borrow().menu_row;
                self.open_row(row, true);
            });
        }
    }
);

define_class!(
    // SAFETY: NSOutlineView has no subclassing requirements and `SearchOutlineView` does not implement Drop.
    /// Return opens the selected match, Esc ends the search.
    #[unsafe(super(NSOutlineView))]
    #[thread_kind = MainThreadOnly]
    #[name = "IDESearchOutlineView"]
    pub struct SearchOutlineView;

    unsafe impl NSObjectProtocol for SearchOutlineView {}

    impl SearchOutlineView {
        #[unsafe(method(keyDown:))]
        fn key_down(&self, event: &NSEvent) {
            if !guarded(false, || self.key(event)) {
                // SAFETY: The superclass's implementation, with this method's signature.
                let _: () = unsafe { msg_send![super(self), keyDown: event] };
            }
        }
    }
);

impl SearchOutlineView {
    pub fn new(mtm: MainThreadMarker, frame: NSRect) -> Retained<SearchOutlineView> {
        // SAFETY: NSOutlineView's designated initializer, called once on the newly allocated object.
        unsafe { msg_send![SearchOutlineView::alloc(mtm), initWithFrame: frame] }
    }

    fn panel(&self) -> Option<Retained<SearchPanel>> {
        // SAFETY: Only `views::Views::new` makes this view, and it sets the panel as the data source.
        let source = unsafe { self.dataSource() }?;
        let object: Retained<AnyObject> = source.into();
        object.downcast::<SearchPanel>().ok()
    }

    fn key(&self, event: &NSEvent) -> bool {
        let Some(panel) = self.panel() else { return false };
        let code = event.keyCode();
        if RETURN_KEYS.contains(&code) {
            panel.open_row(self.selectedRow(), true);
        } else if code == ESCAPE_KEY {
            panel.clear();
        } else {
            return false;
        }
        true
    }
}

/// The panel with `id`, if its window is still open.
fn panel(id: u64) -> Option<Retained<SearchPanel>> {
    PANELS.with(|panels| {
        let mut panels = panels.borrow_mut();
        panels.retain(|p| p.load().is_some());
        panels.iter().filter_map(Weak::load).find(|p| p.ivars().id == id)
    })
}

/// Where `item`, an outline item of a search panel, points.
fn spot_of(item: &AnyObject) -> Option<Spot> {
    let text = item.downcast_ref::<NSString>()?.to_string();
    let mut parts = text.splitn(3, '\0');
    match (parts.next()?, parts.next()?, parts.next()) {
        ("F", path, None) => Some(Spot::File(PathBuf::from(path))),
        ("L", index, Some(path)) => Some(Spot::Line(PathBuf::from(path), index.parse().ok()?)),
        _ => None,
    }
}

fn file_item(path: &Path) -> Retained<NSString> {
    ns(&format!("F\0{}", path.to_string_lossy()))
}

fn line_item(path: &Path, index: usize) -> Retained<NSString> {
    ns(&format!("L\0{index}\0{}", path.to_string_lossy()))
}

/// Matches a line counts: its match ranges, or one for a line that matched only empty.
fn count(lines: &[LineResult]) -> usize {
    lines.iter().map(|l| l.columns.len().max(1)).sum()
}

/// What one search runs on: the panel, its generation, the folder, the options and the unsaved texts of open editors.
struct Job {
    id: u64,
    generation: u64,
    root: PathBuf,
    options: SearchOptions,
    buffers: Vec<(PathBuf, String)>,
    /// Hands what is found over in batches as it comes, rather than all at the end.
    progressive: bool,
}

/// Runs `job` and hands what it finds to its panel, then how it ended.
fn run(job: Job, cancel: Cancel) {
    let Job {
        id,
        generation,
        root,
        options,
        buffers,
        progressive,
    } = job;
    let mut batch = Vec::new();
    let mut flushed = Instant::now();
    let result = ide_project::search_with(&root, &options, &buffers, &cancel, |found| {
        batch.push(found);
        if progressive && flushed.elapsed() >= FLUSH {
            let files = std::mem::take(&mut batch);
            main_soon(move || deliver(id, generation, files, None));
            flushed = Instant::now();
        }
    });
    let outcome = match result {
        Ok(summary) => Outcome::Done(summary),
        Err(error) => Outcome::Failed(error),
    };
    main_soon(move || deliver(id, generation, batch, Some(outcome)));
}

fn deliver(id: u64, generation: u64, files: Vec<FileResult>, outcome: Option<Outcome>) {
    if let Some(panel) = panel(id) {
        panel.take(generation, files, outcome);
    }
}

/// What the status line says about the matches so far, and whether it reports a problem.
fn status_text(state: &State) -> (String, bool) {
    match &state.outcome {
        Some(Outcome::Failed(SearchError::Pattern(why))) => return (tf("Ungültiges Suchmuster: {}", &[why]), true),
        Some(Outcome::Failed(SearchError::Glob(why))) => return (tf("Ungültiges Dateimuster: {}", &[why]), true),
        Some(Outcome::Failed(SearchError::Root(why))) => return (why.clone(), true),
        Some(Outcome::Done(summary)) if summary.limit_reached => {
            return (
                tf("Mehr als {} Treffer — Suche eingrenzen", &[&LIMIT.to_string()]),
                false,
            );
        }
        Some(Outcome::Done(_)) if state.groups.is_empty() => return (t("Keine Treffer").to_string(), false),
        _ => {}
    }
    if state.groups.is_empty() {
        return (t("Suche läuft …").to_string(), false);
    }
    let (matches, files) = (state.matches, state.groups.len());
    let text = match (matches, files) {
        (1, 1) => t("1 Treffer in 1 Datei").to_string(),
        (_, 1) => tf("{} Treffer in 1 Datei", &[&matches.to_string()]),
        _ => tf("{} Treffer in {} Dateien", &[&matches.to_string(), &files.to_string()]),
    };
    (text, false)
}

impl SearchPanel {
    /// The search of `controller`'s folder at `root`.
    pub fn new(mtm: MainThreadMarker, controller: &WindowController, root: &Path) -> Retained<Self> {
        let id = NEXT_ID.with(|next| {
            let id = next.get();
            next.set(id + 1);
            id
        });
        let ivars = SearchIvars {
            id,
            controller: Weak::from(controller),
            root: root.to_path_buf(),
            views: OnceCell::new(),
            state: RefCell::new(State::default()),
        };
        // SAFETY: NSObject's initializer, called once on the newly allocated object.
        let this: Retained<Self> = unsafe { msg_send![super(SearchPanel::alloc(mtm).set_ivars(ivars)), init] };
        let _ = this.ivars().views.set(views::Views::new(mtm, &this));
        PANELS.with(|panels| panels.borrow_mut().push(Weak::from(&*this)));
        this
    }

    /// The panel's page: the fields and their options above the matches.
    pub fn page(&self) -> Option<&NSView> {
        self.ivars().views.get().map(|views| &*views.page)
    }

    /// Puts the keyboard focus into the field, with what it holds selected.
    pub fn focus(&self) {
        let Some(views) = self.ivars().views.get() else { return };
        if let Some(window) = views.page.window() {
            window.makeFirstResponder(Some(&views.field));
        }
    }

    /// Ends a search that runs, for a window that closes or shows another folder.
    pub fn stop(&self) {
        if let Some(cancel) = self.ivars().state.borrow_mut().cancel.take() {
            cancel.cancel();
        }
    }

    /// Empties the field, which clears the matches, and keeps the focus in the field.
    pub fn clear(&self) {
        let Some(views) = self.ivars().views.get() else { return };
        views.field.setStringValue(&ns(""));
        self.schedule();
        self.focus();
    }

    fn options(&self) -> SearchOptions {
        let Some(views) = self.ivars().views.get() else {
            return SearchOptions::new("");
        };
        let globs = |field: &NSTextField| -> Vec<String> {
            field
                .stringValue()
                .to_string()
                .split(',')
                .map(str::trim)
                .filter(|g| !g.is_empty())
                .map(str::to_string)
                .collect()
        };
        let mut options = SearchOptions::new(&views.field.stringValue().to_string());
        options.case_sensitive = views.toggled(Toggle::Case);
        options.whole_word = views.toggled(Toggle::Word);
        options.regex = views.toggled(Toggle::Regex);
        options.include = globs(&views.include);
        options.exclude = globs(&views.exclude);
        options.max_results = LIMIT;
        options
    }

    /// Starts a search for what the field and options hold once typing pauses; an empty field shows the tree at once.
    fn schedule(&self) {
        let pending = {
            let mut state = self.ivars().state.borrow_mut();
            state.pending += 1;
            state.pending
        };
        if self.options().pattern.is_empty() {
            self.start(pending);
            return;
        }
        let id = self.ivars().id;
        main_after(DEBOUNCE, move || {
            if let Some(panel) = panel(id) {
                panel.start(pending);
            }
        });
    }

    fn start(&self, pending: u64) {
        let options = self.options();
        {
            let state = self.ivars().state.borrow();
            let unchanged = state.started.as_ref() == Some(&options);
            if state.pending != pending || unchanged {
                return;
            }
        }
        self.stop();
        let searching = !options.pattern.is_empty();
        let generation = {
            let mut state = self.ivars().state.borrow_mut();
            state.run += 1;
            state.groups.clear();
            state.index.clear();
            state.matches = 0;
            state.outcome = None;
            state.running = searching;
            state.started = searching.then(|| options.clone());
            state.run
        };
        self.show();
        if !searching {
            return;
        }
        self.spawn(generation, options, true);
    }

    /// Runs search `generation` with `options` on a thread of its own.
    fn spawn(&self, generation: u64, options: SearchOptions, progressive: bool) {
        let cancel = Cancel::new();
        let root = self.ivars().root.clone();
        let job = Job {
            id: self.ivars().id,
            generation,
            buffers: crate::editor::unsaved_buffers(&root),
            root,
            options,
            progressive,
        };
        {
            let mut state = self.ivars().state.borrow_mut();
            state.cancel = Some(cancel.clone());
            state.quiet = !progressive;
        }
        let spawned = std::thread::Builder::new()
            .name("ide-search".into())
            .spawn(move || run(job, cancel));
        if let Err(error) = spawned {
            crate::diag::report("starting a project search", error);
            self.ivars().state.borrow_mut().running = false;
        }
    }

    /// Searches again once files or unsaved texts below the folder rest, if matches are on show.
    fn refresh_soon(&self) {
        let refresh = {
            let mut state = self.ivars().state.borrow_mut();
            if state.started.is_none() {
                return;
            }
            state.refresh += 1;
            state.refresh
        };
        let id = self.ivars().id;
        main_after(REFRESH_DELAY, move || {
            if let Some(panel) = panel(id) {
                panel.refresh(refresh);
            }
        });
    }

    /// Searches again for the matches on show and puts the new ones in their place when the search ends.
    fn refresh(&self, refresh: u64) {
        let options = self.options();
        let generation = {
            let mut state = self.ivars().state.borrow_mut();
            if state.refresh != refresh || state.started.as_ref() != Some(&options) {
                return;
            }
            if let Some(cancel) = state.cancel.take() {
                cancel.cancel();
            }
            state.run += 1;
            state.running = true;
            state.run
        };
        self.spawn(generation, options, false);
    }

    /// Takes `files` and maybe the end of search `generation`, unless another search started since.
    fn take(&self, generation: u64, files: Vec<FileResult>, outcome: Option<Outcome>) {
        {
            let mut state = self.ivars().state.borrow_mut();
            if state.run != generation || state.started.is_none() {
                return;
            }
            if state.quiet && outcome.is_some() {
                state.quiet = false;
                state.groups.clear();
                state.index.clear();
                state.matches = 0;
            }
            for found in files {
                self.insert(&mut state, found);
            }
            if let Some(outcome) = outcome {
                state.running = false;
                state.cancel = None;
                state.outcome = Some(outcome);
            }
        }
        self.show();
    }

    /// Puts `found` into the groups in the order of their paths.
    fn insert(&self, state: &mut State, found: FileResult) {
        let relative = found
            .path
            .strip_prefix(&self.ivars().root)
            .unwrap_or(&found.path)
            .to_string_lossy()
            .into_owned();
        let at = state.groups.partition_point(|g| g.relative < relative);
        let matches = count(&found.lines);
        state.matches += matches;
        let line_items = (0..found.lines.len()).map(|i| line_item(&found.path, i)).collect();
        state.groups.insert(
            at,
            Group {
                item: file_item(&found.path),
                path: found.path,
                relative,
                lines: found.lines,
                matches,
                line_items,
            },
        );
        for (position, group) in state.groups.iter().enumerate().skip(at) {
            state.index.insert(group.path.clone(), position);
        }
    }

    /// Shows the tree or the matches with their status, keeping the selected row where it can.
    fn show(&self) {
        let Some(views) = self.ivars().views.get() else { return };
        // The outline calls back into the panel while it reloads and expands, so the state is not held meanwhile.
        let (searching, (status, problem), unfolded) = {
            let state = self.ivars().state.borrow();
            let unfolded: Vec<Retained<NSString>> = state
                .groups
                .iter()
                .filter(|g| !state.collapsed.contains(&g.path))
                .map(|g| g.item.clone())
                .collect();
            (state.started.is_some(), status_text(&state), unfolded)
        };
        views.show_results(searching);
        views.replace_all.setEnabled(self.can_replace_all());
        if !searching {
            return;
        }
        views.set_status(&status, problem);
        let selected = views
            .outline
            .itemAtRow(views.outline.selectedRow())
            .and_then(|i| spot_of(&i));
        views.outline.reloadData();
        views.outline.sizeLastColumnToFit();
        for item in &unfolded {
            // SAFETY: The item is one the data source hands out, kept alive in its group.
            unsafe { views.outline.expandItem(Some(item)) };
        }
        let item = selected.and_then(|spot| self.item_for(&self.ivars().state.borrow(), &spot));
        if let Some(item) = item {
            // SAFETY: The item is one the data source hands out, kept alive in its group.
            let row = unsafe { views.outline.rowForItem(Some(&item)) };
            if let Ok(row) = usize::try_from(row) {
                views
                    .outline
                    .selectRowIndexes_byExtendingSelection(&NSIndexSet::indexSetWithIndex(row), false);
            }
        }
    }

    fn item_for(&self, state: &State, spot: &Spot) -> Option<Retained<NSString>> {
        match spot {
            Spot::File(path) => Some(state.groups.get(*state.index.get(path)?)?.item.clone()),
            Spot::Line(path, index) => state
                .groups
                .get(*state.index.get(path)?)?
                .line_items
                .get(*index)
                .cloned(),
        }
    }

    fn children(&self, item: Option<&AnyObject>) -> usize {
        let state = self.ivars().state.borrow();
        match item.map(spot_of) {
            None => state.groups.len(),
            Some(Some(Spot::File(path))) => state.index.get(&path).map_or(0, |i| state.groups[*i].lines.len()),
            Some(_) => 0,
        }
    }

    fn child_item(&self, index: usize, item: Option<&AnyObject>) -> Option<Retained<NSString>> {
        let state = self.ivars().state.borrow();
        match item.map(spot_of) {
            None => Some(state.groups.get(index)?.item.clone()),
            Some(Some(Spot::File(path))) => state
                .groups
                .get(*state.index.get(&path)?)?
                .line_items
                .get(index)
                .cloned(),
            Some(_) => None,
        }
    }

    fn row_view(&self, item: &AnyObject) -> Option<Retained<NSView>> {
        let views = self.ivars().views.get()?;
        let with = views.replace.stringValue().to_string();
        let options = self.options();
        let replacing = views.replace_shown().then_some(rows::Replacing {
            panel: self,
            with: &with,
            options: &options,
        });
        let state = self.ivars().state.borrow();
        let mtm = self.mtm();
        match spot_of(item)? {
            Spot::File(path) => {
                let group = state.groups.get(*state.index.get(&path)?)?;
                Some(rows::file_row(
                    mtm,
                    &group.path,
                    &group.relative,
                    group.matches,
                    replacing.as_ref(),
                ))
            }
            Spot::Line(path, index) => {
                let group = state.groups.get(*state.index.get(&path)?)?;
                Some(rows::line_row(mtm, group.lines.get(index)?, replacing.as_ref()))
            }
        }
    }

    fn folded(&self, notification: &NSNotification, collapsed: bool) {
        let item = notification
            .userInfo()
            .and_then(|info| info.objectForKey(&ns("NSObject")));
        let Some(Spot::File(path)) = item.and_then(|item| spot_of(&item)) else {
            return;
        };
        let mut state = self.ivars().state.borrow_mut();
        if collapsed {
            state.collapsed.insert(path);
        } else {
            state.collapsed.remove(&path);
        }
    }

    /// Opens the file of `row` and selects its match, or the file's first match; `focus` moves the keyboard focus
    /// to the editor, else it stays in the list.
    fn open_row(&self, row: isize, focus: bool) {
        let Some(views) = self.ivars().views.get() else { return };
        let Some(spot) = views.outline.itemAtRow(row).and_then(|i| spot_of(&i)) else {
            return;
        };
        let target = {
            let state = self.ivars().state.borrow();
            let (path, index) = match &spot {
                Spot::File(path) => (path, 0),
                Spot::Line(path, index) => (path, *index),
            };
            state.index.get(path).and_then(|i| {
                let line = state.groups[*i].lines.get(index)?;
                Some((path.clone(), line.line, line.columns.first().cloned().unwrap_or(0..0)))
            })
        };
        let (Some((path, line, columns)), Some(controller)) = (target, self.ivars().controller.load()) else {
            return;
        };
        crate::editor::reveal(&controller, &path, line, columns);
        if !focus && let Some(window) = views.page.window() {
            window.makeFirstResponder(Some(&views.outline));
        }
    }

    fn is_replace_field(&self, notification: &NSNotification) -> bool {
        let (Some(views), Some(object)) = (self.ivars().views.get(), notification.object()) else {
            return false;
        };
        std::ptr::eq(
            Retained::as_ptr(&object).cast::<()>(),
            Retained::as_ptr(&views.replace).cast(),
        )
    }

    /// Whether every match on show can be replaced: a search that ended with matches and was not cut at its limit.
    fn can_replace_all(&self) -> bool {
        let state = self.ivars().state.borrow();
        let done = matches!(&state.outcome, Some(Outcome::Done(summary)) if !summary.limit_reached);
        done && !state.running && state.matches > 0
    }

    fn fill_menu(&self, menu: &NSMenu) {
        menu.removeAllItems();
        let Some(views) = self.ivars().views.get() else { return };
        let row = views.outline.clickedRow();
        self.ivars().state.borrow_mut().menu_row = row;
        let Some(spot) = views.outline.itemAtRow(row).and_then(|i| spot_of(&i)) else {
            return;
        };
        let mtm = self.mtm();
        let mut entries = vec![(t("Öffnen"), sel!(ideSearchMenuOpen:))];
        if views.replace_shown() {
            let title = match spot {
                Spot::File(_) => t("Alle in dieser Datei ersetzen"),
                Spot::Line(..) => t("Ersetzen"),
            };
            entries.push((title, sel!(ideSearchMenuReplace:)));
        }
        for (title, action) in entries {
            let entry = crate::util::item(mtm, title, Some(action), "");
            crate::util::set_target(&entry, self);
            menu.addItem(&entry);
        }
    }

    /// Asks before replacing every match on show, naming how many there are.
    fn ask_replace_all(&self) {
        let Some(views) = self.ivars().views.get() else { return };
        let Some(window) = views.page.window() else { return };
        if !self.can_replace_all() {
            return;
        }
        let (matches, files) = {
            let state = self.ivars().state.borrow();
            (state.matches, state.groups.len())
        };
        let question = match (matches, files) {
            (1, _) => t("1 Treffer in 1 Datei ersetzen?").to_string(),
            (_, 1) => tf("{} Treffer in 1 Datei ersetzen?", &[&matches.to_string()]),
            _ => tf(
                "{} Treffer in {} Dateien ersetzen?",
                &[&matches.to_string(), &files.to_string()],
            ),
        };
        let with = views.replace.stringValue().to_string();
        let detail = if with.is_empty() {
            t("Die Treffer werden entfernt. In geöffneten Dateien nimmt ⌘Z das zurück, die übrigen werden gleich gesichert.")
                .to_string()
        } else {
            tf(
                "Durch „{}“. In geöffneten Dateien nimmt ⌘Z das zurück, die übrigen werden gleich gesichert.",
                &[&with],
            )
        };
        let alert = objc2_app_kit::NSAlert::new(self.mtm());
        alert.setMessageText(&ns(&question));
        alert.setInformativeText(&ns(&detail));
        alert.addButtonWithTitle(&ns(t("Ersetzen")));
        alert.addButtonWithTitle(&ns(t("Abbrechen")));
        let id = self.ivars().id;
        let handler = block2::RcBlock::new(move |response: objc2_app_kit::NSModalResponse| {
            guarded((), || {
                if response == objc2_app_kit::NSAlertFirstButtonReturn
                    && let Some(panel) = panel(id)
                {
                    let files = panel
                        .ivars()
                        .state
                        .borrow()
                        .groups
                        .iter()
                        .map(|g| g.path.clone())
                        .collect();
                    panel.replace(files, replace::Only::All);
                }
            });
        });
        alert.beginSheetModalForWindow_completionHandler(&window, Some(&handler));
    }

    /// Replaces the matches of the file or the line in `row`.
    fn replace_at(&self, row: isize) {
        let Some(views) = self.ivars().views.get() else { return };
        let Some(spot) = views.outline.itemAtRow(row).and_then(|i| spot_of(&i)) else {
            return;
        };
        let only = match &spot {
            Spot::File(_) => Some(replace::Only::All),
            Spot::Line(path, index) => {
                let state = self.ivars().state.borrow();
                state
                    .index
                    .get(path)
                    .and_then(|i| state.groups[*i].lines.get(*index))
                    .map(|line| replace::Only::Line(line.line, line.columns.clone()))
            }
        };
        let (Spot::File(path) | Spot::Line(path, _)) = spot;
        if let Some(only) = only {
            self.replace(vec![path], only);
        }
    }

    /// Replaces in `files` with what the replace field holds, then searches again and says what it did.
    fn replace(&self, files: Vec<PathBuf>, only: replace::Only) {
        let Some(views) = self.ivars().views.get() else { return };
        let with = views.replace.stringValue().to_string();
        let id = self.ivars().id;
        replace::replace(files, self.options(), with, only, move |done| {
            if let Some(panel) = panel(id) {
                panel.replaced(&done);
            }
        });
    }

    fn replaced(&self, done: &replace::Done) {
        let note = match (done.replaced, done.files) {
            (0, _) => None,
            (1, _) => Some(t("1 Treffer ersetzt").to_string()),
            (n, 1) => Some(tf("{} Treffer in 1 Datei ersetzt", &[&n.to_string()])),
            (n, files) => Some(tf(
                "{} Treffer in {} Dateien ersetzt",
                &[&n.to_string(), &files.to_string()],
            )),
        };
        if let (Some(note), Some(controller)) = (note, self.ivars().controller.load()) {
            controller.show_notice(&note);
        }
        if !done.problems.is_empty() {
            let alert = objc2_app_kit::NSAlert::new(self.mtm());
            alert.setAlertStyle(objc2_app_kit::NSAlertStyle::Warning);
            alert.setMessageText(&ns(t("Nicht alle Treffer wurden ersetzt.")));
            alert.setInformativeText(&ns(&done.problems.join("\n")));
            if let Some(window) = self.ivars().views.get().and_then(|v| v.page.window()) {
                alert.beginSheetModalForWindow_completionHandler(&window, None);
            }
        }
        let refresh = {
            let mut state = self.ivars().state.borrow_mut();
            state.refresh += 1;
            state.refresh
        };
        self.refresh(refresh);
    }

    /// Arrow down moves into the matches, Return opens the selected or first one, Esc empties a field with a query.
    fn field_command(&self, control: &NSControl, command: Sel) -> bool {
        let Some(views) = self.ivars().views.get() else {
            return false;
        };
        let field: &NSControl = &views.field;
        if !std::ptr::eq(control, field) {
            return false;
        }
        let has_query = !views.field.stringValue().to_string().is_empty();
        let rows = views.outline.numberOfRows();
        if command == sel!(moveDown:) && has_query && rows > 0 {
            let row = views.outline.selectedRow().max(0);
            views
                .outline
                .selectRowIndexes_byExtendingSelection(&NSIndexSet::indexSetWithIndex(row.unsigned_abs()), false);
            if let Some(window) = views.page.window() {
                window.makeFirstResponder(Some(&views.outline));
            }
        } else if command == sel!(insertNewline:) && has_query && rows > 0 {
            self.open_row(views.outline.selectedRow().max(0), true);
        } else if command == sel!(cancelOperation:) && has_query {
            self.clear();
        } else {
            return false;
        }
        true
    }
}

/// What the self-test reads from and does with a search panel, through the code its controls run.
#[cfg(feature = "selftest")]
pub mod selftest {
    use objc2::DefinedClass;

    use super::{SearchPanel, Spot, Toggle, spot_of};
    use crate::util::ns;

    /// What the panel shows.
    pub struct Seen {
        /// Whether the matches shown are those of the field and options as they are now, found to the end.
        pub settled: bool,
        /// The files with their match counts, in the order shown.
        pub files: Vec<(String, usize)>,
        /// The rows of the outline.
        pub rows: isize,
        pub status: String,
        pub results_shown: bool,
        pub field_focused: bool,
        /// Whether "Alle ersetzen" can be pressed.
        pub can_replace_all: bool,
    }

    pub fn seen(panel: &SearchPanel) -> Seen {
        let options = panel.options();
        let state = panel.ivars().state.borrow();
        let views = panel.ivars().views.get();
        let focused = views.is_some_and(|views| {
            let editor = views.page.window().and_then(|w| w.firstResponder());
            let editor = editor.and_then(|r| r.downcast::<objc2_app_kit::NSTextView>().ok());
            let owner = editor.and_then(|e| e.delegate());
            owner.is_some_and(|owner| {
                objc2::rc::Retained::as_ptr(&owner).cast::<()>() == objc2::rc::Retained::as_ptr(&views.field).cast()
            })
        });
        let shown = state
            .started
            .as_ref()
            .map_or(options.pattern.is_empty(), |s| *s == options);
        Seen {
            settled: shown && !state.running,
            files: state.groups.iter().map(|g| (g.relative.clone(), g.matches)).collect(),
            rows: views.map_or(0, |v| v.outline.numberOfRows()),
            status: views.map(|v| v.status.stringValue().to_string()).unwrap_or_default(),
            results_shown: views.is_some_and(|v| v.results_shown()),
            field_focused: focused,
            can_replace_all: views.is_some_and(|v| v.replace_all.isEnabled()),
        }
    }

    /// The chevron that opens the replace field.
    pub fn replace_chevron(panel: &SearchPanel) -> Option<objc2::rc::Retained<objc2_app_kit::NSButton>> {
        panel.ivars().views.get().map(|views| views.replace_toggle.clone())
    }

    /// Opens the replace field, as its chevron does, and types `with` into it.
    pub fn replace_with(panel: &SearchPanel, with: &str) {
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
        let state = panel.ivars().state.borrow();
        (0..views.outline.numberOfRows()).find(|row| {
            let spot = views.outline.itemAtRow(*row).and_then(|i| spot_of(&i));
            match (spot, line) {
                (Some(Spot::File(path)), None) => state
                    .index
                    .get(&path)
                    .is_some_and(|i| state.groups[*i].relative == relative),
                (Some(Spot::Line(path, index)), Some(line)) => state.index.get(&path).is_some_and(|i| {
                    let group = &state.groups[*i];
                    group.relative == relative && group.lines.get(index).is_some_and(|l| l.line == line)
                }),
                _ => false,
            }
        })
    }

    /// Clicks the replace button of the row of `relative`, or of its line `line`, which shows while the pointer is on
    /// the row; whether the button was there and hidden before.
    pub fn click_replace(panel: &SearchPanel, relative: &str, line: Option<u64>) -> bool {
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
    pub fn replace_all(panel: &SearchPanel) {
        panel.ask_replace_all();
    }

    /// Types `query` into the field as the keyboard does.
    pub fn type_query(panel: &SearchPanel, query: &str) {
        if let Some(views) = panel.ivars().views.get() {
            views.field.setStringValue(&ns(query));
        }
        panel.schedule();
    }

    /// Switches `toggle` as a click on its button does.
    pub fn set(panel: &SearchPanel, toggle: Toggle, on: bool) {
        if let Some(views) = panel.ivars().views.get() {
            views.set_toggled(toggle, on);
        }
        panel.schedule();
    }

    /// Opens the file filters and types `include` and `exclude` into them.
    pub fn set_globs(panel: &SearchPanel, include: &str, exclude: &str) {
        if let Some(views) = panel.ivars().views.get() {
            views.set_globs_shown(true);
            views.include.setStringValue(&ns(include));
            views.exclude.setStringValue(&ns(exclude));
        }
        panel.schedule();
    }

    /// Opens the match on line `line` of the file `relative` as Return on its row does.
    pub fn open(panel: &SearchPanel, relative: &str, line: u64) -> bool {
        let Some(views) = panel.ivars().views.get() else {
            return false;
        };
        let row = (0..views.outline.numberOfRows()).find(|row| {
            let Some(Spot::Line(path, index)) = views.outline.itemAtRow(*row).and_then(|i| spot_of(&i)) else {
                return false;
            };
            let state = panel.ivars().state.borrow();
            state.index.get(&path).is_some_and(|i| {
                let group = &state.groups[*i];
                group.relative == relative && group.lines.get(index).is_some_and(|l| l.line == line)
            })
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
    pub fn escape(panel: &SearchPanel) {
        panel.focus();
        if let Some(views) = panel.ivars().views.get() {
            let field: &objc2_app_kit::NSControl = &views.field;
            panel.field_command(field, objc2::sel!(cancelOperation:));
        }
    }

    /// What VoiceOver reads for the row `row`.
    pub fn row_label(panel: &SearchPanel, row: isize) -> Option<String> {
        let views = panel.ivars().views.get()?;
        let view = views.outline.viewAtColumn_row_makeIfNecessary(0, row, true)?;
        super::rows::spoken(&view)
    }
}

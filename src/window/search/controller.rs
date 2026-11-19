//! The search panel's controller: reads the fields into a query, runs the searches the model starts on a thread of
//! their own, brings the list in line with the model and carries out opening and replacing.

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use ide_project::{Cancel, FileResult, SearchOptions};
use ide_ui_common::alert::{Alert, Role, first};
use objc2::rc::{Retained, Weak};
use objc2::runtime::Sel;
use objc2::{DefinedClass, MainThreadMarker, MainThreadOnly, msg_send, sel};
use objc2_app_kit::{NSControl, NSMenu, NSTableViewAnimationOptions};
use objc2_foundation::{NSIndexSet, NSMutableIndexSet, NSNotification};
use splat_i18n::t;

use super::model::{MenuCommand, Model, Outcome, Query, Started, Taken, Toggle, replaced_note};
use super::panel::{SearchIvars, SearchPanel, spot_of};
use super::{DEBOUNCE, FLUSH, REFRESH_DELAY, hierarchy, panel, replace, views, with_next_id, with_panels};
use crate::util::{main_after, main_soon, ns};
use crate::window::WindowController;

impl SearchPanel {
    /// The search of `controller`'s folder at `root`.
    pub(crate) fn new(mtm: MainThreadMarker, controller: &WindowController, root: &Path) -> Retained<Self> {
        let id = with_next_id(|next| {
            let id = next.get();
            next.set(id + 1);
            id
        });
        let ivars = SearchIvars::new(id, Weak::from(controller), Model::new(root.to_path_buf()));
        // SAFETY: NSObject's initializer, called once on the newly allocated object.
        let this: Retained<Self> = unsafe { msg_send![super(SearchPanel::alloc(mtm).set_ivars(ivars)), init] };
        let _ = this.ivars().views.set(views::Views::new(mtm, &this));
        let hierarchy = hierarchy::HierarchyPanel::new(mtm, &this);
        if let (Some(page), Some(tree)) = (this.page(), hierarchy.page()) {
            page.addSubview(tree);
            crate::util::pin_edges(tree, page);
        }
        let _ = this.ivars().hierarchy.set(hierarchy);
        with_panels(|panels| panels.borrow_mut().push(Weak::from(&*this)));
        if let Some(views) = this.ivars().views.get() {
            let came = Weak::from(&*this);
            let went = came.clone();
            ide_ui_common::panel::watch_view(
                &views.header,
                move || {
                    if let Some(panel) = came.load() {
                        panel.caught_up();
                    }
                },
                Some((
                    Duration::ZERO,
                    Box::new(move || {
                        if let Some(panel) = went.load()
                            && let Some(refresh) = panel.model().suspend()
                        {
                            panel.ivars().deferred_refresh.set(Some(refresh));
                            panel.ivars().deferred_show.set(true);
                        }
                    }),
                )),
            );
        }
        this
    }

    fn visible(&self) -> bool {
        self.ivars()
            .views
            .get()
            .is_some_and(|views| !ide_ui_common::panel::out_of_view(&views.header))
    }

    fn caught_up(&self) {
        if let Some(pending) = self.ivars().deferred_start.take() {
            self.start(pending);
        }
        if let Some(refresh) = self.ivars().deferred_refresh.take() {
            self.refresh(refresh);
        }
        if self.ivars().deferred_show.replace(false) {
            self.show();
        }
    }

    /// Shows `hierarchy` in place of the text search, which keeps what it holds.
    pub(crate) fn show_hierarchy(&self, hierarchy: ide_ui_lsp::Hierarchy) {
        if let Some(views) = self.ivars().views.get() {
            views.set_text_shown(false);
        }
        if let Some(panel) = self.ivars().hierarchy.get() {
            panel.show(hierarchy);
        }
    }

    /// Shows the text search again in place of a hierarchy.
    pub(super) fn show_text(&self) {
        if let Some(panel) = self.ivars().hierarchy.get() {
            panel.hide();
        }
        if let Some(views) = self.ivars().views.get() {
            views.set_text_shown(true);
        }
    }

    /// Puts the keyboard focus into the field, with what it holds selected, leaving a hierarchy on show.
    pub(crate) fn focus(&self) {
        if self.ivars().hierarchy.get().is_some_and(|h| h.showing()) {
            self.show_text();
        }
        let Some(views) = self.ivars().views.get() else { return };
        if let Some(window) = views.page.window() {
            window.makeFirstResponder(Some(&views.field));
        }
    }

    /// Ends a search that runs, for a window that closes or shows another folder.
    pub(crate) fn stop(&self) {
        self.model().stop();
    }

    /// Empties the field, which clears the matches, and keeps the focus in the field.
    pub(crate) fn clear(&self) {
        let Some(views) = self.ivars().views.get() else { return };
        views.field.setStringValue(&ns(""));
        self.schedule();
        self.focus();
    }

    pub(super) fn options(&self) -> SearchOptions {
        let Some(views) = self.ivars().views.get() else {
            return SearchOptions::new("");
        };
        let text = |field: &objc2_app_kit::NSTextField| field.stringValue().to_string();
        let (field, include, exclude) = (text(&views.field), text(&views.include), text(&views.exclude));
        Query {
            text: &field,
            case: views.toggled(Toggle::Case),
            word: views.toggled(Toggle::Word),
            regex: views.toggled(Toggle::Regex),
            include: &include,
            exclude: &exclude,
        }
        .options()
    }

    /// Starts a search for what the field and options hold once typing pauses; an empty field shows the tree at once.
    pub(super) fn schedule(&self) {
        let pending = self.model().changed();
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
        if !self.visible() && !options.pattern.is_empty() {
            let deferred = &self.ivars().deferred_start;
            deferred.set(Some(deferred.get().map_or(pending, |next| next.max(pending))));
            return;
        }
        let started = self.model().start(pending, &options);
        match started {
            Started::Nothing => {}
            Started::Cleared => self.show(),
            Started::Search { generation } => {
                self.show();
                self.spawn(generation, options, true);
            }
        }
    }

    /// Runs search `generation` with `options` on a thread of its own.
    fn spawn(&self, generation: u64, options: SearchOptions, progressive: bool) {
        let cancel = Cancel::new();
        let root = self.model().root().to_path_buf();
        let job = Job {
            id: self.ivars().id,
            generation,
            buffers: crate::editor::unsaved_texts(&root),
            root,
            options,
            progressive,
        };
        self.model().spawned(cancel.clone(), progressive);
        let spawned = crate::util::spawn("ide-search", move || {
            ide_process::priority::set_qos(ide_process::priority::Qos::UserInitiated);
            run(job, cancel);
        });
        if let Err(error) = spawned {
            crate::diag::report("starting a project search", error);
            self.model().spawn_failed();
        }
    }

    /// Searches again once files or unsaved texts below the folder rest, if matches are on show.
    pub(super) fn refresh_soon(&self) {
        let Some(refresh) = self.model().refresh_soon() else {
            return;
        };
        if !self.visible() {
            self.ivars().deferred_refresh.set(Some(refresh));
            return;
        }
        let id = self.ivars().id;
        main_after(REFRESH_DELAY, move || {
            if let Some(panel) = panel(id) {
                panel.refresh(refresh);
            }
        });
    }

    /// Searches again for the matches on show and puts the new ones in their place when the search ends.
    fn refresh(&self, refresh: u64) {
        if !self.visible() {
            let deferred = &self.ivars().deferred_refresh;
            deferred.set(Some(deferred.get().map_or(refresh, |pending| pending.max(refresh))));
            return;
        }
        let options = self.options();
        let generation = self.model().refresh(refresh, &options);
        if let Some(generation) = generation {
            self.spawn(generation, options, false);
        }
    }

    /// Takes `files` and maybe the end of search `generation`, unless another search started since.
    fn take(&self, generation: u64, files: Vec<FileResult>, outcome: Option<Outcome>) {
        let taken = self.model().take(generation, files, outcome);
        match taken {
            Taken::Nothing => {}
            Taken::Added(added) if self.ivars().synced.get() && self.visible() => self.add(&added),
            Taken::Added(_) | Taken::Replaced => self.show(),
        }
    }

    /// Puts the groups at `added` into the outline, which holds every other group already, and opens those the user
    /// did not fold; the other rows, their selection and folds stay as they are.
    fn add(&self, added: &[usize]) {
        let Some(views) = self.ivars().views.get() else { return };
        let ((status, problem), can_replace_all, unfolded) = {
            let model = self.read();
            (model.status(), model.can_replace_all(), model.unfolded_at(added))
        };
        views.replace_all.setEnabled(can_replace_all);
        views.set_status(&status, problem);
        if added.is_empty() {
            return;
        }
        let indexes = NSMutableIndexSet::new();
        for &at in added {
            indexes.addIndex(at);
        }
        views.outline.beginUpdates();
        // SAFETY: Nil stands for the top, whose children the data source answers for; the indexes are the new groups'
        // positions among them.
        unsafe {
            views.outline.insertItemsAtIndexes_inParent_withAnimation(
                &indexes,
                None,
                NSTableViewAnimationOptions::empty(),
            );
        }
        views.outline.endUpdates();
        for spot in &unfolded {
            let item = self.item(spot);
            // SAFETY: The item is one the data source hands out, kept alive by the panel.
            unsafe { views.outline.expandItem(Some(&item)) };
        }
    }

    /// Shows the tree or the matches with their status, keeping the selected row where it can.
    pub(super) fn show(&self) {
        let Some(views) = self.ivars().views.get() else { return };
        if !self.visible() {
            self.ivars().deferred_show.set(true);
            self.ivars().synced.set(false);
            return;
        }
        // The outline calls back into the panel while it reloads and expands, so the model is not held meanwhile.
        let (searching, (status, problem), unfolded, can_replace_all) = {
            let model = self.read();
            (
                model.searching(),
                model.status(),
                model.unfolded(),
                model.can_replace_all(),
            )
        };
        views.show_results(searching);
        views.replace_all.setEnabled(can_replace_all);
        if !searching {
            self.forget_items();
            self.ivars().synced.set(false);
            return;
        }
        views.set_status(&status, problem);
        let selected = views
            .outline
            .itemAtRow(views.outline.selectedRow())
            .and_then(|i| spot_of(&i));
        self.forget_items();
        views.outline.reloadData();
        self.ivars().synced.set(true);
        views.outline.sizeLastColumnToFit();
        for spot in &unfolded {
            let item = self.item(spot);
            // SAFETY: The item is one the data source hands out, kept alive by the panel.
            unsafe { views.outline.expandItem(Some(&item)) };
        }
        let item = selected
            .filter(|spot| self.read().has(spot))
            .map(|spot| self.item(&spot));
        if let Some(item) = item {
            // SAFETY: The item is one the data source hands out, kept alive by the panel.
            let row = unsafe { views.outline.rowForItem(Some(&item)) };
            if let Ok(row) = usize::try_from(row) {
                views
                    .outline
                    .selectRowIndexes_byExtendingSelection(&NSIndexSet::indexSetWithIndex(row), false);
            }
        }
    }

    pub(super) fn folded(&self, notification: &NSNotification, collapsed: bool) {
        let item = notification
            .userInfo()
            .and_then(|info| info.objectForKey(&ns("NSObject")));
        if let Some(super::model::Spot::File(path)) = item.and_then(|item| spot_of(&item)) {
            self.model().folded(path, collapsed);
        }
    }

    /// Opens the file of `row` and selects its match, or the file's first match; `focus` moves the keyboard focus
    /// to the editor, else it stays in the list.
    pub(super) fn open_row(&self, row: isize, focus: bool) {
        let Some(views) = self.ivars().views.get() else { return };
        let Some(spot) = views.outline.itemAtRow(row).and_then(|i| spot_of(&i)) else {
            return;
        };
        let target = self.read().target(&spot);
        let (Some((path, line, columns)), Some(controller)) = (target, self.ivars().controller.load()) else {
            return;
        };
        crate::editor::reveal(&controller, &path, line, columns, focus);
        if !focus && let Some(window) = views.page.window() {
            window.makeFirstResponder(Some(&views.outline));
        }
    }

    pub(super) fn is_replace_field(&self, notification: &NSNotification) -> bool {
        let (Some(views), Some(object)) = (self.ivars().views.get(), notification.object()) else {
            return false;
        };
        std::ptr::eq(
            Retained::as_ptr(&object).cast::<()>(),
            Retained::as_ptr(&views.replace).cast(),
        )
    }

    pub(super) fn fill_menu(&self, menu: &NSMenu) {
        menu.removeAllItems();
        let Some(views) = self.ivars().views.get() else { return };
        let row = views.outline.clickedRow();
        self.model().menu_row = row;
        let Some(spot) = views.outline.itemAtRow(row).and_then(|i| spot_of(&i)) else {
            return;
        };
        let mtm = self.mtm();
        for (title, command) in Model::menu(&spot, views.replace_shown()) {
            let action = match command {
                MenuCommand::Open => sel!(ideSearchMenuOpen:),
                MenuCommand::Replace => sel!(ideSearchMenuReplace:),
            };
            let entry = crate::util::item(mtm, title, Some(action), "");
            crate::util::set_target(&entry, self);
            menu.addItem(&entry);
        }
    }

    /// Asks before replacing every match on show, naming how many there are.
    pub(super) fn ask_replace_all(&self) {
        let Some(views) = self.ivars().views.get() else { return };
        let Some(window) = views.page.window() else { return };
        if !self.read().can_replace_all() {
            return;
        }
        let with = views.replace.stringValue().to_string();
        let (question, detail) = self.read().replace_question(&with);
        let id = self.ivars().id;
        Alert::new(self.mtm(), &question)
            .informative(&detail)
            .button(t("Ersetzen"), Role::Plain)
            .button(t("Abbrechen"), Role::Plain)
            .sheet(&window, move |response| {
                if first(response)
                    && let Some(panel) = panel(id)
                {
                    let files = panel.read().files();
                    panel.replace(files, super::model::Only::All);
                }
            });
    }

    /// Replaces the matches of the file or the line in `row`.
    pub(super) fn replace_at(&self, row: isize) {
        let Some(views) = self.ivars().views.get() else { return };
        let Some(spot) = views.outline.itemAtRow(row).and_then(|i| spot_of(&i)) else {
            return;
        };
        let replacement = self.read().replacement(&spot);
        if let Some((path, only)) = replacement {
            self.replace(vec![path], only);
        }
    }

    /// Replaces in `files` with what the replace field holds, then searches again and says what it did.
    fn replace(&self, files: Vec<PathBuf>, only: super::model::Only) {
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
        if let (Some(note), Some(controller)) =
            (replaced_note(done.replaced, done.files), self.ivars().controller.load())
        {
            controller.show_notice(&note);
        }
        if !done.problems.is_empty()
            && let Some(window) = self.ivars().views.get().and_then(|v| v.page.window())
        {
            Alert::new(self.mtm(), t("Nicht alle Treffer wurden ersetzt."))
                .warning()
                .informative(&done.problems.join("\n"))
                .sheet(&window, |_| {});
        }
        let refresh = self.model().next_refresh();
        self.refresh(refresh);
    }

    /// Arrow down moves into the matches, Return opens the selected or first one, Esc empties a field with a query.
    pub(super) fn field_command(&self, control: &NSControl, command: Sel) -> bool {
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

/// What one search runs on: the panel, its generation, the folder, the options and the unsaved texts of open editors.
struct Job {
    id: u64,
    generation: u64,
    root: PathBuf,
    options: SearchOptions,
    buffers: Vec<(PathBuf, ide_document::Text)>,
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
    if cancel.is_cancelled() {
        return;
    }
    let mut batch = Vec::new();
    let mut flushed = Instant::now();
    let buffers: Vec<_> = buffers
        .into_iter()
        .map(|(path, text)| (path, text.to_string()))
        .collect();
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

//! The Git panel's controller for what it shows: attaching it to a window, reading git on the git lane when the folder
//! changed, handing the result to the model and bringing the views in line.

use std::path::Path;
use std::time::Duration;

use ide_git::StatusOptions;

use super::model::{Loaded, Phase, RowKey, Section, Start, StashRead};
use super::panel::{GitPanel, with_panel};
use super::text::{Problem, problem};
use super::work;
use ide_ui_common::util::main_after;
use ide_ui_common::window::{Panel, Window};

/// How long changes on disk gather before the panel reads git again.
const DEBOUNCE: Duration = Duration::from_millis(300);

/// How often the ages of the commits on view are said again.
const AGE_INTERVAL: Duration = Duration::from_secs(60);

impl GitPanel {
    /// Builds the panel for `controller`'s folder, puts it on the Git page and starts reading the repository.
    pub fn attach(controller: &Window) {
        let Some(root) = controller.project_root() else { return };
        let root = root.canonicalize().unwrap_or(root);
        crate::host::state().panels.detach(*controller);
        let panel = GitPanel::new(controller.mtm(), controller, root);
        controller.set_panel_page(Panel::Git, &panel.ui().page);
        controller.set_panel_accessory(Panel::Git, &panel.ui().header);
        panel.update_placeholder();
        crate::host::state().panels.insert(panel.id(), panel.clone());
        panel.start_refresh();
        panel.schedule_ages();
    }

    /// Says the ages of the commits again every `AGE_INTERVAL` while the panel shows, until its window closes.
    fn schedule_ages(&self) {
        let id = self.id();
        main_after(AGE_INTERVAL, move || {
            with_panel(id, |panel| {
                panel.refresh_ages();
                panel.schedule_ages();
            });
        });
    }

    /// Reloads the commit rows, whose views compute their age when they are made.
    pub fn refresh_ages(&self) {
        let page = &self.ui().page;
        let showing = page.window().is_some_and(|w| w.isVisible()) && !page.isHiddenOrHasHiddenAncestor();
        if !showing || self.read().model.commits.is_empty() {
            return;
        }
        let item = self.item_for(&RowKey::Section(Section::Commits));
        // SAFETY: The item is one the data source hands to this view, kept alive by the panel.
        unsafe { self.ui().outline.reloadItem_reloadChildren(Some(&item), true) };
    }

    /// Reads git again once changes on disk have settled.
    pub fn schedule_refresh(&self) {
        if self.state().schedule_refresh() {
            let id = self.id();
            main_after(DEBOUNCE, move || with_panel(id, GitPanel::start_refresh));
        }
    }

    /// Reads git now, or right after the read that is running.
    pub fn start_refresh(&self) {
        let trusted = self.controller().is_some_and(|c| c.is_trusted());
        let start = self.state().start_refresh(trusted);
        let limit = match start {
            Start::Untrusted => return self.show_phase(),
            Start::Queued => return,
            Start::Read { limit } => limit,
        };
        let root = self.root().to_path_buf();
        let id = self.id();
        let queued = work::local(
            move || load(&root, limit),
            move |loaded| with_panel(id, |p| p.loaded(loaded)),
        );
        if !queued {
            self.state().refresh_not_started();
            self.schedule_refresh();
        }
    }

    fn loaded(&self, loaded: Option<Loaded>) {
        let again = self.state().refresh_finished();
        let loaded =
            loaded.unwrap_or_else(|| Loaded::Failed(Problem::plain(splat_i18n::t("Git ist unerwartet abgebrochen."))));
        let changed = self.state().apply(loaded);
        if changed {
            self.show_phase();
        }
        if again {
            self.schedule_refresh();
        }
        if let Some(controller) = self.controller()
            && self.read().phase == Phase::Ready
        {
            let trees = self.read().model.worktrees.clone();
            super::diff::follow_trees(&controller, &trees);
        }
        super::diff::refresh_for(self.root());
    }

    /// Puts every view in line with the state.
    pub fn show_phase(&self) {
        let (phase, count) = {
            let state = self.read();
            (state.phase.clone(), state.model.changed_files())
        };
        self.ui().show_phase(self, &phase);
        if phase == Phase::Ready {
            self.update_header();
            self.reload_outline();
            self.update_busy();
            self.update_banner();
            self.update_problem();
        }
        self.refresh_toolbar();
        if let Some(controller) = self.controller() {
            controller.set_symbol_count(Panel::Git, count);
        }
    }

    /// Shows `problem` above the list, or hides it.
    pub fn set_problem(&self, problem: Option<Problem>) {
        self.state().problem = problem;
        self.update_problem();
    }

    fn update_problem(&self) {
        let problem = self.read().problem.clone();
        self.ui().show_problem(problem.as_ref());
    }

    pub fn report(&self, error: &ide_git::Error) {
        if matches!(error, ide_git::Error::Cancelled) {
            return;
        }
        let problem = problem(error);
        let hook = problem.hook;
        self.set_problem(Some(problem));
        if hook {
            self.show_problem_output();
        }
    }
}

/// Everything the panel shows, with up to `limit` commits, read on the git lane.
fn load(root: &Path, limit: usize) -> Loaded {
    let Some(git) = work::git() else { return Loaded::NoGit };
    if git.program().is_none() {
        return Loaded::NoGit;
    }
    let repo = match git.discover(root) {
        Ok(repo) => repo,
        Err(ide_git::Error::NotARepository { .. }) => return Loaded::NotRepository,
        Err(ide_git::Error::GitMissing) => return Loaded::NoGit,
        Err(error) => return Loaded::Failed(problem(&error)),
    };
    let status = match repo.status(StatusOptions::default()) {
        Ok(status) => status,
        Err(error) => return Loaded::Failed(problem(&error)),
    };
    let branches = repo.branches().unwrap_or_default();
    let log = repo
        .history(limit, status.branch.upstream.as_deref())
        .unwrap_or_default();
    let operation = repo.operation().unwrap_or(None);
    let worktrees = crate::worktrees::read(&git, &repo, root, &branches);
    let stash = Box::new(StashRead {
        entries: repo.stashes().unwrap_or_default(),
        apply: repo.stash_apply().unwrap_or(None),
    });
    Loaded::Ready {
        repo,
        status: Box::new(status),
        branches,
        log,
        operation,
        worktrees,
        stash,
    }
}

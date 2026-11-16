//! The Leitstand's views (`ide_ui_factory`) as the app reaches them: the panel of a window's folder, the items of the
//! File menu, the kinds the layout keeps, and the crate's `Host`.
//!
//! STUB: every `Host` method marked "STUB" below answers without the orchestrator in `src/leitstand/`, which the
//! integrator wires in: models come back empty, commands are refused as unknown, nothing is posted or stored.

use std::path::Path;

use ide_core::explain::Explained;
use ide_factory::options::NewOrder;
use ide_factory::panel::{Board, Fleet, Panel};
use ide_factory::workflow::{UserDefaults, WORKFLOW_FILE};
use ide_factory::{InputKind, OrderId, Refusal, Refused};
use ide_ui_common::window::WindowId;
use ide_ui_factory::model::detail::Detail;
use ide_ui_factory::model::settings::ProjectFile;
use ide_ui_factory::model::sheet::SheetContext;
use objc2::MainThreadMarker;
use objc2::rc::Retained;
use objc2_app_kit::NSMenuItem;

use crate::host::AppHost;
use crate::window::WindowController;

/// Puts the Leitstand panel of the window's folder in place.
pub(crate) fn attach(controller: &WindowController) {
    ide_ui_factory::attach(controller.handle(), controller.context().factory_ui());
}

/// The window's folder closes: its Leitstand panel goes.
pub(crate) fn window_closed(controller: &WindowController) {
    ide_ui_factory::window_closed(controller.handle(), controller.context().factory_ui());
}

/// "Neuer Auftrag …" and the submenu "Auftrag" of the File menu.
pub(crate) fn file_items(
    mtm: MainThreadMarker,
    key: &dyn Fn(&str) -> String,
    state: &std::rc::Rc<ide_ui_factory::State>,
) -> Vec<Retained<NSMenuItem>> {
    ide_ui_factory::file_items(mtm, key, state)
}

/// Opens the Leitstand tab that came back with the layout (E64).
pub(crate) fn reopen_leitstand(controller: &WindowController) -> bool {
    ide_ui_factory::show_leitstand(controller.handle(), controller.context().factory_ui())
}

/// Opens the tab of the order `order` that came back with the layout (E64); its order may be gone by now, and the
/// tab then says so.
pub(crate) fn reopen_order(controller: &WindowController, order: &str) -> bool {
    ide_ui_factory::open_order(
        controller.handle(),
        controller.context().factory_ui(),
        &OrderId(order.to_string()),
    )
}

impl ide_ui_factory::Host for AppHost {
    fn panel(&self, _root: &Path) -> Panel {
        // STUB: the factory of `root` from `src/leitstand/`, `Panel::new(&factory, now)`.
        Panel::default()
    }

    fn board(&self, _root: &Path) -> Option<Board> {
        // STUB: `Board::new(&factory, now)`.
        None
    }

    fn fleet(&self, _root: &Path) -> Option<Fleet> {
        // STUB: `Fleet::new(&factory, &quotas, now, day_start, worktree_bytes)` with the app state's quota book.
        None
    }

    fn order(&self, _root: &Path, _order: &OrderId) -> Option<Detail> {
        // STUB: `Detail::new(factory.order(order)?, now_wall, sections)` with the sections of the order's diff.
        None
    }

    fn sheet(&self, _root: &Path) -> SheetContext {
        // STUB: the factory's stage and rules, `git status` of the working copy and the paths' readiness (E58).
        SheetContext::default()
    }

    fn new_order(&self, _root: &Path, _order: NewOrder) -> Result<OrderId, Vec<Refusal>> {
        // STUB: `InputKind::NewOrder` with an id of the data folder and the base branch and commit.
        Err(vec![Refusal {
            order: None,
            why: Refused::NotNow,
        }])
    }

    fn send(&self, _root: &Path, command: InputKind) -> Vec<Refusal> {
        // STUB: `Factory::handle` of the folder's factory, its log written and its effects run; `step.refusals`.
        let order = match &command {
            InputKind::Approve(id)
            | InputKind::DiffOpened(id)
            | InputKind::BackToBuilder(id, _)
            | InputKind::ChangeCriteria(id, _)
            | InputKind::ByHand(id)
            | InputKind::BackToLeitstand(id)
            | InputKind::Abort(id)
            | InputKind::Continue(id)
            | InputKind::Accept { order: id, .. }
            | InputKind::Answer { order: id, .. } => Some(id.clone()),
            InputKind::Resume(id) => id.clone(),
            _ => None,
        };
        vec![Refusal {
            order,
            why: Refused::UnknownOrder,
        }]
    }

    fn open_diff(&self, _window: WindowId, _root: &Path, _order: &OrderId) {
        // STUB: `ide_ui_git`'s diff tab of the order's worktree against its base (C55).
    }

    fn open_session(&self, _window: WindowId, _root: &Path, _order: &OrderId) {
        // STUB: the session of the order's current role from the session registry, read only in stage 1 (E175).
    }

    fn notify(&self, _root: &Path, _notification: ide_ui_factory::Notification) {
        // STUB: `notify::post` once it moved to `ide_ui_common` (F76), never while a self-test runs (R12).
    }

    fn defaults(&self) -> UserDefaults {
        // STUB: the user's defaults from a `PerMac` settings key (E113, E183).
        UserDefaults::default()
    }

    fn set_defaults(&self, _defaults: UserDefaults) {
        // STUB: stores the defaults and hands the new rules to every folder's factory.
    }

    fn project_file(&self, root: &Path) -> ProjectFile {
        let text = std::fs::read_to_string(root.join(WORKFLOW_FILE)).ok();
        // STUB: whether git ignores the file, from `ide_git`.
        ProjectFile::new(text, false, self.defaults())
    }

    fn save_project_file(&self, root: &Path, text: &str) -> Result<(), Explained> {
        let path = root.join(WORKFLOW_FILE);
        let written = path
            .parent()
            .map_or(Ok(()), std::fs::create_dir_all)
            .and_then(|()| std::fs::write(&path, text));
        // STUB: the folder's factory reads its rules again (E190).
        written.map_err(|error| Explained::of(&error))
    }
}

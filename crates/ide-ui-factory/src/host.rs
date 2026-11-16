//! The app as the Leitstand's views reach it: the orders of a folder as models, the user's commands, the diff of an
//! order, the user's defaults and the project's workflow file, and notifications (E99).

use std::cell::{OnceCell, RefCell};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use ide_core::explain::Explained;
use ide_factory::options::NewOrder;
use ide_factory::panel::{Board, Fleet, Panel};
use ide_factory::workflow::rules::Loosening;
use ide_factory::workflow::{UserDefaults, WorkflowError};
use ide_factory::{InputKind, OrderId, Refusal, RequestId, RestoreProblem};
use ide_ui_common::registry::Registry;
use ide_ui_common::window::WindowId;
use objc2::rc::{Retained, Weak};

use crate::model::detail::Detail;
use crate::model::notices::Notices;
use crate::model::settings::ProjectFile;
use crate::model::sheet::SheetContext;

/// The Leitstand's panels, tabs and menu target of every window, and what it last told the user per folder.
#[derive(Default)]
pub struct State {
    pub(crate) panels: Registry<Retained<crate::panel::FactoryPanel>>,
    pub(crate) boards: Registry<Weak<crate::board::BoardTab>>,
    pub(crate) orders: Registry<Weak<crate::order::OrderTab>>,
    pub(crate) commands: OnceCell<Retained<crate::commands::OrderCommands>>,
    pub(crate) notices: RefCell<HashMap<PathBuf, Notices>>,
}

/// A notification about an order (E175), which the app posts as its other notifications (I9), never as a Dock
/// count (E72).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Notification {
    /// The order.
    pub order: OrderId,
    /// The order's title.
    pub title: String,
    /// What happened, in the user's language.
    pub body: String,
}

/// What the user does with an order's specification at G1 (E178), in the view the app's host builds.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SpecAction {
    /// Approve it.
    Approve,
    /// Send it back to the planner with these comments, each under the passage it quotes.
    Comment(String),
    /// Throw it away, which aborts the order after asking.
    Discard,
}

/// What the Leitstand needs of the app's orchestrator and settings; the rest of a window it reaches through
/// `ide_ui_common::window`. Every folder is a window's project root.
pub trait Host {
    /// The orders of `root` as the panel lists them; empty while the folder has none.
    fn panel(&self, root: &Path) -> Panel;
    /// Every order of `root` as the Leitstand tab's strip and table show them, before the tab's filter and sort;
    /// `None` while the folder has no factory yet.
    fn board(&self, root: &Path) -> Option<Board>;
    /// The fleet line above the table; `None` while the folder has no factory yet.
    fn fleet(&self, root: &Path) -> Option<Fleet>;
    /// The order `order` of `root` as its tab shows it, built with [`Detail::new`] from the factory's order and the
    /// sections of its diff; `None` for an order the folder does not know.
    fn order(&self, root: &Path, order: &OrderId) -> Option<Detail>;
    /// What the sheet "Neuer Auftrag" checks against: the stage, the rules, whether the working copy has uncommitted
    /// changes, and what the app knows about each path (E185).
    fn sheet(&self, root: &Path) -> SheetContext;
    /// Starts the order `order` in `root`, with an id and base commit the app chooses; its id, or why the factory
    /// refused it.
    fn new_order(&self, root: &Path, order: NewOrder) -> Result<OrderId, Vec<Refusal>>;
    /// Hands the user's command `command`, one of `InputKind`'s user commands, to the factory of `root`; what it
    /// refused, empty when it took the command.
    fn send(&self, root: &Path, command: InputKind) -> Vec<Refusal>;
    /// Opens the diff of the order `order` against its base in `window`, in the diff tab of its worktree (C55).
    fn open_diff(&self, window: WindowId, root: &Path, order: &OrderId);
    /// Opens the session of `order`'s current role in `window` as an AI tab that writes into it visibly (E184), or,
    /// where the AI tab does not follow the session's path yet, the order's worktree (E175).
    fn open_session(&self, window: WindowId, root: &Path, order: &OrderId);
    /// The specification of `order` as a document with inline comments for `window`, while it waits for G1 (E178,
    /// F63); `then` hears what the user does with it. `None` while there is none to show.
    fn spec_view(
        &self,
        window: WindowId,
        root: &Path,
        order: &OrderId,
        then: std::rc::Rc<dyn Fn(SpecAction)>,
    ) -> Option<Retained<objc2_app_kit::NSView>>;
    /// Posts `notification` about an order of `root` (E175, I9); nothing while a self-test runs (R12).
    fn notify(&self, root: &Path, notification: Notification);
    /// The user's defaults for all projects (E183), as stored per Mac (E113).
    fn defaults(&self) -> UserDefaults;
    /// Stores `defaults` as the user's defaults and hands them to every factory.
    fn set_defaults(&self, defaults: UserDefaults);
    /// The workflow file of `root` as the project dialog edits it (E183).
    fn project_file(&self, root: &Path) -> ProjectFile;
    /// Writes `text` as the workflow file of `root` and has the factory read its rules again; the words of the
    /// failure.
    fn save_project_file(&self, root: &Path, text: &str) -> Result<(), Explained>;
    /// Allows or refuses the permission request `request` of the order's session that waits for the user, since
    /// without O1 requests beyond an order's bounds reach the user (C72); what the factory refused.
    fn decide(&self, root: &Path, order: &OrderId, request: &RequestId, allow: bool) -> Vec<Refusal>;
    /// What the user should know about the folder beyond its orders: errors of the workflow file, loosenings that
    /// wait for confirmation (E190) and logs that could not be read at the start (E167).
    fn notices(&self, root: &Path) -> FolderNotices;
    /// Confirms `loosening` for `root` on this Mac (E190).
    fn confirm_loosening(&self, root: &Path, loosening: &Loosening);
}

/// What the Leitstand tab shows above the fleet line.
#[derive(Debug, Clone, Default)]
pub struct FolderNotices {
    /// What is wrong with the workflow file; while it has errors, no new station starts.
    pub errors: Vec<WorkflowError>,
    /// The loosenings the workflow file asks for that this Mac has not confirmed.
    pub loosenings: Vec<Loosening>,
    /// Logs that could not be read in full at the start.
    pub problems: Vec<RestoreProblem>,
}

static HOST: OnceLock<&'static (dyn Host + Sync)> = OnceLock::new();

/// Makes `host` the app the Leitstand belongs to; once, before the first window opens.
pub fn install(host: &'static (dyn Host + Sync)) {
    let _ = HOST.set(host);
}

pub(crate) fn host() -> &'static (dyn Host + Sync) {
    *HOST
        .get()
        .expect("the app installs its host before the first Leitstand")
}

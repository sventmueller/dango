//! The Leitstand's views (`ide_ui_factory`) as the app reaches them: the panel of a window's folder, the items of the
//! File menu, the kinds the layout keeps, and the crate's `Host`, answered by the runtime in `crate::leitstand`.

use std::path::Path;

use ide_core::explain::Explained;
use ide_factory::options::NewOrder;
use ide_factory::panel::{Board, Fleet, Panel};
use ide_factory::workflow::{UserDefaults, WORKFLOW_FILE};
use ide_factory::{Condition, InputKind, OrderId, Refusal, Refused, WaitReason};
use ide_ui_common::window::WindowId;
use ide_ui_factory::model::detail::Detail;
use ide_ui_factory::model::settings::ProjectFile;
use ide_ui_factory::model::sheet::SheetContext;
use objc2::MainThreadMarker;
use objc2::rc::Retained;
use objc2_app_kit::NSMenuItem;

use crate::leitstand::{Back, Command};
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

/// The Leitstand's views as the runtime answers them; it holds the way back to the app context the runtime binds,
/// so the views look nothing up (E143).
pub(crate) struct Orders(Back);

impl Orders {
    /// Installs the views' host once the runtime is bound to the app context.
    pub(crate) fn install(back: Back) {
        ide_ui_factory::install(Box::leak(Box::new(Orders(back))));
    }
}

/// The runtime's command for a user command the views send, if it is one.
fn command(kind: InputKind) -> Option<Command> {
    Some(match kind {
        InputKind::Approve(id) => Command::Approve(id),
        InputKind::CommentSpec(id, text) => Command::CommentSpec(id, text),
        InputKind::DiffOpened(id) => Command::DiffOpened(id),
        InputKind::Accept {
            order,
            fingerprint,
            rejected,
        } => Command::Accept {
            order,
            fingerprint,
            rejected,
        },
        InputKind::BackToBuilder(id, text) => Command::BackToBuilder(id, text),
        InputKind::ChangeCriteria(id, text) => Command::ChangeCriteria(id, text),
        InputKind::ChangeOption(id, change) => Command::ChangeOption(id, change),
        InputKind::ByHand(id) => Command::ByHand(id),
        InputKind::BackToLeitstand(id) => Command::BackToLeitstand(id),
        InputKind::Abort(id) => Command::Abort(id),
        InputKind::Resume(id) => Command::Resume(id),
        InputKind::Continue(id) => Command::Continue(id),
        InputKind::Answer { order, request, text } => Command::Answer { order, request, text },
        InputKind::ConfirmGates(hash) => Command::ConfirmGates(hash),
        _ => return None,
    })
}

/// The user's defaults for orders (E183).
pub(crate) fn defaults() -> UserDefaults {
    ide_core::settings::Settings::load_user().get(&crate::leitstand::DEFAULTS)
}

/// Keeps `defaults` as the user's; the runtime hears of the written setting and reads every folder's rules again.
pub(crate) fn set_defaults(defaults: &UserDefaults) {
    if let Err(error) = ide_core::settings::Settings::load_user().set(&crate::leitstand::DEFAULTS, defaults) {
        ide_core::diag::report("saving the defaults of orders", error);
    }
}

/// "Weiter" on an order that waits for changed gate commands confirms the folder's current set (E190).
fn gates_confirmed(leitstand: &crate::leitstand::Leitstand, root: &Path, command: Command) -> Command {
    let Command::Continue(order) = &command else {
        return command;
    };
    let waits = leitstand
        .with_order(root, order, |o| {
            *o.condition() == Condition::Waiting(WaitReason::GateCommandsChanged)
        })
        .unwrap_or(false);
    match leitstand.gates(root) {
        Some((hash, false)) if waits => Command::ConfirmGates(hash),
        _ => command,
    }
}

fn not_now() -> Vec<Refusal> {
    vec![Refusal {
        order: None,
        why: Refused::NotNow,
    }]
}

impl ide_ui_factory::Host for Orders {
    fn panel(&self, root: &Path) -> Panel {
        self.0
            .with(|cx| cx.leitstand().panel(root))
            .flatten()
            .unwrap_or_default()
    }

    fn board(&self, root: &Path) -> Option<Board> {
        self.0.with(|cx| cx.leitstand().board(root)).flatten()
    }

    fn fleet(&self, root: &Path) -> Option<Fleet> {
        self.0.with(|cx| cx.leitstand().fleet(root)).flatten()
    }

    fn order(&self, root: &Path, order: &OrderId) -> Option<Detail> {
        self.0.with(|cx| cx.leitstand().detail(root, order)).flatten()
    }

    fn sheet(&self, root: &Path) -> SheetContext {
        self.0.with(|cx| cx.leitstand().sheet(root)).unwrap_or_default()
    }

    fn new_order(&self, root: &Path, order: NewOrder) -> Result<OrderId, Vec<Refusal>> {
        self.0
            .with(|cx| cx.leitstand().command(cx, root, Command::New(order)))
            .flatten()
            .ok_or_else(not_now)
    }

    fn send(&self, root: &Path, kind: InputKind) -> Vec<Refusal> {
        let Some(command) = command(kind) else {
            return not_now();
        };
        self.0
            .with(|cx| {
                let leitstand = cx.leitstand();
                let command = gates_confirmed(leitstand, root, command);
                leitstand.command(cx, root, command);
                leitstand.refusals(root)
            })
            .unwrap_or_default()
    }

    fn notices(&self, root: &Path) -> ide_ui_factory::FolderNotices {
        self.0
            .with(|cx| {
                let leitstand = cx.leitstand();
                ide_ui_factory::FolderNotices {
                    errors: leitstand.errors(root),
                    loosenings: leitstand.pending_loosenings(root),
                    problems: leitstand.restore_problems(root),
                }
            })
            .unwrap_or_default()
    }

    fn confirm_loosening(&self, root: &Path, loosening: &ide_factory::workflow::rules::Loosening) {
        self.0.with(|cx| {
            cx.leitstand()
                .command(cx, root, Command::ConfirmLoosening(loosening.clone()))
        });
    }

    fn decide(&self, root: &Path, order: &OrderId, request: &ide_factory::RequestId, allow: bool) -> Vec<Refusal> {
        self.0
            .with(|cx| {
                let decide = Command::Decide {
                    order: order.clone(),
                    request: request.clone(),
                    allow,
                };
                cx.leitstand().command(cx, root, decide);
                cx.leitstand().refusals(root)
            })
            .unwrap_or_default()
    }

    fn open_diff(&self, window: WindowId, root: &Path, order: &OrderId) {
        self.0.with(|cx| {
            let controller = cx.windows().get(window.get());
            let tree = cx.leitstand().worktree(root, order);
            if let (Some(controller), Some(tree)) = (controller, tree) {
                controller.activate_panel(ide_ui_common::window::Panel::Git);
                ide_ui_git::reveal_tree(cx.git(), &*controller, &tree);
            }
        });
    }

    fn open_session(&self, window: WindowId, root: &Path, order: &OrderId) {
        // Until the AI tab drives the runtime's sessions (B37b), the order's worktree is what there is to show.
        self.open_diff(window, root, order);
    }

    fn notify(&self, _root: &Path, _notification: ide_ui_factory::Notification) {
        // The runtime posts each notification from the factory's own effect; the views' would arrive twice.
    }

    fn defaults(&self) -> UserDefaults {
        defaults()
    }

    fn set_defaults(&self, defaults: UserDefaults) {
        set_defaults(&defaults);
    }

    fn project_file(&self, root: &Path) -> ProjectFile {
        let text = std::fs::read_to_string(root.join(WORKFLOW_FILE)).ok();
        ProjectFile::new(text, false, self.defaults())
    }

    fn save_project_file(&self, root: &Path, text: &str) -> Result<(), Explained> {
        let path = root.join(WORKFLOW_FILE);
        path.parent()
            .map_or(Ok(()), std::fs::create_dir_all)
            .and_then(|()| std::fs::write(&path, text))
            .map_err(|error| Explained::of(&error))?;
        self.0.with(|cx| cx.leitstand().reload_rules(root));
        Ok(())
    }
}

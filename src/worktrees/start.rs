//! Starting a thread in a worktree of its own from the "+" menu: a sheet asks for its title, the worktree is made on
//! the git lane, then an AI tab opens whose session works there.

use objc2::MainThreadOnly;
use objc2_app_kit::{NSAlert, NSAlertFirstButtonReturn, NSTextField};
use objc2_foundation::{NSPoint, NSRect, NSSize};
use splat_i18n::t;

use super::ThreadTree;
use crate::util::ns;
use crate::window::WindowController;
use ide_ui_agent::{AcpAgent, Agent};

/// The agents a worktree thread can run, in the order of the "In eigenem Worktree" menu, and whether each can be
/// opened now, as the agents of the "+" menu are: the own agent once it has a model. The menu, its check and the
/// start all read this list.
pub fn agents() -> Vec<(Agent, bool)> {
    let presets = AcpAgent::presets()
        .into_iter()
        .zip(ide_ui_agent::preset_menu())
        .filter(|(agent, _)| agent.offered())
        .map(|(agent, (_, usable))| (Agent::Acp(agent), usable));
    let own = (Agent::Own, ide_ui_agent::own_agent::default_choice().is_some());
    std::iter::once((Agent::Claude, true))
        .chain(presets)
        .chain(std::iter::once(own))
        .collect()
}

/// Whether the window can start a worktree thread: its folder is trusted and a repository with a commit.
pub fn can_start(controller: &WindowController) -> bool {
    controller.is_trusted() && ide_ui_git::has_commit(controller)
}

/// Asks for the title of a thread of the agent at `index` of [`agents`], then makes its worktree and opens the
/// thread there.
pub fn ask_and_open(controller: &WindowController, index: usize) {
    let Some(window) = controller.window() else { return };
    let Some((agent, true)) = agents().into_iter().nth(index) else {
        return;
    };
    if !can_start(controller) {
        return;
    }
    let mtm = controller.mtm();
    let alert = NSAlert::new(mtm);
    alert.setMessageText(&ns(t("Thread in eigenem Worktree")));
    alert.setInformativeText(&ns(t(
        "Der Thread arbeitet auf einem eigenen Branch in einer eigenen Kopie des Repositorys. Deine Arbeitskopie bleibt unberührt, bis du seine Änderungen im Git-Panel übernimmst.",
    )));
    alert.addButtonWithTitle(&ns(t("Starten")));
    alert.addButtonWithTitle(&ns(t("Abbrechen")));
    let field = NSTextField::textFieldWithString(&ns(""), mtm);
    field.setPlaceholderString(Some(&ns(t("Titel, etwa „Login reparieren“"))));
    field.setFrame(NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(300.0, 24.0)));
    alert.setAccessoryView(Some(&field));
    alert.window().setInitialFirstResponder(Some(&field));
    let target = objc2::rc::Weak::from(controller);
    crate::ci_panel::sheet(&alert, &window, move |response| {
        let typed = field.stringValue().to_string();
        if response == NSAlertFirstButtonReturn
            && let Some(controller) = target.load()
        {
            start(&controller, typed.trim(), &agent);
        }
    });
}

fn start(controller: &WindowController, title: &str, agent: &Agent) {
    let title = if title.is_empty() {
        t("Worktree-Thread").to_string()
    } else {
        title.to_string()
    };
    ide_ui_git::create_tree(controller, title, agent.id().to_string());
}

/// Opens an AI tab whose session runs the thread's agent in the worktree of `tree`.
pub fn open_thread(controller: &WindowController, tree: &ThreadTree) {
    let agent = Agent::of_thread(&tree.agent);
    #[cfg(feature = "selftest")]
    if agent == Agent::Claude && std::env::var_os("IDE_SELFTEST").is_some() {
        ide_ui_agent::open_replay_in(controller, tree.path.clone(), &tree.title);
        return;
    }
    if let Some(tab) = ide_ui_agent::open_agent_in(controller, agent, tree.path.clone(), Some(&tree.title)) {
        tab.focus_composer();
    }
}

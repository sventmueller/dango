//! The Containers panel (B33, E136): containers, images, volumes and Compose projects of a Docker context chosen per
//! window, through the `docker` command line; its commands in the panel's context menu and the Containers menu.

mod docker;
mod panel;

use ide_docker::board::{Node, Row};
use ide_docker::{Action, Compose};
use objc2::rc::Retained;
use objc2::{DefinedClass, MainThreadMarker, MainThreadOnly, sel};
use objc2_app_kit::{NSMenu, NSMenuItem};
use splat_i18n::{t, tf};

use crate::window::{Panel, WindowController};
pub use panel::{ContainersPanel, Reach};

/// Runs `f` with the panel `id`, if its window is still open.
pub fn with_panel(id: u64, f: impl FnOnce(&ContainersPanel)) {
    if let Some(mtm) = MainThreadMarker::new() {
        crate::context::AppContext::get(mtm).containers_panels.with(id, f);
    }
}

fn of(controller: &WindowController) -> Option<Retained<ContainersPanel>> {
    controller.context().containers_panels.of(controller.handle())
}

/// The window shows a folder: its panel reads Docker, whether or not the folder is trusted, since nothing of the
/// folder runs for reading.
pub fn attach(controller: &WindowController) {
    let Some(root) = controller.project_root() else { return };
    window_closed(controller);
    let panels = &controller.context().containers_panels;
    let panel = ContainersPanel::new(controller.mtm(), panels.next_id(), controller.id(), root);
    let (page, header) = panel.build();
    controller.set_panel_page(Panel::Containers, &page);
    controller.set_panel_accessory(Panel::Containers, &header);
    panels.insert(panel.id(), panel.clone());
    if controller.shown_panel() == Some(Panel::Containers) {
        panel.start();
    }
}

/// The window shows its Containers panel: the panel reads Docker the first time, so a window that never shows it
/// runs no `docker`.
pub fn shown(controller: &WindowController) {
    if let Some(panel) = of(controller) {
        panel.start();
    }
}

/// The window's folder closes: its panel stops watching Docker and goes.
pub fn window_closed(controller: &WindowController) {
    let panels = &controller.context().containers_panels;
    let id = controller.id();
    for panel in panels.all().iter().filter(|p| p.window() == id) {
        panel.stop_watching();
    }
    panels.detach(controller.handle());
}

/// What the panel and the Containers menu can do.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Command {
    Start,
    Stop,
    Restart,
    Logs,
    Shell,
    OpenPort,
    Remove,
    Up,
    Down,
    Pull,
    Prune,
    Refresh,
}

const COMMANDS: [Command; 12] = [
    Command::Start,
    Command::Stop,
    Command::Restart,
    Command::Logs,
    Command::Shell,
    Command::OpenPort,
    Command::Remove,
    Command::Up,
    Command::Down,
    Command::Pull,
    Command::Prune,
    Command::Refresh,
];

impl Command {
    fn tag(self) -> isize {
        COMMANDS
            .iter()
            .position(|c| *c == self)
            .and_then(|p| isize::try_from(p + 1).ok())
            .unwrap_or(0)
    }

    fn from_tag(tag: isize) -> Option<Command> {
        COMMANDS.get(usize::try_from(tag).ok()?.checked_sub(1)?).copied()
    }

    fn title(self) -> &'static str {
        match self {
            Command::Start => t("Starten"),
            Command::Stop => t("Stoppen"),
            Command::Restart => t("Neu starten"),
            Command::Logs => t("Logs anzeigen"),
            Command::Shell => t("Shell öffnen"),
            Command::OpenPort => t("Port im Browser öffnen"),
            Command::Remove => t("Löschen …"),
            Command::Up => t("Compose: Starten (up)"),
            Command::Down => t("Compose: Beenden (down)"),
            Command::Pull => t("Compose: Images holen (pull)"),
            Command::Prune => t("Ungenutztes aufräumen …"),
            Command::Refresh => t("Aktualisieren"),
        }
    }

    /// Whether it applies to `row` of `panel`, which may be no row.
    fn applies(self, panel: &ContainersPanel, row: Option<&Row>) -> bool {
        let reach = panel.ivars().reach.borrow().clone();
        if reach == Reach::Missing {
            return false;
        }
        let ready = reach == Reach::Ready;
        let board = panel.ivars().board.borrow();
        let container = match row.map(|r| &r.node) {
            Some(Node::Container(id)) => board.container(id),
            _ => None,
        };
        let running = container.is_some_and(ide_docker::Container::running);
        let file = matches!(row.map(|r| &r.node), Some(Node::Project(_, Some(_))));
        let trusted = crate::project::is_trusted(panel.root());
        match self {
            Command::Start => ready && container.is_some() && !running,
            Command::Stop | Command::Restart | Command::Shell => ready && running,
            Command::Logs => ready && container.is_some(),
            Command::OpenPort => ready && running && container.and_then(ide_docker::Container::url).is_some(),
            Command::Remove => {
                ready
                    && matches!(
                        row.map(|r| &r.node),
                        Some(Node::Container(_) | Node::Image(_) | Node::Volume(_))
                    )
            }
            Command::Up | Command::Down | Command::Pull => ready && file && trusted,
            Command::Prune => ready,
            Command::Refresh => true,
        }
    }
}

fn command_item(mtm: MainThreadMarker, command: Command) -> Retained<NSMenuItem> {
    crate::util::tagged(mtm, command.title(), sel!(ideContainerCommand:), "", command.tag())
}

/// The Containers menu between Debug and Window (E101): every command of the panel, on its selected row.
pub fn menu(mtm: MainThreadMarker) -> Retained<NSMenuItem> {
    let mut items = Vec::new();
    for group in [
        &[Command::Start, Command::Stop, Command::Restart][..],
        &[Command::Logs, Command::Shell, Command::OpenPort],
        &[Command::Up, Command::Down, Command::Pull],
        &[Command::Remove, Command::Prune, Command::Refresh],
    ] {
        if !items.is_empty() {
            items.push(crate::util::separator(mtm));
        }
        items.extend(group.iter().map(|c| command_item(mtm, *c)));
    }
    crate::util::submenu(mtm, t("Container"), items).0
}

/// Fills the panel's context menu with what applies to the clicked row.
pub(crate) fn fill_menu(panel: &ContainersPanel, menu: &NSMenu) {
    menu.removeAllItems();
    let row = panel.chosen();
    let mtm = panel.mtm();
    let mut last_group = 0;
    for (group, command) in [
        (0, Command::Start),
        (0, Command::Stop),
        (0, Command::Restart),
        (1, Command::Logs),
        (1, Command::Shell),
        (1, Command::OpenPort),
        (2, Command::Up),
        (2, Command::Down),
        (2, Command::Pull),
        (3, Command::Remove),
    ] {
        if !command.applies(panel, row.as_ref()) {
            continue;
        }
        if menu.numberOfItems() > 0 && group != last_group {
            menu.addItem(&NSMenuItem::separatorItem(mtm));
        }
        last_group = group;
        menu.addItem(&command_item(mtm, command));
    }
}

/// Whether the Containers menu's `item` applies in `controller`'s window; greyed without `docker` (E101).
pub fn validate(controller: &WindowController, item: &NSMenuItem) -> bool {
    let (Some(panel), Some(command)) = (of(controller), Command::from_tag(item.tag())) else {
        return false;
    };
    command.applies(&panel, panel.chosen().as_ref())
}

/// Does the command of menu item `tag` in `controller`'s window.
pub fn perform(controller: &WindowController, tag: isize) {
    let (Some(panel), Some(command)) = (of(controller), Command::from_tag(tag)) else {
        return;
    };
    let row = panel.chosen();
    if !command.applies(&panel, row.as_ref()) {
        return;
    }
    let node = row.map(|r| r.node);
    let context = panel.context();
    match (command, node) {
        (Command::Start, Some(Node::Container(id))) => panel.act(Action::Start, &id),
        (Command::Stop, Some(Node::Container(id))) => panel.act(Action::Stop, &id),
        (Command::Restart, Some(Node::Container(id))) => panel.act(Action::Restart, &id),
        (Command::Logs, Some(Node::Container(id))) => {
            let title = tf("Logs: {}", &[&name_of(&panel, &id)]);
            crate::terminal::open_passing(controller, panel.root(), &title, ide_docker::logs_line(&context, &id));
        }
        (Command::Shell, Some(Node::Container(id))) => {
            let title = tf("Shell: {}", &[&name_of(&panel, &id)]);
            crate::terminal::open_passing(controller, panel.root(), &title, ide_docker::shell_line(&context, &id));
        }
        (Command::OpenPort, Some(Node::Container(id))) => {
            let url = panel
                .ivars()
                .board
                .borrow()
                .container(&id)
                .and_then(ide_docker::Container::url);
            if let Some(url) = url {
                crate::browser::open_tab(controller, Some(&url));
            }
        }
        (Command::Up | Command::Down | Command::Pull, Some(Node::Project(_, Some(file)))) => {
            let action = match command {
                Command::Up => Compose::Up,
                Command::Down => Compose::Down,
                _ => Compose::Pull,
            };
            panel.compose(action, &file);
        }
        (Command::Remove, Some(node)) => remove(controller, &panel, &node),
        (Command::Prune, _) => prune(controller, &panel),
        (Command::Refresh, _) => panel.refresh(),
        _ => {}
    }
}

fn name_of(panel: &ContainersPanel, id: &str) -> String {
    panel
        .ivars()
        .board
        .borrow()
        .container(id)
        .map_or_else(|| id.to_string(), |c| c.name.clone())
}

/// Asks before deleting a container, an image or a volume; the destructive button is never the default.
fn remove(controller: &WindowController, panel: &ContainersPanel, node: &Node) {
    let Some(window) = controller.window() else { return };
    let context = panel.context();
    let (name, info, command) = match node {
        Node::Container(id) => (
            name_of(panel, id),
            t("Der Container wird entfernt; was er außerhalb von Volumes gespeichert hat, geht verloren."),
            ide_docker::container(&context, Action::Remove, id),
        ),
        Node::Image(reference) => (
            reference.clone(),
            t("Das Image wird von diesem Docker entfernt; es lässt sich wieder holen oder bauen."),
            ide_docker::remove_image(&context, reference),
        ),
        Node::Volume(name) => (
            name.clone(),
            t("Das Volume und alle Daten darin werden gelöscht. Das lässt sich nicht rückgängig machen."),
            ide_docker::remove_volume(&context, name),
        ),
        Node::Section(_) | Node::Project(..) => return,
    };
    let id = panel.id();
    ide_ui_git::confirm_delete(
        &window,
        &tf("„{}“ löschen?", &[&name]),
        info,
        t("Löschen"),
        move || {
            with_panel(id, |panel| panel.run(command));
        },
    );
}

/// Asks before `docker system prune`.
fn prune(controller: &WindowController, panel: &ContainersPanel) {
    let Some(window) = controller.window() else { return };
    let command = ide_docker::prune(&panel.context());
    let id = panel.id();
    ide_ui_git::confirm_delete(
        &window,
        &tf("Ungenutzte Docker-Daten in „{}“ aufräumen?", &[&panel.context()]),
        t(
            "Gelöscht werden gestoppte Container, ungenutzte Netzwerke, Images ohne Namen und der Build-Cache. Volumes bleiben.",
        ),
        t("Aufräumen"),
        move || {
            with_panel(id, |panel| panel.run(command));
        },
    );
}

/// The panel of `controller`'s window, for the self-test.
#[cfg(feature = "selftest")]
pub fn panel_of(controller: &WindowController) -> Option<Retained<ContainersPanel>> {
    of(controller)
}

/// Does the command titled `title`, for the self-test; whether there is one.
#[cfg(feature = "selftest")]
pub fn perform_titled(controller: &WindowController, title: &str) -> bool {
    match COMMANDS.iter().find(|c| c.title() == title) {
        Some(command) => {
            perform(controller, command.tag());
            true
        }
        None => false,
    }
}

/// Whether the Containers menu's item titled `title` is enabled in `controller`'s window, for the self-test.
#[cfg(feature = "selftest")]
pub fn enabled(controller: &WindowController, title: &str) -> bool {
    COMMANDS
        .iter()
        .find(|c| c.title() == title)
        .is_some_and(|c| validate(controller, &command_item(controller.mtm(), *c)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tags_round_trip() {
        for command in COMMANDS {
            assert_eq!(Command::from_tag(command.tag()), Some(command));
        }
        assert_eq!(Command::from_tag(0), None);
        assert_eq!(Command::from_tag(13), None);
    }
}

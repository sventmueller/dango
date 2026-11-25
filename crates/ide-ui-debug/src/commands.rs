//! The Debug menu (E94, E101): every command of the debugger with Xcode's shortcuts, greyed out where it cannot run,
//! acting on the debugger of the window in front.

use objc2::rc::Retained;
use objc2::runtime::{AnyObject, NSObject, Sel};
use objc2::{MainThreadMarker, MainThreadOnly, define_class, msg_send, sel};
use objc2_app_kit::NSMenuItem;
use objc2_foundation::NSObjectProtocol;
use splat_i18n::t;

use super::model::Command;
use super::panel::{self, DebugPanel};
use ide_ui_common::util::{guarded, item, ns, separator, set_target, submenu};
use ide_ui_common::window::{Panel, Panels, Window};

define_class!(
    // SAFETY: NSObject has no subclassing requirements and `DebugCommands` does not implement Drop.
    #[unsafe(super(NSObject))]
    #[thread_kind = MainThreadOnly]
    #[name = "IDEDebugCommands"]
    pub(crate) struct DebugCommands;

    unsafe impl NSObjectProtocol for DebugCommands {}

    impl DebugCommands {
        #[unsafe(method(ideDebugMenuCommand:))]
        fn command(&self, sender: Option<&AnyObject>) {
            guarded((), || {
                if let Some(command) = command_of(sender)
                    && let Some(panel) = front_panel()
                {
                    panel.run_command(command);
                }
            });
        }

        #[unsafe(method(ideDebugMenuStart:))]
        fn start(&self, _sender: Option<&AnyObject>) {
            guarded((), || front(DebugPanel::start_chosen));
        }

        #[unsafe(method(ideDebugMenuCurrent:))]
        fn current(&self, _sender: Option<&AnyObject>) {
            guarded((), || front(DebugPanel::debug_current_program));
        }

        #[unsafe(method(ideDebugMenuAttach:))]
        fn attach(&self, _sender: Option<&AnyObject>) {
            guarded((), || front(DebugPanel::ask_attach));
        }

        #[unsafe(method(ideDebugMenuBreakpoint:))]
        fn breakpoint(&self, _sender: Option<&AnyObject>) {
            guarded((), || {
                if let Some((window, path, line)) = caret_line()
                    && let Some(panel) = panel::of(&window)
                {
                    panel.toggle_breakpoint(&path, line);
                }
            });
        }

        #[unsafe(method(ideDebugMenuConsole:))]
        fn console(&self, _sender: Option<&AnyObject>) {
            guarded((), || front(DebugPanel::show_console));
        }

        #[unsafe(method(validateMenuItem:))]
        fn validate_menu_item(&self, item: &NSMenuItem) -> bool {
            guarded(false, || validate(item))
        }
    }
);

/// The tag of a command's menu item: its place in `MENU_COMMANDS`.
const MENU_COMMANDS: [Command; 8] = [
    Command::Continue,
    Command::StepOver,
    Command::StepInto,
    Command::StepOut,
    Command::Restart,
    Command::Stop,
    Command::ToggleBreakpoints,
    Command::ClearBreakpoints,
];

fn command_of(sender: Option<&AnyObject>) -> Option<Command> {
    let tag = sender?.downcast_ref::<NSMenuItem>()?.tag();
    MENU_COMMANDS.get(usize::try_from(tag).ok()?).copied()
}

/// Whether the item can run now, with its title saying what it does now.
fn validate(item: &NSMenuItem) -> bool {
    let Some(action) = item.action() else { return false };
    let panel = front_panel();
    if action == sel!(ideDebugMenuCommand:) {
        let Some(command) = command_of(Some(item)) else {
            return false;
        };
        let Some(panel) = panel else { return false };
        match command {
            Command::Stop => {
                let detaches = super::model::stops_by_detaching(&panel.read());
                item.setTitle(&ns(if detaches { t("Trennen") } else { t("Stopp") }));
            }
            Command::ToggleBreakpoints => {
                let active = panel.read().breakpoints.active;
                item.setTitle(&ns(if active {
                    t("Haltepunkte ausschalten")
                } else {
                    t("Haltepunkte einschalten")
                }));
            }
            _ => {}
        }
        return panel.can(command);
    }
    if action == sel!(ideDebugMenuBreakpoint:) {
        return caret_line().is_some_and(|(window, _, _)| panel::of(&window).is_some());
    }
    if action == sel!(ideDebugMenuConsole:) {
        return panel.is_some_and(|p| p.has_console());
    }
    // Starting runs what the folder configures, which waits for the user's trust (E22).
    panel.is_some_and(|p| p.can_start())
}

/// The Debug menu, between the app's own menus and Window; `key` gives each command's shortcut.
pub(crate) fn menu(
    mtm: MainThreadMarker,
    key: &dyn Fn(&str) -> String,
    commands: &DebugCommands,
) -> Retained<NSMenuItem> {
    let entry = |title: &str, action: Sel, shortcut: &str| {
        let entry = item(mtm, title, Some(action), &key(shortcut));
        set_target(&entry, commands);
        entry
    };
    let command = |title: &str, command: Command, shortcut: &str| {
        let entry = entry(title, sel!(ideDebugMenuCommand:), shortcut);
        let index = MENU_COMMANDS.iter().position(|c| *c == command).unwrap_or(usize::MAX);
        entry.setTag(isize::try_from(index).unwrap_or(-1));
        entry
    };
    let items = vec![
        command(t("Fortsetzen"), Command::Continue, "debug.continue"),
        command(t("Überspringen"), Command::StepOver, "debug.stepOver"),
        command(t("Hineinspringen"), Command::StepInto, "debug.stepInto"),
        command(t("Heraus"), Command::StepOut, "debug.stepOut"),
        separator(mtm),
        command(t("Neu starten"), Command::Restart, ""),
        command(t("Stopp"), Command::Stop, "debug.stop"),
        separator(mtm),
        entry(t("Starten"), sel!(ideDebugMenuStart:), ""),
        entry(t("Aktuelles Programm debuggen"), sel!(ideDebugMenuCurrent:), ""),
        entry(t("An Prozess anhängen …"), sel!(ideDebugMenuAttach:), ""),
        separator(mtm),
        entry(
            t("Haltepunkt an der Cursorzeile"),
            sel!(ideDebugMenuBreakpoint:),
            "debug.toggleBreakpoint",
        ),
        command(
            t("Haltepunkte ausschalten"),
            Command::ToggleBreakpoints,
            "debug.toggleBreakpoints",
        ),
        command(t("Alle Haltepunkte entfernen"), Command::ClearBreakpoints, ""),
        separator(mtm),
        entry(t("Debug-Konsole zeigen"), sel!(ideDebugMenuConsole:), ""),
    ];
    submenu(mtm, t("Debuggen"), items).0
}

/// The menu's target, which holds no state and aims at the window in front.
pub(crate) fn target(mtm: MainThreadMarker) -> Retained<DebugCommands> {
    // SAFETY: NSObject's initializer, called once on the newly allocated object.
    unsafe { msg_send![DebugCommands::alloc(mtm), init] }
}

/// The debugger of the window in front.
fn front_panel() -> Option<Retained<DebugPanel>> {
    let window = Window::frontmost()?;
    window.parts_ready();
    panel::of(&window)
}

/// Shows the debug panel of the window in front, where the command's result appears, and runs `f` on it.
fn front(f: impl FnOnce(&DebugPanel)) {
    let Some(controller) = Window::frontmost() else {
        return;
    };
    controller.parts_ready();
    let Some(panel) = panel::of(&controller) else { return };
    controller.activate_panel(Panel::Debug);
    f(&panel);
}

/// The window in front, the file of its editor in front and the line its caret is on.
fn caret_line() -> Option<(Window, std::path::PathBuf, usize)> {
    let window = Window::frontmost()?;
    let (path, line) = crate::host::host().caret_line(window.id())?;
    Some((window, path, line))
}

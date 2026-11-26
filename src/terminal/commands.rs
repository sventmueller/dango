//! The terminal's commands in the menu bar (E26, phase 14 stage 3). Tapa's view takes only copying, pasting, selecting
//! all and clearing through the responder chain, so the window runs the rest on the terminal whose pane has the
//! keyboard focus, and `keys` runs their shortcuts before the menus see them. The menus show those shortcuts only while
//! they are open: AppKit lets a disabled item take its shortcut and stops looking for another item with it, so an item
//! that kept ⌘↑ or ⌘I would take it from the editor.

use objc2::rc::Retained;
use objc2::runtime::{AnyObject, ProtocolObject};
use objc2::{MainThreadMarker, msg_send, sel};
use objc2_app_kit::{
    NSApplication, NSControlStateValueOff, NSControlStateValueOn, NSMenu, NSMenuDelegate, NSMenuItem, NSView, NSWindow,
};
use splat_i18n::t;
use term_ui::{TerminalView, all_views};

use crate::context::AppContext;
use crate::util::{separator, submenu, tagged};
use crate::window::WindowController;

/// What the app does on the terminal with the keyboard focus.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Command {
    Find,
    FindNext,
    FindPrevious,
    PreviousPrompt,
    NextPrompt,
    ToggleBlock,
    ToggleBookmark,
    PreviousBookmark,
    NextBookmark,
    CopyMode,
    SelectBlock,
    CopyOutput,
    SaveOutput,
    BlockToAgent,
    CopyFormatted,
    InputField,
}

impl Command {
    /// Every command, in the order of their tags.
    const ALL: [Command; 16] = [
        Command::Find,
        Command::FindNext,
        Command::FindPrevious,
        Command::PreviousPrompt,
        Command::NextPrompt,
        Command::ToggleBlock,
        Command::ToggleBookmark,
        Command::PreviousBookmark,
        Command::NextBookmark,
        Command::CopyMode,
        Command::SelectBlock,
        Command::CopyOutput,
        Command::SaveOutput,
        Command::BlockToAgent,
        Command::CopyFormatted,
        Command::InputField,
    ];

    /// Its name in the shortcut table.
    fn name(self) -> &'static str {
        match self {
            Command::Find => "terminal.find",
            Command::FindNext => "terminal.findNext",
            Command::FindPrevious => "terminal.findPrevious",
            Command::PreviousPrompt => "terminal.previousPrompt",
            Command::NextPrompt => "terminal.nextPrompt",
            Command::ToggleBlock => "terminal.toggleBlock",
            Command::ToggleBookmark => "terminal.toggleBookmark",
            Command::PreviousBookmark => "terminal.previousBookmark",
            Command::NextBookmark => "terminal.nextBookmark",
            Command::CopyMode => "terminal.copyMode",
            Command::SelectBlock => "terminal.selectBlock",
            Command::CopyOutput => "terminal.copyOutput",
            Command::SaveOutput => "terminal.saveOutput",
            Command::BlockToAgent => "terminal.blockToAgent",
            Command::CopyFormatted => "terminal.copyFormatted",
            Command::InputField => "terminal.inputField",
        }
    }

    /// The command the shortcut table names `name`.
    pub(super) fn named(name: &str) -> Option<Command> {
        Self::ALL.into_iter().find(|command| command.name() == name)
    }

    /// Its menu item's title; the Find menu's own items stand for the search.
    fn title(self) -> &'static str {
        match self {
            Command::Find => t("Suchen …"),
            Command::FindNext => t("Weitersuchen"),
            Command::FindPrevious => t("Rückwärts suchen"),
            Command::PreviousPrompt => t("Voriger Prompt"),
            Command::NextPrompt => t("Nächster Prompt"),
            Command::ToggleBlock => t("Block ein-/ausklappen"),
            Command::ToggleBookmark => t("Lesezeichen setzen/entfernen"),
            Command::PreviousBookmark => t("Voriges Lesezeichen"),
            Command::NextBookmark => t("Nächstes Lesezeichen"),
            Command::CopyMode => t("Auswahl mit der Tastatur"),
            Command::SelectBlock => t("Block auswählen"),
            Command::CopyOutput => t("Ausgabe kopieren"),
            Command::SaveOutput => t("Ausgabe sichern …"),
            Command::BlockToAgent => t("Block an Agent übergeben"),
            Command::CopyFormatted => t("Mit Formatierung kopieren"),
            Command::InputField => t("Eingabefeld ein-/ausblenden"),
        }
    }

    fn tag(self) -> isize {
        let index = Self::ALL.iter().position(|command| *command == self).unwrap_or(0);
        isize::try_from(index).unwrap_or(0)
    }

    fn from_tag(tag: isize) -> Option<Command> {
        usize::try_from(tag)
            .ok()
            .and_then(|index| Self::ALL.get(index))
            .copied()
    }

    /// The command of the Find menu's item tagged `tag`, an `NSTextFinder` action.
    fn of_finder(tag: isize) -> Option<Command> {
        match tag {
            1 => Some(Command::Find),
            2 => Some(Command::FindNext),
            3 => Some(Command::FindPrevious),
            _ => None,
        }
    }
}

/// Runs `command` on `view`, the terminal with the keyboard focus in `controller`'s window.
pub(crate) fn run(controller: &WindowController, view: &TerminalView, command: Command) {
    match command {
        Command::Find => view.open_find(),
        Command::FindNext => view.find_again(true),
        Command::FindPrevious => view.find_again(false),
        Command::PreviousPrompt => {
            if !view.jump_to_prompt(-1) {
                view.notify(t("Kein früherer Prompt."));
            }
        }
        Command::NextPrompt => {
            view.jump_to_prompt(1);
        }
        Command::ToggleBlock => {
            view.toggle_block(None);
        }
        Command::ToggleBookmark => {
            view.toggle_bookmark();
        }
        Command::PreviousBookmark => {
            if !view.jump_to_bookmark(-1) {
                view.notify(t("Kein früheres Lesezeichen."));
            }
        }
        Command::NextBookmark => {
            view.jump_to_bookmark(1);
        }
        Command::CopyMode => view.toggle_copy_mode(),
        Command::SelectBlock => {
            view.select_block(None);
        }
        Command::CopyOutput => {
            view.copy_block_output(None);
        }
        Command::SaveOutput => {
            view.save_block_output(None);
        }
        Command::BlockToAgent => super::agent::hand_over(controller, view),
        Command::CopyFormatted => view.copy_formatted(),
        Command::InputField => toggle_field(controller.context(), view),
    }
}

/// Turns the input field of `view` on or off; new terminals follow that choice, as in Tapa (F209).
fn toggle_field(cx: &AppContext, view: &TerminalView) {
    view.toggle_field();
    let on = view.field_enabled();
    if cx.terminals().config().input.field == on {
        return;
    }
    let key = ide_core::settings::keys::TERMINAL_INPUT_FIELD;
    if let Err(error) = ide_core::settings::Settings::load_user().set(&key, &on) {
        crate::diag::report("the input field setting", error);
    }
}

/// The terminal whose pane holds the keyboard focus of `window`: its view, its input field or its find bar.
pub(crate) fn focused_in(window: &NSWindow) -> Option<Retained<TerminalView>> {
    let responder = window.firstResponder()?;
    let focus = responder.downcast_ref::<NSView>()?;
    all_views().into_iter().find(|view| {
        // SAFETY: Asking a view for its superview on the main thread, where views live; it is the stack of the
        // view's pane, which holds it.
        let pane = unsafe { view.superview() };
        pane.is_some_and(|pane| focus.isDescendantOf(&pane))
    })
}

fn focused(controller: &WindowController) -> Option<Retained<TerminalView>> {
    let window = controller.window()?;
    focused_in(&window)
}

/// Runs the command of the menu item tagged `tag` on the terminal with the focus in `controller`'s window.
pub(crate) fn perform(controller: &WindowController, tag: isize) {
    if let (Some(command), Some(view)) = (Command::from_tag(tag), focused(controller)) {
        run(controller, &view, command);
    }
}

/// What the app adds to a terminal's context menu: handing its block to an agent.
pub(crate) fn context_items() -> Vec<term_ui::ContextItem> {
    vec![term_ui::ContextItem {
        title: Command::BlockToAgent.title().to_string(),
        action: sel!(ideTerminalContextCommand:),
    }]
}

/// Hands the block of the terminal whose context menu's item is tagged with its id `tag` to an agent, if that terminal
/// is in `controller`'s window.
pub(crate) fn perform_on_view(controller: &WindowController, tag: isize) {
    let Some(view) = u64::try_from(tag).ok().and_then(term_ui::find) else {
        return;
    };
    if view.window_id().get() == controller.id().get() {
        run(controller, &view, Command::BlockToAgent);
    }
}

/// Runs the Find menu's item tagged `tag` on the terminal with the focus in `controller`'s window, which the window
/// gets since a terminal's view does not answer it.
pub(crate) fn text_finder(controller: &WindowController, tag: isize) {
    if let (Some(command), Some(view)) = (Command::of_finder(tag), focused(controller)) {
        run(controller, &view, command);
    }
}

/// Clears the terminal whose input field or find bar has the focus, which the view's own `termClear:` does not reach.
pub(crate) fn clear(controller: &WindowController) {
    if let Some(view) = focused(controller) {
        let view: &AnyObject = &view;
        // SAFETY: The terminal view answers `termClear:`, which takes the sender, none here.
        let () = unsafe { msg_send![view, termClear: Option::<&AnyObject>::None] };
    }
}

/// Whether the terminal's command of `item` can run, which needs a terminal with the focus in `controller`'s window;
/// `None` for an item of another command.
pub(crate) fn validate(controller: &WindowController, item: &NSMenuItem) -> Option<bool> {
    let action = item.action()?;
    let known = if action == sel!(ideTerminalCommand:) {
        Command::from_tag(item.tag()).is_some()
    } else if action == sel!(performTextFinderAction:) {
        Command::of_finder(item.tag()).is_some()
    } else if action == sel!(termClear:) {
        true
    } else if action == sel!(ideTerminalContextCommand:) {
        return Some(u64::try_from(item.tag()).ok().and_then(term_ui::find).is_some());
    } else {
        return None;
    };
    Some(known && focused(controller).is_some())
}

/// Ticks the item of Secure Keyboard Entry while the user has it on.
pub(crate) fn tick_secure_entry(item: &NSMenuItem) {
    if item.action() == Some(sel!(ideSecureKeyboardEntry:)) {
        item.setState(if term_ui::secure::manual() {
            NSControlStateValueOn
        } else {
            NSControlStateValueOff
        });
    }
}

/// Turns Secure Keyboard Entry on or off, which keeps other apps from reading the keys typed into a terminal.
pub(crate) fn toggle_secure_entry() {
    term_ui::secure::set_manual(!term_ui::secure::manual());
}

fn item(mtm: MainThreadMarker, command: Command) -> Retained<NSMenuItem> {
    tagged(mtm, command.title(), sel!(ideTerminalCommand:), "", command.tag())
}

/// The Edit menu's item after Copy.
pub(crate) fn copy_formatted_item(mtm: MainThreadMarker) -> Retained<NSMenuItem> {
    item(mtm, Command::CopyFormatted)
}

/// The Edit menu's items after Clear Terminal: copy mode and the current block's.
pub(crate) fn block_items(mtm: MainThreadMarker) -> Vec<Retained<NSMenuItem>> {
    std::iter::once(separator(mtm))
        .chain(
            [
                Command::CopyMode,
                Command::SelectBlock,
                Command::CopyOutput,
                Command::SaveOutput,
                Command::BlockToAgent,
            ]
            .map(|command| item(mtm, command)),
        )
        .collect()
}

/// `view_menu` with the terminal's group before the toolbar's: the input field and the prompts and bookmarks, whose
/// submenu `delegate` shows the shortcuts of.
pub(crate) fn with_view_items(
    mtm: MainThreadMarker,
    view_menu: Retained<NSMenuItem>,
    delegate: &ProtocolObject<dyn NSMenuDelegate>,
) -> Retained<NSMenuItem> {
    let Some(menu) = view_menu.submenu() else {
        return view_menu;
    };
    let (marks, marks_menu) = submenu(
        mtm,
        t("Prompts und Lesezeichen"),
        vec![
            item(mtm, Command::PreviousPrompt),
            item(mtm, Command::NextPrompt),
            item(mtm, Command::ToggleBlock),
            separator(mtm),
            item(mtm, Command::ToggleBookmark),
            item(mtm, Command::PreviousBookmark),
            item(mtm, Command::NextBookmark),
        ],
    );
    marks_menu.setDelegate(Some(delegate));
    let toolbar = menu
        .itemArray()
        .iter()
        .position(|entry| entry.action() == Some(sel!(toggleToolbarShown:)));
    let at = toolbar.map_or(menu.numberOfItems(), |i| {
        isize::try_from(i).unwrap_or(0).saturating_sub(1).max(0)
    });
    for (offset, entry) in (0..).zip([separator(mtm), item(mtm, Command::InputField), marks]) {
        menu.insertItem_atIndex(&entry, at + offset);
    }
    view_menu
}

/// Puts the shortcuts of `cx`'s keymap on the terminal's items in `menu` as it opens, or takes them off as it closes;
/// whether `menu` has such items.
pub(crate) fn show_shortcuts(cx: &AppContext, menu: &NSMenu, open: bool) -> bool {
    let keymap = cx.keymap().borrow();
    let mut found = false;
    for entry in menu.itemArray().iter() {
        let Some(command) = command_of(&entry) else { continue };
        found = true;
        let key = if open {
            keymap.key(command.name()).unwrap_or("")
        } else {
            ""
        };
        ide_ui_common::util::set_shortcut(&entry, key);
    }
    if found && open {
        cx.terminals().note_shortcuts(true);
    }
    found
}

/// Takes the shortcuts off every terminal item of the menu bar, in case a menu was updated without closing again.
pub(super) fn hide_shortcuts(cx: &AppContext, mtm: MainThreadMarker) {
    if cx.terminals().note_shortcuts(false)
        && let Some(menu) = NSApplication::sharedApplication(mtm).mainMenu()
    {
        hide_in(&menu);
    }
}

fn hide_in(menu: &NSMenu) {
    for entry in menu.itemArray().iter() {
        match entry.submenu() {
            Some(inner) => hide_in(&inner),
            None if command_of(&entry).is_some() => ide_ui_common::util::set_shortcut(&entry, ""),
            None => {}
        }
    }
}

fn command_of(entry: &NSMenuItem) -> Option<Command> {
    (entry.action() == Some(sel!(ideTerminalCommand:)))
        .then(|| Command::from_tag(entry.tag()))
        .flatten()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_command_has_its_shortcut_in_the_table_and_a_tag_of_its_own() {
        let keymap = ide_core::keymap::Keymap::default();
        let in_table: Vec<&str> = keymap.terminal_keys().map(|(name, _)| name).collect();
        for command in Command::ALL {
            assert_eq!(Command::from_tag(command.tag()), Some(command));
            assert_eq!(Command::named(command.name()), Some(command));
            assert!(
                command == Command::BlockToAgent || in_table.contains(&command.name()),
                "{command:?}"
            );
        }
        assert!(in_table.iter().all(|name| Command::named(name).is_some()));
        assert_eq!(Command::from_tag(-1), None);
        assert_eq!(Command::of_finder(12), None);
    }
}

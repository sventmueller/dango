//! The self-test's hooks of `mod.rs`.

use super::*;

/// The panel of `controller`'s window, for the self-test.
pub(crate) fn panel_of(controller: &WindowController) -> Option<Retained<ContainersPanel>> {
    of(controller)
}

/// Does the command titled `title`, for the self-test; whether there is one.
pub(crate) fn perform_titled(controller: &WindowController, title: &str) -> bool {
    match COMMANDS.iter().find(|c| c.title() == title) {
        Some(command) => {
            perform(controller, command.tag());
            true
        }
        None => false,
    }
}

/// Whether the Containers menu's item titled `title` is enabled in `controller`'s window, for the self-test.
pub(crate) fn enabled(controller: &WindowController, title: &str) -> bool {
    COMMANDS
        .iter()
        .find(|c| c.title() == title)
        .is_some_and(|c| validate(controller, &command_item(controller.mtm(), *c)))
}

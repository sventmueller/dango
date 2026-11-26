//! A terminal's current block for an agent (I8, E22): its command line and output go to a new AI tab as text from
//! outside, with a draft that asks about them, as a failed CI job's log does.

use objc2_app_kit::NSBeep;
use splat_i18n::tf;
use term_session::Session;
use term_session::session::BlockPart;
use term_ui::TerminalView;

use crate::window::WindowController;

/// What an agent gets of a block.
struct Block {
    command: String,
    output: String,
    exit_code: Option<i32>,
}

/// Hands the current block of `view` to a new AI tab of `controller`'s window, the one the CI panel opens for a log;
/// a beep where the block has no output.
pub(super) fn hand_over(controller: &WindowController, view: &TerminalView) {
    let Some(block) = view.with_session(current).flatten() else {
        NSBeep();
        return;
    };
    if let Some(tab) = crate::ci_panel::open_agent(controller) {
        tab.set_draft(&draft(&block));
        tab.attach_outside(ide_ui_agent::terminal_block(
            &block.command,
            block.exit_code,
            &block.output,
        ));
    }
}

/// The block the block actions act on in `session`, unless its output is empty.
fn current(session: &Session) -> Option<Block> {
    let id = session.current_block()?;
    let output = session
        .block_text(id, BlockPart::Output)
        .filter(|output| !output.trim().is_empty())?;
    let command = session.block_text(id, BlockPart::Command).unwrap_or_default();
    let shown = session.blocks_view();
    let exit_code = shown
        .blocks
        .iter()
        .chain(shown.sticky.iter())
        .find(|entry| entry.block.id == id)
        .and_then(|entry| entry.block.exit_code);
    Some(Block {
        command: command.trim().to_string(),
        output,
        exit_code,
    })
}

/// What the AI tab's composer holds for `block`, for the user to complete or send.
fn draft(block: &Block) -> String {
    match block.exit_code {
        Some(code) if code != 0 => tf(
            "Der Befehl „{}“ ist mit Exit-Code {} fehlgeschlagen. Finde die Ursache und schlag eine Korrektur vor; seine Ausgabe hängt an.",
            &[&block.command, &code.to_string()],
        ),
        _ => tf("Die Ausgabe des Befehls „{}“ hängt an.", &[&block.command]),
    }
}

//! What the self-test reads from and does with the parts of `mod.rs`, kept out of its production code.

#[allow(
    unused_imports,
    reason = "the parts moved here use only some of their module's names"
)]
use super::*;

pub use super::project_symbols::held as symbol_servers;

pub use super::actions::selftest as code_action_menu;

pub use super::complete::selftest as completion;

pub use super::edits::{apply_workspace_edit, question as server_question};

pub use super::hover::shown_text as hover_text;

mod probe;

pub use probe::{
    companion_running, install_companions, language_name, marks_of, quiet_for, server_running, token_count,
};

pub use super::rename::rename_to;

pub use super::signature::selftest as signatures;

pub use {navigate::close_list, navigate::listed, navigate::shown_symbols};

/// The id of the main slot of `view`, the source of its own diagnostics.
pub(crate) fn main_slot(view: &EditorTextView) -> u64 {
    view.ivars().lsp.borrow().slot.unwrap_or(0)
}

/// Shows `diagnostics` in `view` as if its server had sent them, for the self-test.
pub fn show_diagnostics(view: &EditorTextView, diagnostics: &[ide_lsp::lsp_types::Diagnostic]) {
    decorate::set_diagnostics(view, diagnostics);
}

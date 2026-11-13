//! What the self-test reads from and does with the parts of `navigate.rs`, kept out of its production code.

#[allow(
    unused_imports,
    reason = "the parts moved here use only some of their module's names"
)]
use super::*;

/// The titles the open location sheet lists, for the self-test.
pub fn listed() -> Vec<String> {
    with_sheet(|cell| {
        cell.borrow()
            .as_ref()
            .map(|s| s.ivars().entries.borrow().iter().map(|e| e.title.clone()).collect())
            .unwrap_or_default()
    })
}

/// Closes the open location sheet, for the self-test.
pub fn close_list() {
    let sheet = with_sheet(|cell| cell.borrow().clone());
    if let Some(sheet) = sheet {
        sheet.close();
    }
}

/// The menu of symbols the last "Symbole" command built, for the self-test.
pub fn shown_symbols() -> Option<Retained<NSMenu>> {
    crate::context::app().location_sheet.symbols.borrow().clone()
}

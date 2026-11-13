//! What the self-test reads from and does with the parts of `agents.rs`, kept out of its production code.

#[allow(
    unused_imports,
    reason = "the parts moved here use only some of their module's names"
)]
use super::*;

/// The texts and button titles in `view`, in order, for the self-test.
pub(super) fn texts(view: &NSView) -> Vec<String> {
    if let Some(label) = view.downcast_ref::<NSTextField>() {
        return vec![label.stringValue().to_string()];
    }
    if let Some(button) = view.downcast_ref::<NSButton>() {
        return vec![format!("[{}]", button.title())];
    }
    let children = match view.downcast_ref::<NSStackView>() {
        Some(stack) => stack.arrangedSubviews(),
        None => view.subviews(),
    };
    children.iter().flat_map(|child| texts(&child)).collect()
}

/// The registry window and the ids it lists, for the self-test.
pub fn register_window() -> Option<(Retained<NSPanel>, Vec<String>)> {
    let panel = with_window(|w| w.borrow().clone())?;
    let ids = with_shown(|s| s.borrow().clone());
    Some((panel, ids))
}

/// Opens the registry window as its button does, for the self-test.
pub fn open_register(target: &AgentsSettings) {
    show_register(target);
}

impl AgentsSettings {
    /// The list of Claude's stored rules, for the self-test.
    pub fn stored_rules(&self) -> Retained<super::super::stored_rules::StoredRules> {
        self.ivars().rules.clone()
    }

    /// What the tab shows, line by line, for the self-test.
    pub fn lines(&self) -> Vec<String> {
        let groups = [self.ivars().agents.get(), self.ivars().register.get()];
        groups
            .into_iter()
            .flatten()
            .flat_map(|group| group.arrangedSubviews().to_vec())
            .filter(|line| line.downcast_ref::<NSStackView>().is_some())
            .map(|line| texts(&line).join(" "))
            .collect()
    }
}

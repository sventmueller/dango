//! The self-test's hooks of `doc.rs`.

use super::*;
use crate::AgentKey;

/// What the document of plan `id` in the AI tab `agent` shows, for the self-test.
pub fn inspect(agent: AgentKey, id: u64) -> Option<DocSeen> {
    let doc = doc(&crate::host::state().plan.docs, agent, id)?;
    let storage = storage(&doc.text)?;
    let notes = doc
        .notes
        .arrangedSubviews()
        .firstObject()
        .and_then(|list| list.downcast::<NSStackView>().ok())
        .map_or(0, |list| list.arrangedSubviews().count());
    Some(DocSeen {
        notes,
        marked: super::super::count_marks(&storage),
        waiting: doc.buttons[3].isEnabled(),
        state: doc.state.stringValue().to_string(),
        revise: doc.buttons[2].title().to_string(),
    })
}

/// The text view of plan `id`'s document, for the self-test.
pub fn text_of(agent: AgentKey, id: u64) -> Option<Retained<NSTextView>> {
    text_in(&crate::host::state().plan.docs, agent, id)
}

//! The self-test's hooks of `editor.rs`.

use super::*;

/// The editor's switches as the General tab shows them: inlay hints, the languages ticked in the pull-down, and
/// blame at the line's end, for the self-test.
pub fn shown() -> Option<(bool, Vec<String>, bool)> {
    with_tab(|tab| {
        let tab = tab.borrow();
        let views = tab.as_ref()?.ivars().views.borrow();
        let views = views.as_ref()?;
        let ticked = views
            .languages
            .itemArray()
            .iter()
            .skip(1)
            .filter(|item| item.state() == NSControlStateValueOn)
            .map(|item| item.title().to_string())
            .collect();
        Some((
            views.hints.state() == NSControlStateValueOn,
            ticked,
            views.blame.state() == NSControlStateValueOn,
        ))
    })
}

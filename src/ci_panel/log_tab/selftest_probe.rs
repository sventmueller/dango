//! The self-test's hooks of `log_tab.rs`.

use super::*;

/// The lines of the log tab of `job` in `controller`'s window, and whether its find bar shows; with `find`, after
/// the find command ⌘F sends.
pub fn inspect(controller: &WindowController, job: u64, find: bool) -> Option<(Vec<LogLine>, bool)> {
    let tab = live_tabs().into_iter().find(|tab| {
        tab.ivars().job.borrow().id == job
            && tab
                .ivars()
                .controller
                .load()
                .is_some_and(|c| std::ptr::eq(&*c, controller))
    })?;
    let ui = tab.ui();
    if find {
        let item = crate::util::item(tab.mtm(), "", Some(sel!(performFindPanelAction:)), "");
        item.setTag(1);
        // SAFETY: The sender is a menu item whose tag names the find action, as the Find menu sends it.
        unsafe { ui.text.performFindPanelAction(Some(&item)) };
    }
    Some((
        tab.lines(),
        objc2_app_kit::NSTextFinderBarContainer::isFindBarVisible(&*ui.scroll),
    ))
}

impl LogTab {
    /// The lines on view, for the self-test.
    pub fn lines(&self) -> Vec<LogLine> {
        self.ivars().lines.borrow().clone()
    }
}

//! The self-test's hooks of `terminal/state.rs`: the notifications the terminals' programs asked for, and what the
//! app keeps of their bell and progress and shows on the Dock icon.

use objc2::DefinedClass;
use objc2_app_kit::{NSApplication, NSView};

use super::*;

/// A notification a terminal's program asked for while the terminal was not in front.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Heard {
    pub(crate) view: u64,
    pub(crate) title: String,
    pub(crate) body: String,
    /// Whether it went to macOS, as it does only while another app is in front; the tab's red dot shows it either way.
    pub(crate) posted: bool,
}

impl Terminals {
    pub(crate) fn note_heard(&self, heard: Heard) {
        self.heard.borrow_mut().push(heard);
    }

    /// The notifications the terminals' programs asked for while out of sight, oldest first.
    pub(crate) fn heard(&self) -> Vec<Heard> {
        self.heard.borrow().clone()
    }

    /// Whether the tab of `view` shows the red dot, and the progress its program reports.
    pub(crate) fn signals_of(&self, view: u64) -> Option<(bool, Option<Progress>)> {
        let shells = self.shells.borrow();
        let shell = shells.get(&view)?;
        Some((shell.waits, shell.progress))
    }

    /// The bar the Dock icon shows, while its tile shows the terminals' progress.
    pub(crate) fn dock_ring(&self, mtm: MainThreadMarker) -> Option<Ring> {
        let view = self.dock.get()?;
        let content = NSApplication::sharedApplication(mtm).dockTile().contentView(mtm)?;
        let ours: &NSView = view;
        std::ptr::eq(&*content, ours).then(|| view.ivars().get()).flatten()
    }
}

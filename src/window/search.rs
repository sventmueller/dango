//! The search panel (⌘⇧F): a field whose matches are listed grouped per file, searched off the main thread and shown as
//! they are found, and a replace field that replaces all of them, those of a file or those of a line. Open editors
//! with unsaved changes are searched in place of their files, and the matches on show follow changes on disk and in
//! those editors. The model decides, the panel draws, the controller runs the searches (E98).

use std::cell::RefCell;
use std::path::Path;
use std::time::Duration;

use objc2::DefinedClass;
use objc2::rc::{Retained, Weak};

mod controller;
mod hierarchy;
pub use hierarchy::HierarchyPanels;
mod model;
mod panel;
mod replace;
mod rows;
#[cfg(feature = "selftest")]
pub mod selftest;
mod views;

#[cfg(feature = "selftest")]
pub use model::LIMIT;
pub use model::Toggle;
use panel::SearchOutlineView;
pub use panel::SearchPanel;

/// How long typing must pause before a search starts.
const DEBOUNCE: Duration = Duration::from_millis(150);

/// How often a running search hands what it found to the main thread.
const FLUSH: Duration = Duration::from_millis(80);

/// How long files and unsaved texts must rest before the matches on show are searched again.
const REFRESH_DELAY: Duration = Duration::from_millis(500);

const RETURN_KEYS: [u16; 2] = [36, 76];
const ESCAPE_KEY: u16 = 53;
/// How far the rows move toward the leading edge from where a source list puts them.
const OUTDENT: f64 = 4.0;
/// The search panels of every window and the id the next one gets.
pub struct SearchPanels {
    panels: RefCell<Vec<Weak<SearchPanel>>>,
    next_id: std::cell::Cell<u64>,
}

impl Default for SearchPanels {
    fn default() -> Self {
        SearchPanels {
            panels: RefCell::new(Vec::new()),
            next_id: std::cell::Cell::new(1),
        }
    }
}

fn with_panels<R>(f: impl FnOnce(&RefCell<Vec<Weak<SearchPanel>>>) -> R) -> R {
    f(&crate::context::app().search_panels.panels)
}

fn with_next_id<R>(f: impl FnOnce(&std::cell::Cell<u64>) -> R) -> R {
    f(&crate::context::app().search_panels.next_id)
}

/// Files below `root` changed on disk: the searches of its folder that show matches run again.
pub fn files_changed(root: &Path) {
    for panel in panels() {
        if panel.read().root() == root {
            panel.refresh_soon();
        }
    }
}

/// The unsaved text of `path` changed: the searches that show matches below its folder run again.
pub fn buffer_edited(path: &Path) {
    for panel in panels() {
        if path.starts_with(panel.read().root()) {
            panel.refresh_soon();
        }
    }
}

fn panels() -> Vec<Retained<SearchPanel>> {
    with_panels(|panels| panels.borrow().iter().filter_map(Weak::load).collect())
}

/// The panel with `id`, if its window is still open.
fn panel(id: u64) -> Option<Retained<SearchPanel>> {
    with_panels(|panels| {
        let mut panels = panels.borrow_mut();
        panels.retain(|p| p.load().is_some());
        panels.iter().filter_map(Weak::load).find(|p| p.ivars().id == id)
    })
}

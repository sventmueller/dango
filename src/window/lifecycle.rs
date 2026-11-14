//! The lifecycle of a window's folder: the parts that live as long as a window shows a folder are listed once in
//! `PARTS`, and every path that opens, trusts, replaces or closes a folder goes through the functions here, so no part
//! misses one of them.

use std::path::Path;

use objc2::DefinedClass;

use super::WindowController;

/// What a part of a window does when its folder opens, becomes trusted or closes.
struct Part {
    /// The window shows a folder now, a new one or the first.
    attached: fn(&WindowController),
    /// The user trusts `root` now (E22).
    trust_changed: fn(&Path),
    /// The window's folder closes, because the window closes or shows another folder; it still reports the old root.
    closed: fn(&WindowController),
}

impl Part {
    const NONE: Part = Part {
        attached: |_| {},
        trust_changed: |_| {},
        closed: |_| {},
    };
}

const PARTS: &[Part] = &[
    Part {
        attached: ide_ui_git::attach,
        trust_changed: ide_ui_git::trust_changed,
        ..Part::NONE
    },
    Part {
        attached: crate::ci_panel::attach,
        trust_changed: crate::ci_panel::trust_changed,
        closed: crate::ci_panel::window_closed,
    },
    Part {
        attached: ide_ui_agent::agent_panel::attach,
        ..Part::NONE
    },
    Part {
        closed: ide_ui_agent::window_closed,
        ..Part::NONE
    },
    Part {
        attached: crate::tests_panel::attach,
        trust_changed: crate::tests_panel::trust_changed,
        closed: crate::tests_panel::window_closed,
    },
    Part {
        attached: crate::containers_panel::attach,
        closed: crate::containers_panel::window_closed,
        ..Part::NONE
    },
    Part {
        attached: crate::debug::attach,
        trust_changed: crate::debug::trust_changed,
        closed: crate::debug::window_closed,
    },
    Part {
        trust_changed: crate::lsp::trust_changed,
        closed: |controller| {
            if let Some(root) = controller.project_root() {
                crate::lsp::window_closed(&root);
            }
        },
        ..Part::NONE
    },
];

impl WindowController {
    /// The window's folder opened; every part takes it up.
    pub(super) fn folder_attached(&self) {
        for part in PARTS {
            (part.attached)(self);
        }
    }

    /// The window's folder closes, for good or for another folder: the search stops and every part lets it go.
    pub(super) fn folder_closes(&self) {
        if let Some(search) = self.ivars().search.borrow().as_ref() {
            search.stop();
        }
        for part in PARTS {
            (part.closed)(self);
        }
    }
}

/// The user trusts `root` now; the parts of every window that shows it may start what they waited for.
pub(super) fn trust_changed(root: &Path) {
    for part in PARTS {
        (part.trust_changed)(root);
    }
}

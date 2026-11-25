//! The lifecycle of a window's folder: the parts that live as long as a window shows a folder are listed once in
//! `PARTS`, and every path that opens, trusts, replaces or closes a folder goes through the functions here, so no part
//! misses one of them.

use std::path::Path;
use std::rc::Rc;

use objc2::DefinedClass;

use crate::context::AppContext;
use crate::window::{WindowController, WindowId};

/// What a part of a window does when its folder opens, becomes trusted or closes.
struct Part {
    /// The window shows a folder now, a new one or the first.
    attached: fn(&WindowController),
    /// The user trusts `root` now (E22).
    trust_changed: fn(&Rc<AppContext>, &Path),
    /// The window's folder closes, because the window closes or shows another folder; it still reports the old root.
    closed: fn(&WindowController),
}

impl Part {
    const NONE: Part = Part {
        attached: |_| {},
        trust_changed: |_, _| {},
        closed: |_| {},
    };
}

const PARTS: &[Part] = &[
    Part {
        attached: ide_ui_git::attach,
        trust_changed: |_, root| ide_ui_git::trust_changed(root),
        ..Part::NONE
    },
    Part {
        attached: crate::ci_panel::attach,
        trust_changed: |cx, root| crate::ci_panel::trust_changed(cx, root),
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
        attached: crate::leitstand_ui::attach,
        closed: crate::leitstand_ui::window_closed,
        ..Part::NONE
    },
    Part {
        attached: |controller| {
            controller
                .context()
                .leitstand()
                .attached(controller.context(), controller)
        },
        trust_changed: |cx, root| cx.leitstand().trust_changed(cx, root),
        closed: |controller| {
            controller
                .context()
                .leitstand()
                .folder_closed(controller.context(), controller)
        },
    },
    Part {
        attached: crate::debug::attach,
        trust_changed: |_, root| crate::debug::trust_changed(root),
        closed: crate::debug::window_closed,
    },
    Part {
        trust_changed: |_, root| ide_ui_lsp::trust_changed(root),
        closed: |controller| {
            if let Some(root) = controller.project_root() {
                ide_ui_lsp::window_closed(&root);
            }
        },
        ..Part::NONE
    },
];

impl WindowController {
    /// The window's folder opened; every part takes it up, one after the other once the window has drawn its first
    /// frame, Git first and the debugger last.
    pub(in crate::window) fn folder_attached(&self) {
        let folder = &self.ivars().folder;
        let round = folder.attachments.get() + 1;
        folder.attachments.set(round);
        folder.waiting_part.set(Some(0));
        let (cx, window) = (Rc::clone(self.context()), self.id());
        crate::util::after_frame(move || attach_next(&cx, window, round));
    }

    /// Attaches the parts that still wait for the first frame, for a restored tab, a panel or a command that needs
    /// them now.
    pub(crate) fn attach_waiting_parts(&self) {
        if let Some(first) = self.ivars().folder.waiting_part.take() {
            attach(self, first..PARTS.len());
            crate::debug::refresh_editors(self);
        }
    }

    /// The window's folder closes, for good or for another folder: the search stops and every part lets it go.
    pub(in crate::window) fn folder_closes(&self) {
        self.ivars().folder.waiting_part.set(None);
        if let Some(search) = self.ivars().folder.search.borrow().as_ref() {
            search.stop();
        }
        for part in PARTS {
            (part.closed)(self);
        }
    }
}

/// The user trusts `root` now; the parts of every window that shows it may start what they waited for.
pub(in crate::window) fn trust_changed(cx: &Rc<AppContext>, root: &Path) {
    for part in PARTS {
        (part.trust_changed)(cx, root);
    }
}

fn attach(controller: &WindowController, parts: std::ops::Range<usize>) {
    for part in &PARTS[parts] {
        (part.attached)(controller);
    }
}

/// Attaches the next waiting part of the folder the window of `window` took up as its `round`-th, and the one after
/// it once the next frame is committed, so input between them goes first.
fn attach_next(cx: &Rc<AppContext>, window: WindowId, round: u64) {
    let Some(controller) = cx.windows().get(window.get()) else {
        return;
    };
    let folder = &controller.ivars().folder;
    let Some(next) = folder.waiting_part.get().filter(|_| folder.attachments.get() == round) else {
        return;
    };
    let last = next + 1 == PARTS.len();
    folder.waiting_part.set((!last).then_some(next + 1));
    (PARTS[next].attached)(&controller);
    if !last {
        let cx = Rc::clone(cx);
        crate::util::after_frame(move || attach_next(&cx, window, round));
    } else {
        // Gutters drawn before the debugger and the tests panel attached lack their marks.
        crate::debug::refresh_editors(&controller);
    }
}

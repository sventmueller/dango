//! The Git menu of the menu bar (E101): every command of the stash and of the commit message also has a menu item,
//! greyed out where it cannot run. The items act on the Git panel of the window in front, a stash entry's on the one
//! selected in its list.

use std::cell::OnceCell;

use objc2::rc::Retained;
use objc2::runtime::{AnyObject, NSObject, Sel};
use objc2::{MainThreadMarker, MainThreadOnly, define_class, msg_send, sel};
use objc2_app_kit::NSMenuItem;
use objc2_foundation::NSObjectProtocol;
use splat_i18n::t;

use super::model::RowKey;
use super::panel::{self, GitPanel, Phase};
use crate::util::{guarded, item, separator, set_target, submenu};
use crate::window::Panel;

thread_local! {
    static COMMANDS: OnceCell<Retained<GitCommands>> = const { OnceCell::new() };
}

define_class!(
    // SAFETY: NSObject has no subclassing requirements and `GitCommands` does not implement Drop.
    #[unsafe(super(NSObject))]
    #[thread_kind = MainThreadOnly]
    #[name = "IDEGitCommands"]
    pub struct GitCommands;

    unsafe impl NSObjectProtocol for GitCommands {}

    impl GitCommands {
        #[unsafe(method(ideGitStash:))]
        fn stash(&self, _sender: Option<&AnyObject>) {
            guarded((), || front(GitPanel::ask_stash));
        }

        #[unsafe(method(ideGitStashStaged:))]
        fn stash_staged(&self, _sender: Option<&AnyObject>) {
            guarded((), || front(GitPanel::stash_staged));
        }

        #[unsafe(method(ideGitApplyStash:))]
        fn apply(&self, _sender: Option<&AnyObject>) {
            guarded((), || front(|panel| panel.apply_stash(&panel.selected_keys(), false)));
        }

        #[unsafe(method(ideGitPopStash:))]
        fn pop(&self, _sender: Option<&AnyObject>) {
            guarded((), || front(|panel| panel.apply_stash(&panel.selected_keys(), true)));
        }

        #[unsafe(method(ideGitDropStash:))]
        fn drop_stash(&self, _sender: Option<&AnyObject>) {
            guarded((), || front(|panel| panel.drop_stash(&panel.selected_keys())));
        }

        #[unsafe(method(ideGitFinishStash:))]
        fn finish(&self, _sender: Option<&AnyObject>) {
            guarded((), || front(GitPanel::finish_stash_conflict));
        }

        #[unsafe(method(ideGitAbortStash:))]
        fn abort(&self, _sender: Option<&AnyObject>) {
            guarded((), || front(GitPanel::abort_stash_conflict));
        }

        #[unsafe(method(ideGitMessage:))]
        fn message(&self, _sender: Option<&AnyObject>) {
            guarded((), || front(GitPanel::generate_message));
        }

        #[unsafe(method(ideGitShowSent:))]
        fn show_sent(&self, _sender: Option<&AnyObject>) {
            guarded((), || front(GitPanel::show_sent));
        }

        #[unsafe(method(validateMenuItem:))]
        fn validate_menu_item(&self, item: &NSMenuItem) -> bool {
            guarded(false, || {
                let Some(action) = item.action() else { return false };
                front_panel().is_some_and(|panel| panel.can(action, &panel.selected_keys()))
            })
        }
    }
);

/// The Git menu, between the app's own menus and Window.
pub fn menu(mtm: MainThreadMarker) -> Retained<NSMenuItem> {
    let commands = COMMANDS.with(|cell| {
        cell.get_or_init(|| {
            // SAFETY: NSObject's initializer, called once on the newly allocated object.
            unsafe { msg_send![GitCommands::alloc(mtm), init] }
        })
        .clone()
    });
    let entries: [(&str, Option<Sel>); 13] = [
        (t("Stash …"), Some(sel!(ideGitStash:))),
        (t("Nur Vorgemerktes stashen"), Some(sel!(ideGitStashStaged:))),
        ("", None),
        (t("Stash anwenden"), Some(sel!(ideGitApplyStash:))),
        (t("Stash anwenden und entfernen"), Some(sel!(ideGitPopStash:))),
        (t("Stash löschen …"), Some(sel!(ideGitDropStash:))),
        ("", None),
        (t("Stash-Konflikt abschließen"), Some(sel!(ideGitFinishStash:))),
        (t("Anwenden des Stashs abbrechen …"), Some(sel!(ideGitAbortStash:))),
        ("", None),
        (t("Commit-Nachricht erzeugen"), Some(sel!(ideGitMessage:))),
        (t("Gesendeten Inhalt zeigen"), Some(sel!(ideGitShowSent:))),
        ("", None),
    ];
    let mut items: Vec<Retained<NSMenuItem>> = entries
        .into_iter()
        .map(|(title, action)| match action {
            Some(action) => {
                let entry = item(mtm, title, Some(action), "");
                set_target(&entry, &commands);
                entry
            }
            None => separator(mtm),
        })
        .collect();
    items.pop();
    submenu(mtm, "Git", items).0
}

/// The Git panel of the window in front.
fn front_panel() -> Option<Retained<GitPanel>> {
    let mtm = MainThreadMarker::new()?;
    let controller = crate::window::frontmost(mtm)?;
    panel::of(&controller)
}

/// Shows the Git panel of the window in front, where the command's result appears, and runs `f` on it.
fn front(f: impl FnOnce(&GitPanel)) {
    let Some(mtm) = MainThreadMarker::new() else { return };
    let Some(controller) = crate::window::frontmost(mtm) else {
        return;
    };
    let Some(panel) = panel::of(&controller) else { return };
    controller.activate_panel(Panel::Git);
    f(&panel);
}

impl GitPanel {
    /// Whether the command `action` can run now, a stash entry's on `keys`.
    pub fn can(&self, action: Sel, keys: &[RowKey]) -> bool {
        let state = self.read();
        let ready = state.phase == Phase::Ready && state.busy.is_none();
        let changed = !state.model.staged.is_empty() || !state.model.changes.is_empty();
        let staged = !state.model.staged.is_empty();
        let conflict = state.stash_apply.is_some();
        let sent = state.message.sent.is_some();
        drop(state);
        if action == sel!(ideGitStash:) {
            ready && changed
        } else if action == sel!(ideGitStashStaged:) {
            ready && staged
        } else if [sel!(ideGitApplyStash:), sel!(ideGitPopStash:), sel!(ideGitDropStash:)].contains(&action) {
            ready && self.has_stash(keys)
        } else if action == sel!(ideGitFinishStash:) || action == sel!(ideGitAbortStash:) {
            ready && conflict
        } else if action == sel!(ideGitMessage:) {
            ready && self.message_unavailable().is_none()
        } else if action == sel!(ideGitShowSent:) {
            sent
        } else {
            true
        }
    }
}

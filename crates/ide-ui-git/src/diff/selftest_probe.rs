//! The self-test's hooks of `diff.rs`.

use super::*;

/// The diff tab showing `key` in `controller`'s window.
pub(crate) fn find(controller: &(impl AsWindow + ?Sized), key: &str) -> Option<Retained<DiffTab>> {
    let controller = &controller.as_window();
    crate::host::state().diff_tabs.live().into_iter().find(|tab| {
        let ivars = tab.ivars();
        let own = match (&ivars.since, &ivars.commit) {
            (_, Some(Fixed::Commit(hash))) => commit_key(hash, &ivars.path),
            (_, Some(Fixed::Stash(hash, _))) => stash_key(hash, &ivars.path),
            (Some(_), None) => since_key(&ivars.repo_root, &ivars.path),
            (None, None) => self::key(&ivars.path, ivars.staged),
        };
        own == key && ivars.controller == *controller
    })
}

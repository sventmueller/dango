use std::cell::Cell;

use objc2_app_kit::NSTextStorage;
use objc2_foundation::NSUndoManager;

/// A flag that is up while the guard lives and down once it drops, also when a panic unwinds past it.
#[must_use = "the flag goes down as soon as the guard drops"]
pub struct FlagGuard<'a>(&'a Cell<bool>);

impl<'a> FlagGuard<'a> {
    pub fn raise(flag: &'a Cell<bool>) -> Self {
        flag.set(true);
        Self(flag)
    }
}

impl Drop for FlagGuard<'_> {
    fn drop(&mut self) {
        self.0.set(false);
    }
}

/// One edit of a text storage, which it processes once the guard drops, also when a panic unwinds past it.
#[must_use = "the edit ends as soon as the guard drops"]
pub(crate) struct EditingGuard<'a>(&'a NSTextStorage);

impl<'a> EditingGuard<'a> {
    pub(crate) fn begin(storage: &'a NSTextStorage) -> Self {
        storage.beginEditing();
        Self(storage)
    }
}

impl Drop for EditingGuard<'_> {
    fn drop(&mut self) {
        self.0.endEditing();
    }
}

/// Runs `f` as one edit of `storage`.
pub fn with_editing<R>(storage: &NSTextStorage, f: impl FnOnce() -> R) -> R {
    let _editing = EditingGuard::begin(storage);
    f()
}

/// An undo group, which ends once the guard drops, also when a panic unwinds past it.
#[must_use = "the group ends as soon as the guard drops"]
pub(crate) struct UndoGroupGuard<'a> {
    undo: &'a NSUndoManager,
    by_event: bool,
}

impl<'a> UndoGroupGuard<'a> {
    /// Opens a group inside the one open, or else a top-level group of its own, during which the manager does not
    /// open one per event.
    pub(crate) fn begin(undo: &'a NSUndoManager) -> Self {
        let by_event = undo.groupsByEvent() && undo.groupingLevel() == 0;
        if by_event {
            undo.setGroupsByEvent(false);
        }
        undo.beginUndoGrouping();
        Self { undo, by_event }
    }
}

impl Drop for UndoGroupGuard<'_> {
    fn drop(&mut self) {
        self.undo.endUndoGrouping();
        if self.by_event {
            self.undo.setGroupsByEvent(true);
        }
    }
}

/// Runs a closure once the guard drops, also when a panic unwinds past it: the reset of state that no other guard
/// covers, such as a field behind a `RefCell`.
#[must_use = "the closure runs as soon as the guard drops"]
pub struct OnDrop<F: FnOnce()>(Option<F>);

impl<F: FnOnce()> OnDrop<F> {
    pub fn new(f: F) -> Self {
        Self(Some(f))
    }
}

impl<F: FnOnce()> Drop for OnDrop<F> {
    fn drop(&mut self) {
        if let Some(f) = self.0.take() {
            f();
        }
    }
}

#[cfg(test)]
mod tests {
    use std::panic::{AssertUnwindSafe, catch_unwind};

    use objc2_foundation::NSString;

    use super::*;

    fn panics(f: impl FnOnce()) {
        assert!(catch_unwind(AssertUnwindSafe(f)).is_err());
    }

    #[test]
    fn a_flag_is_up_only_while_its_guard_lives() {
        let flag = Cell::new(false);
        {
            let _up = FlagGuard::raise(&flag);
            assert!(flag.get());
        }
        assert!(!flag.get());
    }

    #[test]
    fn a_panic_takes_the_flag_down() {
        let flag = Cell::new(false);
        panics(|| {
            let _up = FlagGuard::raise(&flag);
            panic!("in between");
        });
        assert!(!flag.get());
    }

    #[test]
    fn a_panic_still_runs_the_reset() {
        let reset = Cell::new(false);
        panics(|| {
            let _reset = OnDrop::new(|| reset.set(true));
            panic!("in between");
        });
        assert!(reset.get());
    }

    #[test]
    fn a_panic_ends_the_edit_of_a_storage() {
        let storage = NSTextStorage::new();
        panics(|| {
            with_editing(&storage, || {
                storage.replaceCharactersInRange_withString(
                    objc2_foundation::NSRange::new(0, 0),
                    &NSString::from_str("a"),
                );
                assert!(!storage.editedMask().is_empty());
                panic!("in between");
            });
        });
        assert!(storage.editedMask().is_empty(), "the storage processed the edit");
    }
}

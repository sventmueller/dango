//! Open objects by an id of their own (E97), which answers from worker threads carry instead of the object.

use std::cell::{Cell, RefCell};
use std::path::Path;

use objc2::Message;
use objc2::rc::{Retained, Weak};

use crate::window::Window;

/// Numbers that are never handed out twice, from 1 on.
#[derive(Default)]
pub struct Serial(Cell<u64>);

impl Serial {
    pub fn next(&self) -> u64 {
        self.0.set(self.0.get() + 1);
        self.0.get()
    }
}

/// Open objects by an id of their own, which answers from other threads carry instead of the object.
pub struct Registry<T> {
    next: Cell<u64>,
    entries: RefCell<Vec<(u64, T)>>,
}

impl<T> Default for Registry<T> {
    fn default() -> Self {
        Registry {
            next: Cell::new(1),
            entries: RefCell::new(Vec::new()),
        }
    }
}

impl<T: Clone> Registry<T> {
    /// The id the next object gets, which no object had before.
    pub fn next_id(&self) -> u64 {
        let id = self.next.get();
        self.next.set(id + 1);
        id
    }

    pub fn insert(&self, id: u64, object: T) {
        self.entries.borrow_mut().push((id, object));
    }

    pub fn remove(&self, id: u64) -> Option<T> {
        let mut entries = self.entries.borrow_mut();
        let index = entries.iter().position(|(other, _)| *other == id)?;
        Some(entries.remove(index).1)
    }

    /// The object with `id`, unless it closed in the meantime.
    pub fn get(&self, id: u64) -> Option<T> {
        self.entries
            .borrow()
            .iter()
            .find(|(other, _)| *other == id)
            .map(|(_, object)| object.clone())
    }

    /// The open objects in the order they opened.
    pub fn all(&self) -> Vec<T> {
        self.entries.borrow().iter().map(|(_, object)| object.clone()).collect()
    }

    pub fn find(&self, mut f: impl FnMut(&T) -> bool) -> Option<T> {
        self.entries
            .borrow()
            .iter()
            .find(|(_, object)| f(object))
            .map(|(_, object)| object.clone())
    }

    pub fn retain(&self, mut f: impl FnMut(&T) -> bool) {
        self.entries.borrow_mut().retain(|(_, object)| f(object));
    }
}

/// A panel or tab that belongs to one window and shows its folder.
pub trait WindowPart {
    /// The window, unless it closed.
    fn window(&self) -> Option<Window>;
    /// The folder of the window.
    fn root(&self) -> &Path;
}

impl<T: WindowPart + Message> Registry<Retained<T>> {
    /// Runs `f` with the part `id`, if its window is still open.
    pub fn with(&self, id: u64, f: impl FnOnce(&T)) {
        if let Some(part) = self.get(id) {
            f(&part);
        }
    }

    /// The parts of open windows; those of closed windows are let go.
    pub fn live(&self) -> Vec<Retained<T>> {
        self.retain(|part| part.window().is_some());
        self.all()
    }

    /// The parts of windows that show `root`, as given or with its links resolved.
    pub fn for_root(&self, root: &Path) -> Vec<Retained<T>> {
        let canonical = root.canonicalize().unwrap_or_else(|_| root.to_path_buf());
        self.live()
            .into_iter()
            .filter(|part| part.root() == root || part.root() == canonical)
            .collect()
    }

    /// The part of `window`.
    pub fn of(&self, window: Window) -> Option<Retained<T>> {
        self.find(|part| part.window() == Some(window))
    }

    /// Lets go of the parts of `window` and of closed windows.
    pub fn detach(&self, window: Window) {
        self.retain(|part| part.window().is_some_and(|w| w != window));
    }
}

impl<T: Message> Registry<Weak<T>> {
    /// The object `id`, unless it was closed and let go.
    pub(crate) fn load(&self, id: u64) -> Option<Retained<T>> {
        self.get(id)?.load()
    }

    /// Runs `f` with the object `id`, if it is still open.
    pub fn with(&self, id: u64, f: impl FnOnce(&T)) {
        if let Some(object) = self.load(id) {
            f(&object);
        }
    }

    /// The objects still open; the ids of the others are let go.
    pub fn live(&self) -> Vec<Retained<T>> {
        self.retain(|object| object.load().is_some());
        self.entries
            .borrow()
            .iter()
            .filter_map(|(_, object)| object.load())
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use std::rc::Rc;

    use super::*;

    #[test]
    fn an_answer_for_an_object_that_closed_finds_nothing() {
        let open: Registry<Rc<&str>> = Registry::default();
        let first = open.next_id();
        open.insert(first, Rc::new("first"));
        let second = open.next_id();
        open.insert(second, Rc::new("second"));
        let (tx, rx) = std::sync::mpsc::channel();
        let worker = std::thread::spawn(move || tx.send((first, "answer")).unwrap());
        open.remove(first);
        worker.join().unwrap();
        let (id, _answer) = rx.recv().unwrap();
        assert!(open.get(id).is_none());
        assert_eq!(open.get(second).as_deref(), Some(&"second"));
    }

    #[test]
    fn ids_are_not_given_twice() {
        let open: Registry<u8> = Registry::default();
        let first = open.next_id();
        open.insert(first, 1);
        open.remove(first);
        assert_ne!(open.next_id(), first);
    }
}

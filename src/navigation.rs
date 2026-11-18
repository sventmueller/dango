//! Back and forward through the places the user looked at in the editors of a window, as Xcode and VS Code keep them:
//! a place goes into the history when the user leaves it for another file or jumps more than `JUMP` lines away.

use std::cell::{OnceCell, RefCell};
use std::path::{Path, PathBuf};

use objc2::rc::{Retained, Weak};
use objc2::runtime::{AnyObject, NSObject, Sel};
use objc2::{MainThreadMarker, MainThreadOnly, define_class, msg_send, sel};
use objc2_app_kit::{NSImage, NSMenuItem, NSToolbarItem, NSToolbarItemGroup, NSToolbarItemGroupSelectionMode};
use objc2_foundation::{NSArray, NSObjectProtocol, NSRange};
use splat_i18n::t;

use crate::editor::EditorTextView;
use crate::util::{OnDrop, guarded, ns, symbol};
use crate::window::WindowController;

/// How many lines a move must cross within a file to leave a place behind.
const JUMP: usize = 10;
/// How many places each direction keeps.
const LIMIT: usize = 100;
/// The toolbar item of the two buttons.
pub(crate) const TOOLBAR_ID: &str = "ide.navigation";

/// A cursor position in a file.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Place {
    pub(crate) path: PathBuf,
    /// In UTF-16 code units, as the text view counts.
    pub(crate) offset: usize,
    pub(crate) line: usize,
}

/// The places of one window.
#[derive(Default, Debug)]
struct History {
    back: Vec<Place>,
    forward: Vec<Place>,
    here: Option<Place>,
    /// `here` is only where an editor was when it got the focus; the first move in it replaces it.
    tentative: bool,
    /// Back or forward is taking the user to a place, which records nothing.
    moving: bool,
}

impl History {
    /// The user is at `place` now; `focus` when an editor just got the focus rather than its cursor moving.
    fn arrive(&mut self, place: Place, focus: bool) {
        if self.moving {
            self.here = Some(place);
            self.tentative = false;
            return;
        }
        let Some(here) = self.here.take() else {
            self.here = Some(place);
            self.tentative = focus;
            return;
        };
        if here.path == place.path {
            let jumped = here.line.abs_diff(place.line) > JUMP;
            if focus {
                self.here = Some(here);
                return;
            }
            if jumped && !self.tentative {
                self.leave(here);
            }
            self.here = Some(place);
            self.tentative = false;
            return;
        }
        self.leave(here);
        self.here = Some(place);
        self.tentative = focus;
    }

    fn leave(&mut self, place: Place) {
        if self.back.last() != Some(&place) {
            self.back.push(place);
        }
        if self.back.len() > LIMIT {
            self.back.remove(0);
        }
        self.forward.clear();
    }

    /// The place before the one the user is at, which becomes the current one.
    fn back(&mut self) -> Option<Place> {
        let target = self.back.pop()?;
        if let Some(here) = self.here.take() {
            self.forward.push(here);
        }
        self.here = Some(target.clone());
        self.tentative = false;
        Some(target)
    }

    /// The place the last `back` left, which becomes the current one.
    fn forward(&mut self) -> Option<Place> {
        let target = self.forward.pop()?;
        if let Some(here) = self.here.take() {
            self.back.push(here);
        }
        self.here = Some(target.clone());
        self.tentative = false;
        Some(target)
    }
}

/// The back and forward history of every window and the target of the toolbar's arrows.
#[derive(Default)]
pub(crate) struct Navigation {
    histories: RefCell<Vec<(Weak<WindowController>, History)>>,
    navigator: OnceCell<Retained<Navigator>>,
    groups: RefCell<Vec<(Weak<NSToolbarItemGroup>, Weak<WindowController>)>>,
}

fn with_histories<R>(f: impl FnOnce(&RefCell<Vec<(Weak<WindowController>, History)>>) -> R) -> R {
    f(&crate::context::app().navigation().histories)
}

fn with_navigator<R>(f: impl FnOnce(&OnceCell<Retained<Navigator>>) -> R) -> R {
    f(&crate::context::app().navigation().navigator)
}

fn with_groups<R>(f: impl FnOnce(&RefCell<Vec<(Weak<NSToolbarItemGroup>, Weak<WindowController>)>>) -> R) -> R {
    f(&crate::context::app().navigation().groups)
}

fn with_history<R>(controller: &WindowController, f: impl FnOnce(&mut History) -> R) -> R {
    with_histories(|all| {
        let mut all = all.borrow_mut();
        all.retain(|(owner, _)| owner.load().is_some());
        let index = match all
            .iter()
            .position(|(owner, _)| owner.load().is_some_and(|o| std::ptr::eq(&*o, controller)))
        {
            Some(index) => index,
            None => {
                all.push((Weak::from(controller), History::default()));
                all.len() - 1
            }
        };
        f(&mut all[index].1)
    })
}

/// The editor `view` shows its cursor where it is now, after a move or as it got the focus.
pub(crate) fn moved(view: &EditorTextView, focus: bool) {
    let Some(controller) = view.controller() else { return };
    let Some(place) = place_of(view) else { return };
    with_history(&controller, |history| history.arrive(place, focus));
    refresh();
}

fn place_of(view: &EditorTextView) -> Option<Place> {
    let offset = view.selectedRange().location;
    let document = view.document();
    let path = document.path()?.to_path_buf();
    let text = document.text();
    let line = text.utf16_to_byte(offset).map_or(0, |byte| text.line_of(byte));
    Some(Place { path, offset, line })
}

/// Whether `controller` has a place to go back, or forward, to.
pub(crate) fn can_go(controller: &WindowController, back: bool) -> bool {
    with_history(controller, |history| {
        let places = if back { &history.back } else { &history.forward };
        !places.is_empty()
    })
}

/// Takes the user back, or forward, to the next place whose file still exists.
pub(crate) fn go(controller: &WindowController, back: bool) {
    loop {
        let target = with_history(
            controller,
            |history| {
                if back { history.back() } else { history.forward() }
            },
        );
        let Some(target) = target else { break };
        if target.path.is_file() {
            show(controller, &target);
            break;
        }
    }
    refresh();
}

fn show(controller: &WindowController, place: &Place) {
    with_history(controller, |history| history.moving = true);
    let moving = OnDrop::new(|| with_history(controller, |history| history.moving = false));
    controller.open_file(&place.path);
    if let Some(view) = editor_of(controller, &place.path) {
        let at = place.offset.min(view.document().text().len_utf16());
        view.setSelectedRange(NSRange::new(at, 0));
        view.scrollRangeToVisible(NSRange::new(at, 0));
        if let Some(window) = view.window() {
            window.makeFirstResponder(Some(&view));
        }
    }
    drop(moving);
    with_history(controller, |history| history.here = Some(place.clone()));
}

fn editor_of(controller: &WindowController, path: &Path) -> Option<Retained<EditorTextView>> {
    crate::editor::all_editors()
        .into_iter()
        .find(|v| v.document().path() == Some(path) && v.controller().is_some_and(|c| std::ptr::eq(&*c, controller)))
}

define_class!(
    // SAFETY: NSObject has no subclassing requirements and `Navigator` does not implement Drop.
    #[unsafe(super(NSObject))]
    #[thread_kind = MainThreadOnly]
    #[name = "IDENavigator"]
    pub(crate) struct Navigator;

    unsafe impl NSObjectProtocol for Navigator {}

    impl Navigator {
        #[unsafe(method(ideGoBack:))]
        fn go_back(&self, _sender: Option<&AnyObject>) {
            guarded((), || front(|controller| go(controller, true)));
        }

        #[unsafe(method(ideGoForward:))]
        fn go_forward(&self, _sender: Option<&AnyObject>) {
            guarded((), || front(|controller| go(controller, false)));
        }

        /// The toolbar's pair: its first button goes back, its second forward.
        #[unsafe(method(ideGoBackOrForward:))]
        fn go_back_or_forward(&self, sender: Option<&AnyObject>) {
            guarded((), || {
                let index = sender
                    .and_then(|s| s.downcast_ref::<NSToolbarItemGroup>())
                    .map_or(-1, |group| group.selectedIndex());
                if index >= 0 {
                    front(|controller| go(controller, index == 0));
                }
            });
        }

        #[unsafe(method(validateMenuItem:))]
        fn validate_menu_item(&self, item: &NSMenuItem) -> bool {
            guarded(false, || {
                let back = item.action() == Some(sel!(ideGoBack:));
                front_answers(|controller| can_go(controller, back))
            })
        }
    }
);

/// The object that menu items and the toolbar send back and forward to.
pub(crate) fn navigator(mtm: MainThreadMarker) -> Retained<Navigator> {
    with_navigator(|cell| {
        cell.get_or_init(|| {
            // SAFETY: NSObject's initializer, called once on the newly allocated object.
            unsafe { msg_send![Navigator::alloc(mtm), init] }
        })
        .clone()
    })
}

fn front(f: impl FnOnce(&WindowController)) {
    let Some(mtm) = MainThreadMarker::new() else { return };
    if let Some(controller) = crate::window::frontmost(mtm) {
        f(&controller);
    }
}

fn front_answers(f: impl FnOnce(&WindowController) -> bool) -> bool {
    MainThreadMarker::new()
        .and_then(crate::window::frontmost)
        .is_some_and(|controller| f(&controller))
}

/// The toolbar's back and forward buttons, side by side before the title as Finder has them.
pub(crate) fn toolbar_item(controller: &WindowController) -> Retained<NSToolbarItem> {
    let mtm = controller.mtm();
    let images: Vec<Retained<NSImage>> = [("chevron.backward", t("Zurück")), ("chevron.forward", t("Vorwärts"))]
        .into_iter()
        .filter_map(|(name, label)| symbol(name, label))
        .collect();
    let labels = NSArray::from_retained_slice(&[ns(t("Zurück")), ns(t("Vorwärts"))]);
    let navigator = navigator(mtm);
    // SAFETY: The navigator answers the action with a method that takes the sender; the group holds its target
    // weakly and the navigator lives as long as the app.
    let group = unsafe {
        NSToolbarItemGroup::groupWithItemIdentifier_images_selectionMode_labels_target_action(
            &ns(TOOLBAR_ID),
            &NSArray::from_retained_slice(&images),
            NSToolbarItemGroupSelectionMode::Momentary,
            Some(&labels),
            Some(&navigator),
            Some(sel!(ideGoBackOrForward:)),
            mtm,
        )
    };
    group.setLabel(&ns(&format!("{}/{}", t("Zurück"), t("Vorwärts"))));
    group.setNavigational(true);
    group.setAutovalidates(false);
    for item in group.subitems().iter() {
        item.setAutovalidates(false);
    }
    with_groups(|groups| groups.borrow_mut().push((Weak::from(&*group), Weak::from(controller))));
    refresh();
    Retained::into_super(group)
}

/// Enables the toolbar's buttons as their window has places to go to.
fn refresh() {
    let groups: Vec<(Retained<NSToolbarItemGroup>, Retained<WindowController>)> = with_groups(|groups| {
        let mut groups = groups.borrow_mut();
        groups.retain(|(group, owner)| group.load().is_some() && owner.load().is_some());
        groups
            .iter()
            .filter_map(|(group, owner)| Some((group.load()?, owner.load()?)))
            .collect()
    });
    for (group, owner) in groups {
        let (back, forward) = (can_go(&owner, true), can_go(&owner, false));
        for (index, item) in group.subitems().iter().enumerate() {
            item.setEnabled(if index == 0 { back } else { forward });
        }
    }
}

/// The names of the menu items' actions, for the Go menu.
pub(crate) fn actions() -> [(&'static str, Sel, &'static str); 2] {
    [
        (t("Zurück"), sel!(ideGoBack:), "go.back"),
        (t("Vorwärts"), sel!(ideGoForward:), "go.forward"),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(path: &str, line: usize) -> Place {
        Place {
            path: PathBuf::from(path),
            offset: line * 10,
            line,
        }
    }

    #[test]
    fn leaving_a_file_or_jumping_far_leaves_a_place_typing_does_not() {
        let mut history = History::default();
        history.arrive(at("a", 0), true);
        history.arrive(at("a", 3), false);
        history.arrive(at("a", 5), false);
        assert!(history.back.is_empty(), "small moves while typing");
        history.arrive(at("a", 40), false);
        assert_eq!(history.back, [at("a", 5)], "a jump within the file");
        history.arrive(at("b", 0), true);
        assert_eq!(history.back, [at("a", 5), at("a", 40)], "another file");
        history.arrive(at("b", 120), false);
        assert_eq!(
            history.back.len(),
            2,
            "the first move in a file just opened replaces where it opened"
        );
        assert_eq!(history.here, Some(at("b", 120)));
    }

    #[test]
    fn back_and_forward_walk_the_places_and_a_new_move_drops_forward() {
        let mut history = History::default();
        history.arrive(at("a", 0), false);
        history.arrive(at("b", 0), false);
        history.arrive(at("c", 0), false);
        assert_eq!(history.back(), Some(at("b", 0)));
        assert_eq!(history.back(), Some(at("a", 0)));
        assert_eq!(history.back(), None);
        assert_eq!(history.forward(), Some(at("b", 0)));
        history.arrive(at("d", 0), false);
        assert!(history.forward.is_empty(), "a new place ends the way forward");
        assert_eq!(history.back(), Some(at("b", 0)));
    }

    #[test]
    fn going_somewhere_records_nothing_on_the_way() {
        let mut history = History::default();
        history.arrive(at("a", 0), false);
        history.arrive(at("b", 50), false);
        let target = history.back();
        history.moving = true;
        history.arrive(at("a", 0), true);
        history.arrive(at("a", 0), false);
        history.moving = false;
        assert_eq!(target, Some(at("a", 0)));
        assert!(history.back.is_empty());
        assert_eq!(history.forward, [at("b", 50)]);
    }
}

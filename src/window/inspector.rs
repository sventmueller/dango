//! The inspector at the window's trailing edge (E94, E101): an `NSSplitViewItem` of the inspector kind in every window
//! tab, folded away until ⌥⌘I (`toggleInspector:`) shows it. Like the sidebar there is one per folder, which moves into
//! the tab in front with its width and whether it shows (E60); the width is read from the item's view controller, as
//! the split view has more subviews than items on macOS 27.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use objc2::rc::Retained;
use objc2::{DefinedClass, MainThreadMarker, MainThreadOnly};
use objc2_app_kit::{NSSplitViewItem, NSView, NSViewController};
use splat_i18n::t;

use super::WindowController;
use super::tabs::TabWindow;
use crate::util::pin_edges;

pub const MIN_WIDTH: f64 = 220.0;
pub const MAX_WIDTH: f64 = 520.0;
pub const DEFAULT_WIDTH: f64 = 280.0;

/// The folder's inspector: what it shows, whether it shows and how wide.
pub struct State {
    content: RefCell<Option<Retained<NSView>>>,
    shown: Cell<bool>,
    width: Cell<f64>,
}

impl Default for State {
    fn default() -> State {
        State {
            content: RefCell::new(None),
            shown: Cell::new(false),
            width: Cell::new(DEFAULT_WIDTH),
        }
    }
}

/// A window tab's inspector item, folded away, and the view the folder's inspector goes into.
pub(super) fn split_item(mtm: MainThreadMarker) -> (Retained<NSSplitViewItem>, Retained<NSView>) {
    let slot = NSView::new(mtm);
    let controller = NSViewController::new(mtm);
    controller.setView(&slot);
    let item = NSSplitViewItem::inspectorWithViewController(&controller);
    item.setCanCollapse(true);
    // Below the title bar: the window tabs' bar spans the whole window and would run over a full-height inspector.
    item.setAllowsFullHeightLayout(false);
    // The documents give the inspector its room; the window keeps its size.
    item.setCollapseBehavior(objc2_app_kit::NSSplitViewItemCollapseBehavior::PreferResizingSiblingsWithFixedSplitView);
    item.setCollapsed(true);
    item.setMinimumThickness(MIN_WIDTH);
    item.setMaximumThickness(MAX_WIDTH);
    // Above the documents', so resizing the window resizes them and the inspector keeps its width.
    item.setHoldingPriority(260.0);
    (item, slot)
}

impl TabWindow {
    pub fn inspector_shown(&self) -> bool {
        !self.inspector.isCollapsed()
    }

    /// The inspector's width now, from its view controller.
    pub fn inspector_width(&self) -> f64 {
        self.inspector
            .viewController(self.window.mtm())
            .view()
            .frame()
            .size
            .width
    }

    fn adopt_inspector(&self, view: &NSView) {
        // SAFETY: Asking a view for its superview on the main thread, where views live.
        let parent = unsafe { view.superview() };
        if parent.is_some_and(|parent| std::ptr::eq(&*parent, &*self.inspector_slot)) {
            return;
        }
        view.removeFromSuperview();
        for other in self.inspector_slot.subviews().iter() {
            other.removeFromSuperview();
        }
        self.inspector_slot.addSubview(view);
        pin_edges(view, &self.inspector_slot);
    }

    /// Shows or folds the inspector, holding it at `width` until the tab is laid out.
    fn set_inspector(&self, shown: bool, width: f64) {
        if shown {
            self.inspector.setMinimumThickness(width);
            self.inspector.setMaximumThickness(width);
        }
        if self.inspector.isCollapsed() == shown {
            self.inspector.setCollapsed(!shown);
        }
    }

    fn release_inspector(&self) {
        self.inspector.setMinimumThickness(MIN_WIDTH);
        self.inspector.setMaximumThickness(MAX_WIDTH);
    }
}

impl WindowController {
    /// Shows `view` in the folder's inspector, in whichever tab is in front.
    pub fn set_inspector_content(&self, view: &NSView) {
        *self.ivars().inspector.content.borrow_mut() = Some(objc2::Message::retain(view));
        if let Some(tab) = self.current_tab() {
            tab.adopt_inspector(view);
        }
    }

    /// Whether the inspector shows in the tab in front.
    #[cfg(feature = "selftest")]
    pub fn inspector_shown(&self) -> bool {
        self.current_tab().is_some_and(|tab| tab.inspector_shown())
    }

    /// Whether the inspector shows in each tab, the one in front marked, for the self-test.
    #[cfg(feature = "selftest")]
    pub fn inspector_states(&self) -> Vec<(bool, bool)> {
        let current = self.current_tab();
        self.ivars()
            .tabs
            .borrow()
            .iter()
            .map(|tab| {
                (
                    tab.inspector_shown(),
                    current.as_ref().is_some_and(|c| std::rc::Rc::ptr_eq(c, tab)),
                )
            })
            .collect()
    }

    /// The inspector's width in the tab in front.
    #[cfg(feature = "selftest")]
    pub fn inspector_width(&self) -> f64 {
        self.current_tab().map_or(0.0, |tab| tab.inspector_width())
    }

    /// Shows or folds the inspector, as ⌥⌘I does, in whichever tab is in front.
    pub fn show_inspector(&self, shown: bool) {
        let state = &self.ivars().inspector;
        state.shown.set(shown);
        let tabs = self.ivars().tabs.borrow().clone();
        for tab in &tabs {
            tab.set_inspector(shown, state.width.get());
        }
        self.later(|controller| {
            for tab in controller.ivars().tabs.borrow().iter() {
                tab.release_inspector();
            }
        });
    }

    /// The width and whether it shows, as the tab `tab` had them, for the next tab and the next launch.
    pub(super) fn remember_inspector(&self, tab: &TabWindow) {
        let state = &self.ivars().inspector;
        let shown = tab.inspector_shown();
        state.shown.set(shown);
        let width = tab.inspector_width();
        if shown && tab.window.isVisible() && width >= MIN_WIDTH - 1.0 {
            state.width.set(width.clamp(MIN_WIDTH, MAX_WIDTH));
        }
    }

    /// Moves the folder's inspector into `tab`, which comes to the front, as wide and as shown as it was.
    pub(super) fn inspector_follows(&self, tab: &Rc<TabWindow>) {
        let state = &self.ivars().inspector;
        let kept = state.content.borrow().clone();
        let content = kept.unwrap_or_else(|| self.empty_inspector());
        tab.adopt_inspector(&content);
        tab.set_inspector(state.shown.get(), state.width.get());
        let window = Retained::as_ptr(&tab.window) as usize;
        self.later(move |controller| {
            let held = controller
                .ivars()
                .tabs
                .borrow()
                .iter()
                .find(|tab| Retained::as_ptr(&tab.window) as usize == window)
                .cloned();
            if let Some(tab) = held {
                tab.release_inspector();
            }
        });
    }

    /// What the inspector says while nothing fills it.
    fn empty_inspector(&self) -> Retained<NSView> {
        let view = NSView::new(self.mtm());
        let state = super::panels::empty_state(
            self.mtm(),
            "sidebar.trailing",
            t("Inspektor"),
            t("Hier stehen die Variablen des angehaltenen Programms."),
            None,
        );
        view.addSubview(&state);
        state.setTranslatesAutoresizingMaskIntoConstraints(false);
        for constraint in [
            state.centerXAnchor().constraintEqualToAnchor(&view.centerXAnchor()),
            state.centerYAnchor().constraintEqualToAnchor(&view.centerYAnchor()),
        ] {
            constraint.setActive(true);
        }
        *self.ivars().inspector.content.borrow_mut() = Some(view.clone());
        view
    }

    /// Takes what the folder's inspector looked like at the last save.
    pub(super) fn set_saved_inspector(&self, shown: bool, width: f64) {
        let state = &self.ivars().inspector;
        state.shown.set(shown);
        if width > 0.0 {
            state.width.set(width.clamp(MIN_WIDTH, MAX_WIDTH));
        }
    }

    /// Whether it shows and how wide, for the next launch.
    pub(super) fn saved_inspector(&self) -> (bool, f64) {
        if let Some(tab) = self.current_tab() {
            self.remember_inspector(&tab);
        }
        let state = &self.ivars().inspector;
        (state.shown.get(), state.width.get())
    }
}

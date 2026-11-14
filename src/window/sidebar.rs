//! The sidebar of a window: its width per tab, which panel it shows, the symbols of the activity bar and their marks,
//! and moving the focus into a panel.

use objc2::DefinedClass;
use objc2::rc::Retained;
use objc2_app_kit::NSView;

#[cfg(feature = "selftest")]
use super::activity_bar::ActivityBar;
use super::{
    ACTIVITY_WIDTH, PANEL_MAX_WIDTH, PANEL_MIN_WIDTH, Panel, WindowController, first_shown, panels::Panels, tabs,
    tabs::TabWindow,
};
#[cfg(feature = "selftest")]
mod selftest_probe;

/// How long a panel's page stays in the sidebar after another panel showed, so switching back and forth is cheap and
/// a move of the sidebar into another window tab later moves only the page that shows.
const LEAVE_OUT: std::time::Duration = std::time::Duration::from_secs(2);

impl WindowController {
    /// Keeps the width the user gave the sidebar in `tab`.
    pub(super) fn remember_sidebar_width(&self, tab: &TabWindow) {
        let ivars = self.ivars();
        let width = tab.sidebar_width();
        if ivars.panel_open.get() && tab.window.isVisible() && width > ACTIVITY_WIDTH + PANEL_MIN_WIDTH - 1.0 {
            ivars
                .panel_width
                .set((width - ACTIVITY_WIDTH).clamp(PANEL_MIN_WIDTH, PANEL_MAX_WIDTH));
        }
    }

    /// Gives the sidebar of `tab` the width it has in the other tabs, which a tab that has not shown yet takes only
    /// held there; the user may drag it again once the tab is laid out.
    pub(super) fn fit_sidebar(&self, tab: &TabWindow, collapsed: bool) {
        let ivars = self.ivars();
        let open = ivars.panel_open.get();
        let width = if open {
            ACTIVITY_WIDTH + ivars.panel_width.get()
        } else {
            ACTIVITY_WIDTH
        };
        // A tab laid out at that width already keeps it, and pinning it again would lay out its window anew.
        if tab.sidebar_collapsed() == collapsed && (tab.sidebar_width() - width).abs() < 0.5 {
            return;
        }
        tab.pin_sidebar(width, collapsed);
        if open {
            // The tab itself, not the one in front then: another may come to the front meanwhile, and the held one
            // would stay held.
            let window = Retained::as_ptr(&tab.window) as usize;
            self.later(move |controller| {
                let held = controller
                    .ivars()
                    .tabs
                    .borrow()
                    .iter()
                    .find(|tab| Retained::as_ptr(&tab.window) as usize == window)
                    .cloned();
                if controller.ivars().panel_open.get()
                    && let Some(tab) = held
                {
                    tab.release_sidebar(ACTIVITY_WIDTH + PANEL_MIN_WIDTH, ACTIVITY_WIDTH + PANEL_MAX_WIDTH);
                }
            });
        }
    }

    /// Shows `panel`, opening the sidebar if it is closed; the panel that already shows stays open (E61).
    pub fn activate_panel(&self, panel: Panel) {
        if !self.ivars().panel_open.get() {
            self.set_panel_open(true);
        }
        self.show_panel(panel);
    }

    pub(super) fn show_panel(&self, panel: Panel) {
        self.ivars().panel.set(Some(panel));
        if let Some(panels) = self.ivars().panels.borrow().as_ref() {
            panels.show(panel);
        }
        let id = self.id();
        crate::util::main_after(LEAVE_OUT, move || {
            if let Some(controller) = crate::host::window(id)
                && let Some(panels) = controller.ivars().panels.borrow().as_ref()
            {
                panels.leave_out_hidden();
            }
        });
        if let Some(activity) = self.ivars().activity.borrow().as_ref() {
            activity.select(Some(panel));
        }
        if self.shown_panel() == Some(Panel::Containers) {
            crate::containers_panel::shown(self);
        }
    }

    /// The panel the sidebar shows, if it is open.
    pub fn shown_panel(&self) -> Option<Panel> {
        let ivars = self.ivars();
        ivars.panel_open.get().then(|| ivars.panel.get()).flatten()
    }

    /// Hides the activity bar's symbols of `hidden`; a hidden panel that showed gives way to the first that still has
    /// its symbol.
    pub(super) fn leave_out_symbols(&self, hidden: &[Panel]) {
        let ivars = self.ivars();
        if let Some(activity) = ivars.activity.borrow().as_ref() {
            activity.leave_out(hidden);
        }
        if let Some(shown) = ivars.panel.get().filter(|panel| hidden.contains(panel)) {
            let open = ivars.panel_open.get();
            self.show_panel(first_shown(shown, hidden));
            if !open && let Some(activity) = ivars.activity.borrow().as_ref() {
                activity.select(None);
            }
        }
    }

    /// Puts the keyboard focus into the panel beside the activity bar while it shows, else back into the active tab.
    pub(super) fn focus_panel(&self) {
        let Some(window) = self.window() else { return };
        let ivars = self.ivars();
        let page = ivars.panels.borrow().as_ref().and_then(Panels::shown_page);
        let target = page
            .as_deref()
            .filter(|_| ivars.panel_open.get())
            .and_then(tabs::first_responder_in);
        if let Some(view) = target {
            window.makeFirstResponder(Some(&view));
            return;
        }
        let focus = window
            .firstResponder()
            .and_then(|responder| Retained::downcast::<NSView>(responder).ok());
        let in_panel = ivars
            .panels
            .borrow()
            .as_ref()
            .is_some_and(|panels| focus.is_some_and(|focus| focus.isDescendantOf(panels.view())));
        if in_panel
            && let Some(tab) = self.current_tab()
            && let Some(document) = tab.focused()
        {
            tab.activate(document.id);
        }
    }

    /// Opens or closes the panel beside the activity bar, which stays.
    pub(super) fn set_panel_open(&self, open: bool) {
        let ivars = self.ivars();
        let Some(tab) = self.current_tab() else { return };
        if let Some(panels) = ivars.panels.borrow().as_ref() {
            panels.view().setHidden(!open);
        }
        if !open {
            self.remember_sidebar_width(&tab);
        }
        ivars.panel_open.set(open);
        if open && self.shown_panel() == Some(Panel::Containers) {
            crate::containers_panel::shown(self);
        }
        self.fit_sidebar(&tab, tab.sidebar_collapsed());
        if let Some(activity) = ivars.activity.borrow().as_ref() {
            activity.select(open.then(|| ivars.panel.get()).flatten());
        }
    }

    /// Names `words`, such as a failed CI run, in the tooltip of `panel`'s symbol; `None` names only the panel.
    pub fn set_symbol_status(&self, panel: Panel, words: Option<&str>) {
        if let Some(activity) = self.ivars().activity.borrow().as_ref() {
            activity.set_status(panel, words);
        }
    }

    /// Names `count` changes in the tooltip of `panel`'s symbol.
    pub fn set_symbol_count(&self, panel: Panel, count: usize) {
        if let Some(activity) = self.ivars().activity.borrow().as_ref() {
            activity.set_count(panel, count);
        }
    }

    /// Shows `view` as the page of `panel`.
    pub fn set_panel_page(&self, panel: Panel, view: &NSView) {
        if let Some(panels) = self.ivars().panels.borrow().as_ref() {
            panels.set_page(panel, view);
        }
    }

    /// Shows `view` at the end of the title row of `panel`.
    pub fn set_panel_accessory(&self, panel: Panel, view: &NSView) {
        if let Some(panels) = self.ivars().panels.borrow().as_ref() {
            panels.set_accessory(panel, view);
        }
    }
}

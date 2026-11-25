//! What the self-test reads from and does with a window, through the code its controls run.

use objc2::rc::Retained;
use objc2::{DefinedClass, MainThreadOnly};
use objc2_app_kit::NSView;
use splat_i18n::t;

use super::{Panel, TabId, WindowController, new_tab_button, tabs};
use crate::util::ns;

impl WindowController {
    /// The agents the menu of new tabs lists under "In eigenem Worktree".
    pub(crate) fn worktree_menu_titles(&self) -> Vec<String> {
        menu_titles(&self.new_tab_menu(), &[t("In eigenem Worktree")])
    }

    pub(crate) fn inspect(&self) -> Inspection {
        Inspection {
            activity_buttons: self.activity_button_count(),
            panel: self.shown_panel(),
            panel_title: self.panel_title(),
            tabs: self.tab_count(),
            new_tab_items: menu_titles(&self.new_tab_menu(), &[]),
            sidebar_width: self.current_tab().map_or(0.0, |tab| tab.sidebar_width()),
        }
    }
}

/// The titles in the submenu of `menu` that `path` names by the titles of its parents.
fn menu_titles(menu: &objc2_app_kit::NSMenu, path: &[&str]) -> Vec<String> {
    let mut menu = objc2::Message::retain(menu);
    for title in path {
        let next = menu
            .itemArray()
            .iter()
            .find(|item| item.title().to_string() == *title)
            .and_then(|item| item.submenu());
        let Some(next) = next else { return Vec::new() };
        menu = next;
    }
    menu.itemArray()
        .iter()
        .filter(|item| !item.isSeparatorItem())
        .map(|item| item.title().to_string())
        .collect()
}

impl WindowController {
    pub(crate) fn has_sheet(&self) -> bool {
        self.window().is_some_and(|w| w.attachedSheet().is_some())
    }

    /// How many documents the window shows.
    pub(crate) fn tab_count(&self) -> usize {
        self.documents().len()
    }

    /// The view of the document commands act on.
    pub(crate) fn selected_view(&self) -> Option<Retained<NSView>> {
        self.current_tab()?.focused().map(|doc| doc.view.clone())
    }

    /// The titles of all documents, tab by tab.
    pub(crate) fn tab_titles(&self) -> Vec<String> {
        self.documents().iter().map(|doc| doc.title()).collect()
    }

    /// The titles of each window tab's documents, in the order of the tab bar.
    pub(crate) fn window_tabs(&self) -> Vec<Vec<String>> {
        self.ordered_tabs()
            .iter()
            .map(|tab| tab.documents().iter().map(|doc| doc.title()).collect())
            .collect()
    }

    /// The numbers of each window tab's documents.
    pub(crate) fn window_tab_ids(&self) -> Vec<Vec<TabId>> {
        self.ordered_tabs()
            .iter()
            .map(|tab| tab.documents().iter().map(|doc| doc.id).collect())
            .collect()
    }

    /// What each native tab says: its title, its tooltip and whether it shows unsaved changes.
    pub(crate) fn native_tabs(&self) -> Vec<(String, String, bool)> {
        self.ordered_tabs()
            .iter()
            .map(|tab| {
                let native = tab.window.tab();
                (
                    native.title().to_string(),
                    native.toolTip().to_string(),
                    tab.window.isDocumentEdited(),
                )
            })
            .collect()
    }

    /// Whether each tab, in the order of the tab bar, shows an icon before its title.
    pub(crate) fn tab_icons(&self) -> Vec<bool> {
        self.ordered_tabs()
            .iter()
            .map(|tab| {
                tab.window
                    .tab()
                    .attributedTitle()
                    .is_some_and(|title| title.string().to_string().starts_with('\u{fffc}'))
            })
            .collect()
    }

    /// The left document's share of each window tab that shows two.
    pub(crate) fn tab_shares(&self) -> Vec<Option<f64>> {
        self.ordered_tabs().iter().map(|tab| tab.share()).collect()
    }

    /// Whether each tab, in the order of the tab bar, shows the picture of the sidebar in its place.
    pub(crate) fn sidebar_pictures(&self) -> Vec<bool> {
        self.ordered_tabs().iter().map(|tab| tab.shows_picture()).collect()
    }

    /// Whether the window in front shows its tab bar.
    pub(crate) fn tab_bar_visible(&self) -> bool {
        self.window()
            .and_then(|window| window.tabGroup())
            .is_some_and(|group| group.isTabBarVisible())
    }

    /// Opens the menu of new tabs, as ⌘T does; the self-test only records it.
    pub(crate) fn open_new_tab_menu(&self) {
        new_tab_button::open(self);
    }

    /// Brings the next tab to the front, as ⇧⌘] does.
    pub(crate) fn select_next_tab(&self) {
        self.select_relative(true);
    }

    /// Moves the window tab in front to place `index` of its tab bar, as dragging its tab there does.
    pub(crate) fn move_front_tab_to(&self, index: usize) {
        let Some(tab) = self.current_tab() else { return };
        if let Some(group) = tab.window.tabGroup() {
            group.insertWindow_atIndex(&tab.window, isize::try_from(index).unwrap_or(0));
        }
    }

    /// Closes every tab of the window in front, as ⇧⌘W does.
    pub(crate) fn close_all_tabs(&self) {
        self.close_window();
    }

    /// Clicks the close button of the window in front with the mouse, which closes its whole tab group; whether it
    /// had one.
    pub(crate) fn click_close_button(&self) -> bool {
        let Some(window) = self.window() else { return false };
        let Some(button) = window.standardWindowButton(objc2_app_kit::NSWindowButton::CloseButton) else {
            return false;
        };
        let bounds = button.bounds();
        let center = objc2_foundation::NSPoint::new(bounds.size.width / 2.0, bounds.size.height / 2.0);
        post_click(&window, button.convertPoint_toView(center, None));
        true
    }

    /// Sets the left document's share of the tab that shows the document `id` beside another.
    pub(crate) fn set_share_of(&self, id: TabId, share: f64) {
        if let Some(tab) = self.tab_of_document(id) {
            tab.set_share(share);
        }
    }

    /// What the actions marked since the last reading came to.
    pub(crate) fn take_ticks(&self) -> super::TickReading {
        self.ivars().ticks.take()
    }

    /// The menu of new tabs, filled as when the "+" opens it.
    pub(crate) fn new_tab_menu(&self) -> Retained<objc2_app_kit::NSMenu> {
        let menu = tabs::new_tab_menu(self.mtm(), &super::agent_choices::now(self.context()));
        tabs::fill_new_tab_menu(self, &menu);
        menu
    }

    /// Makes the menu of new tabs only recorded from now on, never shown, so the self-test can ask for it.
    pub(crate) fn record_new_tab_menu(&self) {
        new_tab_button::record_only();
    }

    /// Whether the menu of new tabs was asked for since the last call, and whether below the "+".
    pub(crate) fn new_tab_menu_recorded(&self) -> Option<bool> {
        new_tab_button::take_recorded()
    }

    /// Moves the keyboard focus to the other document of the tab in front, as the menu command does.
    pub(crate) fn focus_next_panel(&self) {
        if let Some(tab) = self.current_tab() {
            tab.focus_next_pane();
        }
    }

    /// The title of each document, tab by tab, whether it is the preview document, and the tooltip of its tab.
    pub(crate) fn tab_previews(&self) -> Vec<(String, bool, String)> {
        self.ordered_tabs()
            .iter()
            .flat_map(|tab| {
                let tip = tab.window.tab().toolTip().to_string();
                tab.documents()
                    .into_iter()
                    .map(move |doc| (doc.title(), doc.is_preview(), tip.clone()))
            })
            .collect()
    }

    /// The view of the folder tree.
    pub(crate) fn file_tree_outline(&self) -> Option<Retained<objc2_app_kit::NSOutlineView>> {
        self.file_tree_view()
    }

    /// The size of the text of the title of the native tab in front.
    pub(crate) fn tab_title_points(&self) -> f64 {
        let Some(title) = self.current_tab().and_then(|tab| tab.window.tab().attributedTitle()) else {
            return 0.0;
        };
        let last = title.length().saturating_sub(1);
        // SAFETY: NSFontAttributeName is a constant AppKit sets when it loads, the index lies inside the title, and a
        // null range asks for none.
        let font = unsafe {
            title.attribute_atIndex_effectiveRange(objc2_app_kit::NSFontAttributeName, last, std::ptr::null_mut())
        };
        font.and_then(|font| font.downcast::<objc2_app_kit::NSFont>().ok())
            .map_or(0.0, |font| font.pointSize())
    }

    /// The narrowest and widest the user may drag the sidebar of the window tab of document `id` to now.
    pub(crate) fn sidebar_range_of(&self, id: TabId) -> Option<(f64, f64)> {
        self.tab_of_document(id).map(|tab| tab.sidebar_range())
    }

    /// Moves the sidebar's edge in the window tab in front to `width`, as a drag of it does, and gives the width the
    /// sidebar took and the narrowest it may take.
    pub(crate) fn drag_sidebar(&self, width: f64) -> Option<(f64, f64)> {
        self.current_tab()
            .map(|tab| (tab.drag_sidebar(width), tab.sidebar_range().0))
    }

    /// Folds the sidebar away or back, as ⌃⌘S does.
    pub(crate) fn toggle_sidebar(&self) {
        if let Some(tab) = self.current_tab() {
            tab.toggle_sidebar();
        }
    }

    /// Whether the sidebar of the tab in front is folded away.
    pub(crate) fn sidebar_folded(&self) -> bool {
        self.current_tab().is_some_and(|tab| tab.sidebar_collapsed())
    }

    /// Clicks the tab bar's "+" with the mouse, a press and a release at once, as the user does.
    pub(crate) fn click_plus(&self) -> bool {
        let Some(window) = self.window() else { return false };
        let Some(button) = new_tab_button::plus_button(&window) else {
            return false;
        };
        let bounds = button.bounds();
        let center = objc2_foundation::NSPoint::new(bounds.size.width / 2.0, bounds.size.height / 2.0);
        post_click(&window, button.convertPoint_toView(center, None));
        true
    }

    /// The entries of the "+"'s menu.
    pub(crate) fn plus_menu_titles(&self) -> Vec<String> {
        menu_titles(&new_tab_button::menu_for(self), &[])
    }

    /// The entries of the "+"'s menu that do something, each as the titles that lead to it and its action, with
    /// whether it is enabled.
    pub(crate) fn plus_menu_entries(&self) -> Vec<(Vec<String>, String, bool)> {
        fn walk(menu: &objc2_app_kit::NSMenu, path: &mut Vec<String>, out: &mut Vec<(Vec<String>, String, bool)>) {
            menu.update();
            for item in menu.itemArray().iter() {
                if item.isSeparatorItem() || item.isHidden() {
                    continue;
                }
                path.push(item.title().to_string());
                if let Some(submenu) = item.submenu() {
                    walk(&submenu, path, out);
                } else if let Some(action) = item.action() {
                    out.push((
                        path.clone(),
                        action.name().to_string_lossy().into_owned(),
                        item.isEnabled(),
                    ));
                }
                path.pop();
            }
        }
        let mut out = Vec::new();
        walk(&new_tab_button::menu_for(self), &mut Vec::new(), &mut out);
        out
    }

    /// The title of the document in front.
    pub(crate) fn front_title(&self) -> Option<String> {
        let id = self.selected_tab()?;
        self.documents()
            .into_iter()
            .find(|doc| doc.id == id)
            .map(|doc| doc.title())
    }

    /// Chooses the entry that `path` names, by its title and those of its submenus, from the "+"'s menu.
    pub(crate) fn choose_from_plus_menu(&self, path: &[&str]) -> bool {
        let mut menu = new_tab_button::menu_for(self);
        for (n, title) in path.iter().enumerate() {
            let index = menu.indexOfItemWithTitle(&ns(title));
            let Some(item) = (index >= 0).then(|| menu.itemAtIndex(index)).flatten() else {
                return false;
            };
            if n + 1 == path.len() {
                menu.update();
                if !item.isEnabled() {
                    return false;
                }
                menu.performActionForItemAtIndex(index);
                return true;
            }
            let Some(submenu) = item.submenu() else { return false };
            menu = submenu;
        }
        false
    }

    /// The left document's share of the tab that shows the document `id` beside another.
    pub(crate) fn share_of(&self, id: TabId) -> Option<f64> {
        self.tab_of_document(id)?.share()
    }

    /// What the badges in the tab of the document `id` say, left to right.
    pub(crate) fn badges_of(&self, id: TabId) -> Vec<Option<(String, String)>> {
        self.tab_of_document(id).map(|tab| tab.badges()).unwrap_or_default()
    }

    /// Moves the divider of the tab of the document `id` to `share`, as a drag does.
    pub(crate) fn drag_share_of(&self, id: TabId, share: f64) {
        if let Some(tab) = self.tab_of_document(id) {
            tab.drag_share(share);
        }
    }

    /// The title of the format bar over the tab of the document `id`, while it shows.
    pub(crate) fn format_bar_title(&self, id: TabId) -> Option<String> {
        self.tab_of_document(id)?.format_bar().shown_title()
    }

    /// The button of the format bar over the tab of the document `id` that runs `command`.
    pub(crate) fn format_bar_button(
        &self,
        id: TabId,
        command: crate::markdown::Command,
    ) -> Option<Retained<objc2_app_kit::NSButton>> {
        self.tab_of_document(id)?.format_bar().button_of(command)
    }

    /// How far the toolbar's window slid down from the window's top, which the sidebar follows in full screen.
    pub(crate) fn toolbar_slide(&self) -> f64 {
        self.window()
            .map_or(f64::NAN, |window| super::full_screen::toolbar_slide(&window))
    }

    /// Whether each document, tab by tab, has unsaved changes as its tab shows them.
    pub(crate) fn tab_dirty_marks(&self) -> Vec<bool> {
        self.documents().iter().map(|doc| doc.is_dirty()).collect()
    }

    /// The note at the foot of the documents, while it shows.
    pub(crate) fn notice(&self) -> Option<String> {
        let notice = self.ivars().notice.borrow();
        let (note, text) = notice.as_ref()?;
        // SAFETY: Asking for the superview only reads; the note is alive as the ivar holds it.
        unsafe { note.superview() }.map(|_| text.clone())
    }
}

/// What the self-test reads from a window.
pub(crate) struct Inspection {
    pub(crate) activity_buttons: usize,
    pub(crate) panel: Option<Panel>,
    pub(crate) panel_title: String,
    pub(crate) tabs: usize,
    pub(crate) new_tab_items: Vec<String>,
    pub(crate) sidebar_width: f64,
}

/// A press and a release of the left mouse button at `at` in `window`, queued as the window server delivers them.
fn post_click(window: &objc2_app_kit::NSWindow, at: objc2_foundation::NSPoint) {
    let app = objc2_app_kit::NSApplication::sharedApplication(objc2::MainThreadOnly::mtm(window));
    for kind in [
        objc2_app_kit::NSEventType::LeftMouseDown,
        objc2_app_kit::NSEventType::LeftMouseUp,
    ] {
        let event = objc2_app_kit::NSEvent::mouseEventWithType_location_modifierFlags_timestamp_windowNumber_context_eventNumber_clickCount_pressure(
            kind,
            at,
            objc2_app_kit::NSEventModifierFlags::empty(),
            0.0,
            window.windowNumber(),
            None,
            0,
            1,
            1.0,
        );
        if let Some(event) = event {
            app.postEvent_atStart(&event, false);
        }
    }
}

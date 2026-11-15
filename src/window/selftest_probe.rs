//! What the self-test reads from and does with a window, through the code its controls run.

use std::path::Path;

use objc2::rc::Retained;
use objc2::{DefinedClass, MainThreadOnly};
use objc2_app_kit::{NSView, NSWindow};
use splat_i18n::t;

use super::{
    ACTIVITY_WIDTH, PANEL_MAX_WIDTH, PANEL_MIN_WIDTH, Panel, SearchPanel, TabId, WindowController,
    activity_bar::ActivityBar, files::FileTree, new_tab_button, panels, panels::Panels, tabs,
};
use crate::util::ns;

impl WindowController {
    /// The agents the menu of new tabs lists under "In eigenem Worktree".
    pub fn worktree_menu_titles(&self) -> Vec<String> {
        menu_titles(&self.new_tab_menu(), &[t("In eigenem Worktree")])
    }

    pub fn inspect(&self) -> Inspection {
        let ivars = self.ivars();
        Inspection {
            activity_buttons: ivars.activity.borrow().as_ref().map_or(0, ActivityBar::button_count),
            panel: ivars.panel_open.get().then(|| ivars.panel.get()).flatten(),
            panel_title: ivars.panels.borrow().as_ref().map(Panels::title).unwrap_or_default(),
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
    /// The project search of the files panel.
    pub fn search_panel(&self) -> Option<Retained<SearchPanel>> {
        self.ivars().search.borrow().clone()
    }

    pub fn tree_rows(&self) -> isize {
        self.ivars().files.borrow().as_ref().map_or(0, FileTree::row_count)
    }

    pub fn has_sheet(&self) -> bool {
        self.window().is_some_and(|w| w.attachedSheet().is_some())
    }

    /// Opens Open Quickly with `query` typed into it.
    pub fn quick_open_with(&self, query: &str) {
        self.show_quick_open("");
        if let Some(panel) = self.ivars().quick_open.borrow().as_ref() {
            panel.type_query(query);
        }
    }

    pub fn close_quick_open(&self) {
        if let Some(panel) = self.ivars().quick_open.borrow().as_ref() {
            panel.close();
        }
    }

    /// What Open Quickly says in place of rows, if anything.
    pub fn quick_open_status(&self) -> Option<String> {
        self.ivars().quick_open.borrow().as_ref().and_then(|p| p.status())
    }

    /// Opens row `row` of Open Quickly as Return does.
    pub fn quick_open_row(&self, row: usize) {
        let panel = self.ivars().quick_open.borrow().clone();
        if let Some(panel) = panel {
            panel.open_row(row);
        }
    }

    pub fn quick_open_hits(&self) -> Vec<String> {
        self.ivars()
            .quick_open
            .borrow()
            .as_ref()
            .map(|p| p.hits())
            .unwrap_or_default()
    }

    /// How many documents the window shows.
    pub fn tab_count(&self) -> usize {
        self.documents().len()
    }

    /// The tooltip of `panel`'s symbol, with the marks that show on any symbol.
    pub fn symbol_tip(&self, panel: Panel) -> (Option<String>, usize) {
        let activity = self.ivars().activity.borrow();
        activity.as_ref().map_or((None, 0), |a| (a.tip(panel), a.marks()))
    }

    /// The view of the document commands act on.
    pub fn selected_view(&self) -> Option<Retained<NSView>> {
        self.current_tab()?.focused().map(|doc| doc.view.clone())
    }

    /// The titles of all documents, tab by tab.
    pub fn tab_titles(&self) -> Vec<String> {
        self.documents().iter().map(|doc| doc.title()).collect()
    }

    /// The titles of each window tab's documents, in the order of the tab bar.
    pub fn window_tabs(&self) -> Vec<Vec<String>> {
        self.ordered_tabs()
            .iter()
            .map(|tab| tab.documents().iter().map(|doc| doc.title()).collect())
            .collect()
    }

    /// The numbers of each window tab's documents.
    pub fn window_tab_ids(&self) -> Vec<Vec<TabId>> {
        self.ordered_tabs()
            .iter()
            .map(|tab| tab.documents().iter().map(|doc| doc.id).collect())
            .collect()
    }

    /// What each native tab says: its title, its tooltip and whether it shows unsaved changes.
    pub fn native_tabs(&self) -> Vec<(String, String, bool)> {
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
    pub fn tab_icons(&self) -> Vec<bool> {
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
    pub fn tab_shares(&self) -> Vec<Option<f64>> {
        self.ordered_tabs().iter().map(|tab| tab.share()).collect()
    }

    /// Whether each tab, in the order of the tab bar, shows the picture of the sidebar in its place.
    pub fn sidebar_pictures(&self) -> Vec<bool> {
        self.ordered_tabs().iter().map(|tab| tab.shows_picture()).collect()
    }

    /// The window the sidebar is in now.
    pub fn sidebar_window(&self) -> Option<Retained<NSWindow>> {
        self.ivars().sidebar.borrow().as_ref()?.window()
    }

    /// Whether the window in front shows its tab bar.
    pub fn tab_bar_visible(&self) -> bool {
        self.window()
            .and_then(|window| window.tabGroup())
            .is_some_and(|group| group.isTabBarVisible())
    }

    /// Opens the menu of new tabs, as ⌘T does; the self-test only records it.
    pub fn open_new_tab_menu(&self) {
        new_tab_button::open(self);
    }

    /// Brings the next tab to the front, as ⇧⌘] does.
    pub fn select_next_tab(&self) {
        self.select_relative(true);
    }

    /// Moves the window tab in front to place `index` of its tab bar, as dragging its tab there does.
    pub fn move_front_tab_to(&self, index: usize) {
        let Some(tab) = self.current_tab() else { return };
        if let Some(group) = tab.window.tabGroup() {
            group.insertWindow_atIndex(&tab.window, isize::try_from(index).unwrap_or(0));
        }
    }

    /// Closes every tab of the window in front, as ⇧⌘W does.
    pub fn close_all_tabs(&self) {
        self.close_window();
    }

    /// Clicks the close button of the window in front with the mouse, which closes its whole tab group; whether it
    /// had one.
    pub fn click_close_button(&self) -> bool {
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
    pub fn set_share_of(&self, id: TabId, share: f64) {
        if let Some(tab) = self.tab_of_document(id) {
            tab.set_share(share);
        }
    }

    /// The menu of new tabs, filled as when the "+" opens it.
    pub fn new_tab_menu(&self) -> Retained<objc2_app_kit::NSMenu> {
        let menu = tabs::new_tab_menu(self.mtm(), &super::agent_choices::now(self.context()));
        tabs::fill_new_tab_menu(self, &menu);
        menu
    }

    /// Makes the menu of new tabs only recorded from now on, never shown, so the self-test can ask for it.
    pub fn record_new_tab_menu(&self) {
        new_tab_button::record_only();
    }

    /// Whether the menu of new tabs was asked for since the last call, and whether below the "+".
    pub fn new_tab_menu_recorded(&self) -> Option<bool> {
        new_tab_button::take_recorded()
    }

    /// Moves the keyboard focus to the other document of the tab in front, as the menu command does.
    pub fn focus_next_panel(&self) {
        if let Some(tab) = self.current_tab() {
            tab.focus_next_pane();
        }
    }

    /// The title of each document, tab by tab, whether it is the preview document, and the tooltip of its tab.
    pub fn tab_previews(&self) -> Vec<(String, bool, String)> {
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
    pub fn file_tree_outline(&self) -> Option<Retained<objc2_app_kit::NSOutlineView>> {
        self.file_tree_view()
    }

    /// Selects the row of `path` in the folder tree, as a click on it does.
    pub fn select_in_tree(&self, path: &Path) {
        let files = self.ivars().files.borrow();
        if let Some(files) = files.as_ref() {
            files.select(path);
        }
    }

    /// Gives the sidebar `width`, as a drag of its edge does.
    pub fn set_sidebar_width(&self, width: f64) {
        let ivars = self.ivars();
        ivars
            .panel_width
            .set((width - ACTIVITY_WIDTH).clamp(PANEL_MIN_WIDTH, PANEL_MAX_WIDTH));
        if let Some(tab) = self.current_tab() {
            self.fit_sidebar(&tab, tab.sidebar_collapsed());
        }
    }

    /// The narrowest and widest the user may drag the sidebar of the window tab of document `id` to now.
    pub fn sidebar_range_of(&self, id: TabId) -> Option<(f64, f64)> {
        self.tab_of_document(id).map(|tab| tab.sidebar_range())
    }

    /// Folds the sidebar away or back, as ⌃⌘S does.
    pub fn toggle_sidebar(&self) {
        if let Some(tab) = self.current_tab() {
            tab.toggle_sidebar();
        }
    }

    /// Whether the sidebar of the tab in front is folded away.
    pub fn sidebar_folded(&self) -> bool {
        self.current_tab().is_some_and(|tab| tab.sidebar_collapsed())
    }

    /// Clicks the tab bar's "+" with the mouse, a press and a release at once, as the user does.
    pub fn click_plus(&self) -> bool {
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
    pub fn plus_menu_titles(&self) -> Vec<String> {
        menu_titles(&new_tab_button::menu_for(self), &[])
    }

    /// The entries of the "+"'s menu that do something, each as the titles that lead to it and its action, with
    /// whether it is enabled.
    pub fn plus_menu_entries(&self) -> Vec<(Vec<String>, String, bool)> {
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
    pub fn front_title(&self) -> Option<String> {
        let id = self.selected_tab()?;
        self.documents()
            .into_iter()
            .find(|doc| doc.id == id)
            .map(|doc| doc.title())
    }

    /// Chooses the entry that `path` names, by its title and those of its submenus, from the "+"'s menu.
    pub fn choose_from_plus_menu(&self, path: &[&str]) -> bool {
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
    pub fn share_of(&self, id: TabId) -> Option<f64> {
        self.tab_of_document(id)?.share()
    }

    /// What the badges in the tab of the document `id` say, left to right.
    pub fn badges_of(&self, id: TabId) -> Vec<Option<(String, String)>> {
        self.tab_of_document(id).map(|tab| tab.badges()).unwrap_or_default()
    }

    /// Moves the divider of the tab of the document `id` to `share`, as a drag does.
    pub fn drag_share_of(&self, id: TabId, share: f64) {
        if let Some(tab) = self.tab_of_document(id) {
            tab.drag_share(share);
        }
    }

    /// The close button on the badge of the document `id`.
    pub fn badge_close(&self, id: TabId) -> Option<Retained<objc2_app_kit::NSButton>> {
        self.tab_of_document(id)?.badge_close(id)
    }

    /// Whether the shown panel's title row shows, and the texts and buttons of its page.
    pub fn panel_seen(&self) -> Option<(bool, Vec<String>)> {
        self.ivars().panels.borrow().as_ref().map(panels::Panels::seen)
    }

    /// The shown panel's title and the geometry of its title row.
    pub fn panel_title_row(&self) -> Option<(String, panels::TitleRow)> {
        let panels = self.ivars().panels.borrow();
        let panels = panels.as_ref()?;
        Some((panels.title(), panels.title_row()))
    }

    /// In window coordinates, y up: the top of the tab bar, of the sidebar's safe area, and of the activity bar.
    pub fn sidebar_top(&self) -> (f64, f64, f64) {
        fn tab_bar(view: &NSView) -> Option<Retained<NSView>> {
            if view.class().name().to_bytes() == b"NSTabBar" {
                return Some(objc2::Message::retain(view));
            }
            view.subviews().iter().find_map(|child| tab_bar(&child))
        }
        let top = |view: &NSView, rect: objc2_foundation::NSRect| {
            let rect = view.convertRect_toView(rect, None);
            rect.origin.y + rect.size.height
        };
        let window = self.window();
        // SAFETY: Asking for the superview only reads; the frame view holds the content view while the window shows.
        let frame = window
            .as_ref()
            .and_then(|w| w.contentView())
            .and_then(|v| unsafe { v.superview() });
        let tabs = frame
            .and_then(|f| tab_bar(&f))
            .map_or(f64::NAN, |bar| top(&bar, bar.bounds()));
        let sidebar = self.ivars().sidebar.borrow();
        let (safe, activity) = sidebar.as_ref().map_or((f64::NAN, f64::NAN), |sidebar| {
            sidebar.layoutSubtreeIfNeeded();
            let safe = top(sidebar, sidebar.safeAreaLayoutGuide().frame());
            let activity = self.ivars().activity.borrow();
            (
                safe,
                activity.as_ref().map_or(f64::NAN, |a| top(a.view(), a.view().bounds())),
            )
        });
        (tabs, safe, activity)
    }

    /// How far the toolbar's window slid down from the window's top, which the sidebar follows in full screen.
    pub fn toolbar_slide(&self) -> f64 {
        self.window()
            .map_or(f64::NAN, |window| super::full_screen::toolbar_slide(&window))
    }

    /// Lays the sidebar out below a toolbar that reaches `toolbar` down from its top, as one in full screen does.
    pub fn push_sidebar_below_toolbar(&self, toolbar: f64) {
        self.keep_sidebar_below_toolbar(toolbar);
        if let Some(sidebar) = self.ivars().sidebar.borrow().as_ref() {
            sidebar.layoutSubtreeIfNeeded();
        }
    }

    /// The button of the files panel's header that sends `action`.
    pub fn files_header_button(&self, action: objc2::runtime::Sel) -> Option<Retained<objc2_app_kit::NSButton>> {
        fn find(view: &NSView, action: objc2::runtime::Sel) -> Option<Retained<objc2_app_kit::NSButton>> {
            if let Some(button) = view.downcast_ref::<objc2_app_kit::NSButton>()
                && button.action() == Some(action)
            {
                return Some(objc2::Message::retain(button));
            }
            view.subviews().iter().find_map(|child| find(&child, action))
        }
        let panels = self.ivars().panels.borrow();
        find(panels.as_ref()?.view(), action)
    }

    /// Clicks the row of `path` in the folder tree, twice for a double click.
    pub fn click_in_tree(&self, path: &Path, double: bool) {
        let files = self.ivars().files.borrow();
        if let Some(files) = files.as_ref() {
            files.click(path, double);
        }
    }

    /// Whether each document, tab by tab, has unsaved changes as its tab shows them.
    pub fn tab_dirty_marks(&self) -> Vec<bool> {
        self.documents().iter().map(|doc| doc.is_dirty()).collect()
    }

    /// The note at the foot of the documents, while it shows.
    pub fn notice(&self) -> Option<String> {
        let notice = self.ivars().notice.borrow();
        let (note, text) = notice.as_ref()?;
        // SAFETY: Asking for the superview only reads; the note is alive as the ivar holds it.
        unsafe { note.superview() }.map(|_| text.clone())
    }
}

/// What the self-test reads from a window.
pub struct Inspection {
    pub activity_buttons: usize,
    pub panel: Option<Panel>,
    pub panel_title: String,
    pub tabs: usize,
    pub new_tab_items: Vec<String>,
    pub sidebar_width: f64,
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

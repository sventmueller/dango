//! A folder's window: the activity bar and panel on the left, its documents on the right, each in a native window
//! tab of its own (E60). The controller answers AppKit here and owns its parts, each with its state and module: the
//! folder and trust in `folder`, the window tabs, their documents and panes in `documents`, the sidebar in `sidebar`,
//! the inspector in `inspector`, the toolbar in `toolbar`, and what comes back after a restart in `layout` and `frame`.

mod activity_bar;
mod agent_choices;
mod document_area;
mod documents;
mod editor_commands;
pub(crate) mod file_outline;
mod files;
mod files_header;
mod folder;
mod frame;
mod full_screen;
mod inspector;
pub(crate) mod layout;
mod new_tab_button;
mod notice;
pub(crate) mod panels;
pub(crate) mod panes;
mod progress_ring;
mod quick_open;
mod registry;
mod search;
#[cfg(feature = "selftest")]
mod selftest_probe;
mod sidebar;
mod tab_titles;
mod tabs;
mod ticks;
pub(crate) mod toolbar;
mod validate;
mod welcome;

use std::cell::{Cell, RefCell};
use std::path::PathBuf;
use std::rc::Rc;

use objc2::rc::Retained;
use objc2::runtime::{AnyObject, NSObject, Sel};
use objc2::{DefinedClass, MainThreadMarker, MainThreadOnly, Message, define_class, msg_send, sel};
use objc2_app_kit::{
    NSControl, NSMenuItem, NSMenuItemValidation, NSToolbar, NSToolbarDelegate, NSToolbarItem, NSToolbarItemIdentifier,
    NSView, NSWindow, NSWindowDelegate,
};
use objc2_foundation::{NSArray, NSNotification, NSObjectProtocol};
use splat_i18n::t;

use crate::context::AppContext;
use crate::util::{FlagGuard, guarded, main_soon};
pub(crate) use agent_choices::{AgentChoicesCache, refresh as refresh_agent_choices};
pub(crate) use ide_ui_common::window::TabId;
use ide_ui_common::window::WindowId;
pub(crate) use new_tab_button::NewTabMenu;
pub(crate) use notice::announce;
pub(crate) use progress_ring::tone_color;
#[cfg(feature = "selftest")]
pub(crate) use registry::open_windows;
pub(crate) use registry::{
    deliver, for_project, frontmost, open_projects, records, save_layouts, show_ignored_files, toggle_activity_symbol,
    tree_changed, tree_listed,
};
pub(crate) use search::SearchPanel;
pub(crate) use search::{HierarchyPanels, SearchPanels};
#[cfg(feature = "selftest")]
pub(crate) use search::{LIMIT as SEARCH_LIMIT, Toggle as SearchToggle, selftest as search_selftest};
pub(crate) use search::{buffer_edited, files_changed as files_changed_on_disk};
#[cfg(feature = "selftest")]
pub(crate) use sidebar::ACTIVITY_WIDTH;
#[cfg(feature = "selftest")]
pub(crate) use sidebar::{PANEL_DEFAULT_WIDTH, SIDEBAR_RAISE};
use tabs::Document;
pub(crate) use tabs::TabKind;
#[cfg(feature = "selftest")]
pub(crate) use ticks::Reading as TickReading;

pub(crate) use ide_ui_common::window::Panel;

/// The window controller's state, part by part.
pub(crate) struct Ivars {
    context: Rc<AppContext>,
    /// The window's id among the open ones, which answers from other threads carry.
    id: WindowId,
    folder: folder::State,
    sidebar: sidebar::State,
    inspector: inspector::State,
    documents: documents::State,
    /// The note at the foot of the documents and what it says.
    notice: RefCell<Option<(Retained<objc2_app_kit::NSBox>, String)>>,
    /// Whether the window closed; it stays among the open ones until the main queue gets to it, but has nothing left
    /// to write down.
    closed: Cell<bool>,
    /// Whether all tabs of the window close at once, whose layout was kept before the first went.
    closing: Cell<bool>,
    ticks: ticks::Ticks,
}

define_class!(
    // SAFETY: NSObject has no subclassing requirements and `WindowController` does not implement Drop.
    #[unsafe(super(NSObject))]
    #[thread_kind = MainThreadOnly]
    #[name = "IDEWindowController"]
    #[ivars = Ivars]
    pub(crate) struct WindowController;

    unsafe impl NSObjectProtocol for WindowController {}

    unsafe impl NSWindowDelegate for WindowController {
        #[unsafe(method(windowDidUpdate:))]
        fn did_update(&self, _notification: &NSNotification) {
            guarded((), || {
                if let Some(tab) = self.current_tab() {
                    tab.follow_focus();
                }
            });
        }

        #[unsafe(method(windowDidBecomeKey:))]
        fn did_become_key(&self, notification: &NSNotification) {
            guarded((), || {
                if let Some(tab) = self.tab_of_notification(notification) {
                    self.make_current(&tab);
                    tab.clear_above_sidebar(true);
                    self.follow_toolbar();
                }
                ide_ui_agent::show_pending(self);
                self.tabs_came_forward();
            });
        }

        #[unsafe(method(windowDidChangeOcclusionState:))]
        fn did_change_occlusion(&self, notification: &NSNotification) {
            guarded((), || {
                if let Some(tab) = self.tab_of_notification(notification) {
                    self.tab_visibility_changed(&tab);
                }
                self.tabs_came_forward();
            });
        }

        #[unsafe(method(windowDidEnterFullScreen:))]
        fn did_enter_full_screen(&self, notification: &NSNotification) {
            guarded((), || {
                if let Some(tab) = self.tab_of_notification(notification) {
                    tab.clear_above_sidebar(true);
                }
                self.follow_toolbar();
            });
        }

        #[unsafe(method(windowWillExitFullScreen:))]
        fn will_exit_full_screen(&self, notification: &NSNotification) {
            guarded((), || {
                if let Some(tab) = self.tab_of_notification(notification) {
                    tab.clear_above_sidebar(false);
                }
            });
        }

        #[unsafe(method(windowDidExitFullScreen:))]
        fn did_exit_full_screen(&self, _notification: &NSNotification) {
            guarded((), || self.follow_toolbar());
        }

        #[unsafe(method(windowShouldClose:))]
        fn should_close(&self, window: &NSWindow) -> bool {
            guarded(true, || match self.tab_of_window(window) {
                Some(tab) => {
                    let weak = objc2::rc::Weak::from(window);
                    let close: documents::Close = Rc::new(move |_: &WindowController| {
                        if let Some(window) = weak.load() {
                            window.close();
                        }
                    });
                    (!self.is_last_tab() || self.context().leitstand().may_close(self.context(), self, self.mtm()))
                        && self.documents_may_close(&documents::ids_of(&tab.documents()), close)
                }
                None => true,
            })
        }

        #[unsafe(method(windowWillClose:))]
        fn will_close(&self, notification: &NSNotification) {
            guarded((), || {
                if let Some(tab) = self.tab_of_notification(notification) {
                    self.tab_closes(&tab);
                }
            });
        }
    }

    unsafe impl NSToolbarDelegate for WindowController {
        #[unsafe(method_id(toolbar:itemForItemIdentifier:willBeInsertedIntoToolbar:))]
        fn toolbar_item(
            &self,
            _toolbar: &NSToolbar,
            id: &NSToolbarItemIdentifier,
            _insert: bool,
        ) -> Option<Retained<NSToolbarItem>> {
            guarded(None, || toolbar::make(self, &id.to_string()))
        }

        #[unsafe(method_id(toolbarDefaultItemIdentifiers:))]
        fn toolbar_default(&self, _toolbar: &NSToolbar) -> Retained<NSArray<NSToolbarItemIdentifier>> {
            guarded(NSArray::new(), toolbar::default_identifiers)
        }

        #[unsafe(method_id(toolbarAllowedItemIdentifiers:))]
        fn toolbar_allowed(&self, _toolbar: &NSToolbar) -> Retained<NSArray<NSToolbarItemIdentifier>> {
            guarded(NSArray::new(), toolbar::default_identifiers)
        }
    }

    unsafe impl NSMenuItemValidation for WindowController {
        #[unsafe(method(validateMenuItem:))]
        fn validate_menu_item(&self, item: &NSMenuItem) -> bool {
            guarded(true, || self.validate(item))
        }
    }

    impl WindowController {
        #[unsafe(method(respondsToSelector:))]
        fn responds_to_selector(&self, selector: Sel) -> bool {
            // SAFETY: The overridden method with arguments of its types, which objc2 checks in debug builds.
            let own: bool = unsafe { msg_send![super(self), respondsToSelector: selector] };
            own || guarded(false, || self.editor_for_action(selector).is_some())
        }

        #[unsafe(method_id(forwardingTargetForSelector:))]
        fn forwarding_target(&self, selector: Sel) -> Option<Retained<AnyObject>> {
            guarded(None, || {
                self.editor_for_action(selector).map(|editor| {
                    let editor: &AnyObject = &editor;
                    editor.retain()
                })
            })
        }

        #[unsafe(method(ideTestRunAtCursor:))]
        fn test_run_at_cursor(&self, _sender: Option<&AnyObject>) {
            guarded((), || crate::tests_panel::run_at_cursor(self));
        }

        #[unsafe(method(ideTestDebugAtCursor:))]
        fn test_debug_at_cursor(&self, _sender: Option<&AnyObject>) {
            guarded((), || crate::tests_panel::debug_at_cursor(self));
        }

        #[unsafe(method(ideTestRerun:))]
        fn test_rerun(&self, _sender: Option<&AnyObject>) {
            guarded((), || crate::tests_panel::rerun(self));
        }

        #[unsafe(method(ideContainerCommand:))]
        fn container_command(&self, sender: Option<&AnyObject>) {
            guarded((), || crate::containers_panel::perform(self, tag_of(sender)));
        }

        #[unsafe(method(ideTerminalCommand:))]
        fn terminal_command(&self, sender: Option<&AnyObject>) {
            guarded((), || crate::terminal::perform(self, tag_of(sender)));
        }

        #[unsafe(method(ideTerminalContextCommand:))]
        fn terminal_context_command(&self, sender: Option<&AnyObject>) {
            guarded((), || crate::terminal::perform_on_view(self, tag_of(sender)));
        }

        #[unsafe(method(performTextFinderAction:))]
        fn text_finder_action(&self, sender: Option<&AnyObject>) {
            guarded((), || crate::terminal::text_finder(self, tag_of(sender)));
        }

        #[unsafe(method(termClear:))]
        fn clear_terminal_action(&self, _sender: Option<&AnyObject>) {
            guarded((), || crate::terminal::clear(self));
        }

        #[unsafe(method(ideTestRunAll:))]
        fn test_run_all(&self, _sender: Option<&AnyObject>) {
            guarded((), || crate::tests_panel::run_all(self));
        }

        #[unsafe(method(ideShowPanel:))]
        fn show_panel_action(&self, sender: Option<&AnyObject>) {
            guarded((), || {
                if let Some(panel) = Panel::from_tag(tag_of(sender)) {
                    self.activate_panel(panel);
                    // From the keyboard the focus follows, since Tab cannot leave an editor or a terminal.
                    if sender.is_some_and(|sender| sender.downcast_ref::<NSMenuItem>().is_some()) {
                        self.focus_panel();
                    }
                }
            });
        }

        #[unsafe(method(ideToggleActivitySymbol:))]
        fn toggle_activity_symbol_action(&self, sender: Option<&AnyObject>) {
            guarded((), || {
                if let Some(panel) = Panel::from_tag(tag_of(sender)) {
                    toggle_activity_symbol(self.context(), panel);
                }
            });
        }

        #[unsafe(method(ideToggleInspector:))]
        fn toggle_inspector_action(&self, _sender: Option<&AnyObject>) {
            guarded((), || self.toggle_inspector());
        }

        #[unsafe(method(ideNewTab:))]
        fn new_tab_action(&self, _sender: Option<&AnyObject>) {
            guarded((), || new_tab_button::open(self));
        }

        #[unsafe(method(newWindowForTab:))]
        fn new_window_for_tab(&self, _sender: Option<&AnyObject>) {
            guarded((), || new_tab_button::open(self));
        }

        #[unsafe(method(ideFocusNextPanel:))]
        fn focus_next_panel_action(&self, _sender: Option<&AnyObject>) {
            guarded((), || {
                if let Some(tab) = self.current_tab() {
                    tab.focus_next_pane();
                }
            });
        }

        #[unsafe(method(ideCloseTab:))]
        fn close_tab_action(&self, _sender: Option<&AnyObject>) {
            guarded((), || match self.selected_tab() {
                Some(id) => self.close_tab(id),
                None => self.close_active_pane(),
            });
        }

        #[unsafe(method(ideSplitRight:))]
        fn split_right_action(&self, _sender: Option<&AnyObject>) {
            guarded((), || self.split_active(documents::side_with_option()));
        }

        #[unsafe(method(ideSplitBelow:))]
        fn split_below_action(&self, _sender: Option<&AnyObject>) {
            guarded((), || self.split_active(panes::Side::Below));
        }

        #[unsafe(method(ideClosePane:))]
        fn close_pane_action(&self, sender: Option<&AnyObject>) {
            guarded((), || self.close_pane_by_tag(sender.map(|_| tag_of(sender))));
        }

        #[unsafe(method(ideNewTabInPane:))]
        fn new_tab_in_pane_action(&self, sender: Option<&AnyObject>) {
            guarded((), || self.new_tab_in_pane(tag_of(sender), sender.and_then(|s| s.downcast_ref::<NSView>())));
        }

        #[unsafe(method(ideZoomPane:))]
        fn zoom_pane_action(&self, _sender: Option<&AnyObject>) {
            guarded((), || self.toggle_zoom());
        }

        #[unsafe(method(ideNextPaneTab:))]
        fn next_pane_tab_action(&self, _sender: Option<&AnyObject>) {
            guarded((), || self.select_pane_tab_relative(1));
        }

        #[unsafe(method(idePreviousPaneTab:))]
        fn previous_pane_tab_action(&self, _sender: Option<&AnyObject>) {
            guarded((), || self.select_pane_tab_relative(-1));
        }

        #[unsafe(method(ideCloseWindow:))]
        fn close_window_action(&self, _sender: Option<&AnyObject>) {
            guarded((), || {
                // The command reaches the folder's window also while another window, such as the settings, is the
                // key one, which it closes then.
                let key = objc2_app_kit::NSApplication::sharedApplication(self.mtm()).keyWindow();
                match key {
                    Some(key) if !self.owns_window(&key) => key.performClose(None),
                    _ => self.close_window(),
                }
            });
        }

        #[unsafe(method(ideNextTab:))]
        fn next_tab_action(&self, _sender: Option<&AnyObject>) {
            guarded((), || self.select_relative(true));
        }

        #[unsafe(method(idePreviousTab:))]
        fn previous_tab_action(&self, _sender: Option<&AnyObject>) {
            guarded((), || self.select_relative(false));
        }

        #[unsafe(method(ideCloseTabButton:))]
        fn close_tab_button_action(&self, sender: Option<&AnyObject>) {
            guarded((), || self.close_tab(TabId::new(tag_of(sender))));
        }

        #[unsafe(method(ideBrowserFocusAddress:))]
        fn browser_focus_address(&self, _sender: Option<&AnyObject>) {
            guarded((), || crate::browser::perform(self, sel!(ideBrowserFocusAddress:)));
        }

        #[unsafe(method(ideBrowserReload:))]
        fn browser_reload(&self, _sender: Option<&AnyObject>) {
            guarded((), || crate::browser::perform(self, sel!(ideBrowserReload:)));
        }

        #[unsafe(method(ideBrowserBack:))]
        fn browser_back(&self, _sender: Option<&AnyObject>) {
            guarded((), || crate::browser::perform(self, sel!(ideBrowserBack:)));
        }

        #[unsafe(method(ideBrowserForward:))]
        fn browser_forward(&self, _sender: Option<&AnyObject>) {
            guarded((), || crate::browser::perform(self, sel!(ideBrowserForward:)));
        }

        #[unsafe(method(ideQuickOpen:))]
        fn quick_open_action(&self, _sender: Option<&AnyObject>) {
            guarded((), || self.show_quick_open(""));
        }

        #[unsafe(method(ideProjectSymbols:))]
        fn project_symbols_action(&self, _sender: Option<&AnyObject>) {
            guarded((), || self.show_quick_open("#"));
        }

        #[unsafe(method(ideSearchProject:))]
        fn search_project_action(&self, _sender: Option<&AnyObject>) {
            guarded((), || self.show_search());
        }

        #[unsafe(method(ideNewFile:))]
        fn new_file_action(&self, _sender: Option<&AnyObject>) {
            guarded((), || {
                let size = ide_core::settings::Settings::load_user().get(&ide_core::settings::keys::FONT_SIZE);
                self.at_end(|| crate::editor::new_untitled(self, size));
            });
        }

        #[unsafe(method(ideNewTerminal:))]
        fn new_terminal_action(&self, _sender: Option<&AnyObject>) {
            guarded((), || self.at_end(|| crate::terminal::open_tab(self, None)));
        }

        #[unsafe(method(ideNewBrowser:))]
        fn new_browser_action(&self, _sender: Option<&AnyObject>) {
            guarded((), || self.at_end(|| crate::browser::open_tab(self, None)));
        }

        #[unsafe(method(ideOpenWelcome:))]
        fn open_welcome_action(&self, _sender: Option<&AnyObject>) {
            guarded((), || {
                self.at_end(|| self.open_welcome());
            });
        }

        #[unsafe(method(ideOpenWelcomeRecent:))]
        fn open_welcome_recent_action(&self, sender: Option<&AnyObject>) {
            guarded((), || {
                let recent = welcome::recent_folders();
                let index = usize::try_from(tag_of(sender)).ok();
                if let Some(folder) = index.and_then(|i| recent.get(i)) {
                    crate::app::open_project_from(
                        self.context(),
                        self.mtm(),
                        folder.clone(),
                        Some(objc2::Message::retain(self)),
                    );
                }
            });
        }

        #[unsafe(method(ideNewOwnThread:))]
        fn new_own_thread_action(&self, sender: Option<&AnyObject>) {
            guarded((), || {
                let item = sender.and_then(|s| s.downcast_ref::<NSMenuItem>());
                let represented = item.and_then(|i| i.representedObject()?.downcast::<objc2_foundation::NSString>().ok());
                let choice = represented.and_then(|r| ide_ui_agent::own_menu_choice(&r.to_string()));
                self.at_end(|| {
                    if let Some(tab) = ide_ui_agent::open_own(self, choice) {
                        tab.focus_composer();
                    }
                });
            });
        }

        #[unsafe(method(ideOpenAgentSettings:))]
        fn open_agent_settings_action(&self, _sender: Option<&AnyObject>) {
            guarded((), || crate::settings::show_agent(self.mtm()));
        }

        #[unsafe(method(ideNewAcpThread:))]
        fn new_acp_thread_action(&self, sender: Option<&AnyObject>) {
            guarded((), || {
                let index = usize::try_from(tag_of(sender)).unwrap_or(usize::MAX);
                self.at_end(|| {
                    if let Some(tab) = ide_ui_agent::open_preset(self, index) {
                        tab.focus_composer();
                    }
                });
            });
        }

        #[unsafe(method(ideNewClaudeThread:))]
        fn new_claude_thread_action(&self, _sender: Option<&AnyObject>) {
            guarded((), || {
                self.at_end(|| {
                    if let Some(tab) = ide_ui_agent::open_tab(self, None) {
                        tab.focus_composer();
                    }
                });
            });
        }

        #[unsafe(method(ideNewWorktreeThread:))]
        fn new_worktree_thread_action(&self, sender: Option<&AnyObject>) {
            guarded((), || {
                let index = usize::try_from(tag_of(sender)).unwrap_or(usize::MAX);
                crate::worktrees::ask_and_open(self, index);
            });
        }

        #[unsafe(method(ideFocusClaude:))]
        fn focus_claude_action(&self, _sender: Option<&AnyObject>) {
            guarded((), || ide_ui_agent::toggle_focus(self));
        }

        #[unsafe(method(ideShowSessionSubagents:))]
        fn show_session_subagents_action(&self, _sender: Option<&AnyObject>) {
            guarded((), || ide_ui_agent::show_work(self, crate::activity::Part::Subagents));
        }

        #[unsafe(method(ideShowSessionBackground:))]
        fn show_session_background_action(&self, _sender: Option<&AnyObject>) {
            guarded((), || ide_ui_agent::show_work(self, crate::activity::Part::Background));
        }

        #[unsafe(method(idePlanComment:))]
        fn plan_comment_action(&self, _sender: Option<&AnyObject>) {
            guarded((), || ide_ui_agent::plan::comment_from_keyboard(self));
        }
    }
);

/// Drops the window controller with `id` from the open windows once AppKit is done closing its window.
fn main_soon_unregister(id: WindowId) {
    main_soon(move || {
        crate::context::app().windows().remove(id.get());
        crate::session::changed();
    });
}

impl WindowController {
    /// Runs `f` with this window's controller once the main queue gets to it, if the window is still open.
    fn later(&self, f: impl FnOnce(&WindowController) + Send + 'static) {
        deliver(self.id(), f);
    }

    /// The window's id among the open ones.
    pub(crate) fn id(&self) -> WindowId {
        self.ivars().id
    }

    /// The window as the feature crates know it.
    pub(crate) fn handle(&self) -> ide_ui_common::window::Window {
        ide_ui_common::window::Window::new(self.id())
    }

    pub(crate) fn context(&self) -> &Rc<AppContext> {
        &self.ivars().context
    }

    /// Marks an action whose input came now, which misses its tick unless its frame is committed within a refresh
    /// (SN-M2).
    pub(crate) fn mark_action(&self) {
        if let Some(view) = self.window().and_then(|window| window.contentView()) {
            self.ivars().ticks.mark(&view);
        }
    }

    /// Whether the window closed, though it is still among the open ones until the main queue gets to it.
    pub(crate) fn is_closed(&self) -> bool {
        self.ivars().closed.get()
    }
}

/// The tag of the control or menu item that sent an action.
fn tag_of(sender: Option<&AnyObject>) -> isize {
    let Some(sender) = sender else { return -1 };
    if let Some(item) = sender.downcast_ref::<NSMenuItem>() {
        return item.tag();
    }
    sender.downcast_ref::<NSControl>().map_or(-1, |control| control.tag())
}

/// The menu bar's browser commands, which the browser tab in front answers.
fn browser_commands() -> [Sel; 4] {
    [
        sel!(ideBrowserFocusAddress:),
        sel!(ideBrowserReload:),
        sel!(ideBrowserBack:),
        sel!(ideBrowserForward:),
    ]
}

impl WindowController {
    /// Brings the window of `project` to the front, or puts it into an empty window, or opens a new one.
    pub(crate) fn open(
        cx: &Rc<AppContext>,
        mtm: MainThreadMarker,
        project: Option<PathBuf>,
    ) -> Retained<WindowController> {
        let open = cx.windows();
        let existing = match &project {
            Some(path) => open
                .find(|w| w.project().as_deref() == Some(path))
                .or_else(|| open.find(|w| w.project().is_none())),
            None => None,
        };
        if let Some(controller) = existing {
            if controller.project().is_none() {
                controller.set_project(project);
                controller.restore_layout();
            }
            controller.bring_to_front();
            return controller;
        }
        let controller = WindowController::open_new(cx, mtm, project);
        controller.restore_layout();
        controller
    }

    /// A new window for `project`, even if another shows it, as a launch puts back the windows of the last run.
    pub(crate) fn open_new(
        cx: &Rc<AppContext>,
        mtm: MainThreadMarker,
        project: Option<PathBuf>,
    ) -> Retained<WindowController> {
        new_tab_button::install();
        let controller = WindowController::new(cx, mtm, project);
        agent_choices::refresh(controller.context());
        controller.bring_to_front();
        if let Some(tab) = controller.current_tab() {
            controller.fit_sidebar(&tab, controller.saved_sidebar().collapsed);
            controller.sync_tab_bar(&tab);
        }
        crate::session::changed();
        controller
    }

    /// A window for `project` with its first tab, among the open ones before its folder loads.
    fn new(cx: &Rc<AppContext>, mtm: MainThreadMarker, project: Option<PathBuf>) -> Retained<WindowController> {
        let ivars = Ivars {
            id: WindowId::new(cx.windows().next_id()),
            context: Rc::clone(cx),
            folder: folder::State::new(project),
            sidebar: sidebar::State::default(),
            inspector: inspector::State::default(),
            documents: documents::State::default(),
            notice: RefCell::default(),
            closed: Cell::default(),
            closing: Cell::default(),
            ticks: ticks::Ticks::default(),
        };
        let this = Self::alloc(mtm).set_ivars(ivars);
        // SAFETY: NSObject's initializer, called once on the newly allocated object.
        let this: Retained<Self> = unsafe { msg_send![super(this), init] };
        this.build_sidebar();
        let tab = this.new_tab_window();
        let sidebar = this.saved_sidebar();
        this.start_with_panel_open();
        this.restore_frame(&tab.window);
        this.make_current(&tab);
        this.show_panel(sidebar::first_shown(sidebar.panel, &activity_bar::hidden()));
        if !sidebar.open {
            this.set_panel_open(false);
        }
        this.open_welcome();
        // The window's parts reach it by id from the moment they attach to its folder, which `load_project` starts.
        this.context().windows().insert(this.id().get(), this.clone());
        this.load_project();
        this
    }

    /// The identifier that keeps the window tabs of this window together, and those of other windows out (E56).
    fn tabbing_identifier(&self) -> String {
        format!("folder {:p}", self)
    }

    /// Shows `text` at the foot of the documents of the tab in front for a few seconds.
    pub(crate) fn show_notice(&self, text: &str) {
        let Some(tab) = self.current_tab() else { return };
        let note = notice::show(self.mtm(), tab.content(), text);
        *self.ivars().notice.borrow_mut() = Some((note, text.to_string()));
    }

    /// Closes the tabs of the window in front once each of their documents agrees, asked one after the other; one
    /// that refuses keeps them all. The layout is the one before the first tab closed.
    fn close_window(&self) {
        self.finish_restoring();
        let Some(current) = self.current_tab() else { return };
        let windows: Vec<Retained<NSWindow>> = current
            .window
            .tabbedWindows()
            .map(|tabbed| tabbed.iter().collect())
            .unwrap_or_else(|| vec![current.window.clone()]);
        let documents: Vec<Rc<Document>> = windows
            .iter()
            .filter_map(|window| self.tab_of_window(window))
            .flat_map(|tab| tab.documents())
            .collect();
        let weak: Vec<objc2::rc::Weak<NSWindow>> =
            windows.iter().map(|window| objc2::rc::Weak::from(&**window)).collect();
        let close: documents::Close = Rc::new(move |controller: &WindowController| controller.close_windows(&weak));
        if !self.context().leitstand().may_close(self.context(), self, self.mtm())
            || !self.documents_may_close(&documents::ids_of(&documents), Rc::clone(&close))
        {
            return;
        }
        close(self);
    }

    /// Closes `windows`, the tabs of a window whose documents agreed; the layout is the one before the first closed.
    fn close_windows(&self, windows: &[objc2::rc::Weak<NSWindow>]) {
        layout::save(self);
        let _closing = FlagGuard::raise(&self.ivars().closing);
        for window in windows.iter().filter_map(objc2::rc::Weak::load) {
            window.close();
        }
    }

    pub(crate) fn bring_to_front(&self) {
        if let Some(window) = self.window() {
            window.makeKeyAndOrderFront(None);
        }
    }

    /// The window of the tab in front, which has the sidebar; sheets and the keyboard focus go there.
    pub(crate) fn window(&self) -> Option<Retained<NSWindow>> {
        self.current_tab().map(|tab| tab.window.clone())
    }

    /// The windows of the tabs, in the order of the tab bar.
    pub(crate) fn tab_windows(&self) -> Vec<Retained<NSWindow>> {
        self.ordered_tabs().iter().map(|tab| tab.window.clone()).collect()
    }

    /// Whether `window` is one of this window's tabs.
    pub(crate) fn owns_window(&self, window: &NSWindow) -> bool {
        self.tab_of_window(window).is_some()
    }

    /// The window's last tab closes: its layout is kept, and what runs for its folder ends.
    fn window_closes(&self) {
        layout::save(self);
        self.save_frame();
        self.ivars().closed.set(true);
        self.folder_closes();
        main_soon_unregister(self.id());
    }

    /// Brings the welcome page to the front, or opens it; its number.
    pub(crate) fn open_welcome(&self) -> TabId {
        let open = self
            .documents()
            .into_iter()
            .find(|doc| doc.kind == TabKind::Welcome)
            .map(|doc| doc.id);
        match open {
            Some(id) => {
                self.select_tab_by_id(id);
                id
            }
            None => {
                let view = welcome::view(self.mtm(), self);
                self.add_tab(TabKind::Welcome, None, t("Willkommen"), view)
            }
        }
    }
}

pub(crate) use ide_ui_common::panel::in_front;

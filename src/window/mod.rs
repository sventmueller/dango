//! A folder's window: the activity bar and panel on the left, its documents on the right, each in a native window
//! tab of its own (E60).

mod activity_bar;
mod document_area;
mod document_tabs;
pub(crate) mod file_outline;
mod files;
mod files_header;
mod finding;
mod frame;
mod full_screen;
mod inspector;
pub mod layout;
mod lifecycle;
mod new_tab_button;
mod notice;
pub mod panels;
#[cfg(feature = "selftest")]
mod probe;
mod quick_open;
mod registry;
mod replace;
mod search;
mod sidebar;
mod tabs;
pub mod toolbar;
mod validate;
mod welcome;

use std::cell::{Cell, RefCell};
use std::path::{Path, PathBuf};
use std::rc::Rc;

use objc2::rc::Retained;
use objc2::runtime::{AnyObject, NSObject, Sel};
use objc2::{DefinedClass, MainThreadMarker, MainThreadOnly, define_class, msg_send, sel};
use objc2_app_kit::{
    NSControl, NSMenuItem, NSMenuItemValidation, NSToolbar, NSToolbarDelegate, NSToolbarItem, NSToolbarItemIdentifier,
    NSView, NSWindow, NSWindowDelegate,
};
use objc2_foundation::{NSArray, NSNotification, NSObjectProtocol};
use splat_i18n::t;

use crate::context::AppContext;
use crate::util::{guarded, main_soon, ns};
use activity_bar::ActivityBar;
use files::FileTree;
pub use new_tab_button::NewTabMenu;
pub use notice::announce;
use panels::Panels;
use quick_open::QuickOpenPanel;
#[cfg(feature = "selftest")]
pub use registry::open_windows;
pub use registry::{
    deliver, for_project, frontmost, open_projects, records, save_layouts, show_ignored_files, toggle_activity_symbol,
    tree_changed,
};
pub use search::SearchPanel;
pub use search::{HierarchyPanels, SearchPanels};
#[cfg(feature = "selftest")]
pub use search::{LIMIT as SEARCH_LIMIT, Toggle as SearchToggle, selftest as search_selftest};
pub use search::{buffer_edited, files_changed as files_changed_on_disk};
pub use tabs::TabKind;
use tabs::{Document, TabWindow};

/// How far the activity bar and the panel reach above the sidebar's safe area, which puts their first symbol and title
/// level with the top edge of the tabs, which start where the safe area does.
pub const SIDEBAR_RAISE: f64 = 11.0;
/// How far the panel reaches under the activity bar, so that its content keeps the same distance from the symbols as
/// the symbols keep from the window's edge.
const PANEL_UNDER_ACTIVITY: f64 = 8.5;
/// The width of the activity bar, which stays when the panel beside it is closed.
pub(crate) const ACTIVITY_WIDTH: f64 = 44.0;
const PANEL_MIN_WIDTH: f64 = 200.0;
const PANEL_MAX_WIDTH: f64 = 560.0;
pub(crate) const PANEL_DEFAULT_WIDTH: f64 = 260.0;

pub use ide_ui_common::window::Panel;

pub struct Ivars {
    context: Rc<AppContext>,
    /// The window's id among the open ones, which answers from other threads carry.
    id: u64,
    project: RefCell<Option<PathBuf>>,
    panel: Cell<Option<Panel>>,
    panel_open: Cell<bool>,
    panel_width: Cell<f64>,
    activity: RefCell<Option<ActivityBar>>,
    panels: RefCell<Option<Panels>>,
    inspector: inspector::State,
    /// The activity bar and the panel beside it, which the window tab in front shows.
    sidebar: RefCell<Option<Retained<NSView>>>,
    /// The window tabs, in the order they opened.
    tabs: RefCell<Vec<Rc<TabWindow>>>,
    /// The window tab in front, which has the sidebar.
    current: RefCell<Option<Rc<TabWindow>>>,
    /// The number of the next document.
    next_id: Cell<isize>,
    /// The document a document about to open goes beside, instead of into a window tab of its own.
    beside: Cell<Option<isize>>,
    workspace: RefCell<Option<Rc<crate::project::Project>>>,
    files: RefCell<Option<FileTree>>,
    search: RefCell<Option<Retained<SearchPanel>>>,
    quick_open: RefCell<Option<Retained<QuickOpenPanel>>>,
    /// The note at the foot of the documents and what it says.
    notice: RefCell<Option<(Retained<objc2_app_kit::NSBox>, String)>>,
    /// Whether the window closed; it stays among the open ones until the main queue gets to it, but has nothing left
    /// to write down.
    closed: Cell<bool>,
    /// Whether all tabs of the window close at once, whose layout was kept before the first went.
    closing: Cell<bool>,
}

define_class!(
    // SAFETY: NSObject has no subclassing requirements and `WindowController` does not implement Drop.
    #[unsafe(super(NSObject))]
    #[thread_kind = MainThreadOnly]
    #[name = "IDEWindowController"]
    #[ivars = Ivars]
    pub struct WindowController;

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
                }
                crate::agent::show_pending(self);
            });
        }

        #[unsafe(method(windowDidEnterFullScreen:))]
        fn did_enter_full_screen(&self, notification: &NSNotification) {
            guarded((), || {
                if let Some(tab) = self.tab_of_notification(notification) {
                    tab.clear_above_sidebar(true);
                }
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

        #[unsafe(method(windowShouldClose:))]
        fn should_close(&self, window: &NSWindow) -> bool {
            guarded(true, || match self.tab_of_window(window) {
                Some(tab) => self.documents_may_close(&tab.documents()),
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
                    toggle_activity_symbol(panel);
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
                    tab.focus_other();
                }
            });
        }

        #[unsafe(method(ideCloseTab:))]
        fn close_tab_action(&self, _sender: Option<&AnyObject>) {
            guarded((), || {
                if let Some(id) = self.selected_tab() {
                    self.close_tab(id);
                }
            });
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
            guarded((), || self.close_tab(tag_of(sender)));
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
                crate::editor::new_untitled(self, size);
            });
        }

        #[unsafe(method(ideNewTerminal:))]
        fn new_terminal_action(&self, _sender: Option<&AnyObject>) {
            guarded((), || crate::terminal::open_tab(self, None));
        }

        #[unsafe(method(ideNewBrowser:))]
        fn new_browser_action(&self, _sender: Option<&AnyObject>) {
            guarded((), || crate::browser::open_tab(self, None));
        }

        #[unsafe(method(ideOpenWelcome:))]
        fn open_welcome_action(&self, _sender: Option<&AnyObject>) {
            guarded((), || {
                self.open_welcome();
            });
        }

        #[unsafe(method(ideOpenWelcomeRecent:))]
        fn open_welcome_recent_action(&self, sender: Option<&AnyObject>) {
            guarded((), || {
                let recent = welcome::recent_folders();
                let index = usize::try_from(tag_of(sender)).ok();
                if let Some(folder) = index.and_then(|i| recent.get(i)) {
                    crate::app::open_project_from(self.mtm(), folder.clone(), Some(objc2::Message::retain(self)));
                }
            });
        }

        #[unsafe(method(ideNewOwnThread:))]
        fn new_own_thread_action(&self, sender: Option<&AnyObject>) {
            guarded((), || {
                let item = sender.and_then(|s| s.downcast_ref::<NSMenuItem>());
                let represented = item.and_then(|i| i.representedObject()?.downcast::<objc2_foundation::NSString>().ok());
                let choice = represented.and_then(|r| crate::agent::own_menu_choice(&r.to_string()));
                if let Some(tab) = crate::agent::open_own(self, choice) {
                    tab.focus_composer();
                }
            });
        }

        #[unsafe(method(ideOpenAgentSettings:))]
        fn open_agent_settings_action(&self, _sender: Option<&AnyObject>) {
            guarded((), || crate::settings::show_agent(self.mtm()));
        }

        #[unsafe(method(ideOpenAgentsSettings:))]
        fn open_agents_settings_action(&self, _sender: Option<&AnyObject>) {
            guarded((), || crate::settings::show_agents(self.mtm()));
        }

        #[unsafe(method(ideNewAcpThread:))]
        fn new_acp_thread_action(&self, sender: Option<&AnyObject>) {
            guarded((), || {
                let index = usize::try_from(tag_of(sender)).unwrap_or(usize::MAX);
                if let Some(tab) = crate::agent::open_preset(self, index) {
                    tab.focus_composer();
                }
            });
        }

        #[unsafe(method(ideNewClaudeThread:))]
        fn new_claude_thread_action(&self, _sender: Option<&AnyObject>) {
            guarded((), || {
                if let Some(tab) = crate::agent::open_tab(self, None) {
                    tab.focus_composer();
                }
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
            guarded((), || crate::agent::toggle_focus(self));
        }

        #[unsafe(method(ideShowSessionSubagents:))]
        fn show_session_subagents_action(&self, _sender: Option<&AnyObject>) {
            guarded((), || crate::agent::show_work(self, crate::activity::Part::Subagents));
        }

        #[unsafe(method(ideShowSessionBackground:))]
        fn show_session_background_action(&self, _sender: Option<&AnyObject>) {
            guarded((), || crate::agent::show_work(self, crate::activity::Part::Background));
        }

        #[unsafe(method(idePlanComment:))]
        fn plan_comment_action(&self, _sender: Option<&AnyObject>) {
            guarded((), || crate::agent::plan::comment_from_keyboard(self));
        }
    }
);

/// Drops the window controller with `id` from the open windows once AppKit is done closing its window.
fn main_soon_unregister(id: u64) {
    main_soon(move || {
        crate::context::app().windows.remove(id);
        crate::session::changed();
    });
}

impl WindowController {
    /// Runs `f` with this window's controller once the main queue gets to it, if the window is still open.
    fn later(&self, f: impl FnOnce(&WindowController) + Send + 'static) {
        deliver(self.id(), f);
    }

    /// The window's id among the open ones.
    pub fn id(&self) -> u64 {
        self.ivars().id
    }

    /// The window as the feature crates know it.
    pub fn handle(&self) -> ide_ui_common::window::Window {
        ide_ui_common::window::Window::new(self.id())
    }

    pub fn context(&self) -> &Rc<AppContext> {
        &self.ivars().context
    }
}

/// The open windows, in the order they opened.
fn windows() -> Vec<Retained<WindowController>> {
    crate::context::app().windows.all()
}

/// `panel` unless its symbol is among `hidden`, else the first panel whose symbol shows.
fn first_shown(panel: Panel, hidden: &[Panel]) -> Panel {
    if !hidden.contains(&panel) {
        return panel;
    }
    Panel::ALL.into_iter().find(|p| !hidden.contains(p)).unwrap_or(panel)
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
    pub fn open(mtm: MainThreadMarker, project: Option<PathBuf>) -> Retained<WindowController> {
        let open = &AppContext::get(mtm).windows;
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
        let controller = WindowController::open_new(mtm, project);
        controller.restore_layout();
        controller
    }

    /// A new window for `project`, even if another shows it, as a launch puts back the windows of the last run.
    pub fn open_new(mtm: MainThreadMarker, project: Option<PathBuf>) -> Retained<WindowController> {
        new_tab_button::install();
        let controller = WindowController::new(mtm, project);
        controller.bring_to_front();
        if let Some(tab) = controller.current_tab() {
            controller.fit_sidebar(&tab, controller.saved_sidebar().collapsed);
            show_tab_bar(&tab.window);
        }
        crate::session::changed();
        controller
    }

    fn new(mtm: MainThreadMarker, project: Option<PathBuf>) -> Retained<WindowController> {
        let context = AppContext::get(mtm);
        let ivars = Ivars {
            id: context.windows.next_id(),
            context,
            project: RefCell::new(project),
            panel: Cell::default(),
            panel_open: Cell::default(),
            panel_width: Cell::new(PANEL_DEFAULT_WIDTH),
            activity: RefCell::default(),
            panels: RefCell::default(),
            inspector: inspector::State::default(),
            sidebar: RefCell::default(),
            tabs: RefCell::default(),
            current: RefCell::default(),
            next_id: Cell::new(1),
            beside: Cell::default(),
            workspace: RefCell::default(),
            files: RefCell::default(),
            search: RefCell::default(),
            quick_open: RefCell::default(),
            notice: RefCell::default(),
            closed: Cell::default(),
            closing: Cell::default(),
        };
        let this = Self::alloc(mtm).set_ivars(ivars);
        // SAFETY: NSObject's initializer, called once on the newly allocated object.
        let this: Retained<Self> = unsafe { msg_send![super(this), init] };
        this.build();
        let tab = this.new_tab_window();
        let sidebar = this.saved_sidebar();
        this.ivars().panel_open.set(true);
        this.restore_frame(&tab.window);
        this.make_current(&tab);
        this.show_panel(first_shown(sidebar.panel, &activity_bar::hidden()));
        if !sidebar.open {
            this.set_panel_open(false);
        }
        this.open_welcome();
        // The window's parts reach it by id from the moment they attach to its folder, which `load_project` starts.
        this.context().windows.insert(this.id(), this.clone());
        this.load_project();
        this
    }

    /// The identifier that keeps the window tabs of this window together, and those of other windows out (E56).
    fn tabbing_identifier(&self) -> String {
        format!("folder {:p}", self)
    }

    /// Builds the sidebar, the activity bar and the panel beside it, which moves into the window tab in front.
    fn build(&self) {
        let mtm = self.mtm();
        let sidebar_view = NSView::new(mtm);
        let activity = ActivityBar::new(mtm, self);
        let panels = Panels::new(mtm, self, self.project().as_deref());
        sidebar_view.addSubview(panels.view());
        sidebar_view.addSubview(activity.view());
        let guide = sidebar_view.safeAreaLayoutGuide();
        activity.view().setTranslatesAutoresizingMaskIntoConstraints(false);
        panels.view().setTranslatesAutoresizingMaskIntoConstraints(false);
        for constraint in [
            activity
                .view()
                .leadingAnchor()
                .constraintEqualToAnchor(&sidebar_view.leadingAnchor()),
            activity
                .view()
                .topAnchor()
                .constraintEqualToAnchor_constant(&guide.topAnchor(), -SIDEBAR_RAISE),
            activity
                .view()
                .bottomAnchor()
                .constraintEqualToAnchor(&sidebar_view.bottomAnchor()),
            activity.view().widthAnchor().constraintEqualToConstant(ACTIVITY_WIDTH),
            panels
                .view()
                .leadingAnchor()
                .constraintEqualToAnchor_constant(&activity.view().trailingAnchor(), -PANEL_UNDER_ACTIVITY),
            panels
                .view()
                .trailingAnchor()
                .constraintEqualToAnchor(&sidebar_view.trailingAnchor()),
            panels
                .view()
                .topAnchor()
                .constraintEqualToAnchor_constant(&guide.topAnchor(), -SIDEBAR_RAISE),
            panels
                .view()
                .bottomAnchor()
                .constraintEqualToAnchor(&sidebar_view.bottomAnchor()),
        ] {
            constraint.setActive(true);
        }
        let ivars = self.ivars();
        *ivars.activity.borrow_mut() = Some(activity);
        *ivars.panels.borrow_mut() = Some(panels);
        *ivars.sidebar.borrow_mut() = Some(sidebar_view);
    }

    /// Reads the folder, shows its tree and asks once whether the user trusts it.
    fn load_project(&self) {
        let ivars = self.ivars();
        if let Some(search) = ivars.search.borrow_mut().take() {
            search.stop();
        }
        if let Some(quick_open) = ivars.quick_open.take() {
            quick_open.close();
        }
        *ivars.files.borrow_mut() = None;
        *ivars.workspace.borrow_mut() = None;
        let Some(root) = self.project() else { return };
        let project = match crate::project::Project::open(&root) {
            Ok(project) => Rc::new(project),
            Err(error) => {
                crate::diag::report("opening a folder", error);
                return;
            }
        };
        let tree = FileTree::new(self.mtm(), self, project.clone());
        let search = SearchPanel::new(self.mtm(), self, project.root());
        if let Some(panels) = ivars.panels.borrow().as_ref() {
            panels.set_files(&tree.page(), &tree.header_buttons(self.mtm()));
            if let Some(page) = search.page() {
                panels.set_page(Panel::Search, page);
            }
            if let Some(header) = search.header() {
                panels.set_accessory(Panel::Search, header);
            }
        }
        *ivars.files.borrow_mut() = Some(tree);
        *ivars.search.borrow_mut() = Some(search);
        *ivars.workspace.borrow_mut() = Some(project);
        self.folder_attached();
        if !crate::project::is_trusted(&root) {
            self.ask_trust(root);
        }
    }

    /// Opens the tabs the folder's window had when it closed, as opening a folder does.
    pub fn restore_layout(&self) {
        let size = ide_core::settings::Settings::load_user().get(&ide_core::settings::keys::FONT_SIZE);
        layout::restore(self, &mut |controller, document| match document {
            layout::Document::File(path) => path.is_file() && crate::editor::open_quietly(controller, path, size),
            layout::Document::Untitled(_) => false,
        });
    }

    /// `restore_layout` with `editor` opening each editor's document, and saying whether it did, so the launch can put
    /// the copies a crash left in their places (`session::restore`).
    pub(crate) fn restore_layout_with(&self, editor: &mut layout::Opener) {
        layout::restore(self, editor);
    }

    /// The sheet that asks whether the folder may run what it configures itself (E22).
    fn ask_trust(&self, root: PathBuf) {
        let Some(window) = self.window() else { return };
        let alert = objc2_app_kit::NSAlert::new(self.mtm());
        alert.setMessageText(&ns(&splat_i18n::tf(
            "Vertraust du den Dateien in „{}“?",
            &[&folder_name(&root)],
        )));
        alert.setInformativeText(&ns(t(
            "In einem vertrauten Ordner dürfen Git-Hooks, Sprachserver aus dem Projekt und Agenten laufen. \
             Eingeschränkt kannst du lesen und bearbeiten, aber nichts davon startet.",
        )));
        alert.addButtonWithTitle(&ns(t("Vertrauen")));
        alert.addButtonWithTitle(&ns(t("Eingeschränkt öffnen")));
        let handler = block2::RcBlock::new(move |response: objc2_app_kit::NSModalResponse| {
            guarded((), || {
                if response == objc2_app_kit::NSAlertFirstButtonReturn {
                    crate::project::trust(&root);
                    lifecycle::trust_changed(&root);
                }
            });
        });
        alert.beginSheetModalForWindow_completionHandler(&window, Some(&handler));
    }

    /// Asks again whether the user trusts the folder, for a panel that waits for it.
    pub fn ask_trust_again(&self) {
        if let Some(root) = self.project_root()
            && self.window().is_some_and(|w| w.attachedSheet().is_none())
        {
            self.ask_trust(root);
        }
    }

    /// The folder's root, if the window has one open.
    pub fn project_root(&self) -> Option<PathBuf> {
        self.ivars().workspace.borrow().as_ref().map(|p| p.root().to_path_buf())
    }

    /// Whether the user trusts this window's folder (E22).
    pub fn is_trusted(&self) -> bool {
        self.project_root()
            .is_some_and(|root| crate::project::is_trusted(&root))
    }

    /// What the window shows, for the next launch: its folder, its editors and welcome pages in every tab and which
    /// one is in front. The tabs of the other documents are the layout's, which `layout::restore` opens again.
    pub fn record(&self) -> crate::session::WindowRecord {
        use crate::session::TabRecord;
        let selected = self.selected_tab();
        let mut record = crate::session::WindowRecord {
            project: self.project(),
            ..Default::default()
        };
        for document in self.documents() {
            let tab = match document.kind {
                TabKind::Welcome => TabRecord::Welcome,
                TabKind::Editor => match crate::editor::tab_document(self, document.id) {
                    Some((path, recovery)) => TabRecord::Editor {
                        path,
                        recovery: recovery.as_str().to_string(),
                    },
                    None => continue,
                },
                _ => continue,
            };
            if selected == Some(document.id) {
                record.selected = Some(record.tabs.len());
            }
            record.tabs.push(tab);
        }
        record
    }

    /// Shows `text` at the foot of the documents of the tab in front for a few seconds.
    pub fn show_notice(&self, text: &str) {
        let Some(tab) = self.current_tab() else { return };
        let note = notice::show(self.mtm(), tab.content(), text);
        *self.ivars().notice.borrow_mut() = Some((note, text.to_string()));
    }

    /// Closes the tabs of the window in front once each of their documents agrees, asked one after the other; one
    /// that refuses keeps them all. The layout is the one before the first tab closed.
    fn close_window(&self) {
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
        if !self.documents_may_close(&documents) {
            return;
        }
        layout::save(self);
        self.ivars().closing.set(true);
        for window in windows {
            window.close();
        }
        self.ivars().closing.set(false);
    }

    fn bring_to_front(&self) {
        if let Some(window) = self.window() {
            window.makeKeyAndOrderFront(None);
        }
    }

    /// The window of the tab in front, which has the sidebar; sheets and the keyboard focus go there.
    pub fn window(&self) -> Option<Retained<NSWindow>> {
        self.ivars().current.borrow().as_ref().map(|tab| tab.window.clone())
    }

    /// The windows of the tabs, in the order of the tab bar.
    pub fn tab_windows(&self) -> Vec<Retained<NSWindow>> {
        self.ordered_tabs().iter().map(|tab| tab.window.clone()).collect()
    }

    /// Whether `window` is one of this window's tabs.
    pub fn owns_window(&self, window: &NSWindow) -> bool {
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

    /// The folder this window shows.
    pub fn project(&self) -> Option<PathBuf> {
        self.ivars().project.borrow().clone()
    }

    fn set_project(&self, project: Option<PathBuf>) {
        *self.ivars().project.borrow_mut() = project;
        self.update_title();
        if let Some(panels) = self.ivars().panels.borrow().as_ref() {
            panels.set_project(self.project().as_deref());
        }
        self.load_project();
    }

    fn frame_name(&self) -> String {
        match self.project() {
            Some(path) => format!("ProjectWindow {}", path.display()),
            None => "ProjectWindow".into(),
        }
    }

    fn update_title(&self) {
        for tab in self.ivars().tabs.borrow().iter() {
            self.update_title_of(&tab.window);
        }
    }

    /// Names the folder in the title bar of `window`; the tabs name their documents.
    fn update_title_of(&self, window: &NSWindow) {
        match self.project() {
            Some(path) => {
                window.setTitle(&ns(&folder_name(&path)));
                window.setSubtitle(&ns(&crate::app::display_path(&path)));
                window.setRepresentedURL(Some(&objc2_foundation::NSURL::fileURLWithPath(&ns(
                    &path.to_string_lossy()
                ))));
            }
            None => {
                window.setTitle(&ns(ide_product::NAME));
                window.setSubtitle(&ns(""));
            }
        }
    }

    /// Brings the welcome page to the front, or opens it; its number.
    pub(crate) fn open_welcome(&self) -> isize {
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

pub use ide_ui_common::panel::in_front;

/// Shows the tab bar of `window` also while it has one tab: a folder's documents always show as tabs (E60).
fn show_tab_bar(window: &NSWindow) {
    if window.tabGroup().is_some_and(|group| !group.isTabBarVisible()) {
        window.toggleTabBar(None);
    }
}

fn folder_name(path: &Path) -> String {
    path.file_name().map_or_else(
        || path.display().to_string(),
        |name| name.to_string_lossy().into_owned(),
    )
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::folder_name;

    #[test]
    fn a_window_is_named_after_its_folder_and_the_root_after_itself() {
        assert_eq!(folder_name(Path::new("/Users/a/dango")), "dango");
        assert_eq!(folder_name(Path::new("/")), "/");
    }
}

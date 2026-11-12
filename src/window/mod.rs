//! A folder's window: the activity bar and panel on the left, its documents on the right, each in a native window
//! tab of its own (E60).

mod activity_bar;
mod document_area;
pub(crate) mod file_outline;
mod files;
mod files_header;
mod frame;
mod full_screen;
pub mod layout;
mod new_tab_button;
mod notice;
pub mod panels;
mod quick_open;
mod replace;
mod search;
mod tabs;
pub mod toolbar;
mod welcome;

use std::cell::{Cell, RefCell};
use std::path::{Path, PathBuf};
use std::rc::Rc;

use objc2::rc::Retained;
use objc2::runtime::{AnyObject, NSObject, Sel};
use objc2::{DefinedClass, MainThreadMarker, MainThreadOnly, define_class, msg_send, sel};
use objc2_app_kit::{
    NSColor, NSControl, NSControlStateValueOff, NSControlStateValueOn, NSMenuItem, NSMenuItemValidation, NSToolbar,
    NSToolbarDelegate, NSToolbarItem, NSToolbarItemIdentifier, NSView, NSWindow, NSWindowDelegate,
    NSWindowOrderingMode,
};
use objc2_foundation::{NSArray, NSNotification, NSObjectProtocol};
use splat_i18n::t;

use crate::util::{guarded, main_soon, ns};
use activity_bar::ActivityBar;
use files::FileTree;
pub use notice::announce;
use panels::Panels;
use quick_open::QuickOpenPanel;
pub use search::SearchPanel;
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
const ACTIVITY_WIDTH: f64 = 44.0;
const PANEL_MIN_WIDTH: f64 = 200.0;
const PANEL_MAX_WIDTH: f64 = 560.0;
const PANEL_DEFAULT_WIDTH: f64 = 260.0;

/// What the panel beside the activity bar shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Panel {
    Files,
    Search,
    Agents,
    Git,
    Ci,
}

impl Panel {
    pub const ALL: [Panel; 5] = [Panel::Files, Panel::Search, Panel::Agents, Panel::Git, Panel::Ci];

    /// The tag of the menu items and buttons that show it.
    pub fn tag(self) -> isize {
        match self {
            Panel::Files => 1,
            Panel::Search => 2,
            Panel::Agents => 3,
            Panel::Git => 4,
            Panel::Ci => 5,
        }
    }

    pub fn from_tag(tag: isize) -> Option<Panel> {
        Panel::ALL.into_iter().find(|p| p.tag() == tag)
    }

    pub fn title(self) -> &'static str {
        match self {
            Panel::Files => t("Dateien"),
            Panel::Search => t("Suche"),
            Panel::Agents => t("Agenten"),
            Panel::Git => t("Git"),
            Panel::Ci => t("CI"),
        }
    }

    /// Its name in the settings, which no language translates.
    pub fn id(self) -> &'static str {
        match self {
            Panel::Files => "files",
            Panel::Search => "search",
            Panel::Agents => "agents",
            Panel::Git => "git",
            Panel::Ci => "ci",
        }
    }

    pub fn symbol(self) -> &'static str {
        match self {
            Panel::Files => "doc.on.doc",
            Panel::Search => "magnifyingglass",
            Panel::Agents => "sparkles",
            Panel::Git => "arrow.triangle.branch",
            Panel::Ci => "checkmark.seal",
        }
    }
}

#[derive(Default)]
pub struct Ivars {
    project: RefCell<Option<PathBuf>>,
    panel: Cell<Option<Panel>>,
    panel_open: Cell<bool>,
    panel_width: Cell<f64>,
    activity: RefCell<Option<ActivityBar>>,
    panels: RefCell<Option<Panels>>,
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
            guarded((), || self.show_quick_open());
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

        #[unsafe(method(idePlanComment:))]
        fn plan_comment_action(&self, _sender: Option<&AnyObject>) {
            guarded((), || crate::agent::plan::comment_from_keyboard(self));
        }
    }
);

thread_local! {
    static WINDOWS: RefCell<Vec<Retained<WindowController>>> = const { RefCell::new(Vec::new()) };
}

/// Drops the window controller at `this` from the open windows once AppKit is done closing its window.
fn main_soon_unregister(this: *const WindowController) {
    let address = this as usize;
    main_soon(move || {
        WINDOWS.with(|windows| {
            windows
                .borrow_mut()
                .retain(|w| std::ptr::from_ref::<WindowController>(w) as usize != address);
        });
        crate::session::changed();
    });
}

impl WindowController {
    /// Runs `f` with this window's controller once the main queue gets to it, if the window is still open.
    fn later(&self, f: impl FnOnce(&WindowController) + Send + 'static) {
        let address = std::ptr::from_ref(self) as usize;
        main_soon(move || {
            let controller = WINDOWS.with(|windows| {
                windows
                    .borrow()
                    .iter()
                    .find(|w| std::ptr::from_ref::<WindowController>(w) as usize == address)
                    .cloned()
            });
            if let Some(controller) = controller {
                f(&controller);
            }
        });
    }
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

/// Shows the changed folders `dirs` in the tree of the window that has `root` open, and lets editors of files there
/// take what changed on disk.
pub fn tree_changed(root: &Path, dirs: &[PathBuf]) {
    crate::editor::files_changed(dirs);
    WINDOWS.with(|windows| {
        for window in windows.borrow().iter() {
            let ivars = window.ivars();
            let same = ivars.workspace.borrow().as_ref().is_some_and(|p| p.root() == root);
            if same && let Some(files) = ivars.files.borrow().as_ref() {
                files.reload(dirs);
            }
        }
    });
}

/// Hides the symbol of `panel` in every window's activity bar, or shows it again, and remembers that; the last symbol
/// that shows stays.
pub fn toggle_activity_symbol(panel: Panel) {
    let mut hidden = activity_bar::hidden();
    if hidden.contains(&panel) {
        hidden.retain(|p| *p != panel);
    } else if hidden.len() + 1 < Panel::ALL.len() {
        hidden.push(panel);
    } else {
        return;
    }
    activity_bar::store_hidden(&hidden);
    for window in WINDOWS.with(|windows| windows.borrow().clone()) {
        window.leave_out_symbols(&hidden);
    }
}

/// Lets the tree of every window list what Git ignores and the system hides, or leave it out again.
pub fn show_ignored_files(on: bool) {
    WINDOWS.with(|windows| {
        for window in windows.borrow().iter() {
            let ivars = window.ivars();
            let Some(project) = ivars.workspace.borrow().clone() else {
                continue;
            };
            let changed = project
                .tree()
                .set_options(ide_project::TreeOptions { show_ignored: on });
            if let Some(files) = ivars.files.borrow().as_ref() {
                files.reload(&changed);
            }
        }
    });
}

/// Hands quick-open hits to the window whose folder is `root`.
fn quick_open_answered(root: &Path, asked: u64, hits: Vec<ide_project::Hit>) {
    WINDOWS.with(|windows| {
        for window in windows.borrow().iter() {
            if let Some(panel) = window.ivars().quick_open.borrow().as_ref()
                && panel.root() == root
            {
                panel.answered(asked, hits);
                return;
            }
        }
    });
}

/// Saves the window tabs of every window for its folder, and where the window is, as the app quits.
pub fn save_layouts() {
    for window in WINDOWS.with(|windows| windows.borrow().clone()) {
        if !window.ivars().closed.get() {
            layout::save(&window);
            window.save_frame();
        }
    }
}

/// The folders open in windows, front to back.
pub fn open_projects() -> Vec<PathBuf> {
    WINDOWS.with(|windows| windows.borrow().iter().filter_map(|w| w.project()).collect())
}

/// The project window in front: the main window, else the first on screen.
pub fn frontmost(mtm: MainThreadMarker) -> Option<Retained<WindowController>> {
    let app = objc2_app_kit::NSApplication::sharedApplication(mtm);
    let controllers: Vec<Retained<WindowController>> = WINDOWS.with(|windows| {
        windows
            .borrow()
            .iter()
            .filter(|w| !w.ivars().closed.get())
            .cloned()
            .collect()
    });
    let mut order: Vec<Retained<NSWindow>> = app.mainWindow().into_iter().collect();
    order.extend(app.orderedWindows().iter());
    order
        .iter()
        .find_map(|window| controllers.iter().find(|c| c.owns_window(window)).cloned())
}

/// The window that shows `root`.
pub fn for_project(root: &Path) -> Option<Retained<WindowController>> {
    WINDOWS.with(|windows| {
        windows
            .borrow()
            .iter()
            .find(|w| w.project_root().as_deref() == Some(root))
            .cloned()
    })
}

/// What each open window shows, in the order they opened, for the next launch.
pub fn records() -> Vec<crate::session::WindowRecord> {
    WINDOWS.with(|windows| {
        windows
            .borrow()
            .iter()
            .filter(|w| !w.ivars().closed.get())
            .map(|w| w.record())
            .collect()
    })
}

/// The open windows, for the self-test.
#[cfg(feature = "selftest")]
pub fn open_windows() -> Vec<Retained<WindowController>> {
    WINDOWS.with(|windows| windows.borrow().clone())
}

impl WindowController {
    /// Brings the window of `project` to the front, or puts it into an empty window, or opens a new one.
    pub fn open(mtm: MainThreadMarker, project: Option<PathBuf>) -> Retained<WindowController> {
        let existing = WINDOWS.with(|windows| {
            let windows = windows.borrow();
            match &project {
                Some(path) => windows
                    .iter()
                    .find(|w| w.project().as_deref() == Some(path))
                    .or_else(|| windows.iter().find(|w| w.project().is_none()))
                    .cloned(),
                None => None,
            }
        });
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
        WINDOWS.with(|windows| windows.borrow_mut().push(controller.clone()));
        crate::session::changed();
        controller
    }

    fn new(mtm: MainThreadMarker, project: Option<PathBuf>) -> Retained<WindowController> {
        let ivars = Ivars {
            project: RefCell::new(project),
            panel_width: Cell::new(PANEL_DEFAULT_WIDTH),
            next_id: Cell::new(1),
            ..Ivars::default()
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
        }
        *ivars.files.borrow_mut() = Some(tree);
        *ivars.search.borrow_mut() = Some(search);
        *ivars.workspace.borrow_mut() = Some(project);
        crate::git_panel::attach(self);
        crate::ci_panel::attach(self);
        crate::agent_panel::attach(self);
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
                    crate::git_panel::trust_changed(&root);
                    crate::ci_panel::trust_changed(&root);
                    crate::lsp::trust_changed(&root);
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

    fn show_quick_open(&self) {
        let Some(root) = self.ivars().workspace.borrow().as_ref().map(|p| p.root().to_path_buf()) else {
            return;
        };
        let Some(window) = self.window() else { return };
        let panel = self
            .ivars()
            .quick_open
            .borrow_mut()
            .get_or_insert_with(|| QuickOpenPanel::new(self.mtm(), self, root))
            .clone();
        panel.show(&window);
    }

    /// Shows the search panel with the keyboard focus in its field (⌘⇧F).
    pub fn show_search(&self) {
        let Some(search) = self.ivars().search.borrow().clone() else {
            return;
        };
        if !self.ivars().panel_open.get() {
            self.set_panel_open(true);
        }
        self.show_panel(Panel::Search);
        search.focus();
    }

    /// Asks the folder's index for up to `limit` files matching `query`; `answer` runs on another thread.
    pub fn query_files(&self, query: &str, limit: usize, answer: impl FnOnce(Vec<ide_project::Hit>) + Send + 'static) {
        let workspace = self.ivars().workspace.borrow();
        if let Some(quick_open) = workspace.as_ref().and_then(|p| p.quick_open()) {
            quick_open.query(query, limit, answer);
        }
    }

    /// The outline view of the folder tree, for its context menu.
    fn file_tree_view(&self) -> Option<Retained<objc2_app_kit::NSOutlineView>> {
        self.ivars().files.borrow().as_ref().map(FileTree::outline)
    }

    /// Shows `view` as a document of `kind` and brings it to the front: in a window tab of its own, or in the tab
    /// in front if that one is empty, or beside a document for `open_beside`. `key` names
    /// what it shows, so `select_tab` finds it again. The document's number.
    pub fn add_tab(&self, kind: TabKind, key: Option<String>, title: &str, view: Retained<NSView>) -> isize {
        let document = self.new_document(kind, key, title, view);
        let id = document.id;
        self.place(document);
        id
    }

    /// Shows `view` as the preview document, which takes the place of the one before, as a single click opens a
    /// file; an ordinary document when the user turned preview tabs off.
    pub fn add_preview_tab(&self, kind: TabKind, key: Option<String>, title: &str, view: Retained<NSView>) -> isize {
        if !ide_core::settings::Settings::load_user().get(&ide_core::settings::keys::PREVIEW_TABS) {
            return self.add_tab(kind, key, title, view);
        }
        let old = self.documents().into_iter().find(|doc| doc.is_preview());
        if let Some(old) = old {
            let tab = self.tab_of_document(old.id);
            match tab {
                Some(tab) if crate::editor::may_close(self, old.id) => {
                    let index = tab.documents().iter().position(|doc| doc.id == old.id);
                    self.forget_document(&tab, old.id);
                    let document = self.new_document(kind, key, title, view);
                    document.set_preview(true);
                    let id = document.id;
                    tab.insert(document, index);
                    self.select(&tab);
                    crate::session::changed();
                    return id;
                }
                _ => self.keep_tab(old.id),
            }
        }
        let id = self.add_tab(kind, key, title, view);
        if let Some((tab, document)) = self.find_document(id) {
            document.set_preview(true);
            tab.refresh();
        }
        id
    }

    /// Makes the document `id` an ordinary one if it is the preview document.
    pub fn keep_tab(&self, id: isize) {
        if let Some((tab, document)) = self.find_document(id)
            && document.is_preview()
        {
            document.set_preview(false);
            tab.refresh();
        }
    }

    /// The document showing `key`.
    pub fn tab_with_key(&self, key: &str) -> Option<isize> {
        self.documents()
            .into_iter()
            .find(|doc| doc.key.as_deref() == Some(key))
            .map(|doc| doc.id)
    }

    /// Brings the document showing `key` to the front; whether there is one.
    pub fn select_tab(&self, key: &str) -> bool {
        match self.tab_with_key(key) {
            Some(id) => {
                self.select_tab_by_id(id);
                true
            }
            None => false,
        }
    }

    pub fn set_tab_title(&self, id: isize, title: &str) {
        if let Some((tab, document)) = self.find_document(id) {
            document.set_title(title);
            tab.refresh();
        }
    }

    /// Shows a dot of `color` on the tab of the document `id`, or none.
    pub fn set_tab_status(&self, id: isize, color: Option<&NSColor>) {
        if let Some((tab, document)) = self.find_document(id) {
            document.set_status(color);
            tab.refresh();
        }
    }

    /// Tints the symbol of the document `id`.
    pub fn set_tab_tint(&self, id: isize, color: &NSColor) {
        if let Some((tab, document)) = self.find_document(id) {
            document.set_tint(color);
            tab.refresh();
        }
    }

    /// Shows the symbol of the document `id` in four colours, `colors` at its left, top, right and bottom point.
    pub fn set_tab_four_colours(&self, id: isize, colors: [Retained<NSColor>; 4]) {
        if let Some((tab, document)) = self.find_document(id) {
            document.set_four_colours(colors);
            tab.refresh();
        }
    }

    /// Runs `open`, which opens a document, so that the document shows beside the document `anchor` in its tab.
    pub fn open_beside(&self, anchor: isize, open: impl FnOnce()) {
        let before = self.ivars().beside.replace(Some(anchor));
        open();
        self.ivars().beside.set(before);
    }

    /// Opens `path` beside the document in front, as the folder tree's "Daneben öffnen" does; a file already open
    /// comes to the front where it is.
    pub fn open_file_beside(&self, path: &Path) {
        if let Some(id) = self.tab_with_key(&path.to_string_lossy()) {
            self.select_tab_by_id(id);
            return;
        }
        match self.selected_tab() {
            Some(anchor) => self.open_beside(anchor, || self.open_file(path)),
            None => self.open_file(path),
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

    /// Opens `path` in an editor tab, or selects the tab that has it and keeps it if it is the preview tab; a PDF,
    /// a picture, a media file or an office document opens in a tab that shows it.
    pub fn open_file(&self, path: &Path) {
        if crate::preview::is_document(path) && crate::preview::open_document(self, path, false) {
            return;
        }
        if let Some(id) = self.tab_with_key(&path.to_string_lossy()) {
            self.keep_tab(id);
            self.select_tab_by_id(id);
            return;
        }
        let size = ide_core::settings::Settings::load_user().get(&ide_core::settings::keys::FONT_SIZE);
        crate::editor::open_tab(self, path, size, false);
    }

    /// Opens `path` in the preview tab, as a single click in the folder tree does, or selects the tab that has it.
    pub fn preview_file(&self, path: &Path) {
        if crate::preview::is_document(path) && crate::preview::open_document(self, path, true) {
            return;
        }
        if self.select_tab(&path.to_string_lossy()) {
            return;
        }
        let size = ide_core::settings::Settings::load_user().get(&ide_core::settings::keys::FONT_SIZE);
        crate::editor::open_tab(self, path, size, true);
    }

    /// Marks the document `id` as having unsaved changes, or not; the first change keeps a preview document.
    pub fn set_tab_dirty(&self, id: isize, dirty: bool) {
        if let Some((tab, document)) = self.find_document(id) {
            document.set_dirty(dirty);
            if dirty {
                document.set_preview(false);
            }
            tab.refresh();
        }
    }

    /// Closes the document `id` once it agrees, such as an editor with unsaved changes or a pop-up whose page closed
    /// itself; a window tab left without documents closes with it.
    pub fn close_tab(&self, id: isize) {
        if crate::editor::may_close(self, id) {
            self.close_tab_now(id);
        }
    }

    /// Closes the document `id` without asking; its window tab goes with it when it was the tab's only one, and the
    /// window with its last tab.
    fn close_tab_now(&self, id: isize) {
        let Some(tab) = self.tab_of_document(id) else { return };
        self.forget_document(&tab, id);
        if tab.is_empty() {
            // A closing window has left its tab group already when AppKit tells its delegate.
            let next = self.neighbour(&tab);
            tab.window.close();
            let open = |next: &Rc<TabWindow>| self.ivars().tabs.borrow().iter().any(|t| Rc::ptr_eq(t, next));
            if let Some(next) = next.filter(open) {
                self.select(&next);
            }
        }
        crate::session::changed();
    }

    /// Takes the document `id` out of `tab` and tells what showed it that it closes.
    fn forget_document(&self, tab: &TabWindow, id: isize) {
        crate::editor::closing(self, id);
        crate::preview::closing(self, id);
        tab.remove(id);
    }

    /// Brings the document `id` to the front; the keyboard focus stays where it is in its tab.
    pub(crate) fn select_tab_by_id(&self, id: isize) {
        if let Some(tab) = self.tab_of_document(id) {
            tab.mark_focused(id);
            self.select(&tab);
            crate::agent::show_pending(self);
        }
    }

    /// The documents' numbers, tab by tab in the order of the tab bar, left to right in a tab.
    pub fn tab_ids(&self) -> Vec<isize> {
        self.documents().iter().map(|doc| doc.id).collect()
    }

    /// Closes every document without asking, for a window that is about to get the documents of the last run or of
    /// another folder; the window keeps the tab in front, empty, and closes the others.
    pub(crate) fn clear_tabs(&self) {
        let tabs = self.ivars().tabs.borrow().clone();
        let keep = self.current_tab().or_else(|| tabs.first().cloned());
        for tab in &tabs {
            for document in tab.documents() {
                self.forget_document(tab, document.id);
            }
        }
        for tab in tabs {
            if keep.as_ref().is_some_and(|keep| !Rc::ptr_eq(keep, &tab)) {
                tab.window.close();
            }
        }
    }

    /// Whether `documents` may close: no editor holds unsaved changes the user wants to keep. When one refuses, the
    /// tab stays, and editors whose text the user already gave up keep it after all, with their copies back.
    fn documents_may_close(&self, documents: &[Rc<Document>]) -> bool {
        let all = documents
            .iter()
            .all(|document| crate::editor::may_close(self, document.id));
        if !all {
            crate::editor::store_copies(self);
        }
        all
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

    /// The document commands act on: the focused one of the tab in front.
    pub fn selected_tab(&self) -> Option<isize> {
        self.current_tab()?.focused().map(|doc| doc.id)
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

    /// Brings the next or the previous tab of the window in front to the front.
    fn select_relative(&self, next: bool) {
        let Some(current) = self.current_tab() else { return };
        let Some(tabbed) = current.window.tabbedWindows() else {
            return;
        };
        let windows: Vec<Retained<NSWindow>> = tabbed.iter().collect();
        let Some(index) = windows.iter().position(|w| std::ptr::eq(&**w, &*current.window)) else {
            return;
        };
        let count = windows.len();
        let step = if next { 1 } else { count - 1 };
        let target = windows.get((index + step) % count).and_then(|w| self.tab_of_window(w));
        if let Some(target) = target {
            self.select(&target);
        }
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

    fn current_tab(&self) -> Option<Rc<TabWindow>> {
        self.ivars().current.borrow().clone()
    }

    fn tab_of_window(&self, window: &NSWindow) -> Option<Rc<TabWindow>> {
        self.ivars()
            .tabs
            .borrow()
            .iter()
            .find(|tab| std::ptr::eq(&*tab.window, window))
            .cloned()
    }

    fn tab_of_notification(&self, notification: &NSNotification) -> Option<Rc<TabWindow>> {
        let object = notification.object()?;
        let window = object.downcast_ref::<NSWindow>()?;
        self.tab_of_window(window)
    }

    fn tab_of_document(&self, id: isize) -> Option<Rc<TabWindow>> {
        self.ivars().tabs.borrow().iter().find(|tab| tab.holds(id)).cloned()
    }

    fn find_document(&self, id: isize) -> Option<(Rc<TabWindow>, Rc<Document>)> {
        let tab = self.tab_of_document(id)?;
        let document = tab.document(id)?;
        Some((tab, document))
    }

    /// The window tabs in the order of their tab bars, tab group by tab group.
    fn ordered_tabs(&self) -> Vec<Rc<TabWindow>> {
        let tabs = self.ivars().tabs.borrow().clone();
        let mut ordered: Vec<Rc<TabWindow>> = Vec::with_capacity(tabs.len());
        for tab in &tabs {
            let group: Vec<Retained<NSWindow>> = tab
                .window
                .tabbedWindows()
                .map(|tabbed| tabbed.iter().collect())
                .unwrap_or_else(|| vec![tab.window.clone()]);
            for window in group {
                let found = tabs.iter().find(|t| std::ptr::eq(&*t.window, &*window));
                if let Some(found) = found
                    && !ordered.iter().any(|o| Rc::ptr_eq(o, found))
                {
                    ordered.push(found.clone());
                }
            }
            if !ordered.iter().any(|o| Rc::ptr_eq(o, tab)) {
                ordered.push(tab.clone());
            }
        }
        ordered
    }

    /// The documents, tab by tab in the order of the tab bar.
    fn documents(&self) -> Vec<Rc<Document>> {
        self.ordered_tabs().iter().flat_map(|tab| tab.documents()).collect()
    }

    fn new_document(&self, kind: TabKind, key: Option<String>, title: &str, view: Retained<NSView>) -> Rc<Document> {
        let id = self.ivars().next_id.get();
        self.ivars().next_id.set(id + 1);
        Document::new(self, id, kind, key, title, view)
    }

    /// A new window tab of this window, not yet in its tab group.
    fn new_tab_window(&self) -> Rc<TabWindow> {
        let tab = TabWindow::new(self, &self.tabbing_identifier(), None);
        self.update_title_of(&tab.window);
        self.ivars().tabs.borrow_mut().push(tab.clone());
        tab
    }

    /// Shows `document` where `add_tab` says.
    fn place(&self, document: Rc<Document>) {
        let anchor = self.ivars().beside.get();
        if let Some((tab, _)) = anchor.and_then(|anchor| self.find_document(anchor)) {
            self.put_beside(&tab, anchor.unwrap_or_default(), document);
            crate::session::changed();
            return;
        }
        let current = self.current_tab();
        if let Some(tab) = &current
            && tab.is_empty()
        {
            tab.insert(document, None);
            self.select(tab);
            crate::session::changed();
            return;
        }
        let tab = self.new_tab_window();
        tab.insert(document, None);
        if let Some(current) = &current {
            current
                .window
                .addTabbedWindow_ordered(&tab.window, NSWindowOrderingMode::Above);
        }
        self.select(&tab);
        crate::session::changed();
    }

    /// Puts `document` right of the document `anchor` in `tab`; a document there before gets a tab of its own.
    fn put_beside(&self, tab: &Rc<TabWindow>, anchor: isize, document: Rc<Document>) {
        let others: Vec<Rc<Document>> = tab.documents().into_iter().filter(|doc| doc.id != anchor).collect();
        for other in others {
            tab.remove(other.id);
            let own = self.new_tab_window();
            own.insert(other, None);
            tab.window
                .addTabbedWindow_ordered(&own.window, NSWindowOrderingMode::Above);
            own.refresh();
        }
        tab.insert(document, None);
        tab.mark_focused(anchor);
        self.select(tab);
    }

    /// Brings `tab` to the front of its tab group and gives it the sidebar.
    fn select(&self, tab: &Rc<TabWindow>) {
        if let Some(group) = tab.window.tabGroup() {
            group.setSelectedWindow(Some(&tab.window));
        }
        // A tab draws the icon of its attributed title only when it gets the title in its tab group.
        tab.refresh();
        if self.current_tab().is_some_and(|current| current.window.isVisible()) || tab.window.isVisible() {
            tab.window.makeKeyAndOrderFront(None);
            show_tab_bar(&tab.window);
        }
        self.make_current(tab);
        crate::session::changed();
    }

    /// Makes the tab the group shows the one in front, after AppKit picked another, as a closed tab leaves.
    fn follow_selection(&self) {
        let selected = self
            .current_tab()
            .and_then(|tab| tab.window.tabGroup()?.selectedWindow());
        if let Some(tab) = selected.and_then(|window| self.tab_of_window(&window)) {
            self.make_current(&tab);
        }
    }

    /// Makes `tab` the one in front: the sidebar moves into it at the width it had. The tab it leaves keeps a picture
    /// of it, for a window that stays on screen beside, after a tab was dragged out, and for the overview of all tabs.
    fn make_current(&self, tab: &Rc<TabWindow>) {
        let previous = self.current_tab();
        if previous.as_ref().is_some_and(|previous| Rc::ptr_eq(previous, tab)) {
            self.list_front_tabs();
            return;
        }
        let mut collapsed = false;
        if let Some(previous) = &previous {
            collapsed = previous.sidebar_collapsed();
            self.remember_sidebar_width(previous);
            if let Some(view) = self.ivars().sidebar.borrow().as_ref() {
                previous.keep_picture_of(view);
            }
        }
        *self.ivars().current.borrow_mut() = Some(tab.clone());
        if let Some(view) = self.ivars().sidebar.borrow().as_ref() {
            tab.adopt_sidebar(view);
        }
        self.fit_sidebar(tab, collapsed);
        self.list_front_tabs();
    }

    /// Lists only the tab in front of each tab group in the Window menu and the Dock's menu, so a folder appears once
    /// there however many tabs it has; a tab dragged out into a window of its own stays listed.
    fn list_front_tabs(&self) {
        let current = self.current_tab();
        for tab in self.ivars().tabs.borrow().iter() {
            let hidden = tab.window.tabGroup().is_some_and(|group| {
                let windows = group.windows();
                let front = current
                    .as_ref()
                    .map(|current| current.window.clone())
                    .filter(|window| windows.iter().any(|w| std::ptr::eq(&*w, &**window)))
                    .or_else(|| group.selectedWindow());
                windows.len() > 1 && front.is_some_and(|front| !std::ptr::eq(&*front, &*tab.window))
            });
            tab.window.setExcludedFromWindowsMenu(hidden);
        }
    }

    /// Keeps the width the user gave the sidebar in `tab`.
    fn remember_sidebar_width(&self, tab: &TabWindow) {
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
    fn fit_sidebar(&self, tab: &TabWindow, collapsed: bool) {
        let ivars = self.ivars();
        let open = ivars.panel_open.get();
        let width = if open {
            ACTIVITY_WIDTH + ivars.panel_width.get()
        } else {
            ACTIVITY_WIDTH
        };
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

    /// A window tab closes: its documents go with it; the window goes with its last tab.
    fn tab_closes(&self, tab: &Rc<TabWindow>) {
        let last = self.ivars().tabs.borrow().len() == 1;
        if last {
            self.window_closes();
        }
        for document in tab.documents() {
            self.forget_document(tab, document.id);
        }
        let neighbour = self.neighbour(tab);
        self.ivars().tabs.borrow_mut().retain(|other| !Rc::ptr_eq(other, tab));
        if self.current_tab().is_some_and(|current| Rc::ptr_eq(&current, tab)) {
            *self.ivars().current.borrow_mut() = None;
            let next = neighbour.or_else(|| self.ivars().tabs.borrow().first().cloned());
            if let Some(next) = next {
                self.make_current(&next);
                self.later(WindowController::follow_selection);
            }
        }
        crate::session::changed();
    }

    /// The tab that comes to the front when `tab` closes, as AppKit picks it: the next one in its tab bar, else the one
    /// before.
    fn neighbour(&self, tab: &TabWindow) -> Option<Rc<TabWindow>> {
        let windows: Vec<Retained<NSWindow>> = tab.window.tabbedWindows()?.iter().collect();
        let index = windows.iter().position(|w| std::ptr::eq(&**w, &*tab.window))?;
        windows
            .get(index + 1)
            .or_else(|| index.checked_sub(1).and_then(|before| windows.get(before)))
            .and_then(|window| self.tab_of_window(window))
    }

    /// The window's last tab closes: its layout is kept, and what runs for its folder ends.
    fn window_closes(&self) {
        layout::save(self);
        self.save_frame();
        self.ivars().closed.set(true);
        if let Some(search) = self.ivars().search.borrow().as_ref() {
            search.stop();
        }
        if let Some(root) = self.project_root() {
            crate::lsp::window_closed(&root);
        }
        crate::agent::window_closed(self);
        crate::ci_panel::window_closed(self);
        let this: *const WindowController = self;
        main_soon_unregister(this);
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

    /// Shows `panel`, opening the sidebar if it is closed; the panel that already shows stays open (E61).
    pub fn activate_panel(&self, panel: Panel) {
        if !self.ivars().panel_open.get() {
            self.set_panel_open(true);
        }
        self.show_panel(panel);
    }

    fn show_panel(&self, panel: Panel) {
        self.ivars().panel.set(Some(panel));
        if let Some(panels) = self.ivars().panels.borrow().as_ref() {
            panels.show(panel);
        }
        if let Some(activity) = self.ivars().activity.borrow().as_ref() {
            activity.select(Some(panel));
        }
    }

    /// Hides the activity bar's symbols of `hidden`; a hidden panel that showed gives way to the first that still has
    /// its symbol.
    fn leave_out_symbols(&self, hidden: &[Panel]) {
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
    fn focus_panel(&self) {
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
    fn set_panel_open(&self, open: bool) {
        let ivars = self.ivars();
        let Some(tab) = self.current_tab() else { return };
        if let Some(panels) = ivars.panels.borrow().as_ref() {
            panels.view().setHidden(!open);
        }
        if !open {
            self.remember_sidebar_width(&tab);
        }
        ivars.panel_open.set(open);
        self.fit_sidebar(&tab, tab.sidebar_collapsed());
        if let Some(activity) = ivars.activity.borrow().as_ref() {
            activity.select(open.then(|| ivars.panel.get()).flatten());
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

    fn validate(&self, item: &NSMenuItem) -> bool {
        let action = item.action();
        let ivars = self.ivars();
        if action == Some(sel!(idePlanComment:)) {
            return crate::agent::plan::keyboard_target(self).is_some();
        }
        if action == Some(sel!(ideShowPanel:)) {
            let current = ivars.panel_open.get().then(|| ivars.panel.get()).flatten();
            let on = Panel::from_tag(item.tag()) == current && current.is_some();
            item.setState(if on {
                NSControlStateValueOn
            } else {
                NSControlStateValueOff
            });
            return true;
        }
        if action == Some(sel!(ideToggleActivitySymbol:)) {
            let hidden = activity_bar::hidden();
            let shown = Panel::from_tag(item.tag()).is_some_and(|panel| !hidden.contains(&panel));
            item.setState(if shown {
                NSControlStateValueOn
            } else {
                NSControlStateValueOff
            });
            return !shown || hidden.len() + 1 < Panel::ALL.len();
        }
        if action == Some(sel!(ideQuickOpen:)) || action == Some(sel!(ideSearchProject:)) {
            return ivars.workspace.borrow().is_some();
        }
        if let Some(action) = action.filter(|action| browser_commands().contains(action)) {
            return crate::browser::validate(self, action);
        }
        if action == Some(sel!(ideNewWorktreeThread:)) {
            let index = usize::try_from(item.tag()).unwrap_or(usize::MAX);
            return crate::worktrees::can_start(self) && crate::worktrees::agent_usable(index);
        }
        if action == Some(sel!(ideNewAcpThread:)) {
            let index = usize::try_from(item.tag()).unwrap_or(usize::MAX);
            return crate::agent::preset_menu()
                .get(index)
                .is_some_and(|(_, usable)| *usable);
        }
        if action == Some(sel!(ideNextTab:)) || action == Some(sel!(idePreviousTab:)) {
            return self
                .window()
                .and_then(|window| window.tabbedWindows())
                .is_some_and(|tabbed| tabbed.count() > 1);
        }
        if action == Some(sel!(ideFocusNextPanel:)) {
            return self.current_tab().is_some_and(|tab| tab.documents().len() > 1);
        }
        true
    }

    /// The agents the menu of new tabs lists under "In eigenem Worktree".
    #[cfg(feature = "selftest")]
    pub fn worktree_menu_titles(&self) -> Vec<String> {
        menu_titles(&self.new_tab_menu(), &[t("In eigenem Worktree")])
    }

    /// The context menu of `panel`'s symbol in the activity bar.
    #[cfg(feature = "selftest")]
    pub fn activity_menu(&self, panel: Panel) -> Option<Retained<objc2_app_kit::NSMenu>> {
        self.ivars().activity.borrow().as_ref()?.menu_of(panel)
    }

    /// The panels whose symbol the activity bar shows.
    #[cfg(feature = "selftest")]
    pub fn shown_symbols(&self) -> Vec<Panel> {
        self.ivars()
            .activity
            .borrow()
            .as_ref()
            .map(ActivityBar::shown)
            .unwrap_or_default()
    }

    #[cfg(feature = "selftest")]
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
#[cfg(feature = "selftest")]
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

#[cfg(feature = "selftest")]
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
        self.show_quick_open();
        if let Some(panel) = self.ivars().quick_open.borrow().as_ref() {
            panel.type_query(query);
        }
    }

    pub fn close_quick_open(&self) {
        if let Some(panel) = self.ivars().quick_open.borrow().as_ref() {
            panel.close();
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
    pub fn window_tab_ids(&self) -> Vec<Vec<isize>> {
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

    /// Closes every tab of the window in front, as ⇧⌘W does.
    pub fn close_all_tabs(&self) {
        self.close_window();
    }

    /// Sets the left document's share of the tab that shows the document `id` beside another.
    pub fn set_share_of(&self, id: isize, share: f64) {
        if let Some(tab) = self.tab_of_document(id) {
            tab.set_share(share);
        }
    }

    /// The menu of new tabs, filled as when the "+" opens it.
    pub fn new_tab_menu(&self) -> Retained<objc2_app_kit::NSMenu> {
        let menu = tabs::new_tab_menu(self.mtm());
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
            tab.focus_other();
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
    #[cfg(feature = "selftest")]
    pub fn sidebar_range_of(&self, id: isize) -> Option<(f64, f64)> {
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
        let at = button.convertPoint_toView(center, None);
        let app = objc2_app_kit::NSApplication::sharedApplication(self.mtm());
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
    pub fn share_of(&self, id: isize) -> Option<f64> {
        self.tab_of_document(id)?.share()
    }

    /// What the badges in the tab of the document `id` say, left to right.
    pub fn badges_of(&self, id: isize) -> Vec<Option<(String, String)>> {
        self.tab_of_document(id).map(|tab| tab.badges()).unwrap_or_default()
    }

    /// Moves the divider of the tab of the document `id` to `share`, as a drag does.
    pub fn drag_share_of(&self, id: isize, share: f64) {
        if let Some(tab) = self.tab_of_document(id) {
            tab.drag_share(share);
        }
    }

    /// The close button on the badge of the document `id`.
    pub fn badge_close(&self, id: isize) -> Option<Retained<objc2_app_kit::NSButton>> {
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
#[cfg(feature = "selftest")]
pub struct Inspection {
    pub activity_buttons: usize,
    pub panel: Option<Panel>,
    pub panel_title: String,
    pub tabs: usize,
    pub new_tab_items: Vec<String>,
    pub sidebar_width: f64,
}

/// Whether `window` is on screen and the tab in front of its tab group; the tabs behind count as visible to AppKit.
pub fn in_front(window: &NSWindow) -> bool {
    window.isVisible()
        && window
            .tabGroup()
            .and_then(|group| group.selectedWindow())
            .is_none_or(|selected| std::ptr::eq(&*selected, window))
}

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

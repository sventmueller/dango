//! A project window: the activity bar and panel on the left, tabs on the right.

mod activity_bar;
mod files;
mod files_header;
pub mod layout;
mod notice;
mod panels;
mod panes;
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
use objc2::runtime::{AnyObject, ProtocolObject, Sel};
use objc2::{DefinedClass, MainThreadMarker, MainThreadOnly, define_class, msg_send, sel};
use objc2_app_kit::{
    NSBackingStoreType, NSColor, NSControl, NSControlStateValueOff, NSControlStateValueOn, NSMenuItem,
    NSMenuItemValidation, NSSplitViewController, NSSplitViewItem, NSToolbar, NSToolbarDelegate, NSToolbarDisplayMode,
    NSToolbarItem, NSToolbarItemIdentifier, NSView, NSViewController, NSWindow, NSWindowController, NSWindowDelegate,
    NSWindowStyleMask, NSWindowToolbarStyle,
};
use objc2_foundation::{NSArray, NSNotification, NSObjectProtocol, NSPoint, NSRect, NSSize};
use splat_i18n::t;

use crate::util::{guarded, main_soon, ns};
use activity_bar::ActivityBar;
use files::FileTree;
pub use notice::announce;
use panels::Panels;
use panes::Panes;
pub use panes::Side;
use quick_open::QuickOpenPanel;
pub use search::SearchPanel;
#[cfg(feature = "selftest")]
pub use search::{LIMIT as SEARCH_LIMIT, Toggle as SearchToggle, selftest as search_selftest};
pub use search::{buffer_edited, files_changed as files_changed_on_disk};
use tabs::TabBar;
pub use tabs::TabKind;

/// The width of the activity bar, which stays when the panel beside it is closed.
const ACTIVITY_WIDTH: f64 = 44.0;
const PANEL_MIN_WIDTH: f64 = 200.0;
const PANEL_MAX_WIDTH: f64 = 560.0;
const PANEL_DEFAULT_WIDTH: f64 = 260.0;
const WINDOW_SIZE: NSSize = NSSize::new(1240.0, 780.0);
const MIN_WINDOW_SIZE: NSSize = NSSize::new(720.0, 440.0);

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
    /// The panels of the content area, each with its tabs.
    panes: RefCell<Option<Panes>>,
    split: RefCell<Option<Retained<NSSplitViewController>>>,
    sidebar: RefCell<Option<Retained<NSSplitViewItem>>>,
    workspace: RefCell<Option<Rc<crate::project::Project>>>,
    files: RefCell<Option<FileTree>>,
    search: RefCell<Option<Retained<SearchPanel>>>,
    quick_open: RefCell<Option<Retained<QuickOpenPanel>>>,
    /// The note at the foot of the tab area and what it says.
    notice: RefCell<Option<(Retained<objc2_app_kit::NSBox>, String)>>,
    /// Whether the window closed; it stays among the open ones until the main queue gets to it, but has nothing left
    /// to write down.
    closed: Cell<bool>,
}

define_class!(
    // SAFETY: NSWindowController has no subclassing requirements and `WindowController` does not implement Drop.
    #[unsafe(super(NSWindowController))]
    #[thread_kind = MainThreadOnly]
    #[name = "IDEWindowController"]
    #[ivars = Ivars]
    pub struct WindowController;

    unsafe impl NSObjectProtocol for WindowController {}

    unsafe impl NSWindowDelegate for WindowController {
        #[unsafe(method(windowDidUpdate:))]
        fn did_update(&self, _notification: &NSNotification) {
            guarded((), || {
                self.with_panes(Panes::follow_focus);
            });
        }

        #[unsafe(method(windowShouldClose:))]
        fn should_close(&self, _window: &NSWindow) -> bool {
            guarded(true, || self.tabs_may_close())
        }

        #[unsafe(method(windowWillClose:))]
        fn will_close(&self, _notification: &NSNotification) {
            guarded((), || layout::save(self));
            self.ivars().closed.set(true);
            guarded((), || {
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

        #[unsafe(method(ideNewTab:))]
        fn new_tab_action(&self, sender: Option<&AnyObject>) {
            guarded((), || {
                let number = usize::try_from(tag_of(sender)).unwrap_or(0);
                self.with_panes(|panes| {
                    if let Some(pane) = panes.by_number(number) {
                        panes.activate(&pane);
                    }
                });
                self.with_tabs(TabBar::show_new_tab_menu);
            });
        }

        #[unsafe(method(ideFocusNextPanel:))]
        fn focus_next_panel_action(&self, _sender: Option<&AnyObject>) {
            guarded((), || {
                self.with_panes(Panes::focus_next);
            });
        }

        #[unsafe(method(ideCloseTab:))]
        fn close_tab_action(&self, _sender: Option<&AnyObject>) {
            guarded((), || {
                let selected = self.selected_tab();
                match selected {
                    Some(id) => self.close_tab(id),
                    None => {
                        if let Some(window) = self.window() {
                            window.performClose(None);
                        }
                    }
                }
            });
        }

        #[unsafe(method(ideNextTab:))]
        fn next_tab_action(&self, _sender: Option<&AnyObject>) {
            guarded((), || self.with_tabs(|tabs| tabs.select_relative(1)));
        }

        #[unsafe(method(idePreviousTab:))]
        fn previous_tab_action(&self, _sender: Option<&AnyObject>) {
            guarded((), || self.with_tabs(|tabs| tabs.select_relative(-1)));
        }

        #[unsafe(method(ideSelectTab:))]
        fn select_tab_action(&self, sender: Option<&AnyObject>) {
            guarded((), || self.select_tab_by_id(tag_of(sender)));
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

        #[unsafe(method(ideNewCliThread:))]
        fn new_cli_thread_action(&self, sender: Option<&AnyObject>) {
            guarded((), || {
                let item = sender.and_then(|s| s.downcast_ref::<NSMenuItem>());
                let name = item.and_then(|i| i.representedObject()?.downcast::<objc2_foundation::NSString>().ok());
                if let Some(name) = name {
                    crate::terminal::open_cli_thread(self, &name.to_string());
                }
            });
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

/// Whether macOS docks bars above a split view item's content (macOS 26 and later), where scroll views know them and
/// leave room instead of covering them.
fn split_accessories_available() -> bool {
    objc2::runtime::AnyClass::get(c"NSSplitViewItemAccessoryViewController").is_some()
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

/// Saves the panels of every window for its folder, as the app quits.
pub fn save_layouts() {
    for window in WINDOWS.with(|windows| windows.borrow().clone()) {
        if !window.ivars().closed.get() {
            layout::save(&window);
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
    order.iter().find_map(|window| {
        controllers
            .iter()
            .find(|c| c.window().is_some_and(|own| std::ptr::eq(&*own, &**window)))
            .cloned()
    })
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
        let controller = WindowController::new(mtm, project);
        controller.bring_to_front();
        controller.settle_sidebar();
        WINDOWS.with(|windows| windows.borrow_mut().push(controller.clone()));
        crate::session::changed();
        controller
    }

    fn new(mtm: MainThreadMarker, project: Option<PathBuf>) -> Retained<WindowController> {
        let style = NSWindowStyleMask::Titled
            | NSWindowStyleMask::Closable
            | NSWindowStyleMask::Miniaturizable
            | NSWindowStyleMask::Resizable
            | NSWindowStyleMask::FullSizeContentView;
        // SAFETY: releasedWhenClosed is switched off right below, so Rust's reference stays the owner.
        let window = unsafe {
            NSWindow::initWithContentRect_styleMask_backing_defer(
                NSWindow::alloc(mtm),
                NSRect::new(NSPoint::new(0.0, 0.0), WINDOW_SIZE),
                style,
                NSBackingStoreType::Buffered,
                false,
            )
        };
        // SAFETY: Switching it off keeps AppKit from releasing the window behind Rust's back.
        unsafe { window.setReleasedWhenClosed(false) };
        window.setMinSize(MIN_WINDOW_SIZE);
        // Panels, tabs and their views come and go after the window shows, past the key view loop AppKit makes once.
        window.setAutorecalculatesKeyViewLoop(true);
        window.setToolbarStyle(NSWindowToolbarStyle::Unified);
        let ivars = Ivars {
            project: RefCell::new(project),
            panel_width: Cell::new(PANEL_DEFAULT_WIDTH),
            ..Ivars::default()
        };
        let this = Self::alloc(mtm).set_ivars(ivars);
        // SAFETY: NSWindowController's initializer, called once on the newly allocated object.
        let this: Retained<Self> = unsafe { msg_send![super(this), initWithWindow: Some(&*window)] };
        window.setDelegate(Some(ProtocolObject::from_ref(&*this)));
        this.build(&window);
        let toolbar = NSToolbar::initWithIdentifier(NSToolbar::alloc(mtm), &ns("IDEProjectToolbar"));
        toolbar.setDelegate(Some(ProtocolObject::from_ref(&*this)));
        toolbar.setDisplayMode(NSToolbarDisplayMode::IconOnly);
        window.setToolbar(Some(&toolbar));
        this.update_title();
        this.update_tabbing_identifier();
        this.restore_frame(&window);
        this
    }

    /// Gives the window the tab group of its folder, so that macOS never merges windows of different folders, and a
    /// window without a folder one of its own (E56).
    fn update_tabbing_identifier(&self) {
        let Some(window) = self.window() else { return };
        let identifier = match self.project() {
            Some(path) => format!("folder {}", path.display()),
            None => format!("window {:p}", self),
        };
        window.setTabbingIdentifier(&ns(&identifier));
    }

    /// Puts the window where it was for this folder, else at its default size in the middle, with the sidebar at its
    /// default width; setting the split view shrank it to what its views need.
    fn restore_frame(&self, window: &NSWindow) {
        let name = ns(&self.frame_name());
        if !window.setFrameUsingName(&name) {
            window.setContentSize(WINDOW_SIZE);
            window.center();
        }
        let _ = window.setFrameAutosaveName(&name);
    }

    /// Gives the sidebar the width it had for this folder, else its default; the split view takes a divider position
    /// only once the window shows.
    fn settle_sidebar(&self) {
        let name = format!("Sidebar {}", self.frame_name());
        let saved = objc2_foundation::NSUserDefaults::standardUserDefaults()
            .objectForKey(&ns(&format!("NSSplitView Subview Frames {name}")))
            .is_some();
        if let Some(split) = self.ivars().split.borrow().as_ref() {
            let view = split.splitView();
            if !saved {
                view.setPosition_ofDividerAtIndex(ACTIVITY_WIDTH + PANEL_DEFAULT_WIDTH, 0);
            }
            view.setAutosaveName(Some(&ns(&name)));
        }
    }

    fn build(&self, window: &NSWindow) {
        let mtm = self.mtm();
        let split = NSSplitViewController::new(mtm);

        let sidebar_view = NSView::new(mtm);
        let activity = ActivityBar::new(mtm, self);
        let panels = Panels::new(mtm, self, self.project().as_deref());
        sidebar_view.addSubview(activity.view());
        sidebar_view.addSubview(panels.view());
        let guide = sidebar_view.safeAreaLayoutGuide();
        activity.view().setTranslatesAutoresizingMaskIntoConstraints(false);
        panels.view().setTranslatesAutoresizingMaskIntoConstraints(false);
        for constraint in [
            activity
                .view()
                .leadingAnchor()
                .constraintEqualToAnchor(&sidebar_view.leadingAnchor()),
            activity.view().topAnchor().constraintEqualToAnchor(&guide.topAnchor()),
            activity
                .view()
                .bottomAnchor()
                .constraintEqualToAnchor(&sidebar_view.bottomAnchor()),
            activity.view().widthAnchor().constraintEqualToConstant(ACTIVITY_WIDTH),
            panels
                .view()
                .leadingAnchor()
                .constraintEqualToAnchor(&activity.view().trailingAnchor()),
            panels
                .view()
                .trailingAnchor()
                .constraintEqualToAnchor(&sidebar_view.trailingAnchor()),
            panels.view().topAnchor().constraintEqualToAnchor(&guide.topAnchor()),
            panels
                .view()
                .bottomAnchor()
                .constraintEqualToAnchor(&sidebar_view.bottomAnchor()),
        ] {
            constraint.setActive(true);
        }
        let sidebar_controller = NSViewController::new(mtm);
        sidebar_controller.setView(&sidebar_view);
        let sidebar = NSSplitViewItem::sidebarWithViewController(&sidebar_controller);
        sidebar.setMinimumThickness(ACTIVITY_WIDTH + PANEL_MIN_WIDTH);
        sidebar.setMaximumThickness(ACTIVITY_WIDTH + PANEL_MAX_WIDTH);
        // Above the content's, so resizing the window resizes the content and the sidebar keeps its width.
        sidebar.setHoldingPriority(260.0);

        let panes = Panes::new(mtm, self, split_accessories_available());
        let content = NSSplitViewItem::splitViewItemWithViewController(panes.controller());
        content.setMinimumThickness(360.0);

        split.addSplitViewItem(&sidebar);
        split.addSplitViewItem(&content);
        window.setContentViewController(Some(&split));

        let ivars = self.ivars();
        *ivars.activity.borrow_mut() = Some(activity);
        *ivars.panels.borrow_mut() = Some(panels);
        *ivars.panes.borrow_mut() = Some(panes);
        *ivars.split.borrow_mut() = Some(split);
        *ivars.sidebar.borrow_mut() = Some(sidebar);
        ivars.panel_open.set(true);
        self.show_panel(Panel::Files);
        self.open_welcome();
        self.load_project();
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
            panels.set_files(&tree.page(self.mtm()));
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

    /// Opens the panels and tabs the folder's window had when it closed, as opening a folder does.
    pub fn restore_layout(&self) {
        let size = ide_core::settings::Settings::load_user().get(&ide_core::settings::keys::FONT_SIZE);
        layout::restore(self, &mut |controller, document| match document {
            layout::Document::File(path) => path.is_file() && crate::editor::open_quietly(controller, path, size),
            layout::Document::Untitled(_) => false,
        });
    }

    /// `restore_layout` with `editor` opening each editor's document in the active panel, and saying whether it did,
    /// so the launch can put the copies a crash left in their places (`session::restore`).
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

    /// Shows a dot in `color` on the symbol of `panel` with `words` for VoiceOver, such as a failed CI run; `None`
    /// hides it.
    pub fn set_status_badge(&self, panel: Panel, status: Option<(&NSColor, &str)>) {
        if let Some(activity) = self.ivars().activity.borrow().as_ref() {
            activity.set_status(panel, status);
        }
    }

    /// Shows `count` on the symbol of `panel`; 0 hides it.
    pub fn set_badge(&self, panel: Panel, count: usize) {
        if let Some(activity) = self.ivars().activity.borrow().as_ref() {
            activity.set_badge(panel, count);
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

    /// Adds a tab of `kind` showing `view` to the active panel and selects it; `key` names what it shows, so
    /// `select_tab` finds it again.
    pub fn add_tab(&self, kind: TabKind, key: Option<String>, title: &str, view: Retained<NSView>) -> isize {
        let mut id = -1;
        self.with_tabs(|tabs| id = tabs.add(kind, key, title, view));
        id
    }

    /// Shows `view` in the active panel's preview tab, which takes the place of the one before, as a single click
    /// opens a file; an ordinary tab when the user turned preview tabs off.
    pub fn add_preview_tab(&self, kind: TabKind, key: Option<String>, title: &str, view: Retained<NSView>) -> isize {
        if !ide_core::settings::Settings::load_user().get(&ide_core::settings::keys::PREVIEW_TABS) {
            return self.add_tab(kind, key, title, view);
        }
        let mut old = None;
        self.with_tabs(|tabs| old = tabs.preview_id());
        let replace = old.filter(|id| crate::editor::may_close(self, *id));
        if let Some(old) = replace {
            // The replaced tab closes as any other does: its editor and the preview that opened with it go.
            crate::editor::closing(self, old);
            crate::preview::closing(self, old);
        }
        let mut id = -1;
        self.with_tabs(|tabs| {
            if let Some(old) = old
                && replace.is_none()
            {
                tabs.keep(old);
            }
            id = tabs.add_preview(kind, key, title, view, replace);
        });
        id
    }

    /// Makes the tab `id` an ordinary one if it is its panel's preview tab.
    pub fn keep_tab(&self, id: isize) {
        self.with_tab(id, |tabs| tabs.keep(id));
    }

    /// The tab showing `key`, in any panel.
    pub fn tab_with_key(&self, key: &str) -> Option<isize> {
        self.with_panes(|panes| panes.all().iter().find_map(|pane| pane.tabs.find_key(key)))
            .flatten()
    }

    /// Selects the tab showing `key` in its panel; whether there is one.
    pub fn select_tab(&self, key: &str) -> bool {
        self.with_panes(|panes| {
            let found = panes
                .all()
                .into_iter()
                .find_map(|pane| pane.tabs.find_key(key).map(|id| (pane, id)));
            if let Some((pane, id)) = &found {
                pane.tabs.select(*id);
                panes.set_active(pane);
            }
            found.is_some()
        })
        .unwrap_or(false)
    }

    pub fn set_tab_title(&self, id: isize, title: &str) {
        self.with_tab(id, |tabs| tabs.set_title(id, title));
    }

    /// Shows a dot of `color` on the tab `id`, or none.
    pub fn set_tab_status(&self, id: isize, color: Option<&NSColor>) {
        self.with_tab(id, |tabs| tabs.set_status(id, color));
    }

    /// Tints the symbol of the tab `id`.
    pub fn set_tab_tint(&self, id: isize, color: &NSColor) {
        self.with_tabs(|tabs| tabs.set_tint(id, color));
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

    /// Opens `path` in an editor tab, or selects the tab that has it and keeps it if it is the preview tab; a PDF file
    /// opens in a tab that shows it.
    pub fn open_file(&self, path: &Path) {
        if crate::preview::is_pdf(path) {
            crate::preview::open_pdf(self, path, false);
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
        if crate::preview::is_pdf(path) {
            crate::preview::open_pdf(self, path, true);
            return;
        }
        if self.select_tab(&path.to_string_lossy()) {
            return;
        }
        let size = ide_core::settings::Settings::load_user().get(&ide_core::settings::keys::FONT_SIZE);
        crate::editor::open_tab(self, path, size, true);
    }

    pub fn set_tab_dirty(&self, id: isize, dirty: bool) {
        self.with_tab(id, |tabs| tabs.set_dirty(id, dirty));
    }

    /// Closes the tab `id` once its content agrees, such as an editor with unsaved changes or a pop-up whose page
    /// closed itself; a panel left without tabs goes, unless it is the window's last.
    pub fn close_tab(&self, id: isize) {
        if crate::editor::may_close(self, id) {
            self.close_tab_now(id);
        }
    }

    /// Closes the tab `id` without asking.
    fn close_tab_now(&self, id: isize) {
        crate::editor::closing(self, id);
        crate::preview::closing(self, id);
        self.with_panes(|panes| {
            if let Some(pane) = panes.of_tab(id) {
                pane.tabs.close(id);
                panes.remove_if_empty(&pane);
            }
        });
    }

    /// Selects the tab `id` and makes its panel the active one; the keyboard focus stays where it is.
    pub(crate) fn select_tab_by_id(&self, id: isize) {
        self.with_panes(|panes| {
            if let Some(pane) = panes.of_tab(id) {
                pane.tabs.select(id);
                panes.set_active(&pane);
            }
        });
    }

    /// Selects the tab `id` as a click on it does: its panel becomes the active one and gets the keyboard focus.
    pub(super) fn click_tab(&self, id: isize) {
        self.with_panes(|panes| {
            if let Some(pane) = panes.of_tab(id) {
                pane.tabs.select(id);
                panes.activate(&pane);
            }
        });
    }

    /// The tab ids, panel by panel, left to right.
    pub fn tab_ids(&self) -> Vec<isize> {
        self.with_panes(|panes| panes.all().iter().flat_map(|pane| pane.tabs.ids()).collect())
            .unwrap_or_default()
    }

    /// Closes every tab without asking, for a window that is about to get the tabs of the last run; the panels close
    /// with them.
    pub(crate) fn clear_tabs(&self) {
        for id in self.tab_ids() {
            self.with_panes(|panes| {
                if let Some(pane) = panes.of_tab(id) {
                    pane.tabs.close(id);
                    panes.remove_if_empty(&pane);
                }
            });
        }
    }

    /// Whether every tab may close: no editor holds unsaved changes the user wants to keep. When one refuses, the
    /// window stays, and editors whose text the user already gave up keep it after all, with their copies back.
    fn tabs_may_close(&self) -> bool {
        let all = self.tab_ids().into_iter().all(|id| crate::editor::may_close(self, id));
        if !all {
            crate::editor::store_copies(self);
        }
        all
    }

    /// What the window shows, for the next launch: its folder, its editor and welcome tabs in every panel and which
    /// tab is in front. The panels and the other tabs are the layout's, which `layout::restore` opens again.
    pub fn record(&self) -> crate::session::WindowRecord {
        use crate::session::TabRecord;
        let kinds: Vec<(isize, TabKind)> = self
            .with_panes(|panes| panes.all().iter().flat_map(|pane| pane.tabs.kinds()).collect())
            .unwrap_or_default();
        let selected = self.selected_tab();
        let mut record = crate::session::WindowRecord {
            project: self.project(),
            ..Default::default()
        };
        for (id, kind) in kinds {
            let tab = match kind {
                TabKind::Welcome => TabRecord::Welcome,
                TabKind::Editor => match crate::editor::tab_document(self, id) {
                    Some((path, recovery)) => TabRecord::Editor {
                        path,
                        recovery: recovery.as_str().to_string(),
                    },
                    None => continue,
                },
                TabKind::Terminal
                | TabKind::Browser
                | TabKind::Diff
                | TabKind::Agent
                | TabKind::Log
                | TabKind::Cli
                | TabKind::Plan
                | TabKind::Preview => {
                    continue;
                }
            };
            if selected == Some(id) {
                record.selected = Some(record.tabs.len());
            }
            record.tabs.push(tab);
        }
        record
    }

    /// Shows `text` at the foot of the active panel's tab area for a few seconds.
    pub fn show_notice(&self, text: &str) {
        let Some(note) = self.with_panes(|panes| notice::show(self.mtm(), panes.active().tabs.content(), text)) else {
            return;
        };
        *self.ivars().notice.borrow_mut() = Some((note, text.to_string()));
    }

    /// The tab in front of the active panel.
    pub fn selected_tab(&self) -> Option<isize> {
        self.with_panes(|panes| panes.active().tabs.selected_id()).flatten()
    }

    /// Follows a tab dragged to `point`, in window coordinates.
    pub(super) fn drag_tab(&self, id: isize, point: NSPoint) {
        self.with_panes(|panes| panes.drag(id, point));
    }

    /// Puts down a tab dragged to `point`: in its own bar, another panel's, or beside a panel.
    pub(super) fn drop_tab(&self, id: isize, point: NSPoint) {
        self.with_panes(|panes| panes.drop(id, point));
    }

    /// Moves the tab `id` into a new panel beside its own.
    pub fn split_tab(&self, id: isize, side: Side) {
        self.with_panes(|panes| panes.split(id, side));
    }

    /// Whether the tab `id` can move into a panel of its own.
    pub(super) fn can_split(&self, id: isize) -> bool {
        self.with_panes(|panes| panes.can_split(id)).unwrap_or(false)
    }

    pub(super) fn close_tab_by_id(&self, id: isize) {
        self.close_tab(id);
    }

    /// Closes every tab of the panel of `id` but `id`, or only those right of it.
    pub(super) fn close_other_tabs(&self, id: isize, right_only: bool) {
        let ids = self
            .with_panes(|panes| panes.of_tab(id).map(|pane| pane.tabs.ids()))
            .flatten()
            .unwrap_or_default();
        let Some(position) = ids.iter().position(|other| *other == id) else {
            return;
        };
        for (index, other) in ids.into_iter().enumerate() {
            if other != id && (!right_only || index > position) {
                self.close_tab(other);
            }
        }
    }

    fn bring_to_front(&self) {
        if let Some(window) = self.window() {
            window.makeKeyAndOrderFront(None);
        }
    }

    /// The folder this window shows.
    pub fn project(&self) -> Option<PathBuf> {
        self.ivars().project.borrow().clone()
    }

    fn set_project(&self, project: Option<PathBuf>) {
        *self.ivars().project.borrow_mut() = project;
        self.update_title();
        self.update_tabbing_identifier();
        if let Some(panels) = self.ivars().panels.borrow().as_ref() {
            panels.set_project(self.project().as_deref());
        }
        if let Some(window) = self.window() {
            let _ = window.setFrameAutosaveName(&ns(&self.frame_name()));
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
        let Some(window) = self.window() else { return };
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

    /// Shows `panel`; clicking the panel that already shows closes or reopens it, as in VS Code.
    pub fn activate_panel(&self, panel: Panel) {
        let ivars = self.ivars();
        if ivars.panel.get() == Some(panel) && ivars.panel_open.get() {
            self.set_panel_open(false);
            return;
        }
        if !ivars.panel_open.get() {
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

    /// Puts the keyboard focus into the panel beside the activity bar while it shows, else back into the active tab.
    fn focus_panel(&self) {
        let Some(window) = self.window() else { return };
        let ivars = self.ivars();
        let page = ivars.panels.borrow().as_ref().and_then(Panels::shown_page);
        let target = page
            .as_deref()
            .filter(|_| ivars.panel_open.get())
            .and_then(panes::first_responder_in);
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
        if in_panel {
            self.with_panes(|panes| panes.activate(&panes.active()));
        }
    }

    /// Opens or closes the panel beside the activity bar, which stays.
    fn set_panel_open(&self, open: bool) {
        let ivars = self.ivars();
        let (Some(sidebar), Some(split)) = (ivars.sidebar.borrow().clone(), ivars.split.borrow().clone()) else {
            return;
        };
        if let Some(panels) = ivars.panels.borrow().as_ref() {
            panels.view().setHidden(!open);
        }
        if open {
            sidebar.setMaximumThickness(ACTIVITY_WIDTH + PANEL_MAX_WIDTH);
            sidebar.setMinimumThickness(ACTIVITY_WIDTH + PANEL_MIN_WIDTH);
            split
                .splitView()
                .setPosition_ofDividerAtIndex(ACTIVITY_WIDTH + ivars.panel_width.get(), 0);
        } else {
            let current = sidebar.viewController(self.mtm()).view().frame().size.width;
            if let Some(width) = Some(current).filter(|w| *w > ACTIVITY_WIDTH + 1.0) {
                ivars.panel_width.set(width - ACTIVITY_WIDTH);
            }
            sidebar.setMinimumThickness(ACTIVITY_WIDTH);
            sidebar.setMaximumThickness(ACTIVITY_WIDTH);
            split.splitView().setPosition_ofDividerAtIndex(ACTIVITY_WIDTH, 0);
        }
        ivars.panel_open.set(open);
        if let Some(activity) = ivars.activity.borrow().as_ref() {
            activity.select(open.then(|| ivars.panel.get()).flatten());
        }
    }

    /// Selects the welcome tab, or adds it to the active panel; its id.
    pub(crate) fn open_welcome(&self) -> isize {
        let open = self.with_panes(|panes| {
            panes
                .all()
                .into_iter()
                .find_map(|pane| pane.tabs.find(TabKind::Welcome))
        });
        match open.flatten() {
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

    fn with_panes<R>(&self, f: impl FnOnce(&Panes) -> R) -> Option<R> {
        self.ivars().panes.borrow().as_ref().map(f)
    }

    /// Runs `f` on the tab bar of the active panel.
    fn with_tabs(&self, f: impl FnOnce(&TabBar)) {
        self.with_panes(|panes| f(&panes.active().tabs));
    }

    /// Runs `f` on the tab bar of the panel that holds the tab `id`.
    fn with_tab(&self, id: isize, f: impl FnOnce(&TabBar)) {
        self.with_panes(|panes| {
            if let Some(pane) = panes.of_tab(id) {
                f(&pane.tabs);
            }
        });
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
                .with_panes(|panes| panes.active().tabs.count() > 1)
                .unwrap_or(false);
        }
        if action == Some(sel!(ideFocusNextPanel:)) {
            return self.with_panes(|panes| panes.all().len() > 1).unwrap_or(false);
        }
        true
    }

    /// The agents "+" ▸ AI-Thread ▸ "In eigenem Worktree" lists.
    #[cfg(feature = "selftest")]
    pub fn worktree_menu_titles(&self) -> Vec<String> {
        self.with_panes(|panes| {
            panes
                .active()
                .tabs
                .new_tab_submenu_titles(&[t("AI-Thread"), t("In eigenem Worktree")])
        })
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
            new_tab_items: self
                .with_panes(|panes| panes.active().tabs.new_tab_titles())
                .unwrap_or_default(),
            sidebar_width: ivars
                .sidebar
                .borrow()
                .as_ref()
                .map_or(0.0, |item| item.viewController(self.mtm()).view().frame().size.width),
        }
    }
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

    pub fn tab_count(&self) -> usize {
        self.with_panes(|panes| panes.all().iter().map(|pane| pane.tabs.count()).sum())
            .unwrap_or(0)
    }

    /// The number the badge on `panel`'s symbol shows; `None` while it is hidden.
    pub fn badge(&self, panel: Panel) -> Option<String> {
        self.ivars().activity.borrow().as_ref().and_then(|a| a.badge(panel))
    }

    /// The view of the tab in front of the active panel.
    pub fn selected_view(&self) -> Option<Retained<NSView>> {
        let mut view = None;
        self.with_tabs(|tabs| view = tabs.selected_view());
        view
    }

    /// The titles of all tabs, panel by panel.
    pub fn tab_titles(&self) -> Vec<String> {
        self.panel_titles().into_iter().flatten().collect()
    }

    /// The titles of each panel's tabs, panels left to right and top to bottom.
    pub fn panel_titles(&self) -> Vec<Vec<String>> {
        self.with_panes(|panes| panes.all().iter().map(|pane| pane.tabs.titles()).collect())
            .unwrap_or_default()
    }

    /// The tab ids of each panel.
    pub fn panel_tab_ids(&self) -> Vec<Vec<isize>> {
        self.with_panes(|panes| panes.all().iter().map(|pane| pane.tabs.ids()).collect())
            .unwrap_or_default()
    }

    /// The menu of new tabs of the active panel.
    pub fn new_tab_menu(&self) -> Option<Retained<objc2_app_kit::NSMenu>> {
        self.with_panes(|panes| panes.active().tabs.new_tab_menu())
    }

    /// Whether the "+" of each panel's tab bar comes after its tabs in the reading direction.
    pub fn plus_follows_tabs(&self) -> Vec<bool> {
        self.with_panes(|panes| panes.all().iter().map(|pane| pane.tabs.plus_follows_tabs()).collect())
            .unwrap_or_default()
    }

    /// Moves the keyboard focus to the next panel, as the menu command does.
    pub fn focus_next_panel(&self) {
        self.with_panes(Panes::focus_next);
    }

    /// Which panel's bar shows itself as the active one.
    pub fn panel_marks(&self) -> Vec<bool> {
        self.with_panes(|panes| panes.all().iter().map(|pane| pane.tabs.is_active()).collect())
            .unwrap_or_default()
    }

    /// What VoiceOver calls each panel's tab bar.
    pub fn panel_labels(&self) -> Vec<String> {
        self.with_panes(|panes| panes.all().iter().map(|pane| pane.tabs.label()).collect())
            .unwrap_or_default()
    }

    /// The place of the active panel among the panels.
    pub fn active_panel(&self) -> Option<usize> {
        self.with_panes(|panes| {
            let active = panes.active();
            panes.all().iter().position(|pane| std::rc::Rc::ptr_eq(pane, &active))
        })
        .flatten()
    }

    /// The title of each tab, left to right, whether it is the preview tab, and what VoiceOver calls it.
    pub fn tab_previews(&self) -> Vec<(String, bool, String)> {
        self.with_panes(|panes| panes.all().iter().flat_map(|pane| pane.tabs.previews()).collect())
            .unwrap_or_default()
    }

    /// Clicks the selected tab `count` times.
    pub fn click_selected_tab(&self, count: isize) {
        if let Some(id) = self.selected_tab() {
            self.with_tab(id, |tabs| tabs.click(id, count));
        }
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
        let scroll = self.file_tree_view()?.enclosingScrollView()?;
        // SAFETY: Asking for the superview only reads; the page holds the scroll view while the window shows.
        let page = unsafe { scroll.superview() }?;
        find(&page, action)
    }

    /// Clicks the row of `path` in the folder tree, twice for a double click.
    pub fn click_in_tree(&self, path: &Path, double: bool) {
        let files = self.ivars().files.borrow();
        if let Some(files) = files.as_ref() {
            files.click(path, double);
        }
    }

    /// Whether each tab, panel by panel and left to right, shows the mark of unsaved changes.
    pub fn tab_dirty_marks(&self) -> Vec<bool> {
        self.with_panes(|panes| panes.all().iter().flat_map(|pane| pane.tabs.dirty_marks()).collect())
            .unwrap_or_default()
    }

    /// The note at the foot of the tab area, while it shows.
    pub fn notice(&self) -> Option<String> {
        let notice = self.ivars().notice.borrow();
        let (note, text) = notice.as_ref()?;
        // SAFETY: Asking for the superview only reads; the note is alive as the ivar holds it.
        unsafe { note.superview() }.map(|_| text.clone())
    }

    /// Drags the selected tab to the far left of its strip.
    pub fn drag_selected_tab_first(&self) {
        if let Some(id) = self.selected_tab() {
            self.with_tab(id, |tabs| tabs.drag(id, 0.0));
        }
    }

    /// The shares of the outermost split of the panels.
    pub fn panel_shares(&self) -> Vec<f64> {
        match self.with_panes(|panes| panes.layout(&|_| None)) {
            Some(ide_core::layout::Node::Split { shares, .. }) => shares,
            _ => vec![1.0],
        }
    }

    /// Drags the tab `id` with the mouse onto the bar of the panel at `index`, or onto the part of its content at
    /// `side`, and lets go there when `drop`.
    pub fn drag_tab_onto(&self, id: isize, index: usize, side: Option<Side>, drop: bool) {
        let point = self
            .with_panes(|panes| {
                let pane = panes.all().get(index).cloned()?;
                let (view, x, y) = match side {
                    None => (pane.tabs.view(), 0.5, 0.5),
                    Some(Side::Right) => (pane.tabs.content(), 0.9, 0.5),
                    Some(Side::Below) => (pane.tabs.content(), 0.5, 0.1),
                };
                let size = view.bounds().size;
                Some(view.convertPoint_toView(NSPoint::new(size.width * x, size.height * y), None))
            })
            .flatten();
        if let Some(point) = point {
            self.drag_tab(id, point);
            if drop {
                self.drop_tab(id, point);
            }
        }
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

fn folder_name(path: &Path) -> String {
    path.file_name().map_or_else(
        || path.display().to_string(),
        |name| name.to_string_lossy().into_owned(),
    )
}

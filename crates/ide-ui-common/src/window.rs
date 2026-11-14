//! A window as the feature crates reach it (E99): a handle by the window's id, whose calls the app answers through
//! the `WindowHost` it installs, so that no crate holds the window controller or the app context.

use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use objc2::MainThreadMarker;
use objc2::rc::Retained;
use objc2_app_kit::{NSColor, NSView, NSWindow};
use splat_i18n::t;

/// What the panel beside the activity bar shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Panel {
    Files,
    Search,
    Agents,
    Git,
    Ci,
    Debug,
    Tests,
    Containers,
}

impl Panel {
    pub const ALL: [Panel; 8] = [
        Panel::Files,
        Panel::Search,
        Panel::Agents,
        Panel::Git,
        Panel::Tests,
        Panel::Debug,
        Panel::Containers,
        Panel::Ci,
    ];

    /// The tag of the menu items and buttons that show it; windows save it, so it stays when the order changes.
    pub fn tag(self) -> isize {
        match self {
            Panel::Files => 1,
            Panel::Search => 2,
            Panel::Agents => 3,
            Panel::Git => 4,
            Panel::Ci => 5,
            Panel::Debug => 6,
            Panel::Tests => 7,
            Panel::Containers => 8,
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
            Panel::Debug => t("Debuggen"),
            Panel::Tests => t("Tests"),
            Panel::Containers => t("Container"),
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
            Panel::Debug => "debug",
            Panel::Tests => "tests",
            Panel::Containers => "containers",
        }
    }

    pub fn symbol(self) -> &'static str {
        match self {
            Panel::Files => "doc.on.doc",
            Panel::Search => "magnifyingglass",
            Panel::Agents => "sparkles",
            Panel::Git => "arrow.triangle.branch",
            Panel::Ci => "checkmark.seal",
            Panel::Debug => "ladybug",
            Panel::Tests => "checklist",
            Panel::Containers => "shippingbox",
        }
    }
}

/// What a document tab of a window shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TabKind {
    Welcome,
    Editor,
    Terminal,
    Browser,
    Agent,
    Diff,
    Log,
    /// A plan an agent asks to approve, as a document to comment on.
    Plan,
    /// A document as it reads, beside its editor: Markdown rendered, or a PDF file.
    Preview,
    /// A debug console (E94).
    Console,
}

impl TabKind {
    pub fn symbol(self) -> &'static str {
        match self {
            TabKind::Welcome => "house",
            TabKind::Editor => "doc.text",
            TabKind::Terminal => "terminal",
            TabKind::Browser => "globe",
            TabKind::Agent => "sparkle",
            TabKind::Diff => "plusminus",
            TabKind::Log => "doc.plaintext",
            TabKind::Plan => "list.bullet.clipboard",
            TabKind::Preview => "eye",
            TabKind::Console => "ladybug",
        }
    }
}

/// What a window is and shows: whether it is open, which one is in front or shows a folder, and its native windows.
pub trait IdentityHost {
    /// Whether the window is open and not closing.
    fn is_open(&self, window: WindowId) -> bool;
    /// The window in front, where menu commands act.
    fn frontmost(&self) -> Option<WindowId>;
    /// The window that shows `root`.
    fn for_project(&self, root: &Path) -> Option<WindowId>;
    fn ns_window(&self, window: WindowId) -> Option<Retained<NSWindow>>;
    /// The folder the window shows.
    fn project_root(&self, window: WindowId) -> Option<PathBuf>;
    /// What the window opened: its folder, or a file it shows alone.
    fn project(&self, window: WindowId) -> Option<PathBuf>;
    /// The native windows of the window's tabs, in their order.
    fn tab_windows(&self, window: WindowId) -> Vec<Retained<NSWindow>>;
}

/// The user's trust in a window's folder (E22).
pub trait TrustHost {
    /// Whether the user trusts the window's folder.
    fn is_trusted(&self, window: WindowId) -> bool;
    /// Asks the user again whether to trust the window's folder.
    fn ask_trust_again(&self, window: WindowId);
}

/// A window's document tabs.
pub trait TabsHost {
    /// Adds `view` as a document tab; its id, or `None` once the window closed.
    fn add_tab(
        &self,
        window: WindowId,
        kind: TabKind,
        key: Option<String>,
        title: &str,
        view: Retained<NSView>,
    ) -> Option<isize>;
    /// Adds `view` as a preview tab, which the next preview replaces unless the user keeps it; a plain tab when the
    /// user turned previews off.
    fn add_preview_tab(
        &self,
        window: WindowId,
        kind: TabKind,
        key: Option<String>,
        title: &str,
        view: Retained<NSView>,
    ) -> Option<isize>;
    /// Turns the preview tab `tab` into one that stays.
    fn keep_tab(&self, window: WindowId, tab: isize);
    /// Brings the tab whose key is `key` to the front; whether there is one.
    fn select_tab_with_key(&self, window: WindowId, key: &str) -> bool;
    fn set_tab_title(&self, window: WindowId, tab: isize, title: &str);
    fn select_tab(&self, window: WindowId, tab: isize);
    fn close_tab(&self, window: WindowId, tab: isize);
    /// The tab in front.
    fn selected_tab(&self, window: WindowId) -> Option<isize>;
    /// The tab whose key is `key`, such as a file's path.
    fn tab_with_key(&self, window: WindowId, key: &str) -> Option<isize>;
}

/// How a document tab's symbol shows the state of what it holds.
pub trait TabDecorHost {
    /// Shows `color` as the status dot of `tab`, or none.
    fn set_tab_status(&self, window: WindowId, tab: isize, color: Option<&NSColor>);
    /// Tints the symbol of `tab`.
    fn set_tab_tint(&self, window: WindowId, tab: isize, color: &NSColor);
    /// Gives the symbol of `tab` the colours of its left, top, right and bottom point instead of a tint.
    fn set_tab_four_colours(&self, window: WindowId, tab: isize, colors: [Retained<NSColor>; 4]);
}

/// The panels beside a window's activity bar.
pub trait PanelsHost {
    /// Shows `panel` beside the activity bar.
    fn activate_panel(&self, window: WindowId, panel: Panel);
    fn set_panel_page(&self, window: WindowId, panel: Panel, page: &NSView);
    /// Shows `count` on `panel`'s symbol in the activity bar; none for 0.
    fn set_symbol_count(&self, window: WindowId, panel: Panel, count: usize);
    /// Puts `view` at the trailing end of `panel`'s title row.
    fn set_panel_accessory(&self, window: WindowId, panel: Panel, view: &NSView);
}

/// The inspector at a window's trailing edge.
pub trait InspectorHost {
    fn set_inspector_content(&self, window: WindowId, view: &NSView);
    fn show_inspector(&self, window: WindowId, shown: bool);
}

/// What a window tells the user beside its tabs: a brief notice, and its running work in the activity view.
pub trait NoticesHost {
    /// Shows `text` briefly over the window and has VoiceOver read it.
    fn show_notice(&self, window: WindowId, text: &str);
    /// The activity view of the window reads its running work again.
    fn refresh_activity(&self, window: WindowId);
}

/// The files a window opens in its editors.
pub trait FilesHost {
    /// Opens `path` in an editor tab, or brings its tab to the front.
    fn open_file(&self, window: WindowId, path: &Path);
}

/// What the app knows beside its windows.
pub trait AppEnvHost {
    /// The user's shortcuts as the menus were built with them.
    fn keymap(&self) -> ide_core::keymap::Keymap;
    /// Whether the app is quitting, when nothing should be written that the next start would read.
    fn is_quitting(&self) -> bool;
    /// The login shell's environment (E23), waiting up to `wait` for it; any thread may ask.
    fn environment(&self, wait: std::time::Duration) -> Option<Vec<(String, String)>>;
}

/// What feature crates ask of a window, answered by the app for the window with the given id.
pub trait WindowHost:
    IdentityHost + TrustHost + TabsHost + TabDecorHost + PanelsHost + InspectorHost + NoticesHost + FilesHost + AppEnvHost
{
}

impl<T> WindowHost for T where
    T: IdentityHost
        + TrustHost
        + TabsHost
        + TabDecorHost
        + PanelsHost
        + InspectorHost
        + NoticesHost
        + FilesHost
        + AppEnvHost
{
}

static HOST: OnceLock<&'static (dyn WindowHost + Sync)> = OnceLock::new();

/// Makes `host` the app the windows belong to; once, before the first window opens.
pub fn install(host: &'static (dyn WindowHost + Sync)) {
    let _ = HOST.set(host);
}

fn host() -> &'static (dyn WindowHost + Sync) {
    *HOST
        .get()
        .expect("the app installs its window host before the first window")
}

/// The login shell's environment (E23), waiting up to `wait` for it; empty when it did not come.
pub fn environment(wait: std::time::Duration) -> Vec<(String, String)> {
    host().environment(wait).unwrap_or_default()
}

/// The login shell's environment (E23), waiting up to `wait` for it; `None` while it is not known.
pub fn try_environment(wait: std::time::Duration) -> Option<Vec<(String, String)>> {
    host().environment(wait)
}

/// Whether the app is quitting.
pub fn is_quitting() -> bool {
    host().is_quitting()
}

/// The user's shortcuts as the menus were built with them.
pub fn keymap() -> ide_core::keymap::Keymap {
    host().keymap()
}

/// The number the app gives a window; never reused while the app runs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct WindowId(u64);

impl WindowId {
    /// The id the app's registry of windows handed out as `number`.
    pub const fn new(number: u64) -> WindowId {
        WindowId(number)
    }

    /// The number under which the app's registry of windows keeps the window.
    pub const fn get(self) -> u64 {
        self.0
    }
}

/// A window by its id, which stays valid as a value after the window closed; its calls then do nothing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Window(WindowId);

impl Window {
    /// The window with the id the app gave it.
    pub fn new(id: WindowId) -> Window {
        Window(id)
    }

    /// The window in front, where menu commands act.
    pub fn frontmost() -> Option<Window> {
        host().frontmost().map(Window)
    }

    /// The window that shows `root`.
    pub fn for_project(root: &Path) -> Option<Window> {
        host().for_project(root).map(Window)
    }

    /// The window if it is still open.
    pub fn open(id: WindowId) -> Option<Window> {
        host().is_open(id).then_some(Window(id))
    }

    pub fn id(self) -> WindowId {
        self.0
    }

    pub fn is_open(self) -> bool {
        host().is_open(self.0)
    }

    /// The window while it is open, as a weak reference loads its object.
    pub fn load(self) -> Option<Window> {
        self.is_open().then_some(self)
    }

    /// The main-thread marker, which every window call needs.
    pub fn mtm(self) -> MainThreadMarker {
        MainThreadMarker::new().expect("windows live on the main thread")
    }

    /// The AppKit window, for sheets and alerts.
    pub fn window(self) -> Option<Retained<NSWindow>> {
        host().ns_window(self.0)
    }

    pub fn project_root(self) -> Option<PathBuf> {
        host().project_root(self.0)
    }

    pub fn project(self) -> Option<PathBuf> {
        host().project(self.0)
    }

    pub fn tab_windows(self) -> Vec<Retained<NSWindow>> {
        host().tab_windows(self.0)
    }
}

/// The user's trust in a window's folder (E22).
pub trait Trust: AsWindow {
    fn is_trusted(&self) -> bool {
        host().is_trusted(self.as_window().0)
    }

    fn ask_trust_again(&self) {
        host().ask_trust_again(self.as_window().0);
    }
}

impl Trust for Window {}

/// A window's document tabs.
pub trait Tabs: AsWindow {
    /// Adds `view` as a document tab; its id, or `None` once the window closed.
    fn add_tab(&self, kind: TabKind, key: Option<String>, title: &str, view: Retained<NSView>) -> Option<isize> {
        host().add_tab(self.as_window().0, kind, key, title, view)
    }

    /// Adds `view` as a preview tab; its id, or `None` once the window closed.
    fn add_preview_tab(
        &self,
        kind: TabKind,
        key: Option<String>,
        title: &str,
        view: Retained<NSView>,
    ) -> Option<isize> {
        host().add_preview_tab(self.as_window().0, kind, key, title, view)
    }

    fn keep_tab(&self, tab: isize) {
        host().keep_tab(self.as_window().0, tab);
    }

    /// Brings the tab whose key is `key` to the front; whether there is one.
    fn select_tab(&self, key: &str) -> bool {
        host().select_tab_with_key(self.as_window().0, key)
    }

    fn set_tab_title(&self, tab: isize, title: &str) {
        host().set_tab_title(self.as_window().0, tab, title);
    }

    fn select_tab_by_id(&self, tab: isize) {
        host().select_tab(self.as_window().0, tab);
    }

    fn close_tab(&self, tab: isize) {
        host().close_tab(self.as_window().0, tab);
    }

    fn selected_tab(&self) -> Option<isize> {
        host().selected_tab(self.as_window().0)
    }

    fn tab_with_key(&self, key: &str) -> Option<isize> {
        host().tab_with_key(self.as_window().0, key)
    }
}

impl Tabs for Window {}

/// How a document tab's symbol shows the state of what it holds.
pub trait TabDecor: AsWindow {
    fn set_tab_status(&self, tab: isize, color: Option<&NSColor>) {
        host().set_tab_status(self.as_window().0, tab, color);
    }

    fn set_tab_tint(&self, tab: isize, color: &NSColor) {
        host().set_tab_tint(self.as_window().0, tab, color);
    }

    fn set_tab_four_colours(&self, tab: isize, colors: [Retained<NSColor>; 4]) {
        host().set_tab_four_colours(self.as_window().0, tab, colors);
    }
}

impl TabDecor for Window {}

/// The panels beside a window's activity bar.
pub trait Panels: AsWindow {
    fn activate_panel(&self, panel: Panel) {
        host().activate_panel(self.as_window().0, panel);
    }

    fn set_panel_page(&self, panel: Panel, page: &NSView) {
        host().set_panel_page(self.as_window().0, panel, page);
    }

    fn set_symbol_count(&self, panel: Panel, count: usize) {
        host().set_symbol_count(self.as_window().0, panel, count);
    }

    fn set_panel_accessory(&self, panel: Panel, view: &NSView) {
        host().set_panel_accessory(self.as_window().0, panel, view);
    }
}

impl Panels for Window {}

/// The inspector at a window's trailing edge.
pub trait Inspector: AsWindow {
    fn set_inspector_content(&self, view: &NSView) {
        host().set_inspector_content(self.as_window().0, view);
    }

    fn show_inspector(&self, shown: bool) {
        host().show_inspector(self.as_window().0, shown);
    }
}

impl Inspector for Window {}

/// What a window tells the user beside its tabs: a brief notice, and its running work in the activity view.
pub trait Notices: AsWindow {
    fn show_notice(&self, text: &str) {
        host().show_notice(self.as_window().0, text);
    }

    fn refresh_activity(&self) {
        host().refresh_activity(self.as_window().0);
    }
}

impl Notices for Window {}

/// The files a window opens in its editors.
pub trait Files: AsWindow {
    /// Opens `path` in an editor tab, or brings its tab to the front.
    fn open_file(&self, path: &Path) {
        host().open_file(self.as_window().0, path);
    }
}

impl Files for Window {}

/// What names a window: a handle, or the app's window controller.
pub trait AsWindow {
    fn as_window(&self) -> Window;
}

impl AsWindow for Window {
    fn as_window(&self) -> Window {
        *self
    }
}

impl<T: AsWindow + ?Sized> AsWindow for Retained<T> {
    fn as_window(&self) -> Window {
        (**self).as_window()
    }
}

//! The app context (E97): the open objects, the work that runs and the app's state, which the app delegate creates and
//! hands to the window controllers by `Rc`. docs/thread-locals.md lists what stays outside it and why.

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;

use ide_agent::{Keychain, Memory, SecretStore};

use objc2::MainThreadMarker;
use objc2::rc::{Retained, Weak};
use objc2::runtime::AnyObject;
use objc2_app_kit::NSApplication;

use crate::app::AppDelegate;
use crate::editor::EditorTextView;
use crate::window::WindowController;

/// The app's open objects, running work and state.
#[derive(Default)]
pub struct AppContext {
    windows: Registry<Retained<WindowController>>,
    session: crate::session::Saving,
    navigation: crate::navigation::Navigation,
    new_tab_menu: crate::window::NewTabMenu,
    agent_choices: crate::window::AgentChoicesCache,
    activities: crate::activity::Activities,
    settings: crate::settings::SettingsState,
    /// The user's shortcuts as the menus were built with them.
    keymap: RefCell<ide_core::keymap::Keymap>,
    /// The editors of every window.
    editors: RefCell<Vec<Weak<EditorTextView>>>,
    editor: crate::editor::EditorState,
    predictions: crate::editor::predict::Predictions,
    previews: crate::preview::Previews,
    site_previews: crate::site_preview::SitePreviews,
    settings_sync: crate::settings_sync::SettingsSync,
    tests_panels: Registry<Retained<crate::tests_panel::TestsPanel>>,
    containers_panels: Registry<Retained<crate::containers_panel::ContainersPanel>>,
    exports: Rc<ide_ui_export::State>,
    search_panels: crate::window::SearchPanels,
    hierarchy_panels: crate::window::HierarchyPanels,
    terminal_views: Rc<ide_ui_terminal::State>,
    lsp: Rc<ide_ui_lsp::LspState>,
    agent: Rc<ide_ui_agent::State>,
    git: Rc<ide_ui_git::State>,
    ci_panels: crate::ci_panel::CiPanels,
    ci_log_tabs: crate::ci_panel::LogTabs,
    debug: Rc<ide_ui_debug::State>,
    debug_rotors: crate::debug::Rotors,
    browser: Rc<ide_ui_browser::State>,
    secrets: Secrets,
    #[cfg(feature = "dictation")]
    dictation: Rc<ide_ui_dictation::State>,
}

impl AppContext {
    /// The context of the running app, for code that reaches the main thread without one at hand, such as an answer
    /// from a worker thread, which carries only ids (Review C11).
    pub fn get(mtm: MainThreadMarker) -> Rc<AppContext> {
        Self::of_delegate(mtm).expect("the app delegate exists before anything asks for the app context")
    }

    fn of_delegate(mtm: MainThreadMarker) -> Option<Rc<AppContext>> {
        let delegate = NSApplication::sharedApplication(mtm).delegate()?;
        let object: &AnyObject = delegate.as_ref();
        Some(object.downcast_ref::<AppDelegate>()?.context())
    }
    pub fn windows(&self) -> &Registry<Retained<WindowController>> {
        &self.windows
    }

    pub fn session(&self) -> &crate::session::Saving {
        &self.session
    }

    pub fn navigation(&self) -> &crate::navigation::Navigation {
        &self.navigation
    }

    pub fn new_tab_menu(&self) -> &crate::window::NewTabMenu {
        &self.new_tab_menu
    }

    pub fn agent_choices(&self) -> &crate::window::AgentChoicesCache {
        &self.agent_choices
    }

    pub fn activities(&self) -> &crate::activity::Activities {
        &self.activities
    }

    pub fn settings(&self) -> &crate::settings::SettingsState {
        &self.settings
    }

    pub fn keymap(&self) -> &RefCell<ide_core::keymap::Keymap> {
        &self.keymap
    }

    pub fn editors(&self) -> &RefCell<Vec<Weak<EditorTextView>>> {
        &self.editors
    }

    pub fn editor(&self) -> &crate::editor::EditorState {
        &self.editor
    }

    pub fn predictions(&self) -> &crate::editor::predict::Predictions {
        &self.predictions
    }

    pub fn previews(&self) -> &crate::preview::Previews {
        &self.previews
    }

    pub fn site_previews(&self) -> &crate::site_preview::SitePreviews {
        &self.site_previews
    }

    pub fn settings_sync(&self) -> &crate::settings_sync::SettingsSync {
        &self.settings_sync
    }

    pub fn tests_panels(&self) -> &Registry<Retained<crate::tests_panel::TestsPanel>> {
        &self.tests_panels
    }

    pub fn containers_panels(&self) -> &Registry<Retained<crate::containers_panel::ContainersPanel>> {
        &self.containers_panels
    }

    pub fn exports(&self) -> &Rc<ide_ui_export::State> {
        &self.exports
    }

    pub fn search_panels(&self) -> &crate::window::SearchPanels {
        &self.search_panels
    }

    pub fn hierarchy_panels(&self) -> &crate::window::HierarchyPanels {
        &self.hierarchy_panels
    }

    pub fn terminal_views(&self) -> &Rc<ide_ui_terminal::State> {
        &self.terminal_views
    }

    pub fn lsp(&self) -> &Rc<ide_ui_lsp::LspState> {
        &self.lsp
    }

    pub fn agent(&self) -> &Rc<ide_ui_agent::State> {
        &self.agent
    }

    pub fn git(&self) -> &Rc<ide_ui_git::State> {
        &self.git
    }

    pub fn ci_panels(&self) -> &crate::ci_panel::CiPanels {
        &self.ci_panels
    }

    pub fn ci_log_tabs(&self) -> &crate::ci_panel::LogTabs {
        &self.ci_log_tabs
    }

    pub fn debug(&self) -> &Rc<ide_ui_debug::State> {
        &self.debug
    }

    pub fn debug_rotors(&self) -> &crate::debug::Rotors {
        &self.debug_rotors
    }

    pub fn browser(&self) -> &Rc<ide_ui_browser::State> {
        &self.browser
    }

    pub fn secrets(&self) -> &Secrets {
        &self.secrets
    }

    #[cfg(feature = "dictation")]
    pub fn dictation(&self) -> &Rc<ide_ui_dictation::State> {
        &self.dictation
    }
}

/// The one store of the app's secrets — CI tokens and the own agent's keys — which they share.
pub struct Secrets(Arc<dyn SecretStore>);

impl Default for Secrets {
    fn default() -> Self {
        if cfg!(feature = "selftest") && std::env::var_os("IDE_SELFTEST").is_some() {
            return Secrets(Arc::new(Memory::default()));
        }
        let service = objc2_foundation::NSBundle::mainBundle()
            .bundleIdentifier()
            .map_or_else(|| ide_core::paths::identity().to_string(), |id| id.to_string());
        Secrets(Arc::new(Keychain::new(&service)))
    }
}

impl Secrets {
    pub fn store(&self) -> Arc<dyn SecretStore> {
        self.0.clone()
    }
}

/// `AppContext::get` from code that runs on the main thread but holds no marker for it.
pub fn app() -> Rc<AppContext> {
    AppContext::get(MainThreadMarker::new().expect("the app context lives on the main thread"))
}

/// The app context unless the app has none (yet), for code such as `Drop` that must not fail.
pub fn try_app() -> Option<Rc<AppContext>> {
    AppContext::of_delegate(MainThreadMarker::new()?)
}

pub use ide_ui_common::registry::{Registry, Serial, WindowPart};

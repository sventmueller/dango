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
    pub windows: Registry<Retained<WindowController>>,
    pub session: crate::session::Saving,
    pub navigation: crate::navigation::Navigation,
    pub new_tab_menu: crate::window::NewTabMenu,
    pub activities: crate::activity::Activities,
    pub settings: crate::settings::SettingsState,
    /// The user's shortcuts as the menus were built with them.
    pub keymap: RefCell<ide_core::keymap::Keymap>,
    /// The editors of every window.
    pub editors: RefCell<Vec<Weak<EditorTextView>>>,
    pub editor: crate::editor::EditorState,
    pub predictions: crate::editor::predict::Predictions,
    pub previews: crate::preview::Previews,
    pub site_previews: crate::site_preview::SitePreviews,
    pub settings_sync: crate::settings_sync::SettingsSync,
    pub tests_panels: Registry<Retained<crate::tests_panel::TestsPanel>>,
    pub containers_panels: Registry<Retained<crate::containers_panel::ContainersPanel>>,
    pub exports: Rc<ide_ui_export::State>,
    pub search_panels: crate::window::SearchPanels,
    pub hierarchy_panels: crate::window::HierarchyPanels,
    pub terminal_views: Rc<ide_ui_terminal::State>,
    pub lsp: Rc<ide_ui_lsp::LspState>,
    pub agent: Rc<ide_ui_agent::State>,
    pub git: Rc<ide_ui_git::State>,
    pub ci_panels: crate::ci_panel::CiPanels,
    pub ci_log_tabs: crate::ci_panel::LogTabs,
    pub debug: Rc<ide_ui_debug::State>,
    pub debug_rotors: crate::debug::Rotors,
    pub browser: Rc<ide_ui_browser::State>,
    pub secrets: Secrets,
    #[cfg(feature = "dictation")]
    pub dictation: Rc<ide_ui_dictation::State>,
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

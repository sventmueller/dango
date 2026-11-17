use std::path::PathBuf;
use std::rc::Rc;
use std::time::Duration;

use ide_core::settings::{State, remembered};
use ide_process::Supervisor;
use objc2::rc::Retained;
use objc2::runtime::{AnyObject, NSObject};
use objc2::{DefinedClass, MainThreadMarker, MainThreadOnly, define_class, msg_send, sel};
use objc2_app_kit::{
    NSApplication, NSApplicationDelegate, NSApplicationTerminateReply, NSMenu, NSMenuDelegate, NSMenuItem,
    NSModalResponseOK, NSOpenPanel,
};
use objc2_foundation::{NSArray, NSBundle, NSNotification, NSObjectProtocol, NSString, NSURL};

use crate::context::AppContext;
use crate::util::{guarded, item, main_soon, ns, separator, set_target};
use crate::window::{self, WindowController};
use splat_i18n::t;

/// How long the processes the app started get to end, per step, when it quits.
const QUIT_GRACE: Duration = Duration::from_millis(700);

/// How long a quit waits for the copies of unsaved text to be removed.
const RECOVERY_GRACE: Duration = Duration::from_secs(2);

/// The interface texts in the language macOS picks for the app, as a `.strings` file; empty outside a bundle.
pub(crate) fn localized_strings() -> String {
    NSBundle::mainBundle()
        .pathForResource_ofType(Some(&ns("Localizable")), Some(&ns("strings")))
        .and_then(|p| std::fs::read_to_string(p.to_string()).ok())
        .unwrap_or_default()
}

define_class!(
    // SAFETY: NSObject has no subclassing requirements and `AppDelegate` does not implement Drop.
    #[unsafe(super(NSObject))]
    #[thread_kind = MainThreadOnly]
    #[name = "IDEAppDelegate"]
    #[ivars = Rc<AppContext>]
    pub(crate) struct AppDelegate;

    unsafe impl NSObjectProtocol for AppDelegate {}

    unsafe impl NSApplicationDelegate for AppDelegate {
        #[unsafe(method(applicationDidFinishLaunching:))]
        fn did_finish_launching(&self, _notification: &NSNotification) {
            guarded((), || {
                let mtm = self.mtm();
                let leitstand = self.context().leitstand().back();
                ide_core::settings::on_change(move |_, names| {
                    leitstand.settings_changed(names);
                    let names = names.to_vec();
                    crate::util::main_soon(move || {
                        crate::editor::settings_changed(&names);
                        ide_ui_agent::settings_changed(&names);
                        if let Some(mtm) = MainThreadMarker::new() {
                            crate::settings_sync::request(mtm);
                        }
                    });
                });
                crate::settings_sync::launched(mtm);
                ide_ui_agent::set_text_size(
                    ide_core::settings::Settings::load_user().get(&ide_ui_agent::CHAT_FONT_SIZE),
                );
                open_initial_windows(mtm);
                #[cfg(feature = "dictation")]
                ide_ui_dictation::launched();
                #[cfg(feature = "selftest")]
                crate::selftest::after_launch(mtm);
            });
        }

        #[unsafe(method(applicationShouldTerminate:))]
        fn should_terminate(&self, _app: &NSApplication) -> NSApplicationTerminateReply {
            guarded(NSApplicationTerminateReply::TerminateNow, || {
                let cx = self.context();
                if !crate::quit::agents_may_stop(&cx, self.mtm()) || !crate::editor::confirm_quit(self.mtm()) {
                    return NSApplicationTerminateReply::TerminateCancel;
                }
                remember_open_projects(&cx);
                crate::editor::recovery::quit(RECOVERY_GRACE);
                // Running orders pause with "app ended" before the supervisor ends their builders and gates (E167).
                let orders = cx.leitstand().quit(&cx);
                if Supervisor::global().running().is_empty() {
                    orders.wait();
                    return NSApplicationTerminateReply::TerminateNow;
                }
                let reply = || {
                    main_soon(|| {
                        if let Some(mtm) = MainThreadMarker::new() {
                            NSApplication::sharedApplication(mtm).replyToApplicationShouldTerminate(true);
                        }
                    });
                };
                let stopping = ide_ui_common::util::spawn_with("ide-quit", orders, move |orders| {
                    orders.wait();
                    // Launched programs end and attached ones are let go before the supervisor ends the adapters; the
                    // app quits even if that fails, since macOS waits for the reply.
                    ide_core::lane::contained(|| {
                        ide_dap::stop_all(QUIT_GRACE);
                        Supervisor::global().shutdown(QUIT_GRACE);
                    });
                    reply();
                });
                if let Err((orders, _)) = stopping {
                    orders.wait();
                    ide_core::lane::contained(|| {
                        ide_dap::stop_all(QUIT_GRACE);
                        Supervisor::global().shutdown(QUIT_GRACE);
                    });
                    return NSApplicationTerminateReply::TerminateNow;
                }
                NSApplicationTerminateReply::TerminateLater
            })
        }

        #[unsafe(method(applicationShouldHandleReopen:hasVisibleWindows:))]
        fn should_reopen(&self, _app: &NSApplication, visible: bool) -> bool {
            guarded(true, || {
                if !visible {
                    WindowController::open(self.mtm(), None);
                }
                true
            })
        }

        #[unsafe(method(application:openURLs:))]
        fn open_urls(&self, _app: &NSApplication, urls: &NSArray<NSURL>) {
            guarded((), || {
                // The first folder takes the place of the one in front, the others get windows of their own.
                let mut target = window::frontmost(self.mtm());
                for url in urls.iter() {
                    if let Some(path) = url.path() {
                        let folder = PathBuf::from(path.to_string());
                        let opens = folder.is_dir();
                        open_project_from(self.mtm(), folder, if opens { target.take() } else { None });
                    }
                }
            });
        }

        #[unsafe(method(applicationSupportsSecureRestorableState:))]
        fn supports_secure_restorable_state(&self, _app: &NSApplication) -> bool {
            true
        }
    }

    unsafe impl NSMenuDelegate for AppDelegate {
        #[unsafe(method(menuNeedsUpdate:))]
        fn menu_needs_update(&self, menu: &NSMenu) {
            guarded((), || fill_recent_menu(self, menu));
        }
    }

    impl AppDelegate {
        #[unsafe(method(ideShowAbout:))]
        fn show_about(&self, _sender: Option<&AnyObject>) {
            guarded((), || crate::about::show(self.mtm()));
        }

        #[unsafe(method(ideOpenFolder:))]
        fn open_folder(&self, _sender: Option<&AnyObject>) {
            guarded((), || ask_for_folder(self.mtm()));
        }

        #[unsafe(method(ideNewWindow:))]
        fn new_window(&self, _sender: Option<&AnyObject>) {
            guarded((), || {
                WindowController::open(self.mtm(), None);
            });
        }

        #[unsafe(method(ideOpenRecent:))]
        fn open_recent(&self, sender: Option<&AnyObject>) {
            guarded((), || {
                let Some(item) = sender.and_then(|s| s.downcast_ref::<NSMenuItem>()) else { return };
                let Some(path) = item.representedObject().and_then(|o| o.downcast::<NSString>().ok()) else {
                    return;
                };
                open_project(self.mtm(), PathBuf::from(path.to_string()));
            });
        }

        #[unsafe(method(ideClearRecent:))]
        fn clear_recent(&self, _sender: Option<&AnyObject>) {
            guarded((), || {
                let _ = State::load_user().set(&remembered::RECENT_PROJECTS, &Vec::<PathBuf>::new());
            });
        }

        #[unsafe(method(ideInstallCommand:))]
        fn install_command(&self, _sender: Option<&AnyObject>) {
            guarded((), || crate::command_line::install(self.mtm()));
        }

        #[unsafe(method(ideShowSettings:))]
        fn show_settings(&self, _sender: Option<&AnyObject>) {
            guarded((), || crate::settings::show(self.mtm()));
        }
    }
);

impl AppDelegate {
    pub(crate) fn new(mtm: MainThreadMarker) -> Retained<Self> {
        let context = Rc::new(AppContext::default());
        context.leitstand().bind(&context, mtm);
        context.agent().install_sessions(context.leitstand().tab_sessions());
        crate::leitstand_ui::Orders::install(context.leitstand().back());
        let views = context.factory_ui().clone();
        context
            .leitstand()
            .listen(move |root| ide_ui_factory::changed(&views, root));
        ide_ui_agent::own_agent::use_secrets(context.secrets().store());
        crate::ci_panel::use_secrets(context.secrets().store());
        let this = Self::alloc(mtm).set_ivars(context);
        // SAFETY: NSObject's initializer, called once on the newly allocated object.
        unsafe { msg_send![super(this), init] }
    }

    pub(crate) fn context(&self) -> Rc<AppContext> {
        self.ivars().clone()
    }
}

/// The windows of the last run with their tabs, if the user wants them back, and what a crash left unsaved; else one
/// empty window.
fn open_initial_windows(mtm: MainThreadMarker) {
    if crate::session::restore(mtm) == 0 {
        WindowController::open(mtm, None);
    }
}

fn remember_open_projects(cx: &crate::context::AppContext) {
    let open: Vec<PathBuf> = window::open_projects();
    let _ = State::load_user().set(&remembered::OPEN_PROJECTS, &open);
    crate::session::remember(cx);
    crate::session::quitting();
    cx.session().wait_for_writes();
    ide_ui_agent::agent_panel::wait_for_writes(std::time::Duration::from_secs(5));
}

/// Shows `folder` in its window if one has it open, else in place of the folder of the window in front (E56), else in
/// an empty window or a new one.
pub(crate) fn open_project(mtm: MainThreadMarker, folder: PathBuf) {
    open_project_from(mtm, folder, window::frontmost(mtm));
}

/// `open_project` with `target` in place of the window in front, such as the window whose welcome tab was clicked.
pub(crate) fn open_project_from(mtm: MainThreadMarker, folder: PathBuf, target: Option<Retained<WindowController>>) {
    if !folder.is_dir() {
        return;
    }
    let folder = folder.canonicalize().unwrap_or(folder);
    let _ = State::load_user().note_recent(&folder);
    let shown = window::open_projects().contains(&folder);
    match target {
        Some(target) if !shown && target.project().is_some() => target.open_in_place(folder),
        _ => {
            WindowController::open(mtm, Some(folder));
        }
    }
}

fn ask_for_folder(mtm: MainThreadMarker) {
    let panel = NSOpenPanel::openPanel(mtm);
    panel.setCanChooseDirectories(true);
    panel.setCanChooseFiles(false);
    panel.setAllowsMultipleSelection(false);
    panel.setPrompt(Some(&ns(t("Öffnen"))));
    if panel.runModal() != NSModalResponseOK {
        return;
    }
    if let Some(path) = panel.URL().and_then(|url| url.path()) {
        open_project(mtm, PathBuf::from(path.to_string()));
    }
}

/// The recent folders, latest first, then the item that clears them.
fn fill_recent_menu(delegate: &AppDelegate, menu: &NSMenu) {
    let mtm = delegate.mtm();
    menu.removeAllItems();
    let recent = State::load_user().get(&remembered::RECENT_PROJECTS);
    for folder in recent.iter().filter(|p| p.is_dir()) {
        let entry = item(mtm, &display_path(folder), Some(sel!(ideOpenRecent:)), "");
        // SAFETY: A menu item can represent an object of any type.
        unsafe { entry.setRepresentedObject(Some(&ns(&folder.to_string_lossy()))) };
        set_target(&entry, delegate);
        menu.addItem(&entry);
    }
    if menu.numberOfItems() > 0 {
        menu.addItem(&separator(mtm));
    }
    let clear = item(mtm, t("Liste löschen"), Some(sel!(ideClearRecent:)), "");
    clear.setEnabled(!recent.is_empty());
    set_target(&clear, delegate);
    menu.addItem(&clear);
}

pub(crate) use ide_ui_common::panel::display_path;

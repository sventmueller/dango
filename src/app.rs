use std::cell::{Cell, RefCell};
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::mpsc;
use std::time::{Duration, Instant};

use ide_core::settings::{State, remembered};
use ide_process::Supervisor;
use objc2::rc::Retained;
use objc2::runtime::{AnyObject, NSObject};
use objc2::{DefinedClass, MainThreadMarker, MainThreadOnly, define_class, msg_send, sel};
use objc2_app_kit::{
    NSApplication, NSApplicationDelegate, NSApplicationTerminateReply, NSMenu, NSMenuDelegate, NSMenuItem,
    NSMenuItemValidation, NSModalResponseOK, NSOpenPanel,
};
use objc2_foundation::{NSArray, NSBundle, NSNotification, NSObjectProtocol, NSString, NSURL};

use crate::context::AppContext;
use crate::editor::Answer;
use crate::feedback::Page;
use crate::util::{guarded, item, main_soon, ns, separator, set_target};
use crate::window::{self, WindowController};
use splat_i18n::t;

/// How long the processes the app started get to end, per step, when it quits.
const QUIT_GRACE: Duration = Duration::from_millis(700);

/// How long a quit waits for the copies of unsaved text to be removed.
const RECOVERY_GRACE: Duration = Duration::from_secs(2);

/// How long a quit waits for what the app wrote down to reach the disk; it ends without it after that.
const WRITE_LIMIT: Duration = Duration::from_secs(5);

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
                    let back = leitstand.clone();
                    crate::util::main_soon(move || {
                        back.with(|cx| crate::terminal::settings_changed(cx, &names));
                        crate::editor::settings_changed(&names);
                        ide_ui_common::ui_size::settings_changed(&names);
                        ide_ui_agent::settings_changed(&names);
                        if let Some(mtm) = MainThreadMarker::new() {
                            crate::settings_sync::request(mtm);
                        }
                    });
                });
                let cx = self.context();
                crate::settings_sync::launched(&cx, mtm);
                let settings = ide_core::settings::Settings::load_user();
                ide_ui_agent::set_text_size(settings.get(&ide_ui_agent::CHAT_FONT_SIZE));
                ide_ui_common::ui_size::set(settings.get(&ide_core::settings::keys::INTERFACE_FONT_SIZE));
                open_initial_windows(&cx, mtm);
                cx.early_opens().launched.set(true);
                let early = std::mem::take(&mut *cx.early_opens().paths.borrow_mut());
                open_paths(&cx, mtm, early);
                // Loading Sparkle takes longer than the first frame may wait.
                crate::util::after_frame(move || crate::updates::start(mtm));
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
                if !crate::quit::agents_may_stop(&cx, self.mtm()) {
                    return NSApplicationTerminateReply::TerminateCancel;
                }
                match crate::editor::confirm_quit(&cx, self.mtm(), &quit_again(&cx)) {
                    Answer::Yes => quit(&cx),
                    Answer::No => NSApplicationTerminateReply::TerminateCancel,
                    Answer::Later => NSApplicationTerminateReply::TerminateLater,
                }
            })
        }

        #[unsafe(method(applicationShouldHandleReopen:hasVisibleWindows:))]
        fn should_reopen(&self, _app: &NSApplication, visible: bool) -> bool {
            guarded(true, || {
                if !visible {
                    WindowController::open(&self.context(), self.mtm(), None);
                }
                true
            })
        }

        #[unsafe(method(application:openURLs:))]
        fn open_urls(&self, _app: &NSApplication, urls: &NSArray<NSURL>) {
            guarded((), || {
                let paths: Vec<PathBuf> = urls
                    .iter()
                    .filter_map(|url| url.path())
                    .map(|path| PathBuf::from(path.to_string()))
                    .collect();
                let cx = self.context();
                // macOS hands over what a launch opens before the launch is done; the session comes back first.
                if !cx.early_opens().launched.get() {
                    cx.early_opens().paths.borrow_mut().extend(paths);
                    return;
                }
                open_paths(&cx, self.mtm(), paths);
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
            guarded((), || {
                let terminal = crate::terminal::show_shortcuts(&self.context(), menu, true);
                if !crate::menu::hide_tab_bar_toggle(menu) && !terminal {
                    fill_recent_menu(self, menu);
                }
            });
        }

        #[unsafe(method(menuDidClose:))]
        fn menu_did_close(&self, menu: &NSMenu) {
            guarded((), || {
                crate::terminal::show_shortcuts(&self.context(), menu, false);
            });
        }
    }

    unsafe impl NSMenuItemValidation for AppDelegate {
        #[unsafe(method(validateMenuItem:))]
        fn validate_menu_item(&self, item: &NSMenuItem) -> bool {
            guarded(true, || {
                crate::terminal::tick_secure_entry(item);
                true
            })
        }
    }

    impl AppDelegate {
        #[unsafe(method(ideSecureKeyboardEntry:))]
        fn secure_keyboard_entry(&self, _sender: Option<&AnyObject>) {
            guarded((), crate::terminal::toggle_secure_entry);
        }

        #[unsafe(method(ideShowAbout:))]
        fn show_about(&self, _sender: Option<&AnyObject>) {
            guarded((), || crate::about::show(self.mtm()));
        }

        #[unsafe(method(ideReportProblem:))]
        fn report_problem(&self, _sender: Option<&AnyObject>) {
            guarded((), || crate::feedback::open(&self.context(), Page::Problem));
        }

        #[unsafe(method(ideSuggestFeature:))]
        fn suggest_feature(&self, _sender: Option<&AnyObject>) {
            guarded((), || crate::feedback::open(&self.context(), Page::Idea));
        }

        #[unsafe(method(ideOpenRepository:))]
        fn open_repository(&self, _sender: Option<&AnyObject>) {
            guarded((), || crate::feedback::open(&self.context(), Page::Repository));
        }

        #[unsafe(method(ideOpenFolder:))]
        fn open_folder(&self, _sender: Option<&AnyObject>) {
            guarded((), || ask_for_folder(&self.context(), self.mtm()));
        }

        #[unsafe(method(ideNewWindow:))]
        fn new_window(&self, _sender: Option<&AnyObject>) {
            guarded((), || {
                WindowController::open(&self.context(), self.mtm(), None);
            });
        }

        #[unsafe(method(ideOpenRecent:))]
        fn open_recent(&self, sender: Option<&AnyObject>) {
            guarded((), || {
                let Some(item) = sender.and_then(|s| s.downcast_ref::<NSMenuItem>()) else { return };
                let Some(path) = item.representedObject().and_then(|o| o.downcast::<NSString>().ok()) else {
                    return;
                };
                open_project(&self.context(), self.mtm(), PathBuf::from(path.to_string()));
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
            guarded((), || crate::settings::show(&self.context(), self.mtm()));
        }
    }
);

impl AppDelegate {
    pub(crate) fn new(mtm: MainThreadMarker) -> Retained<Self> {
        let context = Rc::new(AppContext::default());
        context.leitstand().bind(&context, mtm);
        context.agent().install_sessions(context.leitstand().tab_sessions());
        crate::leitstand_ui::Orders::install(context.leitstand().back());
        crate::host::TerminalHost::install(context.leitstand().back());
        crate::terminal::keys::watch(&context);
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
fn open_initial_windows(cx: &Rc<AppContext>, mtm: MainThreadMarker) {
    if crate::session::restore(cx, mtm) == 0 {
        WindowController::open(cx, mtm, None);
    }
}

/// Ends the app once what runs has stopped and the writes are done, on a thread of its own when one starts.
fn quit(cx: &Rc<AppContext>) -> NSApplicationTerminateReply {
    let quit = Quit::begin(cx);
    let waiting = ide_ui_common::util::spawn_with("ide-quit", quit, |quit| {
        quit.finish();
        main_soon(|| reply_to_quit(true));
    });
    match waiting {
        Ok(()) => NSApplicationTerminateReply::TerminateLater,
        Err((quit, _)) => {
            quit.finish();
            NSApplicationTerminateReply::TerminateNow
        }
    }
}

/// What a quit that answered `TerminateLater` for the saves the user asked for does once they are written: it asks
/// about what is still unsaved and ends the app, or stays when a save failed or the user cancels.
fn quit_again(cx: &Rc<AppContext>) -> crate::editor::Again {
    let cx = Rc::clone(cx);
    Rc::new(move |written| {
        let Some(mtm) = MainThreadMarker::new() else { return };
        if !written {
            crate::editor::keep_copies(&cx);
            return reply_to_quit(false);
        }
        match crate::editor::confirm_quit(&cx, mtm, &quit_again(&cx)) {
            Answer::Yes => {
                if quit(&cx) == NSApplicationTerminateReply::TerminateNow {
                    reply_to_quit(true);
                }
            }
            Answer::No => reply_to_quit(false),
            Answer::Later => {}
        }
    })
}

fn reply_to_quit(quits: bool) {
    if let Some(mtm) = MainThreadMarker::new() {
        NSApplication::sharedApplication(mtm).replyToApplicationShouldTerminate(quits);
    }
}

/// What a quit waits for before the app ends, on a thread of its own, so the main thread keeps drawing meanwhile.
struct Quit {
    copies: crate::editor::recovery::QuitCopies,
    orders: Box<dyn FnOnce() + Send>,
    /// Answers once the disk lane reached every write queued before the quit; `None` if the lane took no job.
    written: Option<mpsc::Receiver<()>>,
    /// Whether processes the app started still run.
    stops: bool,
}

impl Quit {
    /// Writes down what the next launch puts back and asks everything that runs to end.
    fn begin(cx: &Rc<AppContext>) -> Quit {
        remember_open_projects(cx);
        let copies = crate::editor::recovery::quit(cx);
        // Running orders pause with "app ended" before the supervisor ends their builders and gates (E167).
        let orders = cx.leitstand().quit(cx);
        let (done, written) = mpsc::channel();
        let queued = ide_core::lane::DISK.try_run(move || {
            let _ = done.send(());
        });
        Quit {
            copies,
            orders: Box::new(move || orders.wait()),
            written: queued.is_ok().then_some(written),
            stops: Supervisor::global().has_managed_processes(),
        }
    }

    /// Waits for the copies, the orders, the processes and the writes, each within its bound.
    fn finish(self) {
        let until = Instant::now() + WRITE_LIMIT;
        self.copies.finish(RECOVERY_GRACE);
        (self.orders)();
        if self.stops {
            // Launched programs end and attached ones are let go before the supervisor ends the adapters; the app
            // quits even if that fails, since macOS waits for the reply.
            ide_core::lane::contained(|| {
                ide_dap::stop_all(QUIT_GRACE);
                Supervisor::global().shutdown(QUIT_GRACE);
            });
        }
        if let Some(written) = self.written {
            let _ = written.recv_timeout(until.saturating_duration_since(Instant::now()));
        }
        while !ide_core::settings::all_written() && Instant::now() < until {
            std::thread::sleep(Duration::from_millis(5));
        }
    }
}

fn remember_open_projects(cx: &AppContext) {
    let open: Vec<PathBuf> = window::open_projects(cx);
    let _ = State::load_user().set(&remembered::OPEN_PROJECTS, &open);
    crate::session::remember(cx);
    crate::session::quitting(cx);
    // Hands the changes the agent panel holds back to the disk lane, which the quit's thread waits for.
    ide_ui_agent::agent_panel::wait_for_writes(Duration::ZERO);
}

/// Shows `folder` in its window if one has it open, else in place of the folder of the window in front (E56), else in
/// an empty window or a new one.
pub(crate) fn open_project(cx: &Rc<AppContext>, mtm: MainThreadMarker, folder: PathBuf) {
    open_project_from(cx, mtm, folder, window::frontmost(mtm));
}

/// `open_project` with `target` in place of the window in front, such as the window whose welcome tab was clicked.
pub(crate) fn open_project_from(
    cx: &Rc<AppContext>,
    mtm: MainThreadMarker,
    folder: PathBuf,
    target: Option<Retained<WindowController>>,
) {
    if !folder.is_dir() {
        return;
    }
    let folder = folder.canonicalize().unwrap_or(folder);
    let _ = State::load_user().note_recent(&folder);
    let shown = window::open_projects(cx).contains(&folder);
    match target {
        Some(target) if !shown && target.project().is_some() => target.open_in_place(folder),
        _ => {
            WindowController::open(cx, mtm, Some(folder));
        }
    }
}

/// What macOS handed over to open before the launch was done, opened once the session is back.
#[derive(Default)]
pub(crate) struct EarlyOpens {
    launched: Cell<bool>,
    paths: RefCell<Vec<PathBuf>>,
}

/// Opens `paths` handed to the app: a file where its folder shows or in a window of its own, the first folder in place
/// of the one in front and the others in windows of their own.
fn open_paths(cx: &Rc<AppContext>, mtm: MainThreadMarker, paths: Vec<PathBuf>) {
    let mut target = window::frontmost(mtm);
    for path in paths {
        if path.is_file() {
            open_file_from(cx, mtm, &path);
        } else {
            open_project_from(cx, mtm, path, target.take());
        }
    }
}

/// Opens the file `file` handed to the app, as by Finder or `open`: in the window whose folder holds it, else in a new
/// window of its repository or, outside one, of its folder.
fn open_file_from(cx: &Rc<AppContext>, mtm: MainThreadMarker, file: &std::path::Path) {
    let file = file.canonicalize().unwrap_or_else(|_| file.to_path_buf());
    let shown = cx
        .windows()
        .find(|w| w.project_root().is_some_and(|root| file.starts_with(root)));
    let controller = shown.unwrap_or_else(|| {
        let home = ide_core::paths::home();
        let folder = splat_git::find(&file)
            .map(|repository| repository.work_tree().to_path_buf())
            .filter(|root| *root != home)
            .or_else(|| file.parent().map(std::path::Path::to_path_buf))
            .unwrap_or_else(|| home.clone());
        let _ = State::load_user().note_recent(&folder);
        WindowController::open(cx, mtm, Some(folder))
    });
    controller.open_file(&file);
    controller.bring_to_front();
    // A file opened at launch may be a restored tab whose editor comes later; typing goes to it once it is there.
    crate::editor::with_opened(&controller, &file, |view| {
        if let Some(window) = view.window() {
            window.makeFirstResponder(Some(view));
        }
    });
}

/// Lets the user pick a folder in an open panel and opens it.
fn ask_for_folder(cx: &Rc<AppContext>, mtm: MainThreadMarker) {
    let panel = NSOpenPanel::openPanel(mtm);
    panel.setCanChooseDirectories(true);
    panel.setCanChooseFiles(false);
    panel.setAllowsMultipleSelection(false);
    panel.setPrompt(Some(&ns(t("Öffnen"))));
    if panel.runModal() != NSModalResponseOK {
        return;
    }
    if let Some(path) = panel.URL().and_then(|url| url.path()) {
        open_project(cx, mtm, PathBuf::from(path.to_string()));
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

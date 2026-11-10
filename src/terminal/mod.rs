//! The terminal tab (E8, E9): the user's login shell in a PTY, drawn by `TerminalView` under banners for a dev server
//! it started and for short notices.

mod access;
mod banner;
mod boxdraw;
mod cli;
mod clip;
mod input;
#[cfg(feature = "selftest")]
pub mod inspect;
mod keys;
mod links;
mod mouse;
mod palette;
mod render;
mod view;

use std::cell::RefCell;
use std::collections::HashMap;
use std::path::Path;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::Duration;

use ide_terminal::Event;
use objc2::rc::{Retained, Weak};
use objc2::{DefinedClass, MainThreadOnly};
use splat_i18n::{t, tf};

use crate::util::main_soon;
use crate::window::{TabKind, WindowController};
pub use cli::{CliStatus, CliThread};
pub use view::TerminalView;

/// How long a new terminal waits for the login shell's environment, which the app reads once at launch.
const ENVIRONMENT_WAIT: Duration = Duration::from_secs(30);

thread_local! {
    static VIEWS: RefCell<HashMap<u64, Weak<TerminalView>>> = RefCell::new(HashMap::new());
}

static NEXT_ID: AtomicU64 = AtomicU64::new(1);

/// The setting `OPTION_AS_META`, read when a terminal opens and set by the settings window.
static OPTION_AS_META: AtomicBool = AtomicBool::new(false);

/// Makes ⌥ Meta for every key in all terminals, or only where it types no character.
pub fn set_option_as_meta(on: bool) {
    OPTION_AS_META.store(on, Ordering::Relaxed);
}

fn option_as_meta() -> bool {
    OPTION_AS_META.load(Ordering::Relaxed)
}

/// Gives every open terminal fonts of `size` points.
pub fn set_font_size(size: f64) {
    for view in all_views() {
        view.set_font_size(size);
    }
}

fn next_id() -> u64 {
    NEXT_ID.fetch_add(1, Ordering::Relaxed)
}

fn register(view: &TerminalView) {
    VIEWS.with(|views| views.borrow_mut().insert(view.id(), Weak::from(view)));
}

fn forget(id: u64) {
    let _ = VIEWS.try_with(|views| views.borrow_mut().remove(&id));
}

fn find(id: u64) -> Option<Retained<TerminalView>> {
    VIEWS.with(|views| views.borrow().get(&id).and_then(Weak::load))
}

/// The folder the shell of the terminal in tab `tab` of `controller`'s window last reported, for opening it again.
pub fn folder_of_tab(controller: &WindowController, tab: isize) -> Option<std::path::PathBuf> {
    use objc2::DefinedClass;
    VIEWS.with(|views| {
        views.borrow().values().filter_map(Weak::load).find_map(|view| {
            let ivars = view.ivars();
            let mine = ivars.controller.load().is_some_and(|c| std::ptr::eq(&*c, controller));
            (mine && ivars.tab.get() == Some(tab)).then(|| ivars.cwd.borrow().clone())
        })
    })
}

/// Hands the session's events, which arrive on its reader thread, to the view on the main thread.
fn listener(id: u64) -> impl Fn(Event) + Send + Sync + 'static {
    move |event| {
        main_soon(move || {
            if let Some(view) = find(id) {
                view.handle(event);
            }
        });
    }
}

/// Opens a terminal tab in `cwd`, else in the window's folder or the home folder.
pub fn open_tab(controller: &WindowController, cwd: Option<&Path>) {
    let cwd = cwd
        .map(Path::to_path_buf)
        .or_else(|| controller.project_root())
        .or_else(|| controller.project())
        .filter(|dir| dir.is_dir())
        .unwrap_or_else(ide_core::paths::home);
    let (view, root) = TerminalView::new(controller.mtm(), controller, cwd, None);
    let tab = controller.add_tab(TabKind::Terminal, None, t("Terminal"), root);
    view.set_tab(tab);
    if let Some(window) = controller.window() {
        window.makeFirstResponder(Some(&view));
    }
    start(&view);
}

/// The terminals open now, in every window.
fn all_views() -> Vec<Retained<TerminalView>> {
    VIEWS.with(|views| views.borrow().values().filter_map(Weak::load).collect())
}

/// The programs that run in the terminals of `controller`'s window in place of a shell that waits.
pub fn running_in(controller: &WindowController) -> Vec<String> {
    all_views()
        .into_iter()
        .filter(|view| {
            view.ivars()
                .controller
                .load()
                .is_some_and(|own| std::ptr::eq(&*own, controller))
        })
        .filter_map(|view| {
            view.with_session(|session| match session.foreground() {
                ide_terminal::Foreground::Shell(_) | ide_terminal::Foreground::Unknown => None,
                ide_terminal::Foreground::Agent(agent) => Some(format!("{agent:?}")),
                ide_terminal::Foreground::Remote(program) | ide_terminal::Foreground::Other(program) => Some(program),
            })
            .flatten()
        })
        .collect()
}

/// The CLI threads of `controller`'s window, oldest first.
pub fn cli_threads(controller: &WindowController) -> Vec<CliThread> {
    cli::threads(controller)
}

/// The supervisor's numbers of the programs of every CLI thread, which the activity view lists as threads.
pub fn cli_processes() -> Vec<ide_process::ProcessId> {
    cli::processes()
}

/// Brings the tab of the CLI thread `key` forward.
pub fn show_cli_thread(controller: &WindowController, key: u64) {
    cli::show(controller, key);
}

/// Ends the CLI thread `key` by closing its tab: Ctrl+C, then SIGTERM to its group.
pub fn stop_cli_thread(controller: &WindowController, key: u64) {
    cli::stop(controller, key);
}

/// Fills the CLI-Thread submenu of the "+" menu: the CLIs found on the login PATH, in trusted folders only.
pub fn fill_cli_menu(controller: &WindowController, menu: &objc2_app_kit::NSMenu) {
    cli::fill_menu(controller, menu);
}

/// Opens a CLI thread of the CLI `name` in the window's folder (I5).
pub fn open_cli_thread(controller: &WindowController, name: &str) {
    cli::open(controller, name);
}

/// Opens a CLI thread that runs `program` as the CLI `name`, for the self-test's stand-in.
#[cfg(feature = "selftest")]
pub fn open_cli_program(controller: &WindowController, name: &str, program: std::path::PathBuf) {
    cli::open_program(controller, name, program);
}

/// Starts the shell now if the login shell's environment is known, else once it is.
fn start(view: &TerminalView) {
    if let Some(env) = crate::environment::get() {
        view.spawn(env);
        return;
    }
    let id = view.id();
    let waiting = std::thread::Builder::new()
        .name("terminal-environment".into())
        .spawn(move || {
            let env = crate::environment::wait(ENVIRONMENT_WAIT);
            main_soon(move || {
                let Some(view) = find(id) else { return };
                match env {
                    Some(env) => view.spawn(env),
                    None => view.notify(&tf(
                        "Das Terminal konnte nicht starten: {}",
                        &[t("Die Umgebung der Login-Shell fehlt.")],
                    )),
                }
            });
        });
    if let Err(error) = waiting {
        view.notify(&tf("Das Terminal konnte nicht starten: {}", &[&error.to_string()]));
    }
}

/// The terminals of `controller`'s window, the one in view first.
fn of_window(controller: &WindowController) -> Vec<Retained<TerminalView>> {
    let window = controller.window();
    let mut views: Vec<Retained<TerminalView>> =
        VIEWS.with(|views| views.borrow().values().filter_map(Weak::load).collect());
    views.retain(|view| {
        view.window()
            .is_some_and(|w| window.as_deref().is_some_and(|own| std::ptr::eq(&*w, own)))
    });
    views.sort_by_key(|view| (view.isHiddenOrHasHiddenAncestor(), std::cmp::Reverse(view.id())));
    views
}

/// The tab of the terminal in `controller`'s window whose shell is `pid` or started it, such as a dev server run
/// there.
pub fn tab_of_process(controller: &WindowController, pid: u32) -> Option<isize> {
    let shells: Vec<(u32, isize)> = of_window(controller)
        .iter()
        .filter_map(|view| {
            Some((
                view.with_session(ide_terminal::Session::pid)?,
                objc2::DefinedClass::ivars(&**view).tab.get()?,
            ))
        })
        .collect();
    let mut current = pid;
    for _ in 0..32 {
        if let Some((_, tab)) = shells.iter().find(|(shell, _)| *shell == current) {
            return Some(*tab);
        }
        current = parent_of(current).filter(|parent| *parent > 1)?;
    }
    None
}

/// The parent of process `pid`.
fn parent_of(pid: u32) -> Option<u32> {
    let mut info: libc::proc_bsdinfo = unsafe_zeroed();
    let size = libc::c_int::try_from(std::mem::size_of::<libc::proc_bsdinfo>()).ok()?;
    let pid = libc::c_int::try_from(pid).ok()?;
    // SAFETY: The buffer is a `proc_bsdinfo` of the size passed, which PROC_PIDTBSDINFO fills at most.
    let written = unsafe {
        libc::proc_pidinfo(
            pid,
            libc::PROC_PIDTBSDINFO,
            0,
            std::ptr::from_mut(&mut info).cast(),
            size,
        )
    };
    (written == size).then_some(info.pbi_ppid)
}

fn unsafe_zeroed() -> libc::proc_bsdinfo {
    // SAFETY: `proc_bsdinfo` is plain data of integers and byte arrays, for which all zeros is a valid value.
    unsafe { std::mem::zeroed() }
}

/// The terminal in view in `controller`'s window, else the one opened last: its title and the text on its screen.
pub fn recent_output(controller: &WindowController) -> Option<(String, String)> {
    let view = of_window(controller).into_iter().next()?;
    let lines = view.with_session(ide_terminal::Session::visible_text)?;
    let end = lines
        .iter()
        .rposition(|line| !line.trim().is_empty())
        .map_or(0, |n| n + 1);
    Some((view.label(), lines[..end].join("\n")))
}

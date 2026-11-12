//! The terminal tab (E8, E9): the user's login shell in a PTY, drawn by `TerminalView` under banners for a dev server
//! it started and for short notices.

mod access;
mod banner;
mod boxdraw;
mod clip;
mod frame;
mod gpu;
mod input;
#[cfg(feature = "selftest")]
pub mod inspect;
mod keys;
mod links;
mod model;
mod mouse;
mod palette;
mod render;
mod session;
mod sprites;
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
pub use view::TerminalView;

/// How long a new terminal waits for the login shell's environment, which the app reads once at launch.
const ENVIRONMENT_WAIT: Duration = Duration::from_secs(30);

/// The terminal views of every window by id.
#[derive(Default)]
pub struct TerminalViews {
    pub(super) views: RefCell<HashMap<u64, Weak<TerminalView>>>,
}

fn with_views<R>(f: impl FnOnce(&RefCell<HashMap<u64, Weak<TerminalView>>>) -> R) -> R {
    f(&crate::context::app().terminal_views.views)
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
    with_views(|views| views.borrow_mut().insert(view.id(), Weak::from(view)));
}

fn forget(id: u64) {
    if let Some(cx) = crate::context::try_app() {
        cx.terminal_views.views.borrow_mut().remove(&id);
    }
}

fn find(id: u64) -> Option<Retained<TerminalView>> {
    with_views(|views| views.borrow().get(&id).and_then(Weak::load))
}

/// The folder the shell of the terminal in tab `tab` of `controller`'s window last reported, for opening it again.
pub fn folder_of_tab(controller: &WindowController, tab: isize) -> Option<std::path::PathBuf> {
    use objc2::DefinedClass;
    with_views(|views| {
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
    let (view, root) = TerminalView::new(controller.mtm(), controller, cwd);
    let tab = controller.add_tab(TabKind::Terminal, None, t("Terminal"), root);
    view.set_tab(tab);
    if let Some(window) = controller.window() {
        window.makeFirstResponder(Some(&view));
    }
    start(&view);
}

/// The terminals open now, in every window.
fn all_views() -> Vec<Retained<TerminalView>> {
    with_views(|views| views.borrow().values().filter_map(Weak::load).collect())
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
    let mut views: Vec<Retained<TerminalView>> =
        with_views(|views| views.borrow().values().filter_map(Weak::load).collect());
    views.retain(|view| {
        view.ivars()
            .controller
            .load()
            .is_some_and(|own| std::ptr::eq(&*own, controller))
    });
    views.sort_by_key(|view| (!in_view(view), std::cmp::Reverse(view.id())));
    views
}

/// Whether `view` shows: in the window tab in front and not behind another view.
fn in_view(view: &TerminalView) -> bool {
    !view.isHiddenOrHasHiddenAncestor() && view.window().is_some_and(|w| crate::window::in_front(&w))
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

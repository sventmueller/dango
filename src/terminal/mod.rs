//! The terminal tab (E8, E9): the user's login shell in a PTY, drawn by `TerminalView` under banners for a dev server
//! it started and for short notices.

use std::path::Path;
use std::time::Duration;

use ide_ui_terminal::{all_views, find};
use objc2::MainThreadOnly;
use objc2::rc::Retained;
use splat_i18n::{t, tf};

use crate::util::main_soon;
use crate::window::{TabKind, WindowController};
#[cfg(feature = "selftest")]
pub use ide_ui_terminal::inspect;
pub use ide_ui_terminal::{TerminalView, set_option_as_meta};

/// How long a new terminal waits for the login shell's environment, which the app reads once at launch.
const ENVIRONMENT_WAIT: Duration = Duration::from_secs(30);

/// Gives every open terminal fonts of `size` points.
pub fn set_font_size(size: f64) {
    for view in all_views() {
        view.set_font_size(size);
    }
}

/// The folder the shell of the terminal in tab `tab` of `controller`'s window last reported, for opening it again.
pub fn folder_of_tab(controller: &WindowController, tab: isize) -> Option<std::path::PathBuf> {
    all_views()
        .into_iter()
        .find(|view| view.window_id() == controller.id() && view.tab() == Some(tab))
        .map(|view| view.cwd())
}

/// Opens a terminal tab in `cwd`, else in the window's folder or the home folder.
pub fn open_tab(controller: &WindowController, cwd: Option<&Path>) {
    let cwd = cwd
        .map(Path::to_path_buf)
        .or_else(|| controller.project_root())
        .or_else(|| controller.project())
        .filter(|dir| dir.is_dir())
        .unwrap_or_else(ide_core::paths::home);
    let (view, root) = TerminalView::new(controller.mtm(), controller.id(), cwd);
    let tab = controller.add_tab(TabKind::Terminal, None, t("Terminal"), root);
    view.set_tab(tab);
    if let Some(window) = controller.window() {
        window.makeFirstResponder(Some(&view));
    }
    start(&view);
}

/// Opens a terminal tab in `cwd` and types `line` into its shell once the shell runs, as a debug adapter's
/// `runInTerminal` asks; the program then belongs to the terminal like any other (Review C7). `then` gets the shell's
/// process number, or `None` if it did not start.
pub fn open_running(
    controller: &WindowController,
    cwd: &Path,
    line: String,
    then: impl FnOnce(Option<u32>) + Send + 'static,
) {
    open_tab(controller, Some(cwd));
    let Some(view) = of_window(controller).into_iter().next() else {
        return then(None);
    };
    type_when_started(view.id(), line, Box::new(then), 100);
}

/// Types `line` into the shell of terminal `id` once it has started, trying `tries` more times.
fn type_when_started(id: u64, line: String, then: Box<dyn FnOnce(Option<u32>) + Send>, tries: u32) {
    let typed = find(id).and_then(|view| {
        view.with_session(|session| {
            session.write(line.clone().into_bytes());
            session.pid()
        })
    });
    match typed {
        Some(pid) => then(Some(pid)),
        None if tries > 0 && find(id).is_some() => crate::util::main_after(Duration::from_millis(100), move || {
            type_when_started(id, line, then, tries - 1);
        }),
        None => then(None),
    }
}

/// The programs that run in the terminals of `controller`'s window in place of a shell that waits.
pub fn running_in(controller: &WindowController) -> Vec<String> {
    all_views()
        .into_iter()
        .filter(|view| view.window_id() == controller.id())
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

/// The coding agents that run in a terminal of any window, by name, for the question before quitting (E23).
pub fn agents_running() -> Vec<String> {
    all_views()
        .into_iter()
        .filter_map(|view| {
            view.with_session(|session| match session.foreground() {
                ide_terminal::Foreground::Agent(agent) => Some(format!("{agent:?}")),
                _ => None,
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
    let mut views = all_views();
    views.retain(|view| view.window_id() == controller.id());
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
        .filter_map(|view| Some((view.with_session(ide_terminal::Session::pid)?, view.tab()?)))
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

#[cfg(test)]
mod tests {
    use super::parent_of;

    #[test]
    fn a_process_parent_is_the_one_the_system_names() {
        // SAFETY: getppid has no preconditions.
        let parent = unsafe { libc::getppid() };
        assert_eq!(parent_of(std::process::id()), u32::try_from(parent).ok());
    }

    #[test]
    fn a_process_that_does_not_exist_has_no_parent() {
        assert_eq!(parent_of(u32::MAX), None);
        assert_eq!(parent_of(99_999_999), None);
    }
}

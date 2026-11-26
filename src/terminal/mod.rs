//! The terminal tab (E8, E9, E199): Tapa's session view with the user's login shell, under a banner for a dev server
//! it started. The views reach the app through `crate::host::TerminalHost`; this module opens them in tabs, starts
//! their shells, runs the commands of the menus and shortcuts on the one with the keyboard focus and answers what the
//! rest of the app asks of them.

mod agent;
mod changes;
mod commands;
mod config;
mod dev_server;
mod dock;
pub(crate) mod keys;
mod processes;
mod progress;
mod signals;
mod state;

use crate::window::TabId;
use std::path::{Path, PathBuf};
use std::time::Duration;

use objc2::MainThreadOnly;
use objc2::rc::Retained;
use splat_i18n::{t, tf};
use term_session::{Foreground, Session};
use term_ui::{all_views, find};

use crate::util::main_soon;
use crate::window::{TabKind, WindowController};
pub(crate) use changes::{changed, opened};
pub(crate) use commands::{
    block_items, clear, context_items, copy_formatted_item, perform, perform_on_view, show_shortcuts, text_finder,
    tick_secure_entry, toggle_secure_entry, validate, with_view_items,
};
pub(crate) use config::{settings_changed, switch_off};
pub(crate) use processes::Spawner;
pub(crate) use progress::{Ring, Tone};
pub(crate) use signals::came_to_front;
pub(crate) use state::Terminals;
#[cfg(feature = "selftest")]
pub(crate) use term_ui::inspect;
pub(crate) use term_ui::{TerminalView, set_option_as_meta};

/// How long a new terminal waits for the login shell's environment, which the app reads once at launch.
const ENVIRONMENT_WAIT: Duration = Duration::from_secs(30);

/// The fish script of the app's own terminal before E199, which fish would load beside Tapa's.
const OLD_FISH_INTEGRATION: &str = "shell-integration/fish/vendor_conf.d/ide-shell-integration.fish";

/// A terminal as the layout keeps it, opened again as a new shell in the folder it was in.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub(crate) struct KeptTerminal {
    /// The shell's working folder.
    pub(crate) folder: PathBuf,
}

impl ide_core::layout::Kind for KeptTerminal {
    const NAME: &'static str = "terminal";
}

/// A CLI thread of an older version, which the app no longer has; it opens again as a terminal.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub(crate) struct KeptCli {
    /// The CLI's name, such as `claude`.
    pub(crate) name: String,
    /// The CLI's id for the session, where the app could tell it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) session: Option<String>,
}

impl ide_core::layout::Kind for KeptCli {
    const NAME: &'static str = "cli";
}

/// Readies the terminal crates as the app starts: the suggestions' programs run through the supervisor, and the
/// shell integration of the app's own terminal before E199 goes.
pub(crate) fn install() {
    term_suggest::install_programs(&processes::Programs);
    let old = term_base::cache_dir().join(OLD_FISH_INTEGRATION);
    let queued = ide_core::lane::DISK.try_run(move || {
        if let Err(error) = ide_core::atomic_write::remove(&old) {
            crate::diag::report("removing the old fish integration", error);
        }
    });
    if queued.is_err() {
        crate::diag::report("removing the old fish integration", "the disk lane is full");
    }
}

/// Gives every open terminal fonts of `size` points.
pub(crate) fn set_font_size(size: f64) {
    for view in all_views() {
        view.set_font_size(size);
    }
}

/// The folder the shell of the terminal in tab `tab` of `controller`'s window last reported, for opening it again.
pub(crate) fn folder_of_tab(controller: &WindowController, tab: TabId) -> Option<PathBuf> {
    controller
        .context()
        .terminals()
        .kept_in(controller.id().get(), tab)
        .map(|view| view.cwd())
}

/// The document `tab` of `controller`'s window closes: the shell of a terminal in it ends now, as in Tapa, not only
/// once AppKit lets go of the view, and what it reported leaves the tab and the Dock icon.
pub(crate) fn closing(controller: &WindowController, tab: TabId) {
    for view in controller.context().terminals().shown_in(controller.id().get(), tab) {
        view.with_session(Session::close);
    }
    signals::forget(controller, tab);
}

/// Opens a terminal tab in `cwd`, else in the window's folder or the home folder. A terminal the window restores
/// starts its shell only once it first shows.
pub(crate) fn open_tab(controller: &WindowController, cwd: Option<&Path>) {
    open(controller, cwd);
}

/// `open_tab`, which hands back the view it opened.
fn open(controller: &WindowController, cwd: Option<&Path>) -> Retained<TerminalView> {
    let cwd = cwd
        .map(Path::to_path_buf)
        .or_else(|| controller.project_root())
        .or_else(|| controller.project())
        .filter(|dir| dir.is_dir())
        .unwrap_or_else(ide_core::paths::home);
    let cx = controller.context();
    cx.terminals().read_history(cx.leitstand().back());
    let mtm = controller.mtm();
    let window = term_ui::kit::WindowId::new(controller.id().get());
    let (view, root) = TerminalView::new(mtm, window, cwd.clone());
    let banner = dev_server::DevServerBanner::new(mtm, controller);
    let content = dev_server::above(mtm, &banner, &root);
    let restored = controller.restoring_editor_tabs();
    if restored {
        let id = view.id();
        ide_ui_common::panel::when_first_shown(&content, move || {
            if let Some(view) = find(id) {
                start(&view);
            }
        });
    }
    let tab = controller.add_tab(TabKind::Terminal, None, t("Terminal"), content);
    cx.terminals().add(view.id(), tab, &banner, cwd);
    if let Some(window) = controller.window() {
        window.makeFirstResponder(Some(&view));
    }
    if !restored {
        start(&view);
    }
    view.shown();
    view
}

/// Opens a terminal tab in `cwd` and types `line` into its shell once the shell runs, as a debug adapter's
/// `runInTerminal` asks; the program then belongs to the terminal like any other (Review C7). `then` gets the shell's
/// process number, or `None` if it did not start.
pub(crate) fn open_running(
    controller: &WindowController,
    cwd: &Path,
    line: String,
    then: impl FnOnce(Option<u32>) + Send + 'static,
) {
    let view = open(controller, Some(cwd));
    type_when_started(view.id(), line, Box::new(then), 100);
}

/// Opens a terminal tab in `cwd` that runs the shell command `line` and does not open again after a restart, as the
/// Containers panel's logs and shells (G30); its title is `title`.
pub(crate) fn open_passing(controller: &WindowController, cwd: &Path, title: &str, line: String) {
    let view = open(controller, Some(cwd));
    let terminals = controller.context().terminals();
    terminals.set_passing(view.id());
    if let Some(tab) = terminals.tab_of(view.id()) {
        controller.set_tab_title(tab, title);
    }
    type_when_started(view.id(), line + "\n", Box::new(|_| {}), 100);
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
pub(crate) fn running_in(controller: &WindowController) -> Vec<String> {
    all_views()
        .into_iter()
        .filter(|view| view.window_id().get() == controller.id().get())
        .filter_map(|view| {
            view.with_session(|session| match session.foreground() {
                Foreground::Shell(_) | Foreground::Unknown => None,
                Foreground::Agent(agent) => Some(format!("{agent:?}")),
                Foreground::Remote(program) | Foreground::Other(program) => Some(program),
            })
            .flatten()
        })
        .collect()
}

/// The coding agents that run in a terminal of any window, by name, for the question before quitting (E23).
pub(crate) fn agents_running() -> Vec<String> {
    all_views()
        .into_iter()
        .filter_map(|view| {
            view.with_session(|session| match session.foreground() {
                Foreground::Agent(agent) => Some(format!("{agent:?}")),
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
    let waiting = crate::util::spawn("terminal-environment", move || {
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

/// The terminals of `controller`'s window, the one in view first, then the newest.
fn of_window(controller: &WindowController) -> Vec<Retained<TerminalView>> {
    let mut views = all_views();
    views.retain(|view| view.window_id().get() == controller.id().get());
    views.sort_by_key(|view| (!in_view(view), std::cmp::Reverse(view.id())));
    views
}

/// Whether `view` shows: in the window tab in front and not behind another view.
fn in_view(view: &TerminalView) -> bool {
    !view.isHiddenOrHasHiddenAncestor() && view.window().is_some_and(|w| crate::window::in_front(&w))
}

/// The tab of the terminal in `controller`'s window whose shell is `pid` or started it, such as a dev server run
/// there.
pub(crate) fn tab_of_process(controller: &WindowController, pid: u32) -> Option<TabId> {
    let terminals = controller.context().terminals();
    let shells: Vec<(u32, TabId)> = of_window(controller)
        .iter()
        .filter_map(|view| Some((view.with_session(Session::pid)?, terminals.tab_of(view.id())?)))
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
pub(crate) fn recent_output(controller: &WindowController) -> Option<(String, String)> {
    let view = of_window(controller).into_iter().next()?;
    let lines = view.with_session(Session::visible_text)?;
    let end = lines
        .iter()
        .rposition(|line| !line.trim().is_empty())
        .map_or(0, |n| n + 1);
    Some((view.label(), lines[..end].join("\n")))
}

#[cfg(feature = "selftest")]
mod selftest_probe;
#[cfg(feature = "selftest")]
pub(crate) use selftest_probe::*;

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

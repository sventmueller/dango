//! What the app keeps of its terminals (E97): the registry of views, which term-ui reaches through its host, the
//! settings and themes they draw with, the command history, the key monitor of their shortcuts, the Dock icon's
//! progress bar, and for each view its tab, its dev server banner, what its shell reported last, which the
//! suggestions weigh (Tapa's E8), and the dot and the progress its program asked for.

use std::cell::{Cell, OnceCell, RefCell};
use std::collections::HashMap;
use std::path::PathBuf;
use std::rc::Rc;
use std::time::{SystemTime, UNIX_EPOCH};

use ide_ui_common::notify::Clicks;
use objc2::MainThreadMarker;
use objc2::rc::{Retained, Weak};
use objc2::runtime::AnyObject;
use term_config::Config;
use term_config::theme::Themes;
use term_session::event::Progress;
use term_suggest::{Context, Entry, History};

use super::dev_server::DevServerBanner;
use super::dock::DockProgress;
use super::progress::Ring;
use crate::leitstand::Back;
use crate::window::TabId;

/// The terminals of every window.
#[derive(Default)]
pub(crate) struct Terminals {
    views: Rc<term_ui::State>,
    config: RefCell<Option<Rc<Config>>>,
    themes: OnceCell<Rc<Themes>>,
    history: RefCell<Kept>,
    shells: RefCell<HashMap<u64, Shell>>,
    /// The key monitor that runs the terminal's shortcuts.
    monitor: RefCell<Option<Retained<AnyObject>>>,
    /// Whether a menu put the shortcuts on the terminal's items since they were last all taken off.
    shortcuts: Cell<bool>,
    dock: OnceCell<Retained<DockProgress>>,
    /// What hears of clicks on the terminals' notifications, once the first was posted; none outside a bundle.
    clicks: OnceCell<Option<Retained<Clicks>>>,
    #[cfg(feature = "selftest")]
    heard: RefCell<Vec<Heard>>,
}

/// The history, read on a thread of its own once the first terminal opens.
#[derive(Default)]
enum Kept {
    #[default]
    Unread,
    /// Being read; the commands that ended meanwhile go in once it is.
    Reading(Vec<Entry>),
    Read(History),
}

/// What the app knows of one terminal view.
struct Shell {
    tab: TabId,
    /// Its tab does not open again after a restart, since it shows a command, not a shell of its own (G30).
    passing: bool,
    banner: Weak<DevServerBanner>,
    folder: PathBuf,
    branch: Option<String>,
    /// The command that runs now, as the shell integration sent it.
    running: Option<String>,
    previous: Option<String>,
    previous_exit: Option<i32>,
    /// The progress its program reports.
    progress: Option<Progress>,
    /// Whether its tab shows the red dot for a bell or a notification the user has not seen yet.
    waits: bool,
}

impl Terminals {
    pub(crate) fn views(&self) -> &Rc<term_ui::State> {
        &self.views
    }

    /// The terminal's settings in force.
    pub(crate) fn config(&self) -> Rc<Config> {
        if let Some(config) = self.config.borrow().as_ref() {
            return Rc::clone(config);
        }
        self.reload_config()
    }

    /// Makes the terminal's settings anew from the user's.
    pub(crate) fn reload_config(&self) -> Rc<Config> {
        let settings = ide_core::settings::Settings::load_user();
        term_ui::set_option_as_meta(settings.get(&ide_core::settings::keys::OPTION_AS_META));
        let config = Rc::new(super::config::of(&settings));
        *self.config.borrow_mut() = Some(Rc::clone(&config));
        config
    }

    pub(super) fn keep_monitor(&self, monitor: Option<Retained<AnyObject>>) {
        *self.monitor.borrow_mut() = monitor;
    }

    /// Notes whether the terminal's menu items may carry their shortcuts; whether they might before.
    pub(super) fn note_shortcuts(&self, shown: bool) -> bool {
        self.shortcuts.replace(shown)
    }

    /// Tapa's own themes, "Tapa Light" and "Tapa Dark".
    pub(crate) fn themes(&self) -> Rc<Themes> {
        Rc::clone(self.themes.get_or_init(|| Rc::new(Themes::load(&[]))))
    }

    /// Keeps view `view`, which shows in tab `tab` with `banner` above it and starts in `folder`.
    pub(super) fn add(&self, view: u64, tab: TabId, banner: &DevServerBanner, folder: PathBuf) {
        let branch = term_ui::git::branch(&folder);
        let mut shells = self.shells.borrow_mut();
        shells.retain(|id, _| term_ui::find(*id).is_some());
        shells.insert(
            view,
            Shell {
                tab,
                passing: false,
                banner: Weak::from(banner),
                folder,
                branch,
                running: None,
                previous: None,
                previous_exit: None,
                progress: None,
                waits: false,
            },
        );
    }

    pub(super) fn set_passing(&self, view: u64) {
        if let Some(shell) = self.shells.borrow_mut().get_mut(&view) {
            shell.passing = true;
        }
    }

    /// The tab that shows view `view`.
    pub(crate) fn tab_of(&self, view: u64) -> Option<TabId> {
        self.shells.borrow().get(&view).map(|shell| shell.tab)
    }

    /// Keeps `progress` as what the program of `view` reports; whether that changed.
    pub(super) fn set_progress(&self, view: u64, progress: Option<Progress>) -> bool {
        let mut shells = self.shells.borrow_mut();
        let Some(shell) = shells.get_mut(&view) else {
            return false;
        };
        std::mem::replace(&mut shell.progress, progress) != progress
    }

    /// Keeps whether the tab of `view` shows the red dot; whether that changed.
    pub(super) fn set_waits(&self, view: u64, waits: bool) -> bool {
        let mut shells = self.shells.borrow_mut();
        let Some(shell) = shells.get_mut(&view) else {
            return false;
        };
        std::mem::replace(&mut shell.waits, waits) != waits
    }

    /// The views of the window `window` whose tabs show the red dot.
    pub(super) fn waiting_in(&self, window: u64) -> Vec<u64> {
        self.shells
            .borrow()
            .iter()
            .filter(|(id, shell)| {
                shell.waits && term_ui::find(**id).is_some_and(|view| view.window_id().get() == window)
            })
            .map(|(id, _)| *id)
            .collect()
    }

    /// Forgets the views in tab `tab` of the window `window`, which closes; whether one of them reported progress.
    pub(super) fn forget_tab(&self, window: u64, tab: TabId) -> bool {
        let mut shells = self.shells.borrow_mut();
        let mut had_progress = false;
        shells.retain(|id, shell| {
            let closes = shell.tab == tab && term_ui::find(*id).is_none_or(|view| view.window_id().get() == window);
            had_progress |= closes && shell.progress.is_some();
            !closes
        });
        had_progress
    }

    /// Shows on the Dock icon the progress of every terminal's program, or the icon alone when none reports any.
    pub(super) fn show_progress_on_dock(&self, mtm: MainThreadMarker) {
        let rings: Vec<Ring> = self
            .shells
            .borrow()
            .iter()
            .filter(|(id, _)| term_ui::find(**id).is_some())
            .filter_map(|(_, shell)| shell.progress.map(Ring::of))
            .collect();
        DockProgress::show(&self.dock, mtm, Ring::combined(&rings));
    }

    /// Keeps what `make` makes to hear of clicks on notifications, unless one was made already.
    pub(super) fn keep_clicks(&self, make: impl FnOnce() -> Option<Retained<Clicks>>) {
        self.clicks.get_or_init(make);
    }

    pub(super) fn banner_of(&self, view: u64) -> Option<Retained<DevServerBanner>> {
        self.shells.borrow().get(&view)?.banner.load()
    }

    /// The shell of `view` works in `folder` now.
    pub(super) fn moved(&self, view: u64, folder: PathBuf) {
        if let Some(shell) = self.shells.borrow_mut().get_mut(&view) {
            shell.branch = term_ui::git::branch(&folder);
            shell.folder = folder;
        }
    }

    /// The shell of `view` prompts again, perhaps on another branch.
    pub(super) fn prompted(&self, view: u64) {
        if let Some(shell) = self.shells.borrow_mut().get_mut(&view) {
            shell.branch = term_ui::git::branch(&shell.folder);
        }
    }

    pub(super) fn started(&self, view: u64, line: Option<String>) {
        if let Some(shell) = self.shells.borrow_mut().get_mut(&view) {
            shell.running = line;
        }
    }

    /// The command of `view` ended with `code`: it goes into the history as the one before the next.
    pub(super) fn finished(&self, view: u64, code: Option<i32>) {
        let entry = {
            let mut shells = self.shells.borrow_mut();
            let Some(shell) = shells.get_mut(&view) else { return };
            let Some(line) = shell.running.take() else { return };
            let line = line.trim().to_string();
            let entry = Entry {
                line: line.clone(),
                folder: shell.folder.clone(),
                branch: shell.branch.clone(),
                previous: shell.previous.clone(),
                exit_code: code,
                started: SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .map_or(0, |since| since.as_secs()),
            };
            shell.previous = Some(line);
            shell.previous_exit = code;
            entry
        };
        match &mut *self.history.borrow_mut() {
            Kept::Read(history) => history.record(entry),
            Kept::Reading(waiting) => waiting.push(entry),
            Kept::Unread => {}
        }
    }

    /// Where the user types in view `view`, while the settings want suggestions.
    pub(crate) fn context(&self, view: u64) -> Option<Context> {
        if !self.config().input.suggestions {
            return None;
        }
        let shells = self.shells.borrow();
        let shell = shells.get(&view)?;
        Some(Context {
            folder: shell.folder.clone(),
            branch: shell.branch.clone(),
            previous: shell.previous.clone(),
            exit_code: shell.previous_exit,
        })
    }

    /// The command the history suggests after `prefix` in view `view`, or without one the command most likely next.
    pub(crate) fn suggest(&self, view: u64, prefix: &str) -> Option<String> {
        let context = self.context(view)?;
        match &*self.history.borrow() {
            Kept::Read(history) if prefix.is_empty() => history.next_command(&context),
            Kept::Read(history) => history.suggest(prefix, &context),
            Kept::Unread | Kept::Reading(_) => None,
        }
    }

    /// Reads the history on a thread of its own unless that began already; `back` brings it to the main thread.
    pub(super) fn read_history(&self, back: Back) {
        let mut kept = self.history.borrow_mut();
        if !matches!(*kept, Kept::Unread) {
            return;
        }
        *kept = Kept::Reading(Vec::new());
        drop(kept);
        let path = ide_core::paths::data_dir().join("terminal-history.sqlite");
        let started = crate::util::spawn("terminal-history", move || {
            let history = History::open(&path);
            crate::util::main_soon(move || {
                back.with(|cx| cx.terminals().history_read(history));
            });
        });
        if let Err(why) = started {
            crate::diag::report("reading the terminal history", why);
            self.history_read(History::in_memory());
        }
    }

    fn history_read(&self, mut history: History) {
        let waiting = match std::mem::take(&mut *self.history.borrow_mut()) {
            Kept::Reading(waiting) => waiting,
            Kept::Unread | Kept::Read(_) => Vec::new(),
        };
        for entry in waiting {
            history.record(entry);
        }
        *self.history.borrow_mut() = Kept::Read(history);
    }

    /// The view in tab `tab` of the window `window`, unless it shows a command that does not open again.
    pub(super) fn kept_in(&self, window: u64, tab: TabId) -> Option<Retained<term_ui::TerminalView>> {
        self.in_tab(window, tab, |shell| !shell.passing).into_iter().next()
    }

    /// The views in tab `tab` of the window `window`.
    pub(super) fn shown_in(&self, window: u64, tab: TabId) -> Vec<Retained<term_ui::TerminalView>> {
        self.in_tab(window, tab, |_| true)
    }

    fn in_tab(&self, window: u64, tab: TabId, wanted: impl Fn(&Shell) -> bool) -> Vec<Retained<term_ui::TerminalView>> {
        let ids: Vec<u64> = self
            .shells
            .borrow()
            .iter()
            .filter(|(_, shell)| shell.tab == tab && wanted(shell))
            .map(|(id, _)| *id)
            .collect();
        ids.into_iter()
            .filter_map(term_ui::find)
            .filter(|view| view.window_id().get() == window)
            .collect()
    }
}

#[cfg(feature = "selftest")]
mod selftest_probe;
#[cfg(feature = "selftest")]
pub(crate) use selftest_probe::*;

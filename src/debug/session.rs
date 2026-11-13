//! Starting a session (from a launch configuration, as "debug the current program" or by attaching to a process) and
//! what its events do in the window. Starting runs what the folder configures, so it waits for the user's trust (E22);
//! no session starts by itself, also not after a restart (Review C18).

use std::time::Duration;

use ide_core::debugging::ConsoleKind;
use ide_dap::{Adapters, Event, LaunchConfig, Request, Session, StartError};
use ide_process::Supervisor;
use objc2::{DefinedClass, MainThreadOnly};
use objc2_app_kit::{NSMenuItem, NSPopUpButton};
use splat_i18n::{t, tf};

use super::console::ConsoleTab;
use super::controller::Link;
use super::model::session::{Caps, Origin, Session as Shown};
use super::panel::{DebugPanel, with_panel};
use crate::util::main_soon;

/// Where a session's events go, on the thread its adapter reports on.
pub(super) type Events = Box<dyn Fn(ide_dap::SessionId, Event) + Send + Sync>;

/// The number of a session the panel shows while its start has not returned one yet.
pub(super) const PENDING: u64 = 0;

/// How long a start may go unanswered before the panel says it waits, as for macOS's Developer Tools Access.
const ANSWER_WAIT: Duration = Duration::from_secs(2);

/// How long a start waits for the login shell's environment.
const ENVIRONMENT_WAIT: Duration = Duration::from_secs(30);

/// The tags of the configuration menu's own entries; launch.json's entries have their place from 0.
const CURRENT_TAG: isize = -1;
const ATTACH_TAG: isize = -2;

impl DebugPanel {
    /// Whether a session can start now: in a trusted folder, while no other is being started.
    pub fn can_start(&self) -> bool {
        self.controller().is_some_and(|c| c.is_trusted()) && self.ivars().links.borrow().starting.is_none()
    }

    /// Whether the window has a debug console.
    pub fn has_console(&self) -> bool {
        !self.ivars().links.borrow().consoles.is_empty()
    }

    /// Brings the newest console tab to the front.
    pub fn show_console(&self) {
        let console = self.ivars().links.borrow().consoles.last().cloned();
        if let (Some(console), Some(controller)) = (console, self.controller()) {
            controller.select_tab_by_id(console.tab);
        }
    }

    /// Reads the configurations of `launch.json` and offers them with "debug the current program" and attaching.
    pub fn load_configurations(&self) {
        let trusted = self.controller().is_some_and(|c| c.is_trusted());
        let variables = ide_dap::Variables {
            workspace_folder: self.root().to_path_buf(),
            env: crate::environment::get().map(<[_]>::to_vec).unwrap_or_default(),
            ..ide_dap::Variables::default()
        };
        let (entries, problem) = match ide_dap::launch_configs(self.root(), trusted, &variables) {
            Ok(entries) => (entries, None),
            Err(ide_dap::LaunchError::Missing | ide_dap::LaunchError::Untrusted) => (Vec::new(), None),
            Err(error) => (Vec::new(), Some(error.to_string())),
        };
        {
            let mut links = self.ivars().links.borrow_mut();
            links.configurations = entries;
            links.configuration_problem = problem;
        }
        self.fill_configurations();
    }

    fn fill_configurations(&self) {
        let popup: &NSPopUpButton = &self.ui().configurations;
        let mtm = self.mtm();
        let Some(menu) = popup.menu() else { return };
        menu.removeAllItems();
        menu.setAutoenablesItems(false);
        let add = |title: &str, tag: isize, enabled: bool| {
            let entry = crate::util::item(mtm, title, None, "");
            entry.setTag(tag);
            entry.setEnabled(enabled);
            menu.addItem(&entry);
        };
        add(t("Aktuelles Programm debuggen"), CURRENT_TAG, true);
        let links = self.ivars().links.borrow();
        if !links.configurations.is_empty() {
            menu.addItem(&NSMenuItem::separatorItem(mtm));
        }
        for (index, entry) in links.configurations.iter().enumerate() {
            add(&entry.name, isize::try_from(index).unwrap_or(-3), entry.config.is_ok());
        }
        menu.addItem(&NSMenuItem::separatorItem(mtm));
        add(t("An Prozess anhängen …"), ATTACH_TAG, true);
        let chosen = self.read().configuration.clone();
        let index = chosen
            .and_then(|name| links.configurations.iter().position(|e| e.name == name))
            .and_then(|i| isize::try_from(i).ok());
        popup.selectItemWithTag(index.unwrap_or(CURRENT_TAG));
    }

    /// The user picked a configuration; it is kept for the next time.
    pub(super) fn configuration_chosen(&self) {
        let tag = self.ui().configurations.selectedTag();
        let name = usize::try_from(tag).ok().and_then(|index| {
            self.ivars()
                .links
                .borrow()
                .configurations
                .get(index)
                .map(|e| e.name.clone())
        });
        self.model().configuration = name;
        self.apply(vec![super::model::Effect::Save]);
    }

    /// Starts what the configuration menu shows.
    pub fn start_chosen(&self) {
        match self.ui().configurations.selectedTag() {
            CURRENT_TAG => self.debug_current_program(),
            ATTACH_TAG => self.ask_attach(),
            tag => {
                let config = usize::try_from(tag).ok().and_then(|index| {
                    let links = self.ivars().links.borrow();
                    links.configurations.get(index).and_then(|e| e.config.clone().ok())
                });
                match config {
                    Some(config) if config.process_pick => self.pick_process(move |panel, pid| {
                        panel.start_config(config.with_process(pid));
                    }),
                    Some(config) => self.start_config(config),
                    None => {}
                }
            }
        }
    }

    /// Debugs the file of the editor in front without a configuration: Rust after `cargo build`, Go by its package,
    /// Node and Python by the file itself.
    pub fn debug_current_program(&self) {
        let Some(controller) = self.controller() else { return };
        let editor = controller
            .selected_tab()
            .and_then(|tab| crate::editor::editor_of_tab(&controller, tab));
        let Some(editor) = editor else {
            self.tell(t("Öffne zuerst die Datei des Programms, das du debuggen willst."));
            return;
        };
        let (Some(file), language) = (
            editor.document().path().map(std::path::Path::to_path_buf),
            editor.document().language().map(|l| l.name().to_string()),
        ) else {
            self.tell(t("Die Datei ist noch nicht gesichert."));
            return;
        };
        let language = language.unwrap_or_default();
        let name = file
            .file_name()
            .map_or_else(String::new, |n| n.to_string_lossy().into_owned());
        self.begin(name, Origin::Launched, move |adapters, root, trusted, cancel, line| {
            ide_dap::current_program(adapters, root, trusted, &file, &language, cancel, line)
                .map_err(|e| current_words(&e))
        });
    }

    /// Starts `config`.
    pub fn start_config(&self, config: LaunchConfig) {
        let origin = match config.request {
            Request::Launch => Origin::Launched,
            Request::Attach => Origin::Attached,
        };
        self.begin(config.name.clone(), origin, move |_, _, _, _, _| Ok(config));
    }

    /// Starts a session named `name` with the configuration `make` returns, off the main thread.
    fn begin(
        &self,
        name: String,
        origin: Origin,
        make: impl FnOnce(
            &Adapters,
            &std::path::Path,
            bool,
            &ide_lsp::Cancel,
            &mut dyn FnMut(&str),
        ) -> Result<LaunchConfig, String>
        + Send
        + 'static,
    ) {
        let (panel, root) = (self.id(), self.root().to_path_buf());
        let trusted = self.controller().is_some_and(|c| c.is_trusted());
        self.launch(name, origin, move |setup, cancel, line, events| {
            let env = crate::environment::wait(ENVIRONMENT_WAIT)
                .map(<[_]>::to_vec)
                .unwrap_or_default();
            let adapters = Adapters::new(Supervisor::global(), env);
            let config = make(&adapters, &root, trusted, cancel, line)?;
            ide_dap::start(&adapters, &root, trusted, config, setup, cancel, line, events).map_err(|e| {
                if matches!(&e, StartError::Missing(m) if m.install == Some(ide_dap::Install::JsDebug)) {
                    main_soon(move || with_panel(panel, DebugPanel::offer_js_debug));
                }
                start_words(&e)
            })
        });
    }

    /// Starts a session named `name` with `run`, off the main thread: it gets the breakpoints, a way to cancel, where
    /// to report progress, and the callback for the session's events, and returns the session or why it did not start.
    pub(super) fn launch(
        &self,
        name: String,
        origin: Origin,
        run: impl FnOnce(ide_dap::Setup, &ide_lsp::Cancel, &mut dyn FnMut(&str), Events) -> Result<Session, String>
        + Send
        + 'static,
    ) {
        if !self.can_start() {
            if self.controller().is_some_and(|c| !c.is_trusted()) {
                self.tell(t(
                    "Debuggen startet Programme des Ordners und geht erst, wenn du ihm vertraust.",
                ));
            }
            return;
        }
        let cancel = ide_lsp::Cancel::new();
        {
            let mut links = self.ivars().links.borrow_mut();
            links.starting = Some((name.clone(), origin));
            links.cancel = Some(cancel.clone());
        }
        let console = self.console_for_start(&name);
        console.push(ConsoleKind::Note, &tf("„{}“ startet …", &[&name]));
        self.model().started(Shown::new(PENDING, name.clone(), origin));
        if let Some(controller) = self.controller() {
            controller.show_inspector(true);
        }
        let (panel, setup) = (self.id(), self.setup());
        let started = std::thread::Builder::new().name("debug-start".into()).spawn(move || {
            let mut line = |text: &str| {
                let text = format!("{text}\n");
                main_soon(move || with_panel(panel, |p| p.note_start(&text)));
            };
            let events: Events = Box::new(move |id, event| {
                main_soon(move || with_panel(panel, |p| p.event(id.get(), event)));
            });
            let result = run(setup, &cancel, &mut line, events);
            main_soon(move || with_panel(panel, |p| p.started(result)));
        });
        if started.is_err() {
            self.started(Err(t("Der Debugger konnte nicht starten.").to_string()));
        }
        crate::util::main_after(ANSWER_WAIT, move || with_panel(panel, DebugPanel::check_answer));
        self.refresh();
    }

    /// The start has gone unanswered for a while: the panel and the console say the session waits, as when macOS
    /// asks for the password before a debugger may take control of a program, and Stop stays at hand (R23).
    fn check_answer(&self) {
        let waiting: Vec<u64> = {
            let mut model = self.model();
            let ids: Vec<u64> = model.sessions.iter().map(|s| s.id).collect();
            ids.into_iter().filter(|id| model.still_starting(*id)).collect()
        };
        for id in waiting {
            let console = self.console_for(id).or_else(|| self.waiting_console());
            if let Some(console) = console {
                console.push(
                    ConsoleKind::Note,
                    &format!("{}\n", t("Wartet auf Freigabe durch macOS …")),
                );
            }
        }
        self.refresh();
    }

    /// Stop on a session whose start has not returned yet: a build is cancelled, and a session that still comes is
    /// stopped as soon as it does.
    pub(super) fn stop_pending_start(&self, id: u64) {
        let cancel = {
            let mut links = self.ivars().links.borrow_mut();
            links.stop_pending = true;
            links.cancel.clone()
        };
        if let Some(cancel) = cancel {
            cancel.cancel();
        }
        self.model().ended(id, None, None);
        self.refresh();
    }

    /// The console a new session writes to: a finished one is used again, else a new tab opens.
    fn console_for_start(&self, name: &str) -> std::rc::Rc<ConsoleTab> {
        let reused = self
            .ivars()
            .links
            .borrow()
            .consoles
            .iter()
            .find(|c| c.is_finished() && c.session.get().is_none())
            .cloned();
        let controller = self.controller().expect("a panel that starts a session has its window");
        if let Some(console) = reused {
            console.start_over(&controller, name);
            return console;
        }
        let front = controller.selected_tab();
        let console = ConsoleTab::open(self, &controller, 0, name);
        console.session.set(None);
        if let Some(front) = front {
            controller.select_tab_by_id(front);
        }
        self.ivars().links.borrow_mut().consoles.push(console.clone());
        console
    }

    /// The console that waits for the session being started.
    fn waiting_console(&self) -> Option<std::rc::Rc<ConsoleTab>> {
        self.ivars()
            .links
            .borrow()
            .consoles
            .iter()
            .rev()
            .find(|c| !c.is_finished() && c.session.get().is_none())
            .cloned()
    }

    fn note_start(&self, text: &str) {
        if let Some(console) = self.waiting_console() {
            console.push(ConsoleKind::Output, text);
        }
    }

    /// The start ended: the session runs, or the console says why it did not.
    fn started(&self, result: Result<Session, String>) {
        let starting = self.ivars().links.borrow_mut().starting.take();
        self.ivars().links.borrow_mut().cancel = None;
        let stop = std::mem::take(&mut self.ivars().links.borrow_mut().stop_pending);
        match result {
            Ok(session) if stop => {
                session.stop();
                self.register(session, None, "", Origin::Launched);
            }
            Ok(session) => {
                let id = session.id().get();
                let (name, origin) = starting.unwrap_or_else(|| (session.name().to_string(), Origin::Launched));
                self.register(session, None, &name, origin);
                if let Some(console) = self.waiting_console() {
                    console.session.set(Some(id));
                }
            }
            Err(why) => {
                self.model().ended(PENDING, None, (!stop).then(|| why.clone()));
                if let Some(console) = self.waiting_console() {
                    console.finish(&why);
                }
                if !stop {
                    self.tell(&why);
                }
            }
        }
        self.refresh();
    }

    /// Keeps `session` as one of the window's, beneath `parent` if the adapter started it under another.
    fn register(&self, session: Session, parent: Option<u64>, name: &str, origin: Origin) {
        let id = session.id().get();
        if parent.is_none() {
            self.model().renumber(PENDING, id);
        }
        if self.read().session(id).is_none() {
            self.model().started(Shown::new(id, name, origin));
        }
        let caps = caps_of(&session.capabilities());
        if caps != Caps::default() {
            let effects = self.model().capabilities(id, caps);
            self.apply(effects);
        }
        let mut links = self.ivars().links.borrow_mut();
        if !links.sessions.iter().any(|l| l.id == id) {
            links.sessions.push(Link { id, session, parent });
        }
    }

    /// The session `id` exists in the model, also when its first events come before its start returned.
    fn ensure(&self, id: u64) {
        if self.read().session(id).is_some() {
            return;
        }
        if self.ivars().links.borrow().starting.is_some() && self.read().session(PENDING).is_some() {
            self.model().renumber(PENDING, id);
            if let Some(console) = self.waiting_console() {
                console.session.set(Some(id));
            }
            return;
        }
        let (name, origin) = self
            .ivars()
            .links
            .borrow()
            .starting
            .clone()
            .unwrap_or_else(|| (tf("Sitzung {}", &[&id.to_string()]), Origin::Launched));
        self.model().started(Shown::new(id, name, origin));
        if let Some(console) = self.waiting_console() {
            console.session.set(Some(id));
        }
    }

    /// What a session reports.
    pub fn event(&self, id: u64, event: Event) {
        self.ensure(id);
        let effects = match event {
            Event::Initialized(caps) | Event::Capabilities(caps) => self.model().capabilities(id, caps_of(&caps)),
            Event::Running => {
                self.model().continued(id);
                super::editor::forget_values(self);
                Vec::new()
            }
            Event::Stopped(stop) => {
                let thread = stop
                    .thread
                    .map(|t| t.0)
                    .or_else(|| self.read().session(id).and_then(|s| s.threads.first().map(|t| t.id)));
                let reason = stop_reason(&stop.reason);
                self.model().stopped(id, thread.unwrap_or(1), reason)
            }
            Event::Continued { .. } => {
                self.model().continued(id);
                super::editor::forget_values(self);
                Vec::new()
            }
            Event::ThreadStarted(_) | Event::ThreadExited(_) => vec![super::model::Effect::Threads(id)],
            Event::Output(output) => {
                self.output(id, &output);
                return;
            }
            Event::Exited(code) => {
                if let Some(console) = self.console_for(id) {
                    console.push(
                        ConsoleKind::Note,
                        &tf("Das Programm endete mit Code {}.", &[&code.to_string()]),
                    );
                }
                return;
            }
            Event::RunInTerminal(request, answer) => {
                self.run_in_terminal(request, answer);
                return;
            }
            Event::Child(child) => {
                let name = tf(
                    "{} · {}",
                    &[
                        &self.read().session(id).map(|s| s.name.clone()).unwrap_or_default(),
                        child.name(),
                    ],
                );
                let origin = self.read().session(id).map_or(Origin::Launched, |s| s.origin);
                self.register(child, Some(id), &name, origin);
                Vec::new()
            }
            Event::Invalidated => self.invalidated(id),
            Event::Ended(end) => {
                super::editor::forget_values(self);
                self.ended(id, &end);
                Vec::new()
            }
            Event::Breakpoint(_) | Event::BreakpointsSet { .. } | Event::Program(_) | Event::Stopping => return,
        };
        self.apply(effects);
    }

    fn output(&self, id: u64, output: &ide_dap::Output) {
        let kind = match output.category {
            ide_dap::OutputCategory::Stderr => ConsoleKind::Error,
            _ => ConsoleKind::Output,
        };
        if let Some(console) = self.console_for(id) {
            console.push(kind, &output.text);
        }
    }

    /// Values shown are out of date: the stopped thread's stack is asked for again.
    fn invalidated(&self, id: u64) -> Vec<super::model::Effect> {
        let thread = self.read().session(id).and_then(|s| s.paused_thread());
        match thread {
            Some(thread) => vec![
                super::model::Effect::Threads(id),
                super::model::Effect::Stack(id, thread),
            ],
            None => Vec::new(),
        }
    }

    fn ended(&self, id: u64, end: &ide_dap::End) {
        let problem = match &end.reason {
            ide_dap::EndReason::Failed(why) => Some(why.clone()),
            ide_dap::EndReason::AdapterCrashed { stderr, .. } => {
                Some(tf("Der Debug-Adapter ist abgestürzt. {}", &[stderr.trim()]))
            }
            ide_dap::EndReason::Finished | ide_dap::EndReason::Stopped => None,
        };
        self.model().ended(id, end.exit_code, problem.clone());
        let console = self.console_for(id);
        let parent = self
            .ivars()
            .links
            .borrow()
            .sessions
            .iter()
            .find(|l| l.id == id)
            .and_then(|l| l.parent);
        self.ivars().links.borrow_mut().sessions.retain(|l| l.id != id);
        if parent.is_none()
            && let Some(console) = console
        {
            if let Some(problem) = &problem {
                console.push(ConsoleKind::Error, &format!("{problem}\n"));
            }
            console.finish(t("Beendet"));
        }
        if let Some(problem) = problem {
            self.tell(&problem);
        }
        self.refresh();
    }

    /// Runs the program in a terminal tab as the adapter asks; it belongs to that terminal (Review C7).
    fn run_in_terminal(&self, request: ide_dap::TerminalRequest, answer: ide_dap::TerminalAnswer) {
        let Some(controller) = self.controller() else {
            return answer.failed("the window closed");
        };
        let mut line = String::new();
        for (name, value) in &request.env {
            match value {
                Some(value) => line.push_str(&format!("export {name}={}; ", quote(value))),
                None => line.push_str(&format!("unset {name}; ")),
            }
        }
        line.push_str("exec ");
        line.push_str(&request.args.iter().map(|a| quote(a)).collect::<Vec<_>>().join(" "));
        line.push('\n');
        crate::terminal::open_running(&controller, &request.cwd, line, move |shell| match shell {
            Some(pid) => answer.started(None, Some(pid)),
            None => answer.failed("the terminal did not start"),
        });
    }

    /// Lets the user pick a running process to attach to, in a sheet with a list; `then` gets its number.
    pub fn pick_process(&self, then: impl FnOnce(&DebugPanel, u32) + 'static) {
        let panel = self.id();
        self.ivars().links.borrow_mut().picking = Some(Box::new(then));
        std::thread::spawn(move || {
            let env = crate::environment::wait(ENVIRONMENT_WAIT)
                .map(<[_]>::to_vec)
                .unwrap_or_default();
            let found = ide_dap::processes(Supervisor::global(), &env).unwrap_or_default();
            main_soon(move || with_panel(panel, |p| super::sheets::show_processes(p, found)));
        });
    }

    /// Asks for a process and attaches the debugger of the editor's language to it, else lldb-dap.
    pub fn ask_attach(&self) {
        let kind = self
            .controller()
            .and_then(|c| crate::editor::editor_of_tab(&c, c.selected_tab()?))
            .and_then(|e| e.document().language().map(|l| l.name().to_string()))
            .and_then(|l| ide_dap::AdapterKind::for_language(&l))
            .unwrap_or(ide_dap::AdapterKind::Lldb);
        self.pick_process(move |panel, pid| {
            let config = LaunchConfig::attach(kind, pid, &tf("Prozess {}", &[&pid.to_string()]));
            panel.start_config(config);
        });
    }

    /// Asks whether to install vscode-js-debug, which the app installs in its pinned version on request (E93).
    fn offer_js_debug(&self) {
        let Some(window) = self.controller().and_then(|c| c.window()) else {
            return;
        };
        let alert = objc2_app_kit::NSAlert::new(self.mtm());
        alert.setMessageText(&crate::util::ns(t("vscode-js-debug installieren?")));
        alert.setInformativeText(&crate::util::ns(t(
            "Zum Debuggen von JavaScript und TypeScript lädt die App vscode-js-debug in fester Version mit Prüfsumme in ihren eigenen Ordner.",
        )));
        alert.addButtonWithTitle(&crate::util::ns(t("Installieren")));
        alert.addButtonWithTitle(&crate::util::ns(t("Abbrechen")));
        let panel = self.id();
        super::sheets::answer(&alert, &window, move |agreed| {
            if agreed {
                std::thread::spawn(move || {
                    let env = crate::environment::wait(ENVIRONMENT_WAIT)
                        .map(<[_]>::to_vec)
                        .unwrap_or_default();
                    let adapters = Adapters::new(Supervisor::global(), env);
                    let done = adapters.install(
                        ide_dap::Install::JsDebug,
                        &ide_lsp::Http::new(),
                        &ide_lsp::Cancel::new(),
                        &mut |_| {},
                    );
                    let words = match done {
                        Ok(_) => t("vscode-js-debug ist installiert. Starte das Debuggen noch einmal.").to_string(),
                        Err(error) => tf(
                            "vscode-js-debug ließ sich nicht installieren: {}",
                            &[&error.to_string()],
                        ),
                    };
                    main_soon(move || with_panel(panel, |p| p.tell(&words)));
                });
            }
        });
    }

    /// Tells the user `text` at the foot of the window.
    fn tell(&self, text: &str) {
        if let Some(controller) = self.controller() {
            controller.show_notice(text);
        }
    }
}

/// `text` in single quotes for the shell.
fn quote(text: &str) -> String {
    format!("'{}'", text.replace('\'', r"'\''"))
}

/// What the model keeps of an adapter's capabilities.
fn caps_of(caps: &ide_dap::Capabilities) -> Caps {
    Caps {
        restart: caps.restart,
        terminate: caps.terminate,
        conditions: caps.conditional_breakpoints,
        hit_conditions: caps.hit_conditional_breakpoints,
        log_points: caps.log_points,
        hover_values: caps.evaluate_for_hovers,
        exception_filters: caps
            .exception_filters
            .iter()
            .map(|f| (f.filter.clone(), f.label.clone(), f.default))
            .collect(),
    }
}

/// The protocol's word for why the program stopped, which the panel says in the user's language.
fn stop_reason(reason: &ide_dap::StopReason) -> &str {
    match reason {
        ide_dap::StopReason::Step => "step",
        ide_dap::StopReason::Breakpoint => "breakpoint",
        ide_dap::StopReason::Exception => "exception",
        ide_dap::StopReason::Pause => "pause",
        ide_dap::StopReason::Entry => "entry",
        ide_dap::StopReason::FunctionBreakpoint => "function breakpoint",
        ide_dap::StopReason::DataBreakpoint => "data breakpoint",
        ide_dap::StopReason::Other(other) => other,
    }
}

/// Why the current file cannot be debugged, in the user's language.
fn current_words(error: &ide_dap::CurrentError) -> String {
    use ide_dap::CurrentError;
    match error {
        CurrentError::Untrusted => {
            t("Debuggen startet Programme des Ordners und geht erst, wenn du ihm vertraust.").into()
        }
        CurrentError::Unsupported(language) => tf(
            "Für {}-Dateien gibt es keinen Debugger ohne Startkonfiguration.",
            &[language],
        ),
        CurrentError::Missing(program) => tf("{} ist nicht installiert.", &[program]),
        CurrentError::NoPackage => t("Die Datei gehört zu keinem Cargo-Paket.").into(),
        CurrentError::WhichBinary(names) => tf(
            "Das Paket hat mehrere Programme ({}); lege in .vscode/launch.json fest, welches.",
            &[&names.join(", ")],
        ),
        CurrentError::Build(errors) => tf("Der Build ist fehlgeschlagen:\n{}", &[errors]),
        CurrentError::Cancelled => t("Der Build wurde abgebrochen.").into(),
        CurrentError::NoProgram => t("Der Build hat kein Programm erzeugt.").into(),
    }
}

/// Why a session did not start, in the user's language, with the command that installs a missing adapter.
fn start_words(error: &StartError) -> String {
    match error {
        StartError::Untrusted => {
            t("Debuggen startet Programme des Ordners und geht erst, wenn du ihm vertraust.").into()
        }
        StartError::NeedsProcess => t("Wähle einen Prozess, an den sich der Debugger anhängt.").into(),
        StartError::Missing(missing) => match (&missing.command, missing.needs) {
            (_, Some(needs)) => tf("{} fehlt; es braucht {}.", &[missing.program, needs]),
            (Some(command), None) => tf("{} fehlt. Installiere es mit: {}", &[missing.program, command]),
            (None, None) => tf("{} fehlt.", &[missing.program]),
        },
        StartError::Build(error) => current_words(error),
        StartError::Io(error) => tf("Der Debug-Adapter konnte nicht starten: {}", &[&error.to_string()]),
    }
}

#[cfg(test)]
mod tests {
    use ide_dap::{CurrentError, Missing, StopReason};

    use super::*;

    #[test]
    fn a_shell_word_keeps_its_quotes() {
        assert_eq!(quote("it's here"), r"'it'\''s here'");
        assert_eq!(quote(""), "''");
    }

    #[test]
    fn the_model_keeps_what_the_panel_needs_of_the_capabilities() {
        let caps = ide_dap::Capabilities {
            restart: true,
            conditional_breakpoints: true,
            log_points: true,
            evaluate_for_hovers: true,
            exception_filters: vec![ide_dap::ExceptionFilter {
                filter: "panic".into(),
                label: "Panics".into(),
                description: None,
                default: true,
                supports_condition: false,
            }],
            ..ide_dap::Capabilities::default()
        };
        assert_eq!(
            caps_of(&caps),
            Caps {
                restart: true,
                terminate: false,
                conditions: true,
                hit_conditions: false,
                log_points: true,
                hover_values: true,
                exception_filters: vec![("panic".into(), "Panics".into(), true)],
            }
        );
        let reasons = [
            StopReason::Step,
            StopReason::Breakpoint,
            StopReason::Exception,
            StopReason::Pause,
            StopReason::Entry,
            StopReason::FunctionBreakpoint,
            StopReason::DataBreakpoint,
            StopReason::Other("goto".into()),
        ];
        let words: Vec<&str> = reasons.iter().map(stop_reason).collect();
        assert_eq!(
            words,
            [
                "step",
                "breakpoint",
                "exception",
                "pause",
                "entry",
                "function breakpoint",
                "data breakpoint",
                "goto"
            ]
        );
    }

    #[test]
    fn why_the_current_program_cannot_be_debugged_is_said_in_the_users_words() {
        let cases = [
            (
                CurrentError::Untrusted,
                "Debuggen startet Programme des Ordners und geht erst, wenn du ihm vertraust.",
            ),
            (
                CurrentError::Unsupported("cpp".into()),
                "Für cpp-Dateien gibt es keinen Debugger ohne Startkonfiguration.",
            ),
            (CurrentError::Missing("cargo"), "cargo ist nicht installiert."),
            (CurrentError::NoPackage, "Die Datei gehört zu keinem Cargo-Paket."),
            (
                CurrentError::WhichBinary(vec!["a".into(), "b".into()]),
                "Das Paket hat mehrere Programme (a, b); lege in .vscode/launch.json fest, welches.",
            ),
            (
                CurrentError::Build("E0425".into()),
                "Der Build ist fehlgeschlagen:\nE0425",
            ),
            (CurrentError::Cancelled, "Der Build wurde abgebrochen."),
            (CurrentError::NoProgram, "Der Build hat kein Programm erzeugt."),
        ];
        for (error, words) in cases {
            assert_eq!(current_words(&error), words);
        }
    }

    #[test]
    fn why_a_session_did_not_start_names_the_missing_adapter_and_how_to_get_it() {
        let missing = |needs, command: Option<&str>| {
            StartError::Missing(Missing {
                kind: ide_dap::AdapterKind::Go,
                program: "dlv",
                needs,
                install: None,
                command: command.map(str::to_string),
            })
        };
        let cases = [
            (
                StartError::Untrusted,
                "Debuggen startet Programme des Ordners und geht erst, wenn du ihm vertraust.".to_string(),
            ),
            (
                StartError::NeedsProcess,
                "Wähle einen Prozess, an den sich der Debugger anhängt.".to_string(),
            ),
            (
                missing(Some("go"), Some("go install")),
                "dlv fehlt; es braucht go.".to_string(),
            ),
            (
                missing(None, Some("go install dlv")),
                "dlv fehlt. Installiere es mit: go install dlv".to_string(),
            ),
            (missing(None, None), "dlv fehlt.".to_string()),
            (
                StartError::Build(CurrentError::NoPackage),
                "Die Datei gehört zu keinem Cargo-Paket.".to_string(),
            ),
            (
                StartError::Io(std::io::Error::other("refused")),
                "Der Debug-Adapter konnte nicht starten: refused".to_string(),
            ),
        ];
        for (error, words) in cases {
            assert_eq!(start_words(&error), words);
        }
    }
}

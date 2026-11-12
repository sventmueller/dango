//! What the debugger of a window decides without AppKit (E94, E98): its sessions with their threads and call stacks,
//! which frame is selected, the breakpoints, the inspector's variables and watches, which commands can run, and what
//! the controller asks the adapter or does in the window when something happens.

pub mod breakpoints;
pub mod console;
pub mod rows;
pub mod session;
pub mod variables;

use std::path::PathBuf;

use ide_core::debugging::{Debugging, ExceptionFilter};

use breakpoints::Breakpoints;
use session::{Frame, Origin, Phase, Session};
use variables::{Node, Path, Variables};

/// A command of the panel's title row and the Debug menu.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Command {
    Continue,
    StepOver,
    StepInto,
    StepOut,
    Restart,
    Stop,
    /// Breakpoints on or off at all (⌘Y).
    ToggleBreakpoints,
    ClearBreakpoints,
}

impl Command {
    /// The controls of the panel's title row, in their order.
    pub const CONTROLS: [Command; 6] = [
        Command::Continue,
        Command::StepOver,
        Command::StepInto,
        Command::StepOut,
        Command::Restart,
        Command::Stop,
    ];
}

/// What the controller does for the model.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Effect {
    /// Ask the session for its threads.
    Threads(u64),
    /// Ask for the call stack of a thread.
    Stack(u64, i64),
    /// Ask for the scopes of a frame, for the inspector's `generation`.
    Scopes { session: u64, frame: i64, generation: u64 },
    /// Ask for the children of the inspector's row at `path`.
    Children {
        session: u64,
        reference: i64,
        path: Path,
        generation: u64,
    },
    /// Evaluate a watch expression in a frame.
    Watch {
        session: u64,
        frame: i64,
        expression: String,
        generation: u64,
    },
    /// Show a place in an editor, as the session stopped there or a frame was chosen.
    Reveal { path: PathBuf, line: usize },
    /// Send the breakpoints of `path` to every running session.
    SendBreakpoints(PathBuf),
    /// Send the exception filters to every running session.
    SendExceptionFilters,
    /// Tell the session to continue, step or stop.
    Run(u64, Command, i64),
    /// Keep what the project keeps of debugging.
    Save,
}

/// What is selected in the panel: a session, one of its threads and a frame of its call stack.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Selection {
    pub session: u64,
    pub thread: Option<i64>,
    pub frame: usize,
}

/// The debugger of a window.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Model {
    pub sessions: Vec<Session>,
    pub selected: Option<Selection>,
    pub breakpoints: Breakpoints,
    pub variables: Variables,
    pub filters: Vec<ExceptionFilter>,
    pub configuration: Option<String>,
    /// The panel's sections the user folded, by name.
    pub folded: Vec<String>,
}

impl Model {
    /// The debugger of a project that kept `debugging`.
    pub fn new(debugging: Debugging) -> Model {
        Model {
            sessions: Vec::new(),
            selected: None,
            breakpoints: Breakpoints::new(debugging.breakpoints, debugging.active),
            variables: Variables::new(debugging.watches),
            filters: debugging.exception_filters,
            configuration: debugging.configuration,
            folded: debugging.folded,
        }
    }

    /// What the project keeps of it.
    pub fn kept(&self) -> Debugging {
        Debugging {
            breakpoints: self.breakpoints.all().to_vec(),
            active: self.breakpoints.active,
            watches: self.variables.expressions(),
            exception_filters: self.filters.clone(),
            configuration: self.configuration.clone(),
            folded: self.folded.clone(),
        }
    }

    pub fn session(&self, id: u64) -> Option<&Session> {
        self.sessions.iter().find(|s| s.id == id)
    }

    fn session_mut(&mut self, id: u64) -> Option<&mut Session> {
        self.sessions.iter_mut().find(|s| s.id == id)
    }

    /// The session commands act on: the selected one, else the newest that has not ended.
    pub fn current(&self) -> Option<&Session> {
        self.selected
            .and_then(|selection| self.session(selection.session))
            .or_else(|| self.sessions.iter().rev().find(|s| !s.is_over()))
    }

    /// Whether `command` can run now: stepping needs a paused session, restarting an adapter that can, stopping a
    /// session that has not ended.
    pub fn can(&self, command: Command) -> bool {
        let session = self.current();
        match command {
            Command::Continue | Command::StepOver | Command::StepInto | Command::StepOut => {
                session.is_some_and(Session::is_paused)
            }
            Command::Restart => session.is_some_and(|s| !s.is_over() && s.caps.restart),
            Command::Stop => session.is_some_and(|s| !s.is_over()),
            Command::ToggleBreakpoints | Command::ClearBreakpoints => !self.breakpoints.is_empty(),
        }
    }

    /// Runs `command` on the current session, in its paused or selected thread.
    pub fn run(&mut self, command: Command) -> Vec<Effect> {
        if !self.can(command) {
            return Vec::new();
        }
        match command {
            Command::ToggleBreakpoints => {
                self.breakpoints.active = !self.breakpoints.active;
                return self.breakpoints_changed(None);
            }
            Command::ClearBreakpoints => {
                let files = self.breakpoints.files();
                self.breakpoints.clear(None);
                let mut effects: Vec<Effect> = files.into_iter().map(Effect::SendBreakpoints).collect();
                effects.push(Effect::Save);
                return effects;
            }
            _ => {}
        }
        let Some(session) = self.current() else {
            return Vec::new();
        };
        let thread = session
            .paused_thread()
            .or_else(|| self.selected.and_then(|s| s.thread))
            .unwrap_or(0);
        vec![Effect::Run(session.id, command, thread)]
    }

    /// The session `from`, started before its adapter gave it a number, takes the number `to`.
    pub fn renumber(&mut self, from: u64, to: u64) {
        if self.session(to).is_some() {
            return;
        }
        if let Some(session) = self.session_mut(from) {
            session.id = to;
        }
        if let Some(selection) = self.selected.as_mut().filter(|s| s.session == from) {
            selection.session = to;
        }
    }

    /// The start of session `id` is still unanswered after a while; whether that is news.
    pub fn still_starting(&mut self, id: u64) -> bool {
        match self.session_mut(id) {
            Some(session) if session.phase == Phase::Starting && !session.waiting => {
                session.waiting = true;
                true
            }
            _ => false,
        }
    }

    /// A session starts.
    pub fn started(&mut self, session: Session) {
        self.selected = Some(Selection {
            session: session.id,
            thread: None,
            frame: 0,
        });
        self.sessions.retain(|s| !s.is_over());
        self.sessions.push(session);
    }

    /// The adapter said what it can do: the exception filters it offers join the list, keeping the user's choices.
    pub fn capabilities(&mut self, id: u64, caps: session::Caps) -> Vec<Effect> {
        for (filter, label, default) in &caps.exception_filters {
            match self.filters.iter_mut().find(|f| &f.id == filter) {
                Some(known) => known.label.clone_from(label),
                None => self.filters.push(ExceptionFilter {
                    id: filter.clone(),
                    label: label.clone(),
                    enabled: *default,
                }),
            }
        }
        if let Some(session) = self.session_mut(id) {
            session.caps = caps;
        }
        vec![Effect::Save]
    }

    /// The exception filter ids sessions get.
    pub fn enabled_filters(&self) -> Vec<String> {
        self.filters
            .iter()
            .filter(|f| f.enabled)
            .map(|f| f.id.clone())
            .collect()
    }

    /// The user switches the exception filter `id`.
    pub fn set_filter(&mut self, id: &str, enabled: bool) -> Vec<Effect> {
        match self.filters.iter_mut().find(|f| f.id == id) {
            Some(filter) if filter.enabled != enabled => {
                filter.enabled = enabled;
                vec![Effect::SendExceptionFilters, Effect::Save]
            }
            _ => Vec::new(),
        }
    }

    /// The session stopped in `thread`: it and its call stack are selected, and the adapter is asked for both.
    pub fn stopped(&mut self, id: u64, thread: i64, reason: &str) -> Vec<Effect> {
        let Some(session) = self.session_mut(id) else {
            return Vec::new();
        };
        session.phase = Phase::Paused {
            thread,
            reason: reason.to_string(),
        };
        session.waiting = false;
        session.forget_frames();
        self.selected = Some(Selection {
            session: id,
            thread: Some(thread),
            frame: 0,
        });
        self.variables.show(None);
        vec![Effect::Threads(id), Effect::Stack(id, thread)]
    }

    /// The program runs on: call stacks and variables are gone until it stops again.
    pub fn continued(&mut self, id: u64) {
        if let Some(session) = self.session_mut(id)
            && !session.is_over()
        {
            session.phase = Phase::Running;
            session.waiting = false;
            session.forget_frames();
        }
        if self.selected.is_some_and(|s| s.session == id) {
            self.variables.show(None);
        }
    }

    /// The session ended, with the program's exit code or why.
    pub fn ended(&mut self, id: u64, code: Option<i64>, problem: Option<String>) {
        if let Some(session) = self.session_mut(id) {
            session.phase = Phase::Ended { code, problem };
            session.forget_frames();
        }
        if self.selected.is_some_and(|s| s.session == id) {
            self.variables.show(None);
        }
    }

    pub fn threads(&mut self, id: u64, threads: Vec<(i64, String)>) {
        if let Some(session) = self.session_mut(id) {
            session.set_threads(threads);
        }
    }

    /// The call stack of `thread` came; for the selected thread its frame is shown.
    pub fn stack(&mut self, id: u64, thread: i64, frames: Vec<Frame>) -> Vec<Effect> {
        let Some(session) = self.session_mut(id) else {
            return Vec::new();
        };
        if !session.is_paused() {
            return Vec::new();
        }
        match session.thread_mut(thread) {
            Some(known) => known.frames = Some(frames),
            None => session.threads.push(session::Thread {
                id: thread,
                name: format!("{thread}"),
                frames: Some(frames),
            }),
        }
        match self.selected {
            Some(selection) if selection.session == id && selection.thread == Some(thread) => {
                self.select(id, thread, selection.frame)
            }
            _ => Vec::new(),
        }
    }

    /// The user chose a thread, or a frame of its call stack: its place shows in the editor, its variables in the
    /// inspector, and a thread whose stack is not known yet is asked for it.
    pub fn select(&mut self, id: u64, thread: i64, frame: usize) -> Vec<Effect> {
        self.selected = Some(Selection {
            session: id,
            thread: Some(thread),
            frame,
        });
        let Some(session) = self.session(id).filter(|s| s.is_paused()) else {
            return Vec::new();
        };
        let Some(frames) = session.thread(thread).and_then(|t| t.frames.as_ref()) else {
            return vec![Effect::Stack(id, thread)];
        };
        let Some(chosen) = frames.get(frame).or_else(|| frames.first()).cloned() else {
            return Vec::new();
        };
        let generation = self.variables.show(Some((id, chosen.id)));
        let mut effects = Vec::new();
        if let Some(path) = chosen.path.clone() {
            effects.push(Effect::Reveal {
                path,
                line: chosen.line,
            });
        }
        effects.push(Effect::Scopes {
            session: id,
            frame: chosen.id,
            generation,
        });
        effects.extend(self.variables.watches.iter().map(|watch| Effect::Watch {
            session: id,
            frame: chosen.id,
            expression: watch.expression.clone(),
            generation,
        }));
        effects
    }

    /// The selected frame, while its session is paused.
    pub fn frame(&self) -> Option<(u64, &Frame)> {
        let selection = self.selected?;
        let session = self.session(selection.session).filter(|s| s.is_paused())?;
        let frames = session.thread(selection.thread?)?.frames.as_ref()?;
        Some((session.id, frames.get(selection.frame)?))
    }

    /// The innermost frame of the thread the current session stopped in, where the editor draws its arrow.
    pub fn paused_at(&self) -> Option<&Frame> {
        let session = self.current()?;
        session.thread(session.paused_thread()?)?.frames.as_ref()?.first()
    }

    /// The scopes came: the first one opens, as its variables are what the user wants to see.
    pub fn scopes(&mut self, generation: u64, scopes: Vec<Node>) -> Vec<Effect> {
        let first = scopes.first().map(|scope| scope.name.clone());
        if !self.variables.set_scopes(generation, scopes) {
            return Vec::new();
        }
        first
            .map(|name| self.open(&Path::Scope(vec![name])))
            .unwrap_or_default()
    }

    /// A row of the inspector opens: its children are asked for once.
    pub fn open(&mut self, path: &Path) -> Vec<Effect> {
        let (Some(reference), Some((session, _))) = (self.variables.wanted(path), self.variables.frame) else {
            return Vec::new();
        };
        vec![Effect::Children {
            session,
            reference,
            path: path.clone(),
            generation: self.variables.generation,
        }]
    }

    /// Adds a watch expression, evaluated at once while a frame is selected.
    pub fn add_watch(&mut self, expression: &str) -> Vec<Effect> {
        let Some(index) = self.variables.add_watch(expression) else {
            return Vec::new();
        };
        let mut effects = vec![Effect::Save];
        if let Some((session, frame)) = self.variables.frame {
            effects.push(Effect::Watch {
                session,
                frame,
                expression: self.variables.watches[index].expression.clone(),
                generation: self.variables.generation,
            });
        }
        effects
    }

    pub fn remove_watch(&mut self, index: usize) -> Vec<Effect> {
        if self.variables.remove_watch(index) {
            vec![Effect::Save]
        } else {
            Vec::new()
        }
    }

    /// Something about breakpoints changed: the files they are in, or all with `None`, go to the sessions again.
    pub fn breakpoints_changed(&self, path: Option<PathBuf>) -> Vec<Effect> {
        let mut effects: Vec<Effect> = match path {
            Some(path) => vec![Effect::SendBreakpoints(path)],
            None => self
                .breakpoints
                .files()
                .into_iter()
                .map(Effect::SendBreakpoints)
                .collect(),
        };
        effects.push(Effect::Save);
        effects
    }

    /// Whether the panel section `name` is folded.
    pub fn is_folded(&self, name: &str) -> bool {
        self.folded.iter().any(|f| f == name)
    }

    /// The user folds or opens the section `name`.
    pub fn fold(&mut self, name: &str, folded: bool) -> Vec<Effect> {
        if self.is_folded(name) == folded {
            return Vec::new();
        }
        if folded {
            self.folded.push(name.to_string());
        } else {
            self.folded.retain(|f| f != name);
        }
        vec![Effect::Save]
    }
}

/// Whether a session started for a program or attached to one says "Stopp" or "Trennen".
pub fn stops_by_detaching(model: &Model) -> bool {
    model.current().is_some_and(|s| s.origin == Origin::Attached)
}

#[cfg(test)]
mod tests;

//! The controller of the debugger: carries out what the model returns, by asking the session's adapter through
//! `ide_dap` and by acting in the window, and hands the answers, which arrive on other threads, back to the model on
//! the main thread by the panel's id (Review C11).

use std::path::{Path, PathBuf};
use std::rc::Rc;

use ide_dap::{EvaluateContext, FrameId, Session, SourceBreakpoint, ThreadId, VariablesRef};
use objc2::{DefinedClass, MainThreadOnly};
use splat_i18n::tf;

use super::console::ConsoleTab;
use super::model::variables::Node;
use super::model::{Command, Effect};
use super::panel::{DebugPanel, with_panel};
use crate::util::main_soon;

/// What happens with the process the user picks to attach to.
pub type Pick = Box<dyn FnOnce(&DebugPanel, u32)>;

/// A running session of the window, with the session it belongs to if the adapter started it beneath another.
pub struct Link {
    pub id: u64,
    pub session: Session,
    pub parent: Option<u64>,
}

/// What the debugger of a window holds besides its model: the running sessions, the console tabs, the configurations
/// of `launch.json` and the start under way.
#[derive(Default)]
pub struct Links {
    pub sessions: Vec<Link>,
    pub consoles: Vec<Rc<ConsoleTab>>,
    pub configurations: Vec<ide_dap::Entry>,
    /// Why `launch.json` could not be read, if there is one that could not.
    pub configuration_problem: Option<String>,
    /// The name and kind of the session that is being started, for its first events.
    pub starting: Option<(String, super::model::session::Origin)>,
    /// Cancels a build that a start waits for.
    pub cancel: Option<ide_lsp::Cancel>,
    /// The user stopped the start under way before it returned a session, which then stops at once.
    pub stop_pending: bool,
    /// What happens with the process the user picks to attach to.
    pub picking: Option<Pick>,
}

impl DebugPanel {
    /// The handle of the session `id`.
    pub fn session(&self, id: u64) -> Option<Session> {
        let links = self.ivars().links.borrow();
        links.sessions.iter().find(|l| l.id == id).map(|l| l.session.clone())
    }

    /// The sessions that are not beneath another; the adapter passes their breakpoints on to the children.
    fn top_sessions(&self) -> Vec<Session> {
        let links = self.ivars().links.borrow();
        links
            .sessions
            .iter()
            .filter(|l| l.parent.is_none())
            .map(|l| l.session.clone())
            .collect()
    }

    /// Carries out `effects` and shows the model again.
    pub fn apply(&self, effects: Vec<Effect>) {
        for effect in effects {
            self.carry_out(effect);
        }
        self.refresh();
    }

    fn carry_out(&self, effect: Effect) {
        match effect {
            Effect::Threads(id) => self.ask_threads(id),
            Effect::Stack(id, thread) => self.ask_stack(id, thread),
            Effect::Scopes {
                session,
                frame,
                generation,
            } => self.ask_scopes(session, frame, generation),
            Effect::Children {
                session,
                reference,
                path,
                generation,
            } => self.ask_children(session, reference, path, generation),
            Effect::Watch {
                session,
                frame,
                expression,
                generation,
            } => self.ask_watch(session, frame, expression, generation),
            Effect::Reveal { path, line } => self.reveal(&path, line),
            Effect::SendBreakpoints(path) => self.send_breakpoints(&path),
            Effect::SendExceptionFilters => {
                let filters = self.read().enabled_filters();
                for session in self.top_sessions() {
                    session.set_exception_filters(filters.clone(), Box::new(|_| {}));
                }
            }
            Effect::Run(id, command, thread) => self.run_on(id, command, thread),
            Effect::Save => self.save(),
        }
    }

    /// Keeps what the project keeps of debugging, unless the app is quitting (E64: nothing is written from then on).
    fn save(&self) {
        if crate::session::is_quitting() {
            return;
        }
        let kept = self.read().kept();
        if let Err(error) = ide_core::debugging::save(self.root(), &kept) {
            crate::diag::report("saving breakpoints", error);
        }
    }

    /// The breakpoints of `path` as the adapter takes them, lines from 1.
    pub fn source_breakpoints(&self, path: &Path) -> Vec<SourceBreakpoint> {
        self.read()
            .breakpoints
            .for_adapter(path)
            .into_iter()
            .map(|b| SourceBreakpoint {
                line: u32::try_from(b.line + 1).unwrap_or(u32::MAX),
                column: None,
                condition: b.condition,
                hit_condition: b.hit_condition,
                log_message: b.log_message,
            })
            .collect()
    }

    fn send_breakpoints(&self, path: &Path) {
        let breakpoints = self.source_breakpoints(path);
        for session in self.top_sessions() {
            session.set_breakpoints(path, breakpoints.clone(), Box::new(|_| {}));
        }
    }

    fn run_on(&self, id: u64, command: Command, thread: i64) {
        let Some(session) = self.session(id) else {
            if command == Command::Stop {
                self.stop_pending_start(id);
            }
            return;
        };
        let thread = ThreadId(thread);
        let panel = self.id();
        let failed = move |result: Result<(), ide_dap::Error>| {
            if let Err(error) = result {
                main_soon(move || with_panel(panel, |p| p.note_failure(id, &error.to_string())));
            }
        };
        match command {
            Command::Continue => session.resume(thread, Box::new(failed)),
            Command::StepOver => session.next(thread, Box::new(failed)),
            Command::StepInto => session.step_in(thread, Box::new(failed)),
            Command::StepOut => session.step_out(thread, Box::new(failed)),
            Command::Restart => session.restart(Box::new(failed)),
            Command::Stop => session.stop(),
            Command::ToggleBreakpoints | Command::ClearBreakpoints => {}
        }
    }

    /// A request failed: the console of its session says so.
    fn note_failure(&self, id: u64, why: &str) {
        if let Some(console) = self.console_for(id) {
            console.push(ide_core::debugging::ConsoleKind::Error, &format!("{why}\n"));
        }
    }

    fn ask_threads(&self, id: u64) {
        let Some(session) = self.session(id) else { return };
        let panel = self.id();
        session.threads(Box::new(move |answer| {
            let Ok(threads) = answer else { return };
            let threads: Vec<(i64, String)> = threads.into_iter().map(|t| (t.id.0, t.name)).collect();
            main_soon(move || {
                with_panel(panel, |p| {
                    p.model().threads(id, threads);
                    p.refresh();
                });
            });
        }));
    }

    fn ask_stack(&self, id: u64, thread: i64) {
        let Some(session) = self.session(id) else { return };
        let panel = self.id();
        session.stack_trace(
            ThreadId(thread),
            Box::new(move |answer| {
                let Ok(frames) = answer else { return };
                let frames: Vec<super::model::session::Frame> = frames
                    .into_iter()
                    .map(|f| super::model::session::Frame {
                        id: f.id.0,
                        name: f.name,
                        path: f.source.and_then(|s| s.path),
                        line: usize::try_from(f.line.saturating_sub(1)).unwrap_or(0),
                        column: usize::try_from(f.column.saturating_sub(1)).unwrap_or(0),
                    })
                    .collect();
                main_soon(move || with_panel(panel, |p| p.stack_came(id, thread, frames)));
            }),
        );
    }

    fn stack_came(&self, id: u64, thread: i64, frames: Vec<super::model::session::Frame>) {
        let paused_here = self.read().session(id).and_then(|s| s.paused_thread()) == Some(thread);
        let effects = self.model().stack(id, thread, frames);
        self.apply(effects);
        if paused_here {
            self.select_paused_frame();
            self.announce_stop();
        }
    }

    /// Says where the program stopped, for VoiceOver (E101).
    fn announce_stop(&self) {
        let words = {
            let model = self.read();
            let Some(frame) = model.paused_at() else { return };
            let place = frame
                .path
                .as_deref()
                .map(|p| super::rows::place(p, frame.line))
                .unwrap_or_else(|| frame.name.clone());
            let reason = model.current().map(super::rows::phase_words).unwrap_or_default();
            tf("{} in {}", &[&reason, &place])
        };
        if let Some(controller) = self.controller() {
            crate::window::announce(controller.mtm(), &words);
        }
    }

    fn ask_scopes(&self, id: u64, frame: i64, generation: u64) {
        let Some(session) = self.session(id) else { return };
        let panel = self.id();
        session.scopes(
            FrameId(frame),
            Box::new(move |answer| {
                let Ok(scopes) = answer else { return };
                let nodes: Vec<Node> = scopes
                    .into_iter()
                    .map(|s| Node::new(s.name, "", s.variables.0))
                    .collect();
                main_soon(move || {
                    with_panel(panel, |p| {
                        let effects = p.model().scopes(generation, nodes);
                        p.apply(effects);
                    });
                });
            }),
        );
    }

    fn ask_children(&self, id: u64, reference: i64, path: super::model::variables::Path, generation: u64) {
        let Some(session) = self.session(id) else { return };
        let panel = self.id();
        session.variables(
            VariablesRef(reference),
            Box::new(move |answer| {
                let Ok(variables) = answer else { return };
                let nodes: Vec<Node> = variables
                    .into_iter()
                    .map(|v| {
                        let mut node = Node::new(v.name, v.value, v.children.0);
                        node.kind = v.type_name;
                        node
                    })
                    .collect();
                main_soon(move || {
                    with_panel(panel, |p| {
                        if p.model().variables.set_children(generation, &path, nodes) {
                            p.refresh();
                        }
                    });
                });
            }),
        );
    }

    fn ask_watch(&self, id: u64, frame: i64, expression: String, generation: u64) {
        let Some(session) = self.session(id) else { return };
        let panel = self.id();
        let asked = expression.clone();
        session.evaluate(
            &asked,
            Some(FrameId(frame)),
            EvaluateContext::Watch,
            Box::new(move |answer| {
                let value = answer
                    .map(|e| {
                        let mut node = Node::new("", e.result, e.children.0);
                        node.kind = e.type_name;
                        node
                    })
                    .map_err(|e| e.to_string());
                main_soon(move || {
                    with_panel(panel, |p| {
                        if p.model().variables.set_watch(generation, &expression, value) {
                            p.refresh();
                        }
                    });
                });
            }),
        );
    }

    /// Evaluates `expression` typed into the console of session `id`, in its selected frame while it is paused.
    pub fn evaluate_in_console(&self, id: u64, expression: &str) {
        let Some(session) = self.session(id) else { return };
        let frame = self
            .read()
            .frame()
            .filter(|(session, _)| *session == id)
            .map(|(_, frame)| FrameId(frame.id));
        let panel = self.id();
        session.evaluate(
            expression,
            frame,
            EvaluateContext::Repl,
            Box::new(move |answer| {
                main_soon(move || {
                    with_panel(panel, |p| {
                        if let Some(console) = p.console_for(id) {
                            match answer {
                                Ok(value) => console.push(ide_core::debugging::ConsoleKind::Result, &value.result),
                                Err(error) => console.push(ide_core::debugging::ConsoleKind::Error, &error.to_string()),
                            }
                        }
                    });
                });
            }),
        );
    }

    /// Evaluates `expression` for the hover in the selected frame; `then` gets its value on the main thread. Whether
    /// it asked.
    pub fn evaluate_for_hover(&self, expression: &str, then: impl FnOnce(String) + Send + 'static) -> bool {
        let Some((id, frame)) = self.read().frame().map(|(id, frame)| (id, frame.id)) else {
            return false;
        };
        let Some(session) = self.session(id) else { return false };
        session.evaluate(
            expression,
            Some(FrameId(frame)),
            EvaluateContext::Hover,
            Box::new(move |answer| {
                if let Ok(value) = answer {
                    main_soon(move || then(value.result));
                }
            }),
        );
        true
    }

    /// Shows `line` of `path` in an editor, as the session stopped there or the user chose a frame.
    fn reveal(&self, path: &Path, line: usize) {
        let Some(controller) = self.controller() else { return };
        if !path.is_file() {
            return;
        }
        controller.open_file(path);
        // The keyboard focus stays where it is, such as in the list of frames the user goes through.
        let Some(editor) = crate::editor::editor_for(&controller, path) else {
            return;
        };
        let at = {
            let document = editor.document();
            let text = document.text();
            let byte = text.offset(line, 0, ide_document::Unit::Utf16).ok();
            byte.and_then(|byte| text.byte_to_utf16(byte).ok())
        };
        if let Some(at) = at {
            editor.setSelectedRange(objc2_foundation::NSRange::new(at, 0));
            editor.scrollRangeToVisible(objc2_foundation::NSRange::new(at, 0));
        }
    }

    /// The console session `id` writes to: its own, or that of the session it runs beneath.
    pub fn console_for(&self, id: u64) -> Option<Rc<ConsoleTab>> {
        if let Some(console) = self.console_of(id) {
            return Some(console);
        }
        let parent = self
            .ivars()
            .links
            .borrow()
            .sessions
            .iter()
            .find(|l| l.id == id)
            .and_then(|l| l.parent)?;
        self.console_for(parent)
    }

    /// The files with breakpoints, each with what the adapter gets.
    pub fn setup(&self) -> ide_dap::Setup {
        let files: Vec<PathBuf> = self.read().breakpoints.files();
        let mut setup = ide_dap::Setup::default();
        for path in files {
            let breakpoints = self.source_breakpoints(&path);
            if !breakpoints.is_empty() {
                setup.breakpoints.insert(path, breakpoints);
            }
        }
        let model = self.read();
        if !model.filters.is_empty() {
            setup.exception_filters = Some(model.enabled_filters());
        }
        setup
    }

    /// Stops every session of the window, as it closes.
    pub fn end_all(&self) {
        let sessions: Vec<Session> = self
            .ivars()
            .links
            .borrow()
            .sessions
            .iter()
            .map(|l| l.session.clone())
            .collect();
        for session in sessions {
            session.stop();
        }
        if let Some(cancel) = self.ivars().links.borrow_mut().cancel.take() {
            cancel.cancel();
        }
    }
}

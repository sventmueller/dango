//! What the self-test reads from and does with a window's debugger, through the code its controls run.

use std::path::{Path, PathBuf};

use objc2::DefinedClass;
use objc2::rc::Retained;
use objc2_app_kit::{NSOutlineView, NSView};

pub use super::model::Command;
use super::model::breakpoints::Look;
use super::model::variables::Path as VariablePath;
use super::panel::{self, DebugPanel};
use crate::editor::EditorTextView;
use crate::window::WindowController;

/// What the debugger of a window shows.
#[derive(Debug, Default)]
pub struct Seen {
    /// Whether each control of the title row is enabled, in the order of `Command::CONTROLS`.
    pub controls: Vec<bool>,
    /// Each breakpoint's file, line from 0 and whether it is enabled.
    pub breakpoints: Vec<(PathBuf, usize, bool)>,
    /// Each session's name and how it stands.
    pub sessions: Vec<(String, String)>,
    /// The frames of the stopped thread, innermost first.
    pub frames: Vec<String>,
    /// The paused place, line from 0.
    pub paused: Option<(PathBuf, usize)>,
    /// The innermost scope's variables with their values.
    pub locals: Vec<(String, String)>,
    /// Each watch expression and its value or error.
    pub watches: Vec<(String, Option<String>)>,
    /// Each console's title, text, and whether it takes input.
    pub consoles: Vec<(String, String, bool)>,
    pub configurations: Vec<String>,
}

pub fn of(controller: &WindowController) -> Option<Retained<DebugPanel>> {
    panel::of(controller)
}

pub fn seen(controller: &WindowController) -> Option<Seen> {
    let panel = of(controller)?;
    let ui = panel.ui();
    let model = panel.read();
    let paused_frames = model
        .current()
        .and_then(|s| s.thread(s.paused_thread()?))
        .and_then(|t| t.frames.clone())
        .unwrap_or_default();
    let consoles = panel.ivars().links.borrow().consoles.clone();
    Some(Seen {
        controls: ui.controls.iter().map(|b| b.isEnabled()).collect(),
        breakpoints: model
            .breakpoints
            .all()
            .iter()
            .map(|b| (b.path.clone(), b.line, b.enabled))
            .collect(),
        sessions: model
            .sessions
            .iter()
            .map(|s| (s.name.clone(), super::rows::phase_words(s)))
            .collect(),
        frames: paused_frames.iter().map(|f| f.name.clone()).collect(),
        paused: model.paused_at().and_then(|f| Some((f.path.clone()?, f.line))),
        locals: model.variables.locals(),
        watches: model
            .variables
            .watches
            .iter()
            .map(|w| {
                let value = w.value.as_ref().map(|v| match v {
                    Ok(node) => node.value.clone(),
                    Err(error) => format!("error: {error}"),
                });
                (w.expression.clone(), value)
            })
            .collect(),
        consoles: consoles
            .iter()
            .map(|c| (c.title.borrow().clone(), c.shown_text(), c.takes_input()))
            .collect(),
        configurations: ui.configurations.itemTitles().iter().map(|t| t.to_string()).collect(),
    })
}

/// Starts a session on the scripted adapter `script`, as the start button starts one on a real adapter.
pub fn start_fake(controller: &WindowController, script: ide_dap::fake::Script) {
    let Some(panel) = of(controller) else { return };
    let config = script.launch_config();
    let name = script
        .path
        .file_name()
        .map_or_else(String::new, |n| n.to_string_lossy().into_owned());
    panel.launch(
        name,
        super::model::session::Origin::Launched,
        move |setup, _, _, events| {
            ide_dap::Session::start(
                ide_process::Supervisor::global(),
                ide_dap::Adapter::fake(script),
                config,
                setup,
                events,
            )
            .map_err(|e| e.to_string())
        },
    );
}

pub fn run(controller: &WindowController, command: Command) {
    if let Some(panel) = of(controller) {
        panel.run_command(command);
    }
}

pub fn toggle_breakpoint(controller: &WindowController, path: &Path, line: usize) {
    if let Some(panel) = of(controller) {
        panel.toggle_breakpoint(path, line);
    }
}

pub fn switch_breakpoint(controller: &WindowController, path: &Path, line: usize, on: bool) {
    if let Some(panel) = of(controller) {
        panel.change_breakpoint(path, line, super::model::breakpoints::Edit::Enable(on));
    }
}

/// Types `expression` into the inspector's field and presses Return.
pub fn add_watch(controller: &WindowController, expression: &str) {
    if let Some(panel) = of(controller) {
        let field = panel.ui().inspector.watch.clone();
        field.setStringValue(&crate::util::ns(expression));
        panel.add_watch(&field.stringValue().to_string());
        field.setStringValue(&crate::util::ns(""));
    }
}

/// Opens the inspector's row of the innermost scope's variable `name`, as a click on its triangle does.
pub fn open_variable(controller: &WindowController, name: &str) {
    let Some(panel) = of(controller) else { return };
    let scope = panel
        .read()
        .variables
        .scopes
        .as_ref()
        .and_then(|s| s.first().map(|s| s.name.clone()));
    let Some(scope) = scope else { return };
    let path = VariablePath::Scope(vec![scope, name.to_string()]);
    let item = panel.inspector_item(&super::model::variables::Row::Node(path));
    // SAFETY: The item is one of the panel's row objects, which the inspector's list knows.
    unsafe { panel.ui().inspector.outline.expandItem(Some(&item)) };
}

/// The children of the innermost scope's variable `name`, once they came.
pub fn children_of(controller: &WindowController, name: &str) -> Vec<String> {
    let Some(panel) = of(controller) else { return Vec::new() };
    let model = panel.read();
    let Some(scope) = model
        .variables
        .scopes
        .as_ref()
        .and_then(|s| s.first().map(|s| s.name.clone()))
    else {
        return Vec::new();
    };
    let path = VariablePath::Scope(vec![scope, name.to_string()]);
    model
        .variables
        .children(Some(&path))
        .into_iter()
        .filter_map(|p| model.variables.node(&p).map(|n| format!("{} = {}", n.name, n.value)))
        .collect()
}

/// Types `expression` into the newest console and presses Return.
pub fn evaluate(controller: &WindowController, expression: &str) {
    let Some(panel) = of(controller) else { return };
    let console = panel.ivars().links.borrow().consoles.last().cloned();
    let Some(console) = console else { return };
    if let Some(asked) = console.ask(expression)
        && let Some(session) = console.session.get()
    {
        panel.evaluate_in_console(session, &asked);
    }
}

pub fn outline(controller: &WindowController) -> Option<Retained<NSOutlineView>> {
    Some(of(controller)?.ui().outline.clone())
}

pub fn inspector_page(controller: &WindowController) -> Option<Retained<NSView>> {
    Some(of(controller)?.ui().inspector.page.clone())
}

/// The breakpoint of `line` of `editor` and whether the program is paused there, as the gutter draws them.
pub fn gutter(editor: &EditorTextView, line: usize) -> (Option<Look>, bool) {
    super::gutter::mark(editor, line)
}

/// The values at the line ends of `editor`.
pub fn inline_values(editor: &EditorTextView) -> Vec<String> {
    crate::editor::insets::of(editor, crate::editor::insets::Layer::Debug)
        .into_iter()
        .map(|inset| inset.text)
        .collect()
}

/// The labels of `editor`'s rotors and the entries of its "Haltepunkte" rotor.
pub fn rotors(editor: &EditorTextView) -> (Vec<String>, Vec<String>) {
    (
        super::rotor::labels(editor),
        super::rotor::items(editor)
            .into_iter()
            .map(|(_, label)| label)
            .collect(),
    )
}

/// What the hover popover would say about the expression at UTF-16 offset `index` of `editor`.
pub fn expression_at(editor: &EditorTextView, index: usize) -> Option<String> {
    super::editor::expression_at(editor, index).map(|(text, _)| text)
}

/// Asks the hover for `index` of `editor` as a resting mouse does; whether the debugger took it.
pub fn hover(editor: &EditorTextView, index: usize) -> bool {
    super::editor::hover(editor, index)
}

/// Whether `look` is drawn filled or outlined.
pub fn is_outlined(look: Option<Look>) -> bool {
    look == Some(Look::Disabled)
}

/// Gives the breakpoint at `line` of `path` the condition `condition`, as its context menu's sheet does.
pub fn set_condition(controller: &WindowController, path: &Path, line: usize, condition: &str) {
    if let Some(panel) = of(controller) {
        panel.change_breakpoint(
            path,
            line,
            super::model::breakpoints::Edit::Condition(Some(condition.to_string())),
        );
    }
}

/// The condition of the breakpoint at `line` of `path`.
pub fn condition(controller: &WindowController, path: &Path, line: usize) -> Option<String> {
    of(controller)?.read().breakpoints.at(path, line)?.condition.clone()
}

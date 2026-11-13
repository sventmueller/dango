//! What the self-test reads from and does with a window's debugger, through the code its controls run.

use std::path::{Path, PathBuf};

use objc2::AnyThread;
use objc2::DefinedClass;
use objc2::rc::Retained;
use objc2_app_kit::{NSMenu, NSOutlineView, NSView};

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

/// The configuration menu's entries with whether each can be chosen, separators left out.
pub fn configuration_items(controller: &WindowController) -> Vec<(String, bool)> {
    let Some(panel) = of(controller) else { return Vec::new() };
    let Some(menu) = panel.ui().configurations.menu() else {
        return Vec::new();
    };
    menu.itemArray()
        .iter()
        .filter(|item| !item.isSeparatorItem())
        .map(|item| (item.title().to_string(), item.isEnabled()))
        .collect()
}

/// Picks the configuration `title` in the menu and presses the start button, as the user does; whether it is there.
pub fn start_configuration(controller: &WindowController, title: &str) -> bool {
    let Some(panel) = of(controller) else { return false };
    let ui = panel.ui();
    let popup = &ui.configurations;
    popup.selectItemWithTitle(&crate::util::ns(title));
    if popup.titleOfSelectedItem().is_none_or(|t| t.to_string() != title) {
        return false;
    }
    // SAFETY: The popup's action is the panel's `ideDebugChoose:`, which takes any sender.
    unsafe { popup.sendAction_to(popup.action(), popup.target().as_deref()) };
    // SAFETY: A click without a sender, as the button sends its own action.
    unsafe { ui.start.performClick(None) };
    true
}

/// The configuration the panel keeps as chosen.
pub fn chosen_configuration(controller: &WindowController) -> Option<String> {
    of(controller)?.read().configuration.clone()
}

/// Whether the panel waits for the user to pick a process.
pub fn is_picking(controller: &WindowController) -> bool {
    of(controller).is_some_and(|p| p.ivars().links.borrow().picking.is_some())
}

/// Each exception filter with whether it is on.
pub fn filters(controller: &WindowController) -> Vec<(String, bool)> {
    of(controller).map_or_else(Vec::new, |panel| {
        panel.read().filters.iter().map(|f| (f.id.clone(), f.enabled)).collect()
    })
}

/// The row of the panel's list that shows `key`, once the list holds it.
fn row(panel: &DebugPanel, key: &super::model::rows::Key) -> Option<isize> {
    let item = panel.panel_item(key);
    // SAFETY: The item is one of the panel's row objects.
    let row = unsafe { panel.ui().outline.rowForItem(Some(&item)) };
    (row >= 0).then_some(row)
}

/// The row of the breakpoint at `line` of `path` in the panel's list.
pub fn breakpoint_row(controller: &WindowController, path: &Path, line: usize) -> Option<isize> {
    let panel = of(controller)?;
    row(&panel, &super::model::rows::Key::Breakpoint(path.to_path_buf(), line))
}

/// The row of the exception filter `id` in the panel's list.
pub fn filter_row(controller: &WindowController, id: &str) -> Option<isize> {
    let panel = of(controller)?;
    row(&panel, &super::model::rows::Key::Filter(id.to_string()))
}

/// The row of the breakpoints' section heading.
pub fn breakpoints_section_row(controller: &WindowController) -> Option<isize> {
    let key = super::model::rows::Key::Section(super::model::rows::Section::Breakpoints);
    let panel = of(controller)?;
    row(&panel, &key)
}

/// The row of frame `index` of the paused thread of the current session.
pub fn frame_row(controller: &WindowController, index: usize) -> Option<isize> {
    let panel = of(controller)?;
    let key = {
        let model = panel.read();
        let session = model.current()?;
        super::model::rows::Key::Frame(session.id, session.paused_thread()?, index)
    };
    row(&panel, &key)
}

/// Clicks the checkbox in row `row` of the panel's list; whether the row has one.
pub fn click_switch(controller: &WindowController, row: isize) -> bool {
    let Some(panel) = of(controller) else { return false };
    let view = panel.ui().outline.viewAtColumn_row_makeIfNecessary(0, row, true);
    match view.and_then(|v| v.downcast::<objc2_app_kit::NSButton>().ok()) {
        Some(button) => {
            // SAFETY: A click without a sender, as the checkbox sends its own action.
            unsafe { button.performClick(None) };
            true
        }
        None => false,
    }
}

/// Whether row `row` of the panel's list stands open.
pub fn is_open(controller: &WindowController, row: isize) -> bool {
    let Some(panel) = of(controller) else { return false };
    let outline = &panel.ui().outline;
    let item = outline.itemAtRow(row);
    // SAFETY: The item came from this outline view.
    item.is_some_and(|item| unsafe { outline.isItemExpanded(Some(&item)) })
}

/// Return on row `row` of the panel's list.
pub fn activate(controller: &WindowController, row: isize) {
    if let Some(panel) = of(controller) {
        let outline = panel.ui().outline.clone();
        panel.activate_in(&outline, row);
    }
}

/// Selects row `row` of the panel's list, as a click does.
pub fn select_row(controller: &WindowController, row: isize) {
    if let Some(panel) = of(controller)
        && let Ok(index) = usize::try_from(row)
    {
        let set = objc2_foundation::NSIndexSet::indexSetWithIndex(index);
        panel.ui().outline.selectRowIndexes_byExtendingSelection(&set, false);
    }
}

/// The paused session's selected thread and frame.
pub fn selected_frame(controller: &WindowController) -> Option<(i64, usize)> {
    let selection = of(controller)?.read().selected?;
    Some((selection.thread?, selection.frame))
}

/// Delete on row `row` of the panel's list, or of the inspector's; whether it removed something.
pub fn delete_row(controller: &WindowController, inspector: bool, row: isize) -> bool {
    let Some(panel) = of(controller) else { return false };
    let ui = panel.ui();
    let view = if inspector { &ui.inspector.outline } else { &ui.outline }.clone();
    panel.delete_in(&view, row)
}

/// The row of watch expression `index` in the inspector.
pub fn watch_row(controller: &WindowController, index: usize) -> Option<isize> {
    let panel = of(controller)?;
    let item = panel.inspector_item(&super::model::variables::Row::Node(VariablePath::Watch(
        index,
        Vec::new(),
    )));
    // SAFETY: The item is one of the panel's row objects.
    let row = unsafe { panel.ui().inspector.outline.rowForItem(Some(&item)) };
    (row >= 0).then_some(row)
}

/// Types `expression` into the inspector's field and sends its action, as Return does.
pub fn type_watch(controller: &WindowController, expression: &str) {
    if let Some(panel) = of(controller) {
        let field = panel.ui().inspector.watch.clone();
        field.setStringValue(&crate::util::ns(expression));
        // SAFETY: The field's action is the panel's `ideDebugAddWatch:`, which takes the field as its sender.
        unsafe { field.sendAction_to(field.action(), field.target().as_deref()) };
    }
}

/// Types `expression` into the newest console's field and sends its action, as Return does.
pub fn type_in_console(controller: &WindowController, expression: &str) {
    let Some(panel) = of(controller) else { return };
    let console = panel.ivars().links.borrow().consoles.last().cloned();
    if let Some(console) = console {
        let field = console.input_field();
        field.setStringValue(&crate::util::ns(expression));
        // SAFETY: The field's action is the panel's `ideDebugEvaluate:`, which takes the field as its sender.
        unsafe { field.sendAction_to(field.action(), field.target().as_deref()) };
    }
}

/// Presses ↑ (`older`) or ↓ in the newest console's field, through the panel as the field's delegate; what the field
/// then holds.
pub fn console_arrow(controller: &WindowController, older: bool) -> Option<String> {
    let panel = of(controller)?;
    let console = panel.ivars().links.borrow().consoles.last().cloned()?;
    let field = console.input_field();
    let window = controller.window()?;
    // SAFETY: The window makes or returns its field editor for the field, which it holds.
    let editor = unsafe { window.fieldEditor_forObject(true, Some(&field)) }?;
    let editor = editor.downcast::<objc2_app_kit::NSTextView>().ok()?;
    let command = if older {
        objc2::sel!(moveUp:)
    } else {
        objc2::sel!(moveDown:)
    };
    let control: &objc2_app_kit::NSControl = &field;
    // SAFETY: The panel is the field's delegate and implements this method of NSControlTextEditingDelegate.
    let _: bool =
        unsafe { objc2::msg_send![&*panel, control: control, textView: &*editor, doCommandBySelector: command] };
    Some(field.stringValue().to_string())
}

/// A breakpoint's condition, hit count, log message and whether it is on.
pub type Settings = (Option<String>, Option<String>, Option<String>, bool);

/// The breakpoint at `line` of `path`.
pub fn breakpoint_at(controller: &WindowController, path: &Path, line: usize) -> Option<Settings> {
    let panel = of(controller)?;
    let model = panel.read();
    let b = model.breakpoints.at(path, line)?;
    Some((
        b.condition.clone(),
        b.hit_condition.clone(),
        b.log_message.clone(),
        b.enabled,
    ))
}

/// The context menu of row `row` of the panel's list or the inspector's, as a right click on the row opens it, filled
/// as AppKit has its delegate fill it before it shows.
pub fn context_menu(controller: &WindowController, inspector: bool, row: isize) -> Option<Retained<NSMenu>> {
    let panel = of(controller)?;
    let ui = panel.ui();
    let view = if inspector { &ui.inspector.outline } else { &ui.outline }.clone();
    let event = right_click(&view, row)?;
    let menu = view.menuForEvent(&event)?;
    let delegate = menu.delegate()?;
    // SAFETY: The panel is the menu's delegate and implements `menuNeedsUpdate:` of NSMenuDelegate.
    unsafe { objc2::msg_send![&*delegate, menuNeedsUpdate: &*menu] }
    Some(menu)
}

/// A right click on the middle of row `row` of `view`.
fn right_click(view: &NSOutlineView, row: isize) -> Option<Retained<objc2_app_kit::NSEvent>> {
    let window = view.window()?;
    let rect = view.rectOfRow(row);
    let middle = objc2_foundation::NSPoint::new(
        rect.origin.x + rect.size.width / 2.0,
        rect.origin.y + rect.size.height / 2.0,
    );
    let point = view.convertPoint_toView(middle, None);
    objc2_app_kit::NSEvent::mouseEventWithType_location_modifierFlags_timestamp_windowNumber_context_eventNumber_clickCount_pressure(
        objc2_app_kit::NSEventType::RightMouseDown,
        point,
        objc2_app_kit::NSEventModifierFlags::empty(),
        0.0,
        window.windowNumber(),
        None,
        0,
        1,
        1.0,
    )
}

/// A double click on row `row` of the panel's list: the list notes the clicked row and sends its double action.
pub fn double_click(controller: &WindowController, row: isize) {
    let Some(panel) = of(controller) else { return };
    let outline = panel.ui().outline.clone();
    if right_click(&outline, row)
        .and_then(|event| outline.menuForEvent(&event))
        .is_none()
    {
        return;
    }
    // SAFETY: The double action is the panel's `ideDebugDoubleClicked:`, which takes any sender.
    unsafe { outline.sendAction_to(outline.doubleAction(), outline.target().as_deref()) };
}

/// Steps the rotor "Haltepunkte" of `editor` from the line at UTF-16 offset `from`, or from the start, forwards or
/// back, as VoiceOver does: what it reads there and where it lands.
pub fn rotor_step(editor: &EditorTextView, from: Option<usize>, forward: bool) -> Option<(String, usize)> {
    use objc2_app_kit::{
        NSAccessibilityCustomRotor, NSAccessibilityCustomRotorItemResult, NSAccessibilityCustomRotorItemSearchDelegate,
        NSAccessibilityCustomRotorSearchDirection, NSAccessibilityCustomRotorSearchParameters,
    };
    // SAFETY: As in `rotors`, the getter takes no arguments and may answer nil.
    let rotors: Option<Retained<objc2_foundation::NSArray<NSAccessibilityCustomRotor>>> =
        unsafe { objc2::msg_send![editor, accessibilityCustomRotors] };
    let rotor = rotors?
        .to_vec()
        .into_iter()
        .find(|r| r.label().to_string() == splat_i18n::t("Haltepunkte"))?;
    let delegate = rotor.itemSearchDelegate()?;
    let parameters = NSAccessibilityCustomRotorSearchParameters::new();
    parameters.setSearchDirection(if forward {
        NSAccessibilityCustomRotorSearchDirection::Next
    } else {
        NSAccessibilityCustomRotorSearchDirection::Previous
    });
    if let Some(from) = from {
        let element: &NSView = editor;
        let current = NSAccessibilityCustomRotorItemResult::initWithTargetElement(
            NSAccessibilityCustomRotorItemResult::alloc(),
            objc2::runtime::ProtocolObject::from_ref(element),
        );
        current.setTargetRange(objc2_foundation::NSRange::new(from, 0));
        parameters.setCurrentItem(Some(&current));
    }
    let found = delegate.rotor_resultForSearchParameters(&rotor, &parameters)?;
    Some((found.customLabel()?.to_string(), found.targetRange().location))
}

/// Clicks the control of `command` in the panel's title row; whether it was enabled.
pub fn click_control(controller: &WindowController, command: Command) -> bool {
    let Some(panel) = of(controller) else { return false };
    let index = Command::CONTROLS.iter().position(|c| *c == command);
    let Some(button) = index.and_then(|i| panel.ui().controls.get(i).cloned()) else {
        return false;
    };
    let enabled = button.isEnabled();
    // SAFETY: A click without a sender, as the button sends its own action.
    unsafe { button.performClick(None) };
    enabled
}

/// Selects row `row` of the panel's list and puts the keyboard focus there, for the keys pressed next.
pub fn focus_list(controller: &WindowController, row: isize) -> bool {
    let Some(panel) = of(controller) else { return false };
    let outline = panel.ui().outline.clone();
    select_row(controller, row);
    controller
        .window()
        .is_some_and(|window| window.makeFirstResponder(Some(&outline)))
}

/// Whether the panel says that nothing starts in an untrusted folder, and whether its start button is enabled.
pub fn trust_state(controller: &WindowController) -> Option<(bool, bool)> {
    let panel = of(controller)?;
    let ui = panel.ui();
    Some((!ui.untrusted.isHidden(), ui.start.isEnabled()))
}

/// Clicks the button beside the panel's note about trust; whether there is one.
pub fn press_trust(controller: &WindowController) -> bool {
    let Some(panel) = of(controller) else { return false };
    let button = panel
        .ui()
        .untrusted
        .arrangedSubviews()
        .iter()
        .find_map(|view| view.downcast::<objc2_app_kit::NSButton>().ok());
    match button {
        Some(button) => {
            // SAFETY: A click without a sender, as the button sends its own action.
            unsafe { button.performClick(None) };
            true
        }
        None => false,
    }
}

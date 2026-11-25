//! Code actions (E82): a lightbulb in the line number column of the cursor's line once the server has actions for the
//! place, asked for after the cursor rests and dropped on the next keystroke; ⌥⌘A, the Editor menu, the context menu
//! and the bulb list them as a menu at the cursor, the preferred first. Choosing one applies its edit through `apply`,
//! bound to the revision it was made for (R21), and runs its command on the server.

use crate::EditorSerial;
use ide_document::Revision;
use std::cell::RefCell;
use std::ops::Range;
use std::time::Duration;

use ide_lsp::Ticket;
use ide_lsp::lsp_types::{CodeAction, CodeActionKind, CodeActionOrCommand, Command, Diagnostic, Range as LspRange};
use objc2::rc::Retained;
use objc2::runtime::AnyObject;
use objc2::{MainThreadMarker, MainThreadOnly, define_class, msg_send, sel};
use objc2_app_kit::{NSButton, NSColor, NSMenu, NSMenuItem, NSView};
use objc2_foundation::{NSObject, NSObjectProtocol, NSPoint, NSRect, NSSize};
use splat_i18n::t;

use super::{LspState, server_by_id, server_of, servers_of, text};
use crate::editor::Editor;
use crate::host as editor;
use ide_ui_common::util::{guarded, main_after, main_on_run_loop, main_soon, ns};

/// How long the cursor rests before the server is asked whether the place has actions.
const REST: Duration = Duration::from_millis(400);

/// Which actions a menu lists.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// Everything for the place except source actions, as "Schnellkorrektur …" and the bulb show them.
    Fixes,
    /// The refactorings the server offers for the place.
    Refactor,
    /// The actions on the whole file, such as organising its imports.
    Source,
}

impl Kind {
    fn only(self) -> Option<Vec<CodeActionKind>> {
        match self {
            Kind::Fixes => None,
            Kind::Refactor => Some(vec![CodeActionKind::REFACTOR]),
            Kind::Source => Some(vec![CodeActionKind::SOURCE]),
        }
    }

    fn admits(self, action: &CodeActionOrCommand) -> bool {
        let kind = match action {
            CodeActionOrCommand::Command(_) => return self == Kind::Fixes,
            CodeActionOrCommand::CodeAction(action) => action.kind.as_ref().map_or("", |k| k.as_str()),
        };
        let within = |family: &str| kind == family || kind.starts_with(&format!("{family}."));
        match self {
            Kind::Fixes => !within("source"),
            Kind::Refactor => within("refactor"),
            Kind::Source => within("source"),
        }
    }
}

/// The actions found for a place of the document.
struct Found {
    revision: Revision,
    line: usize,
    range: Range<usize>,
    actions: Vec<CodeActionOrCommand>,
    /// The slot of the server each action came from.
    origins: Vec<u64>,
}

/// What the code actions of one editor are doing.
#[derive(Default)]
pub(crate) struct State {
    /// Counts cursor moves and edits, so that an answer to an older question is dropped.
    asked: u64,
    tickets: Vec<Ticket>,
    found: Option<Found>,
    bulb: Option<Retained<NSButton>>,
    /// When the cursor will have rested long enough to ask, while a timer waits for it; one timer per editor,
    /// however often the cursor moves.
    rest_until: Option<std::time::Instant>,
}

/// A menu of actions on screen, with what its items stand for.
struct Shown {
    serial: EditorSerial,
    revision: Revision,
    actions: Vec<CodeActionOrCommand>,
    origins: Vec<u64>,
    menu: Retained<NSMenu>,
}

/// The code actions on view and the menu that offers them.
#[derive(Default)]
pub(crate) struct CodeActions {
    shown: RefCell<Option<Shown>>,
    chooser: RefCell<Option<Retained<Chooser>>>,
}

define_class!(
    // SAFETY: NSObject has no subclassing requirements and `Chooser` does not implement Drop.
    #[unsafe(super(NSObject))]
    #[thread_kind = MainThreadOnly]
    #[name = "IDECodeActionChooser"]
    pub(crate) struct Chooser;

    unsafe impl NSObjectProtocol for Chooser {}

    impl Chooser {
        #[unsafe(method(ideCodeAction:))]
        fn code_action(&self, sender: Option<&AnyObject>) {
            guarded((), || {
                let tag = sender
                    .and_then(|s| s.downcast_ref::<NSMenuItem>())
                    .map_or(-1, |item| item.tag());
                if let Ok(index) = usize::try_from(tag) {
                    choose(&editor::state(), index);
                }
            });
        }

        #[unsafe(method(ideBulb:))]
        fn bulb(&self, sender: Option<&AnyObject>) {
            guarded((), || {
                let serial = sender
                    .and_then(|s| s.downcast_ref::<NSButton>())
                    .map_or(0, |button| button.tag());
                if let Some(view) = u64::try_from(serial).ok().map(crate::EditorSerial).and_then(editor::find_editor) {
                    open(&editor::state(), &*view, Kind::Fixes);
                }
            });
        }
    }
);

/// The target of the menus and bulbs of `actions`, made when there is none yet.
fn chooser(actions: &CodeActions, mtm: MainThreadMarker) -> Retained<Chooser> {
    actions
        .chooser
        .borrow_mut()
        .get_or_insert_with(|| {
            // SAFETY: NSObject's initializer, called once on the newly allocated object.
            unsafe { msg_send![Chooser::alloc(mtm), init] }
        })
        .clone()
}

/// The cursor's place: its selection in UTF-16 code units and the line it is on.
fn place(view: &dyn Editor) -> (Range<usize>, usize) {
    let selection = view.selectedRange();
    let range = selection.location..selection.location + selection.length;
    let document = view.document();
    let text = document.text();
    let line = text
        .utf16_to_byte(range.start.min(text.len_utf16()))
        .map_or(0, |byte| text.line_of(byte));
    (range, line)
}

/// The cursor moved or the text changed: questions under way end, the bulb goes unless it is still for the place,
/// and a rest asks again.
pub(crate) fn moved(state: &LspState, view: &dyn Editor) {
    let (_, line) = place(view);
    let revision = view.document().revision();
    let (serial, waiting, gone) = {
        let Ok(mut view_state) = view.lsp().try_borrow_mut() else {
            // Moved from inside code that holds the state: the move counts once that code is done.
            view.later(objc2::sel!(ideActionsMoved), 0.0);
            return;
        };
        let serial = view_state.serial;
        let actions = &mut view_state.actions;
        actions.asked += 1;
        let waiting = actions.rest_until.replace(std::time::Instant::now() + REST).is_some();
        for ticket in actions.tickets.drain(..) {
            ticket.cancel();
        }
        let gone = actions
            .found
            .take_if(|found| found.revision != revision || found.line != line)
            .is_some();
        (serial, waiting, gone)
    };
    if gone {
        view.redraw_gutter();
    }
    if server_of(state, view).is_none() {
        view.lsp().borrow_mut().actions.rest_until = None;
        return;
    }
    if !waiting {
        main_after(REST, move || rested(serial));
    }
}

/// The timer of the editor `serial` fired: it asks for the fixes of the cursor's place once the cursor rested, and
/// waits on while it still moves.
fn rested(serial: EditorSerial) {
    let Some(view) = editor::find_editor(serial) else {
        return;
    };
    let Some(until) = view.lsp().borrow().actions.rest_until else {
        return;
    };
    let now = std::time::Instant::now();
    if now < until {
        main_after(until - now, move || rested(serial));
        return;
    }
    view.lsp().borrow_mut().actions.rest_until = None;
    request(&editor::state(), &*view, Kind::Fixes, false);
}

/// The diagnostics of the server of slot `source` that touch `range`, with their ranges where their text is now.
fn diagnostics(view: &dyn Editor, range: &Range<usize>, source: u64) -> Vec<Diagnostic> {
    let state = view.lsp().borrow();
    let document = view.document();
    let text = document.text();
    state
        .marks
        .iter()
        .filter(|mark| mark.source == source)
        .filter(|mark| mark.range.start <= range.end && range.start <= mark.range.end)
        .filter_map(|mark| {
            let mut diagnostic = mark.diagnostic.clone();
            diagnostic.range = LspRange::new(
                text::position(text, mark.range.start)?,
                text::position(text, mark.range.end)?,
            );
            Some(diagnostic)
        })
        .collect()
}

/// The actions every server of an editor offered for one question, with the slot each came from (E140), and how many
/// servers have yet to answer.
struct Gathered {
    actions: Vec<CodeActionOrCommand>,
    origins: Vec<u64>,
    waiting: usize,
}

/// Asks every server of `view` for the actions of `kind` at the cursor; `show` lists them in a menu once all answered.
fn request(state: &LspState, view: &dyn Editor, kind: Kind, show: bool) {
    let servers = servers_of(state, view);
    if servers.is_empty() {
        return;
    }
    let (range, line) = place(view);
    let revision = view.document().revision();
    let span = {
        let document = view.document();
        text::position(document.text(), range.start).zip(text::position(document.text(), range.end))
    };
    let Some((start, end)) = span else { return };
    let (serial, asked) = {
        let state = view.lsp().borrow();
        (state.serial, state.actions.asked)
    };
    let gathered = std::sync::Arc::new(std::sync::Mutex::new(Gathered {
        actions: Vec::new(),
        origins: Vec::new(),
        waiting: servers.len(),
    }));
    for (origin, server, uri) in servers {
        let diagnostics = diagnostics(view, &range, origin);
        let shared = gathered.clone();
        let found_range = range.clone();
        let started = server.code_actions(
            &uri,
            LspRange::new(start, end),
            diagnostics,
            kind.only(),
            move |answer| {
                main_soon(move || {
                    arrived(
                        serial,
                        asked,
                        revision,
                        (kind, show, line, found_range),
                        origin,
                        answer,
                        &shared,
                    );
                });
            },
        );
        match started {
            Ok(ticket) => view.lsp().borrow_mut().actions.tickets.push(ticket),
            Err(error) => {
                if show {
                    ide_ui_common::diag::report("code actions", error);
                }
                {
                    use ide_core::LockIgnoringPoison;
                    gathered.lock_ignoring_poison().waiting -= 1;
                }
            }
        }
    }
}

/// One server's answer to a question of `request`; the last one shows them all.
fn arrived(
    serial: EditorSerial,
    asked: u64,
    revision: Revision,
    (kind, show, line, range): (Kind, bool, usize, Range<usize>),
    origin: u64,
    answer: Result<Vec<CodeActionOrCommand>, ide_lsp::Error>,
    gathered: &std::sync::Arc<std::sync::Mutex<Gathered>>,
) {
    use ide_core::LockIgnoringPoison;
    let done = {
        let mut gathered = gathered.lock_ignoring_poison();
        gathered.waiting = gathered.waiting.saturating_sub(1);
        match answer {
            Ok(actions) => {
                gathered.origins.extend(std::iter::repeat_n(origin, actions.len()));
                gathered.actions.extend(actions);
            }
            Err(error) if show => ide_ui_common::diag::report("code actions", error),
            Err(_) => {}
        }
        gathered.waiting == 0
    };
    if !done {
        return;
    }
    let Some(view) = editor::find_editor(serial) else {
        return;
    };
    let current = view.lsp().borrow().actions.asked == asked;
    if !current || view.document().revision() != revision {
        return;
    }
    let (actions, origins) = {
        let gathered = gathered.lock_ignoring_poison();
        (gathered.actions.clone(), gathered.origins.clone())
    };
    if kind == Kind::Fixes {
        let found = Found {
            revision,
            line,
            range,
            actions: actions.clone(),
            origins: origins.clone(),
        };
        view.lsp().borrow_mut().actions.found = Some(found);
        view.redraw_gutter();
    }
    if show {
        present(&editor::state(), &*view, kind, actions, origins);
    }
}

/// Lists the actions of `kind` at the cursor in a menu there, as ⌥⌘A, the menu items and the bulb do.
pub(crate) fn open(state: &LspState, view: &dyn Editor, kind: Kind) {
    let (range, _) = place(view);
    let revision = view.document().revision();
    let known = {
        let view_state = view.lsp().borrow();
        view_state
            .actions
            .found
            .as_ref()
            .filter(|found| kind == Kind::Fixes && found.revision == revision && found.range == range)
            .map(|found| (found.actions.clone(), found.origins.clone()))
    };
    match known {
        Some((actions, origins)) => present(state, view, kind, actions, origins),
        None => request(state, view, kind, true),
    }
}

/// The actions of `kind` among `actions`, the preferred first, each with its origin.
fn ordered(kind: Kind, actions: Vec<(u64, CodeActionOrCommand)>) -> Vec<(u64, CodeActionOrCommand)> {
    let preferred = |(_, action): &(u64, CodeActionOrCommand)| {
        matches!(
            action,
            CodeActionOrCommand::CodeAction(CodeAction {
                is_preferred: Some(true),
                ..
            })
        )
    };
    let (mut first, rest): (Vec<_>, Vec<_>) = actions
        .into_iter()
        .filter(|(_, action)| kind.admits(action))
        .partition(preferred);
    first.extend(rest);
    first
}

fn title(action: &CodeActionOrCommand) -> &str {
    match action {
        CodeActionOrCommand::Command(command) => &command.title,
        CodeActionOrCommand::CodeAction(action) => &action.title,
    }
}

/// Shows `actions` as a menu at the cursor of `view`; a menu with nothing to choose says so, greyed out.
fn present(state: &LspState, view: &dyn Editor, kind: Kind, actions: Vec<CodeActionOrCommand>, origins: Vec<u64>) {
    let mtm = view.mtm();
    let (origins, actions): (Vec<u64>, Vec<CodeActionOrCommand>) =
        ordered(kind, origins.into_iter().zip(actions).collect())
            .into_iter()
            .unzip();
    let menu = NSMenu::initWithTitle(NSMenu::alloc(mtm), &ns(""));
    menu.setAutoenablesItems(false);
    let target = chooser(&state.code_actions, mtm);
    for (index, action) in actions.iter().enumerate() {
        let item = ide_ui_common::util::tagged(
            mtm,
            title(action),
            sel!(ideCodeAction:),
            "",
            isize::try_from(index).unwrap_or(-1),
        );
        ide_ui_common::util::set_target(&item, &target);
        if let CodeActionOrCommand::CodeAction(CodeAction {
            disabled: Some(disabled),
            ..
        }) = action
        {
            item.setEnabled(false);
            item.setToolTip(Some(&ns(&disabled.reason)));
        }
        menu.addItem(&item);
    }
    if actions.is_empty() {
        let none = ide_ui_common::util::item(mtm, t("Keine Aktionen"), None, "");
        none.setEnabled(false);
        menu.addItem(&none);
    }
    let caret = view.selectedRange().location;
    let at = super::rect_of(view, caret..caret).map_or(NSPoint::new(0.0, 0.0), |rect| {
        NSPoint::new(rect.origin.x, rect.origin.y + rect.size.height)
    });
    let shown = Shown {
        serial: view.lsp().borrow().serial,
        revision: view.document().revision(),
        actions,
        origins,
        menu: menu.clone(),
    };
    let serial = shown.serial;
    *state.code_actions.shown.borrow_mut() = Some(shown);
    // A tracking menu takes the keyboard even from the app in front, which a self-test must never do
    // (docs/development.md); the scenario chooses from the menu it built instead.
    #[cfg(feature = "selftest")]
    if std::env::var_os("IDE_SELFTEST").is_some() {
        return;
    }
    // A menu tracks in a run loop of its own; from a dispatch callback it would hold up every other one until it
    // closed, the server's answers included.
    main_on_run_loop(move || {
        let menu = editor::state()
            .code_actions
            .shown
            .borrow()
            .as_ref()
            .filter(|s| s.serial == serial)
            .map(|s| s.menu.clone());
        if let (Some(menu), Some(view)) = (menu, editor::find_editor(serial)) {
            menu.popUpMenuPositioningItem_atLocation_inView(None, at, Some(&view));
        }
    });
}

/// The user chose item `index` of the menu `state` shows.
fn choose(state: &LspState, index: usize) {
    let chosen = state.code_actions.shown.borrow().as_ref().and_then(|shown| {
        let origin = shown.origins.get(index).copied().unwrap_or(0);
        Some((shown.serial, shown.revision, shown.actions.get(index)?.clone(), origin))
    });
    let Some((serial, revision, action, origin)) = chosen else {
        return;
    };
    let Some(view) = editor::find_editor(serial) else {
        return;
    };
    match action {
        CodeActionOrCommand::Command(command) => execute(state, &*view, origin, &command),
        CodeActionOrCommand::CodeAction(action) if action.edit.is_none() && action.data.is_some() => {
            resolve(state, &*view, origin, revision, action);
        }
        CodeActionOrCommand::CodeAction(action) => perform(state, &*view, origin, revision, &action),
    }
}

/// Asks the server for the edit of `action`, which it computes only once chosen, and performs it.
fn resolve(state: &LspState, view: &dyn Editor, origin: u64, revision: Revision, action: CodeAction) {
    let Some((server, uri)) = server_by_id(state, view, origin).or_else(|| server_of(state, view)) else {
        return;
    };
    let serial = view.lsp().borrow().serial;
    let unresolved = action.clone();
    let started = server.resolve_code_action(&uri, action, move |answer| {
        main_soon(move || {
            let Some(view) = editor::find_editor(serial) else {
                return;
            };
            match answer {
                Ok(action) => perform(&editor::state(), &*view, origin, revision, &action),
                Err(error) => failed(error.to_string()),
            }
        });
    });
    match started {
        Ok(_) => {}
        Err(ide_lsp::Error::Unsupported(_)) => perform(state, view, origin, revision, &unresolved),
        Err(error) => failed(error.to_string()),
    }
}

/// Applies the edit of `action`, made for `revision` of `view`'s text, then runs its command.
fn perform(state: &LspState, view: &dyn Editor, origin: u64, revision: Revision, action: &CodeAction) {
    if view.document().revision() != revision {
        failed(t("Die Datei hat sich inzwischen geändert").to_string());
        return;
    }
    let Some(edit) = &action.edit else {
        if let Some(command) = &action.command {
            execute(state, view, origin, command);
        }
        return;
    };
    let Some(folder) = view.project_root() else {
        return;
    };
    let serial = view.lsp().borrow().serial;
    let command = action.command.clone();
    super::edits::apply_workspace_edit(&folder, edit, move |applied| match applied {
        Err(failure) => failed(failure.message),
        Ok(()) => {
            if let (Some(command), Some(view)) = (command, editor::find_editor(serial)) {
                execute(&editor::state(), &*view, origin, &command);
            }
        }
    });
}

/// Runs `command` on the server; edits it makes come back as `workspace/applyEdit`.
fn execute(state: &LspState, view: &dyn Editor, origin: u64, command: &Command) {
    let Some((server, _)) = server_by_id(state, view, origin).or_else(|| server_of(state, view)) else {
        return;
    };
    let arguments = command.arguments.clone().unwrap_or_default();
    let started = server.execute_command(&command.command, arguments, |answer| {
        if let Err(error) = answer {
            main_soon(move || failed(error.to_string()));
        }
    });
    if let Err(error) = started {
        failed(error.to_string());
    }
}

/// Tells the user that a server's change was not applied, and why.
pub(crate) fn failed(message: String) {
    ide_ui_common::diag::report("code action", &message);
    main_on_run_loop(move || {
        if let Some(mtm) = MainThreadMarker::new() {
            ide_ui_common::alert::warn(mtm, t("Änderung nicht angewandt"), &message);
        }
    });
}

/// Whether the menu command `action` can run in `view`, for the code action commands; `None` for every other one.
pub(crate) fn validate(state: &LspState, view: &dyn Editor, action: objc2::runtime::Sel) -> Option<bool> {
    [sel!(ideQuickFix:), sel!(ideRefactor:), sel!(ideSourceActions:)]
        .contains(&action)
        .then(|| {
            server_of(state, view)
                .is_some_and(|(server, _)| server.capabilities().is_some_and(|c| c.code_action_provider.is_some()))
        })
}

/// The line the bulb is on, if the place of the cursor has actions to offer.
pub fn bulb_line(view: &dyn Editor) -> Option<usize> {
    let state = view.lsp().try_borrow().ok()?;
    let found = state.actions.found.as_ref()?;
    let offers = found.actions.iter().any(|action| {
        Kind::Fixes.admits(action)
            && !matches!(
                action,
                CodeActionOrCommand::CodeAction(CodeAction { disabled: Some(_), .. })
            )
    });
    (offers && found.revision == view.document().revision()).then_some(found.line)
}

/// Puts the bulb of `view` into `row` of the line number column `gutter`, the rectangle of the number it covers, or
/// hides it without one; the column calls it whenever it draws.
pub fn place_bulb(view: &dyn Editor, gutter: &NSView, row: Option<NSRect>) {
    let existing = view.lsp().borrow().actions.bulb.clone();
    let Some(row) = row else {
        if let Some(bulb) = existing {
            bulb.setHidden(true);
        }
        return;
    };
    let bulb = existing.unwrap_or_else(|| {
        let bulb = make_bulb(&editor::state().code_actions, view);
        view.lsp().borrow_mut().actions.bulb = Some(bulb.clone());
        bulb
    });
    if !bulb.isDescendantOf(gutter) {
        gutter.addSubview(&bulb);
    }
    let side = row.size.height.min(16.0);
    let right = row.origin.x + row.size.width;
    let frame = NSRect::new(
        NSPoint::new((right - side).max(0.0), row.origin.y + (row.size.height - side) / 2.0),
        NSSize::new(side, side),
    );
    if bulb.frame() != frame {
        bulb.setFrame(frame);
    }
    bulb.setHidden(false);
}

/// The bulb of `view`, aimed at the chooser of `actions`.
fn make_bulb(actions: &CodeActions, view: &dyn Editor) -> Retained<NSButton> {
    let mtm = view.mtm();
    let target = chooser(actions, mtm);
    let label = t("Aktionen für diese Stelle");
    let bulb = ide_ui_common::util::symbol_button(
        mtm,
        "lightbulb",
        label,
        ide_ui_common::style::symbol::INLINE,
        Some(&target),
        sel!(ideBulb:),
    );
    ide_ui_common::util::set_label(&bulb, Some(label));
    bulb.setContentTintColor(Some(&NSColor::systemYellowColor()));
    bulb.setTag(isize::try_from(view.lsp().borrow().serial.0).unwrap_or(0));
    bulb
}

/// What the self-test reads from and does with code actions.
#[cfg(feature = "selftest")]
pub mod selftest {

    use objc2_app_kit::NSAccessibility;

    use crate::editor::Editor;

    /// The titles of the menu of actions on screen for `view`, in order.
    pub fn shown(view: &dyn Editor) -> Option<Vec<String>> {
        let serial = view.lsp().borrow().serial;
        let state = crate::host::state();
        let shown = state.code_actions.shown.borrow();
        let shown = shown.as_ref().filter(|s| s.serial == serial)?;
        Some(
            shown
                .menu
                .itemArray()
                .iter()
                .map(|item| item.title().to_string())
                .collect(),
        )
    }

    /// Closes the menu on screen and chooses its first item whose title starts with `title`, as a click does.
    pub fn choose(title: &str) -> bool {
        let state = crate::host::state();
        let found = state.code_actions.shown.borrow().as_ref().and_then(|shown| {
            shown.menu.cancelTrackingWithoutAnimation();
            shown
                .actions
                .iter()
                .position(|action| super::title(action).starts_with(title))
        });
        found.is_some_and(|index| {
            super::choose(&state, index);
            true
        })
    }

    /// Clicks the first item of the menu on screen whose title starts with `title`, which sends its action to the
    /// menu's chooser as a click does; whether there was one.
    pub fn click(title: &str) -> bool {
        let menu = crate::host::state()
            .code_actions
            .shown
            .borrow()
            .as_ref()
            .map(|shown| shown.menu.clone());
        let Some(menu) = menu else { return false };
        let index = menu
            .itemArray()
            .iter()
            .position(|item| item.title().to_string().starts_with(title));
        index
            .and_then(|index| isize::try_from(index).ok())
            .is_some_and(|index| {
                menu.performActionForItemAtIndex(index);
                true
            })
    }

    /// Presses the bulb of `view` as a click does; whether it was on view.
    pub fn press_bulb(view: &dyn Editor) -> bool {
        let Some(bulb) = view.lsp().borrow().actions.bulb.clone() else {
            return false;
        };
        let (Some(action), Some(mtm)) = (bulb.action(), objc2::MainThreadMarker::new()) else {
            return false;
        };
        if bulb.isHidden() {
            return false;
        }
        let app = objc2_app_kit::NSApplication::sharedApplication(mtm);
        // SAFETY: The bulb's own action and target, with the bulb as the sender, as AppKit sends them on a click.
        unsafe { app.sendAction_to_from(action, bulb.target().as_deref(), Some(&bulb)) }
    }

    /// Forgets the menu shown, so the next one is told apart.
    pub fn forget() {
        if let Some(shown) = crate::host::state().code_actions.shown.borrow_mut().take() {
            shown.menu.cancelTrackingWithoutAnimation();
        }
    }

    /// What VoiceOver calls the bulb of `view` while it shows, and its frame in the window.
    pub fn bulb(view: &dyn Editor) -> Option<(String, objc2_foundation::NSRect)> {
        let bulb = view.lsp().borrow().actions.bulb.clone()?;
        if bulb.isHidden() || bulb.window().is_none() {
            return None;
        }
        let label = bulb.accessibilityLabel().map(|l| l.to_string()).unwrap_or_default();
        Some((label, bulb.convertRect_toView(bulb.bounds(), None)))
    }
}

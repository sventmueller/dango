//! What an order's commands do, from its tab, a context menu or "Ablage ▸ Auftrag": the factory gets the command
//! through the host, after a sheet where the command needs words or can lose work (E101), and every view of the
//! folder shows the result.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::rc::Weak;

use ide_factory::{HunkId, InputKind, OrderId, Refusal};
use ide_ui_common::alert::{Alert, Role, first};
use ide_ui_common::util::ns;
use ide_ui_common::window::{Files, Window};
use objc2::rc::Retained;
use objc2::{MainThreadMarker, MainThreadOnly};
use objc2_app_kit::{NSAccessibility, NSScrollView, NSTextField, NSTextView, NSWindow};
use objc2_foundation::{NSPoint, NSRect, NSSize};
use splat_i18n::{t, tf};

use crate::host::{State, host};
use crate::model::detail::{AcceptBlock, Command, Detail};
use crate::model::words;

/// Where a command runs: the window, its folder and the crate's state.
#[derive(Clone)]
pub(crate) struct Target {
    window: Window,
    root: PathBuf,
    state: Weak<State>,
}

impl Target {
    pub(crate) fn new(window: Window, root: PathBuf, state: Weak<State>) -> Target {
        Target { window, root, state }
    }

    pub(crate) fn window(&self) -> Window {
        self.window
    }

    pub(crate) fn root(&self) -> &Path {
        &self.root
    }

    pub(crate) fn state(&self) -> Option<std::rc::Rc<State>> {
        self.state.upgrade()
    }

    /// Every view of the folder reads the factory again.
    pub(crate) fn refresh(&self) {
        if let Some(state) = self.state() {
            crate::changed(&state, &self.root);
        }
    }
}

/// Runs `command` on the order `id`, as its model at this moment allows.
pub(crate) fn run_on(target: &Target, id: &OrderId, command: Command) {
    if let Some(detail) = host().order(target.root(), id) {
        run(target, &detail, command, &BTreeSet::new());
    }
}

/// Runs `command` on `detail`'s order; `rejected` are the sections G2 turns down (C55).
pub(crate) fn run(target: &Target, detail: &Detail, command: Command, rejected: &BTreeSet<HunkId>) {
    if !detail.can(command) {
        if command == Command::Accept && detail.accept_block() == Some(AcceptBlock::OpenDiffFirst) {
            tell(target, t("Bei hoher Risikoklasse zuerst den Diff öffnen."));
        }
        return;
    }
    if let Some(input) = detail.input(command, rejected) {
        confirm_rejected(target, input);
        return;
    }
    let id = detail.id().clone();
    match command {
        Command::Accept => accept(target, detail, rejected),
        Command::OpenDiff => {
            host().open_diff(target.window().id(), target.root(), &id);
            send(target, InputKind::DiffOpened(id));
        }
        Command::OpenSession => host().open_session(target.window().id(), target.root(), &id),
        Command::OpenReport => open_reports(target, detail),
        Command::OpenTerminal => {
            if let Some(tree) = detail.worktree() {
                host().open_terminal(target.window().id(), Path::new(tree));
            }
        }
        Command::MergeRequest => {
            if let Some(branch) = detail.merge_branch() {
                host().open_merge_request(target.window().id(), target.root(), branch);
            }
        }
        Command::ShowChange => {
            if let Some(url) = detail.change_url() {
                host().open_change(target.window().id(), url);
            }
        }
        Command::BackToBuilder => ask_text(
            target,
            &tf("„{}“ zurück an den Builder", &[detail.title()]),
            t("Der Kommentar geht mit dem nächsten Zug an den Builder."),
            t("Zurückgeben"),
            "",
            move |target, text| send(target, InputKind::BackToBuilder(id, text)),
        ),
        Command::ChangeCriteria => ask_text(
            target,
            &tf("Kriterien von „{}“ ändern", &[detail.title()]),
            t("Ein Kriterium je Zeile. Die Änderung macht den Nachweis ungültig und geht an Builder und Prüfer."),
            t("Ändern"),
            detail.criteria_text(),
            move |target, text| send(target, InputKind::ChangeCriteria(id, text)),
        ),
        Command::Abort => abort(target, detail),
        _ => {}
    }
}

/// Hands `input` to the factory and shows what it refused; then every view of the folder shows the result.
pub(crate) fn send(target: &Target, input: InputKind) {
    let refused = host().send(target.root(), input);
    target.refresh();
    refusals(target, &refused);
}

/// Tells the user why the factory refused, if it did.
pub(crate) fn refusals(target: &Target, refused: &[Refusal]) {
    let words: Vec<String> = refused.iter().map(words::refusal).collect();
    if !words.is_empty() {
        tell(target, &words.join("\n"));
    }
}

fn tell(target: &Target, words: &str) {
    let Some(window) = target.window().window() else { return };
    Alert::new(window.mtm(), t("Die Software Factory hat den Befehl nicht ausgeführt"))
        .informative(words)
        .button(t("OK"), Role::Plain)
        .sheet(&window, |_| {});
}

/// Opens each Markdown file of the path scope the order wrote in its worktree, or says that it wrote none.
fn open_reports(target: &Target, detail: &Detail) {
    let Some(tree) = detail.worktree() else { return };
    let written: Vec<PathBuf> = detail
        .reports()
        .iter()
        .map(|report| Path::new(tree).join(report))
        .filter(|path| path.is_file())
        .collect();
    if written.is_empty() {
        tell(
            target,
            &tf("Der Auftrag hat {} nicht geschrieben.", &[&detail.reports().join(", ")]),
        );
    }
    for path in written {
        target.window().open_file(&path);
    }
}

fn accept(target: &Target, detail: &Detail, rejected: &BTreeSet<HunkId>) {
    if let Some(input) = detail.accept(rejected) {
        confirm_rejected(target, input);
    }
}

/// Sends `input`, after asking first when its G2 turns sections down (C55).
fn confirm_rejected(target: &Target, input: InputKind) {
    let count = match &input {
        InputKind::Accept { rejected, .. }
        | InputKind::KeepBranch { rejected, .. }
        | InputKind::Publish { rejected, .. } => rejected.len(),
        _ => 0,
    };
    if count == 0 {
        send(target, input);
        return;
    }
    let Some(window) = target.window().window() else { return };
    let target = target.clone();
    Alert::new(window.mtm(), &tf("{} Abschnitte verwerfen und den Rest abnehmen?", &[&count.to_string()]))
        .informative(t(
            "Aus den übrigen Abschnitten entsteht ein neuer Stand. Gates und Prüfer laufen für ihn erneut, danach schließt die Abnahme für genau diesen Stand ab.",
        ))
        .button(t("Verwerfen und abnehmen"), Role::Destructive)
        .button(t("Abbrechen"), Role::CancelOnReturn)
        .sheet(&window, move |response| {
            if first(response) {
                send(&target, input);
            }
        });
}

fn abort(target: &Target, detail: &Detail) {
    let Some(window) = target.window().window() else { return };
    let id = detail.id().clone();
    let target = target.clone();
    Alert::new(window.mtm(), &tf("„{}“ abbrechen?", &[detail.title()]))
        .warning()
        .informative(t(
            "Der Worktree des Auftrags wird entfernt, mit allem, was noch nicht übernommen ist. Protokoll und Nachweis bleiben.",
        ))
        .button(t("Auftrag abbrechen"), Role::Destructive)
        .button(t("Abbrechen"), Role::CancelOnReturn)
        .sheet(&window, move |response| {
            if first(response) {
                send(&target, InputKind::Abort(id));
            }
        });
}

/// Asks in a sheet for a text that starts as `start`; `then` gets it unless the user cancels or leaves it empty.
fn ask_text(
    target: &Target,
    message: &str,
    info: &str,
    action: &str,
    start: &str,
    then: impl FnOnce(&Target, String) + 'static,
) {
    let Some(window) = target.window().window() else { return };
    let (scroll, view) = text_area(&window, start);
    view.setAccessibilityLabel(Some(&ns(message)));
    let target = target.clone();
    let typed = view.clone();
    Alert::new(window.mtm(), message)
        .informative(info)
        .accessory(&scroll)
        .focus(&view)
        .button(action, Role::Plain)
        .button(t("Abbrechen"), Role::CancelOnEscape)
        .sheet(&window, move |response| {
            let text = typed.string().to_string().trim().to_string();
            if first(response) && !text.is_empty() {
                then(&target, text);
            }
        });
}

/// A text view of several lines in a scroll view, for an alert's accessory.
pub(crate) fn text_area(window: &NSWindow, start: &str) -> (Retained<NSScrollView>, Retained<NSTextView>) {
    let mtm: MainThreadMarker = window.mtm();
    let frame = NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(360.0, 120.0));
    let scroll = NSScrollView::initWithFrame(NSScrollView::alloc(mtm), frame);
    scroll.setHasVerticalScroller(true);
    scroll.setBorderType(objc2_app_kit::NSBorderType::BezelBorder);
    let view = NSTextView::initWithFrame(NSTextView::alloc(mtm), frame);
    view.setRichText(false);
    view.setFont(Some(&ide_ui_common::style::body()));
    view.setString(&ns(start));
    view.setAutoresizingMask(objc2_app_kit::NSAutoresizingMaskOptions::ViewWidthSizable);
    scroll.setDocumentView(Some(&view));
    (scroll, view)
}

/// A field of one line for an alert's accessory.
pub(crate) fn line_field(mtm: MainThreadMarker, start: &str, placeholder: &str) -> Retained<NSTextField> {
    let field = NSTextField::textFieldWithString(&ns(start), mtm);
    field.setPlaceholderString(Some(&ns(placeholder)));
    ide_ui_common::util::set_label(&field, Some(placeholder));
    field
}

//! The `editor-line-diagnostics` scenario (E130): with View ▸ Diagnostics at Line End on, each line shows its most
//! serious message at its end with a symbol for its severity and a count of the others, VoiceOver's rotor says the
//! severity, an edited line waits a second before it shows its message again, a line with a debug value and blame
//! keeps the value, cuts the message short and drops the blame, and 500 diagnostics in 10 000 lines keep a keystroke
//! under 5 ms (E25).

use std::path::PathBuf;
use std::time::Duration;

use ide_lsp::lsp_types::{Diagnostic, DiagnosticSeverity, Position, Range};
use objc2::MainThreadOnly;
use objc2::rc::Retained;
use objc2_app_kit::NSApplication;
use objc2_foundation::NSRange;

use super::helpers::{self, wait_until, wait_within};
use super::{check, finish, give_up, give_up_saying, snapshot};
use crate::editor::insets::{self, Inset, Layer, Place};
use crate::editor::{self, EditorTextView};
use crate::window;

const SCENARIO: &str = "editor-line-diagnostics";
const PROJECT: &str = "diagnostics-project";
const FILE: &str = "src/lib.rs";
const TEXT: &str =
    "fn a() -> u32 { missing }\nfn b() { let unused = 1; }\nfn c() {}\nfn d(value: u32) -> u32 { value }\n";
const LONG: &str = "this function's parameter is shadowed by a binding further down whose type cannot be inferred \
                    without an annotation, and the message goes on well past the editor's edge";

fn root() -> PathBuf {
    helpers::root(PROJECT)
}

fn view() -> Option<Retained<EditorTextView>> {
    editor::editor_for(&*window::for_project(&root())?, &root().join(FILE))
}

fn picture(name: &str) {
    if let Some(window) = window::for_project(&root()).and_then(|c| c.window()) {
        window.displayIfNeeded();
        snapshot(&window, name);
    }
}

fn diagnostic(line: u32, severity: DiagnosticSeverity, message: &str) -> Diagnostic {
    Diagnostic {
        range: Range::new(Position::new(line, 3), Position::new(line, 4)),
        severity: Some(severity),
        message: message.to_string(),
        ..Diagnostic::default()
    }
}

fn diagnostics() -> Vec<Diagnostic> {
    vec![
        diagnostic(
            0,
            DiagnosticSeverity::ERROR,
            "cannot find value `missing` in this scope",
        ),
        diagnostic(
            1,
            DiagnosticSeverity::INFORMATION,
            "consider prefixing with an underscore",
        ),
        diagnostic(1, DiagnosticSeverity::WARNING, "unused variable: `unused`"),
        diagnostic(2, DiagnosticSeverity::INFORMATION, "function `c` is never used"),
        diagnostic(3, DiagnosticSeverity::WARNING, LONG),
    ]
}

fn toggle(view: &EditorTextView) {
    let app = NSApplication::sharedApplication(view.mtm());
    // SAFETY: An action the editor answers, sent to it with no sender, as the menu item without a target sends it.
    unsafe { app.sendAction_to_from(objc2::sel!(ideToggleDiagnosticsAtLineEnd:), Some(view), None) };
}

fn shown_text(view: &EditorTextView) -> Vec<(usize, String)> {
    insets::of(view, Layer::Diagnostics)
        .into_iter()
        .map(|i| (i.at, i.text))
        .collect()
}

pub(crate) fn run() {
    if let Err(error) = helpers::make_project(PROJECT, &[(FILE, TEXT)]) {
        return give_up_saying(SCENARIO, "the scenario's folder is made", error.to_string());
    }
    let Some(mtm) = objc2::MainThreadMarker::new() else {
        return;
    };
    crate::app::open_project(mtm, root());
    helpers::wait_for_window(&root(), |controller| {
        let Some(controller) = controller else {
            return give_up(SCENARIO, "the project's window opens");
        };
        controller.open_file(&root().join(FILE));
        wait_until(
            || view().is_some_and(|v| v.document().is_highlighted()),
            |_| at_line_ends(),
        );
    });
}

fn at_line_ends() {
    let Some(view) = view() else {
        return give_up(SCENARIO, "the file opens in an editor");
    };
    ide_ui_lsp::show_diagnostics(&*view, &diagnostics());
    let off = insets::of(&view, Layer::Diagnostics).len();
    toggle(&view);
    let shown = shown_text(&view);
    check(
        "off at first; on, each line shows its most serious message, the others counted",
        off == 0
            && shown.len() == 4
            && shown[0].1.starts_with("cannot find value")
            && shown[1].1.starts_with("unused variable")
            && shown[1].1.ends_with("(+1)"),
        format!("{off} / {shown:?}"),
    );
    let severities: Vec<_> = insets::of(&view, Layer::Diagnostics)
        .iter()
        .map(|i| i.severity)
        .collect();
    let rotor = insets::rotor_items(&view);
    check(
        "each carries its severity, which VoiceOver's rotor says first",
        severities.iter().all(Option::is_some)
            && rotor
                .iter()
                .any(|item| item.label.contains("Fehler: cannot find value")),
        format!("{severities:?} / {rotor:?}"),
    );
    with_others(&view);
    picture("line-diagnostics-light");
    appearances();
    resting(&view);
}

/// A debug value and blame on the line of the long message: the value stays, the message is cut, blame gives way.
fn with_others(view: &EditorTextView) {
    let at = view.line_start(3).unwrap_or(0) + 5;
    insets::set(view, Layer::Debug, vec![Inset::new(at, "value = 7", Place::LineEnd)]);
    insets::set(
        view,
        Layer::Blame,
        vec![Inset::new(at, "Ada, 3 days ago • Write it", Place::LineEnd)],
    );
    if let Some(window) = view.window() {
        window.displayIfNeeded();
    }
    insets::draw(view, view.visibleRect());
    let drawn = insets::shown(view);
    let debug = drawn.iter().find(|s| s.layer == Layer::Debug).map(|s| s.rect);
    let message = drawn
        .iter()
        .find(|s| s.layer == Layer::Diagnostics && s.rect.origin.y == debug.map_or(-1.0, |d| d.origin.y));
    let blame = drawn.iter().any(|s| s.layer == Layer::Blame);
    check(
        "at a crowded line's end the debug value comes first, the message is cut short and blame gives way",
        debug.is_some() && message.is_some_and(|m| debug.is_some_and(|d| m.rect.origin.x > d.origin.x)) && !blame,
        format!("{drawn:?}"),
    );
}

fn appearances() {
    use objc2_app_kit::{
        NSAppearanceNameAccessibilityHighContrastAqua, NSAppearanceNameAccessibilityHighContrastDarkAqua,
        NSAppearanceNameDarkAqua,
    };
    let Some(controller) = window::for_project(&root()) else {
        return;
    };
    for (name, label) in [
        (crate::util::constant!(NSAppearanceNameDarkAqua), "dark"),
        (
            crate::util::constant!(NSAppearanceNameAccessibilityHighContrastAqua),
            "light-contrast",
        ),
        (
            crate::util::constant!(NSAppearanceNameAccessibilityHighContrastDarkAqua),
            "dark-contrast",
        ),
    ] {
        super::agent::appearance(&controller, name);
        insets::force_increased_contrast(label.ends_with("contrast"));
        picture(&format!("line-diagnostics-{label}"));
    }
    insets::force_increased_contrast(false);
    if let Some(window) = controller.window() {
        objc2_app_kit::NSAppearanceCustomization::setAppearance(&*window, None);
    }
}

/// Typing on the first line hides its message until the line has rested a second.
fn resting(editor_view: &EditorTextView) {
    let end = editor_view.line_start(1).unwrap_or(1) - 1;
    editor_view.setSelectedRange(NSRange::new(end, 0));
    editor::type_text(editor_view, " ");
    let while_typing = shown_text(editor_view).iter().any(|(at, _)| *at < end);
    let busy = editor::busy_lines(editor_view);
    wait_within(
        Duration::from_secs(5),
        || view().is_some_and(|v| shown_text(&v).iter().any(|(at, _)| *at < 10)),
        move |back| {
            check(
                "the edited line hides its message while the user types and shows it after a second's rest",
                !while_typing && busy.contains(&0) && back,
                format!("while typing {while_typing}, busy {busy:?}, back {back}"),
            );
            budget();
        },
    );
}

/// E25: 500 diagnostics at line ends in a file of 10 000 lines keep a keystroke within `insets::keystroke()`.
fn budget() {
    let big: String = (0..10_000)
        .map(|n| format!("    let value_{n} = compute({n}, other_{n});\n"))
        .collect();
    let path = root().join("big.rs");
    if let Err(error) = std::fs::write(&path, &big) {
        return give_up_saying(SCENARIO, "the big file is written", error.to_string());
    }
    let Some(controller) = window::for_project(&root()) else {
        return give_up(SCENARIO, "the folder's window is open");
    };
    controller.open_file(&path);
    let opened = path.clone();
    let big_view = move || window::for_project(&root()).and_then(|c| editor::editor_for(&c, &opened));
    let ready = big_view.clone();
    wait_until(
        move || ready().is_some_and(|v| v.line_start(9_999).is_some()),
        move |_| {
            let Some(view) = big_view() else {
                return give_up(SCENARIO, "the big file opens");
            };
            let many: Vec<Diagnostic> = (0..500)
                .map(|n| diagnostic(n * 20, DiagnosticSeverity::WARNING, "unused variable"))
                .collect();
            let at = view.line_start(5_001).unwrap_or(0) + 10;
            view.scrollRangeToVisible(NSRange::new(at, 0));
            view.setSelectedRange(NSRange::new(at, 0));
            ide_ui_lsp::show_diagnostics(&*view, &many);
            let count = insets::of(&view, Layer::Diagnostics).len();
            let ((median, slowest), (plain, plain_slowest)) = super::insets::compare(
                &view,
                || ide_ui_lsp::show_diagnostics(&*view, &many),
                || ide_ui_lsp::show_diagnostics(&*view, &[]),
            );
            check(
                "E25 with diagnostics at line ends: 500 of them in 10 000 lines keep a keystroke under 5 ms and add \
                 under 2 ms",
                count == 500
                    && median < super::insets::keystroke()
                    && median.saturating_sub(plain) < super::insets::ADDED,
                format!("{count} shown, median {median:?}, slowest {slowest:?}; without {plain:?}, {plain_slowest:?}"),
            );
            let _ = editor::save_all();
            finish(SCENARIO);
        },
    );
}

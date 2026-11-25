//! Diagnostics at the end of their line (E130): the line's most serious message, cut to one line, with a count of the
//! others behind it and all of them in the tooltip. A line shows its message once it has not been edited for a second,
//! the caret's line never while the user types.

use std::cell::Cell;
use std::time::{Duration, Instant};

use ide_core::settings::{Settings, keys};
use ide_document::Change;
use objc2::{DefinedClass, MainThreadMarker};
use objc2_app_kit::NSMenuItem;
use splat_i18n::tf;

use super::insets::{self, Inset, Layer, Place};
use super::view::EditorTextView;

/// How long a line must rest after an edit before it shows its diagnostic.
const REST: Duration = Duration::from_secs(1);
const LONGEST: usize = 160;

/// Whether diagnostics show at the line's end, as the user last chose; read from the settings once.
#[derive(Default)]
pub(crate) struct Shown(Cell<Option<bool>>);

impl Shown {
    /// Reads the setting again on next use.
    pub(crate) fn forget(&self) {
        self.0.set(None);
    }

    /// Whether editors show diagnostics at the end of their lines, read from the settings on first use.
    fn get(&self) -> bool {
        self.0.get().unwrap_or_else(|| {
            let value = Settings::load_user().get(&keys::DIAGNOSTICS_AT_LINE_END);
            self.0.set(Some(value));
            value
        })
    }
}

/// The lines of an editor edited lately, with when.
#[derive(Default)]
pub(crate) struct Edits(Vec<(usize, Instant)>);

fn context() -> Option<std::rc::Rc<crate::context::AppContext>> {
    MainThreadMarker::new().map(crate::context::AppContext::get)
}

/// Whether editors show diagnostics at the end of their lines.
pub(crate) fn shown() -> bool {
    let Some(cx) = context() else { return false };
    cx.editor().line_diagnostics.get()
}

/// Turns diagnostics at the line's end on or off in every editor and remembers the choice.
pub(crate) fn toggle() {
    let cx = crate::context::app();
    let shown = &cx.editor().line_diagnostics;
    let value = !shown.get();
    shown.0.set(Some(value));
    let mut settings = Settings::load_user();
    if let Err(error) = settings.set(&keys::DIAGNOSTICS_AT_LINE_END, &value) {
        crate::diag::report("saving the setting of diagnostics at the line's end", error);
    }
    for view in cx.editors().live() {
        refresh(&view);
    }
}

/// Whether `item` is the menu item of diagnostics at the line's end, which then gets its check mark.
pub(crate) fn validate(cx: &crate::context::AppContext, item: &NSMenuItem) -> Option<bool> {
    if item.action() != Some(objc2::sel!(ideToggleDiagnosticsAtLineEnd:)) {
        return None;
    }
    item.setState(if cx.editor().line_diagnostics.get() {
        objc2_app_kit::NSControlStateValueOn
    } else {
        objc2_app_kit::NSControlStateValueOff
    });
    Some(true)
}

/// Edits of `view`: their lines wait before they show a diagnostic again.
pub(crate) fn edited(view: &EditorTextView, changes: &[Change]) {
    if !shown() {
        return;
    }
    let now = Instant::now();
    let mut edits = view.ivars().line_edits.borrow_mut();
    edits.0.retain(|(_, at)| now.duration_since(*at) < REST);
    for change in changes {
        for line in change.start.line..=change.new_end.line {
            edits.0.push((line, now));
        }
    }
    drop(edits);
    view.later(objc2::sel!(ideDiagnosticsRested), REST.as_secs_f64() + 0.05);
    refresh(view);
}

/// Shows the diagnostics of `view` at the end of their lines, or none while the setting is off.
pub(crate) fn refresh(view: &EditorTextView) {
    if !shown() {
        insets::clear(view, Layer::Diagnostics);
        return;
    }
    let now = Instant::now();
    let resting = |at: &Instant| now.duration_since(*at) >= REST;
    let (busy, typing): (Vec<usize>, bool) = {
        let edits = view.ivars().line_edits.borrow();
        let busy = edits
            .0
            .iter()
            .filter(|(_, at)| !resting(at))
            .map(|(line, _)| *line)
            .collect();
        (busy, edits.0.iter().any(|(_, at)| !resting(at)))
    };
    let caret_line = typing.then(|| caret_line(view)).flatten();
    let wanted: Vec<Inset> = ide_ui_lsp::line_messages(view)
        .into_iter()
        .filter(|(line, _)| !busy.contains(line) && caret_line != Some(*line))
        .filter_map(|(_, (at, messages))| inset(at, &messages))
        .collect();
    let same = {
        let had = insets::of(view, Layer::Diagnostics);
        had.len() == wanted.len()
            && had
                .iter()
                .zip(&wanted)
                .all(|(a, b)| a.at == b.at && a.text == b.text && a.severity == b.severity)
    };
    if !same {
        insets::set(view, Layer::Diagnostics, wanted);
    }
}

fn caret_line(view: &EditorTextView) -> Option<usize> {
    let at = view.selectedRange().location;
    let document = view.document();
    let text = document.text();
    let byte = text.utf16_to_byte(at.min(text.len_utf16())).ok()?;
    Some(text.line_of(byte))
}

/// The inset of a line with `messages`, the most serious first, starting at UTF-16 offset `at`.
fn inset(at: usize, messages: &[(ide_ui_lsp::Severity, String)]) -> Option<Inset> {
    let (severity, first) = messages.first()?;
    let one_line: String = first.lines().next().unwrap_or_default().chars().take(LONGEST).collect();
    let text = if messages.len() > 1 {
        tf("{} (+{})", &[&one_line, &(messages.len() - 1).to_string()])
    } else {
        one_line
    };
    let mut inset = Inset::new(at, text, Place::LineEnd);
    inset.severity = Some(*severity);
    inset.tooltip = Some(
        messages
            .iter()
            .map(|(severity, message)| format!("{} {}", severity.symbol(), message))
            .collect::<Vec<_>>()
            .join("\n"),
    );
    Some(inset)
}

#[cfg(feature = "selftest")]
mod selftest_probe;
#[cfg(feature = "selftest")]
pub(crate) use selftest_probe::*;

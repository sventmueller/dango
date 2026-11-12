//! The commit message a model writes (E88) under the rules for generative AI (E101): the request is built once and
//! checked by gitleaks, and only that checked text goes out, after the user allowed the provider once; the message
//! appears as it streams, stays editable, is one step to undo, and says that AI wrote it until the user changes it.
//! What went out can be read again; nothing goes out when the check finds something, fails or cannot run.

mod claude;

pub(crate) use claude::MODEL as CLAUDE_MODEL;
pub mod source;

use std::sync::atomic::{AtomicU64, Ordering};

use ide_git::{CheckedRequest, Place, RequestFinding, RequestScan};
use objc2_app_kit::NSAccessibility;
use objc2_foundation::NSRange;
use splat_i18n::{t, tf};

use self::source::{Failure, Source};
use super::model::{Phase, Task};
use super::panel::{GitPanel, with_panel};
use super::text::Problem;
use super::{sheets, work};
use crate::util::{main_soon, ns};

/// The panel's part of the commit message a model writes.
#[derive(Default)]
pub struct MessageState {
    /// The checked request that went out last, and to whom.
    pub sent: Option<(String, String)>,
    /// The message the model wrote, until the user changes it.
    pub written: Option<String>,
    pub running: Option<Running>,
    /// Why the last attempt sent nothing, which the button says until the next one.
    pub blocked: Option<String>,
    /// The panel sets the text itself, which is no change of the user's.
    applying: bool,
    /// What the progress row says while it runs.
    pub progress: String,
}

/// A request on its way.
pub struct Running {
    id: u64,
    /// The message before, which undo brings back.
    before: String,
    cancel: ide_agent::Cancel,
    received: String,
}

/// The installation command the button names when gitleaks is missing.
pub const INSTALL_GITLEAKS: &str = "brew install gitleaks";

impl GitPanel {
    /// Why the button cannot ask a model now, if it cannot.
    pub fn message_unavailable(&self) -> Option<String> {
        let state = self.read();
        if state.phase != Phase::Ready {
            return Some(t("Git ist noch nicht bereit.").to_string());
        }
        if state.model.staged.is_empty() && state.model.changes.is_empty() {
            return Some(t("Es gibt keine Änderungen, aus denen eine Nachricht entstehen könnte.").to_string());
        }
        drop(state);
        source::unavailable().map(String::from)
    }

    /// Asks the model for the commit message, after asking the user once for a provider outside the Mac.
    pub fn generate_message(&self) {
        if let Some(why) = self.message_unavailable() {
            self.set_problem(Some(Problem::plain(why)));
            return;
        }
        let source = match Source::current() {
            Ok(source) => source,
            Err(why) => return self.set_problem(Some(Problem::plain(why))),
        };
        match source.consent_id() {
            Some(consent) if !source::consented(&consent) => {
                let Some(window) = self.controller().and_then(|c| c.window()) else {
                    return;
                };
                let id = self.id();
                let name = source.name();
                sheets::consent(&window, &name, move || {
                    source::consent(&consent);
                    with_panel(id, |panel| panel.start_message(source));
                });
            }
            _ => self.start_message(source),
        }
    }

    fn start_message(&self, source: Source) {
        static NEXT: AtomicU64 = AtomicU64::new(1);
        let Some(repo) = self.repo() else { return };
        if !self.begin(Task::Message, Some(ide_git::Cancel::new())) {
            return;
        }
        let run = NEXT.fetch_add(1, Ordering::Relaxed);
        let files = {
            let state = self.read();
            if state.model.staged.is_empty() {
                state.model.changes.len()
            } else {
                state.model.staged.len()
            }
        };
        {
            let mut state = self.state();
            state.message.progress = match files {
                1 => t("Prüft 1 geänderte Datei auf Secrets …").to_string(),
                n => tf("Prüft {} geänderte Dateien auf Secrets …", &[&n.to_string()]),
            };
            state.message.blocked = None;
            state.message.running = Some(Running {
                id: run,
                before: self.ui().message.text(),
                cancel: ide_agent::Cancel::new(),
                received: String::new(),
            });
        }
        self.update_busy();
        let id = self.id();
        work::long(
            move || {
                let request = repo.message_request()?;
                repo.scan_request(request)
            },
            move |scan| with_panel(id, |panel| panel.message_checked(run, source, scan)),
        );
    }

    /// Whether `run` is the request on its way.
    fn is_running(&self, run: u64) -> bool {
        self.read().message.running.as_ref().is_some_and(|r| r.id == run)
    }

    fn message_checked(&self, run: u64, source: Source, scan: Option<ide_git::Result<RequestScan>>) {
        if !self.is_running(run) {
            return;
        }
        let cancelled = self
            .read()
            .message
            .running
            .as_ref()
            .is_some_and(|r| r.cancel.is_cancelled());
        if cancelled {
            return self.message_done(run, Err(Failure::Cancelled));
        }
        let blocked = match scan {
            Some(Ok(RequestScan::Clean(checked))) if checked.files() > 0 => {
                return self.send_message(run, source, checked);
            }
            Some(Ok(RequestScan::Clean(_))) => {
                t("Es gibt keine Änderungen, aus denen eine Nachricht entstehen könnte.").to_string()
            }
            Some(Ok(RequestScan::NotInstalled)) => tf(
                "Nichts gesendet: Ohne gitleaks lässt sich der Text nicht auf Secrets prüfen. Installiere es mit „{}“.",
                &[INSTALL_GITLEAKS],
            ),
            Some(Ok(RequestScan::Found(findings))) => found(&findings),
            Some(Err(error)) => tf(
                "Nichts gesendet: Die Secrets-Prüfung ist fehlgeschlagen ({}). Prüf gitleaks oder installiere es neu mit „{}“.",
                &[&super::text::problem(&error).message, INSTALL_GITLEAKS],
            ),
            None => t("Git ist unerwartet abgebrochen.").to_string(),
        };
        self.state().message.blocked = Some(blocked.clone());
        self.message_done(run, Err(Failure::Failed(blocked)));
    }

    fn send_message(&self, run: u64, source: Source, checked: CheckedRequest) {
        let cancel = {
            let mut state = self.state();
            state.message.sent = Some((checked.text().to_string(), source.name()));
            state.message.progress = match checked.files() {
                1 => t("Fasst 1 geänderte Datei zusammen …").to_string(),
                n => tf("Fasst {} geänderte Dateien zusammen …", &[&n.to_string()]),
            };
            state.message.running.as_ref().map(|r| r.cancel.clone())
        };
        let Some(cancel) = cancel else { return };
        self.update_busy();
        self.set_streaming(true);
        self.show_text("");
        let id = self.id();
        let spawned = std::thread::Builder::new()
            .name("commit-message".into())
            .spawn(move || {
                let mut forward = |piece: &str| {
                    let piece = piece.to_string();
                    main_soon(move || with_panel(id, |panel| panel.message_piece(run, &piece)));
                };
                let result = source.send(checked.text(), &cancel, &mut forward);
                main_soon(move || with_panel(id, |panel| panel.message_done(run, result)));
            });
        if let Err(error) = spawned {
            self.message_done(run, Err(Failure::Failed(error.to_string())));
        }
    }

    fn message_piece(&self, run: u64, piece: &str) {
        let text = {
            let mut state = self.state();
            let Some(running) = state.message.running.as_mut().filter(|r| r.id == run) else {
                return;
            };
            running.received.push_str(piece);
            running.received.clone()
        };
        self.show_text(&text);
    }

    fn message_done(&self, run: u64, result: Result<String, Failure>) {
        if !self.is_running(run) {
            return;
        }
        let Some(running) = self.state().message.running.take() else {
            return;
        };
        self.set_streaming(false);
        self.end();
        match result.map(|text| cleaned(&text)) {
            Ok(text) if !text.is_empty() => {
                self.put_text(&running.before, &text);
                self.state().message.written = Some(text);
            }
            Ok(_) => {
                self.show_text(&running.before);
                self.set_problem(Some(Problem::plain(t("Das Modell hat keine Nachricht geschrieben."))));
            }
            Err(Failure::Cancelled) => self.show_text(&running.before),
            Err(Failure::Failed(why)) => {
                self.show_text(&running.before);
                let blocked = self.read().message.blocked.is_some();
                let text = if blocked {
                    why
                } else {
                    tf("Die Commit-Nachricht ließ sich nicht erzeugen: {}", &[&why])
                };
                self.set_problem(Some(Problem::plain(text)));
            }
        }
        self.update_placeholder();
        self.update_message_controls();
    }

    /// Stops the request on its way.
    pub fn cancel_message(&self) {
        if let Some(running) = self.read().message.running.as_ref() {
            running.cancel.cancel();
        }
    }

    /// The text changed: a change of the user's ends the note that AI wrote it.
    pub fn message_edited(&self) {
        let ended = {
            let mut state = self.state();
            !state.message.applying && state.message.written.take().is_some()
        };
        if ended {
            self.update_message_controls();
        }
    }

    /// Takes the last change of the message back, a message the model wrote as a whole.
    pub fn undo_message(&self) {
        let undo = self.message_undo();
        if undo.canUndo() {
            undo.undo();
        }
    }

    /// Shows the text that went out, exactly as gitleaks checked it.
    pub fn show_sent(&self) {
        let Some((text, to)) = self.read().message.sent.clone() else {
            return;
        };
        let Some(window) = self.controller().and_then(|c| c.window()) else {
            return;
        };
        sheets::output(
            &window,
            &tf("Dieser Text ging an {}, so wie gitleaks ihn geprüft hat.", &[&to]),
            &text,
        );
    }

    /// The button, its tooltip and the note under the message, as the state is.
    pub fn update_message_controls(&self) {
        let ui = self.ui();
        let (running, written, sent, blocked) = {
            let state = self.read();
            (
                state.message.running.is_some(),
                state.message.written.is_some(),
                state.message.sent.is_some(),
                state.message.blocked.clone(),
            )
        };
        let busy = self.read().busy.is_some();
        let unavailable = self.message_unavailable();
        ui.ai.setEnabled(!busy && !running && unavailable.is_none());
        let tip = unavailable
            .or(blocked)
            .unwrap_or_else(|| t("Commit-Nachricht erzeugen").to_string());
        ui.ai.setToolTip(Some(&ns(&tip)));
        crate::util::set_label(&ui.ai, Some(t("Commit-Nachricht erzeugen")));
        ui.ai.setAccessibilityHelp(Some(&ns(&tip)));
        ui.ai_box.written.setHidden(!written);
        ui.ai_box.sent.setHidden(!sent);
        ui.ai_box.view.setHidden(!written && !sent);
    }

    /// Whether the message waits for a model, during which the user cannot type into it.
    fn set_streaming(&self, on: bool) {
        let view = &self.ui().message;
        let ours: &objc2_app_kit::NSResponder = view;
        if on
            && let Some(window) = view.window()
            && window.firstResponder().is_some_and(|r| std::ptr::eq(&*r, ours))
        {
            window.makeFirstResponder(None);
        }
        view.setEditable(!on);
        view.setSelectable(!on);
    }

    /// Puts `text` into the message without an undo step, as the model's answer streams in.
    fn show_text(&self, text: &str) {
        self.state().message.applying = true;
        let undo = self.message_undo();
        undo.disableUndoRegistration();
        self.ui().message.setString(&ns(text));
        undo.enableUndoRegistration();
        self.state().message.applying = false;
        self.update_placeholder();
    }

    /// Replaces `before` with `text` as one step that undo takes back.
    fn put_text(&self, before: &str, text: &str) {
        self.show_text(before);
        self.state().message.applying = true;
        let view = &self.ui().message;
        let range = NSRange::new(0, view.string().length());
        let replacement = ns(text);
        view.breakUndoCoalescing();
        if view.shouldChangeTextInRange_replacementString(range, Some(&replacement)) {
            view.replaceCharactersInRange_withString(range, &replacement);
            view.didChangeText();
        }
        view.breakUndoCoalescing();
        self.message_undo().setActionName(&ns(t("Commit-Nachricht von AI")));
        self.state().message.applying = false;
    }
}

/// What the message says about possible secrets in the request: file and line of each, up to three.
fn found(findings: &[RequestFinding]) -> String {
    let places: Vec<String> = findings
        .iter()
        .take(3)
        .map(|f| match &f.place {
            Place::File { path, line } => format!("{}:{line}", path.display()),
            Place::Request { line } => tf("Zeile {} der Anfrage", &[&line.to_string()]),
        })
        .collect();
    let mut list = places.join(", ");
    if findings.len() > 3 {
        list.push_str(", …");
    }
    match findings.len() {
        1 => tf(
            "Nichts gesendet: Möglicherweise ein Secret in {} ({}).",
            &[&list, &findings[0].rule],
        ),
        _ => tf("Nichts gesendet: Möglicherweise Secrets in {}.", &[&list]),
    }
}

/// The message without the fences or quotes a model may wrap it in.
fn cleaned(text: &str) -> String {
    let mut text = text.trim();
    if let Some(inner) = text.strip_prefix("```") {
        let inner = inner.split_once('\n').map_or("", |(_, rest)| rest);
        text = inner.strip_suffix("```").unwrap_or(inner).trim();
    }
    for quote in ['"', '“', '„'] {
        if let Some(inner) = text.strip_prefix(quote)
            && let Some(inner) = inner.strip_suffix(['"', '”', '“'])
            && !inner.contains(['"', '”', '“'])
        {
            text = inner.trim();
        }
    }
    text.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fences_and_quotes_around_a_message_go() {
        assert_eq!(cleaned("  Fix the parser\n\nBody.\n"), "Fix the parser\n\nBody.");
        assert_eq!(cleaned("```text\nFix it\n```"), "Fix it");
        assert_eq!(cleaned("```\nFix it\n```\n"), "Fix it");
        assert_eq!(cleaned("\"Fix it\""), "Fix it");
        assert_eq!(cleaned("\"Fix\" the \"thing\""), "\"Fix\" the \"thing\"");
    }

    #[test]
    fn findings_name_file_and_line() {
        let finding = |place: Place| RequestFinding {
            rule: "aws-access-token".into(),
            description: String::new(),
            place,
        };
        let one = found(&[finding(Place::File {
            path: "config.env".into(),
            line: 3,
        })]);
        assert!(
            one.contains("config.env:3") && one.contains("aws-access-token"),
            "{one}"
        );
        let many = found(&[
            finding(Place::File {
                path: "a".into(),
                line: 1,
            }),
            finding(Place::Request { line: 7 }),
        ]);
        assert!(many.contains("a:1") && many.contains('7'), "{many}");
    }
}

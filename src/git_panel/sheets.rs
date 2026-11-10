//! The sheets of the Git panel and the diff tab: confirmations, git's output, findings of the secrets check and the
//! name of a new branch.

use std::cell::Cell;

use block2::RcBlock;
use ide_git::{Finding, PullMode};
use objc2::MainThreadOnly;
use objc2::rc::Retained;
use objc2_app_kit::{
    NSAlert, NSAlertFirstButtonReturn, NSAlertSecondButtonReturn, NSAlertStyle, NSFont, NSModalResponse, NSScrollView,
    NSTextField, NSTextView, NSWindow,
};
use objc2_foundation::{NSPoint, NSRect, NSSize};
use splat_i18n::{t, tf};

use crate::util::{guarded, ns};

/// Asks before something that cannot be undone; `then` runs when the user agrees.
pub fn confirm(window: &NSWindow, message: &str, info: &str, action: &str, then: impl FnOnce() + 'static) {
    let alert = NSAlert::new(window.mtm());
    alert.setAlertStyle(NSAlertStyle::Warning);
    alert.setMessageText(&ns(message));
    alert.setInformativeText(&ns(info));
    let button = alert.addButtonWithTitle(&ns(action));
    button.setHasDestructiveAction(true);
    alert.addButtonWithTitle(&ns(t("Abbrechen")));
    begin(&alert, window, move |agreed| {
        if agreed {
            then();
        }
    });
}

/// Shows what git or a hook printed.
pub fn output(window: &NSWindow, message: &str, text: &str) {
    let alert = NSAlert::new(window.mtm());
    alert.setMessageText(&ns(message));
    alert.addButtonWithTitle(&ns(t("OK")));
    alert.setAccessoryView(Some(&text_box(window, text)));
    begin(&alert, window, |_| {});
}

/// Lists possible secrets before a commit or push; `then` learns whether the user goes on anyway.
pub fn secrets(window: &NSWindow, findings: &[Finding], action: &str, then: impl FnOnce(bool) + 'static) {
    let lines: Vec<String> = findings
        .iter()
        .map(|f| {
            let place = match &f.commit {
                Some(commit) => tf(
                    "{}:{} in {}",
                    &[
                        &f.file.display().to_string(),
                        &f.line.to_string(),
                        &commit.chars().take(8).collect::<String>(),
                    ],
                ),
                None => format!("{}:{}", f.file.display(), f.line),
            };
            format!("{place}\n    {} — {}", f.rule, f.description)
        })
        .collect();
    let alert = NSAlert::new(window.mtm());
    alert.setAlertStyle(NSAlertStyle::Critical);
    alert.setMessageText(&ns(&match findings.len() {
        1 => t("Möglicherweise ein Secret gefunden").to_string(),
        n => tf("Möglicherweise {} Secrets gefunden", &[&n.to_string()]),
    }));
    alert.setInformativeText(&ns(t(
        "gitleaks hält diese Stellen für Zugangsdaten. Prüf sie, bevor sie das Repository verlassen.",
    )));
    alert.setAccessoryView(Some(&text_box(window, &lines.join("\n"))));
    let anyway = alert.addButtonWithTitle(&ns(action));
    anyway.setHasDestructiveAction(true);
    let cancel = alert.addButtonWithTitle(&ns(t("Abbrechen")));
    cancel.setKeyEquivalent(&ns("\r"));
    anyway.setKeyEquivalent(&ns(""));
    begin(&alert, window, then);
}

/// Asks how to join a branch that diverged from its upstream; `then` gets the choice, `None` when the user cancels.
pub fn pull_mode(window: &NSWindow, then: impl FnOnce(Option<PullMode>) + 'static) {
    let alert = NSAlert::new(window.mtm());
    alert.setMessageText(&ns(t("Lokaler und entfernter Branch sind auseinandergelaufen")));
    alert.setInformativeText(&ns(t(
        "Zusammenführen legt einen Merge-Commit an; Rebase setzt deine Commits auf die neuen des Remotes. \
         Beides verwirft nichts.",
    )));
    alert.addButtonWithTitle(&ns(t("Zusammenführen")));
    alert.addButtonWithTitle(&ns(t("Rebase")));
    alert.addButtonWithTitle(&ns(t("Abbrechen")));
    answer(&alert, window, move |response| {
        then(match response {
            r if r == NSAlertFirstButtonReturn => Some(PullMode::Merge),
            r if r == NSAlertSecondButtonReturn => Some(PullMode::Rebase),
            _ => None,
        });
    });
}

/// Asks for the name of a new branch; `then` gets it unless the user cancels or leaves it empty.
pub fn branch_name(window: &NSWindow, then: impl FnOnce(String) + 'static) {
    let mtm = window.mtm();
    let alert = NSAlert::new(mtm);
    alert.setMessageText(&ns(t("Neuer Branch")));
    alert.setInformativeText(&ns(t(
        "Er beginnt beim aktuellen Commit, und Git wechselt gleich zu ihm.",
    )));
    alert.addButtonWithTitle(&ns(t("Anlegen")));
    alert.addButtonWithTitle(&ns(t("Abbrechen")));
    let field = NSTextField::textFieldWithString(&ns(""), mtm);
    field.setFrame(NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(260.0, 24.0)));
    field.setPlaceholderString(Some(&ns(t("feature/name"))));
    alert.setAccessoryView(Some(&field));
    alert.window().setInitialFirstResponder(Some(&field));
    let typed = field.clone();
    begin(&alert, window, move |agreed| {
        let name = typed.stringValue().to_string().trim().to_string();
        if agreed && !name.is_empty() {
            then(name);
        }
    });
}

/// Shows `alert` as a sheet on `window`; `then` learns whether the first button ended it.
fn begin(alert: &NSAlert, window: &NSWindow, then: impl FnOnce(bool) + 'static) {
    answer(alert, window, move |response| {
        then(response == NSAlertFirstButtonReturn)
    });
}

/// Shows `alert` as a sheet on `window`; `then` gets the button that ended it.
pub fn answer(alert: &NSAlert, window: &NSWindow, then: impl FnOnce(NSModalResponse) + 'static) {
    let then = Cell::new(Some(then));
    let handler = RcBlock::new(move |response: NSModalResponse| {
        guarded((), || {
            if let Some(then) = then.take() {
                then(response);
            }
        });
    });
    alert.beginSheetModalForWindow_completionHandler(window, Some(&handler));
}

/// A scrolling box of monospaced text that can be selected and copied.
fn text_box(window: &NSWindow, text: &str) -> Retained<NSScrollView> {
    let mtm = window.mtm();
    let frame = NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(460.0, 200.0));
    let scroll = NSScrollView::initWithFrame(NSScrollView::alloc(mtm), frame);
    scroll.setHasVerticalScroller(true);
    scroll.setHasHorizontalScroller(true);
    scroll.setBorderType(objc2_app_kit::NSBorderType::BezelBorder);
    let view = NSTextView::initWithFrame(NSTextView::alloc(mtm), frame);
    view.setEditable(false);
    view.setSelectable(true);
    view.setRichText(false);
    view.setFont(Some(&NSFont::monospacedSystemFontOfSize_weight(
        11.0,
        // SAFETY: AppKit sets its font weight constants when it loads.
        unsafe { objc2_app_kit::NSFontWeightRegular },
    )));
    view.setString(&ns(text));
    view.setHorizontallyResizable(true);
    view.setMaxSize(NSSize::new(f64::MAX, f64::MAX));
    view.setAutoresizingMask(objc2_app_kit::NSAutoresizingMaskOptions::ViewWidthSizable);
    scroll.setDocumentView(Some(&view));
    scroll
}

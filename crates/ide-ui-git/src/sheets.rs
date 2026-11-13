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

use ide_ui_common::util::{guarded, ns};

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

/// Asks before something is deleted for good; the destructive button is not the default, Return cancels (E101).
pub fn confirm_delete(window: &NSWindow, message: &str, info: &str, action: &str, then: impl FnOnce() + 'static) {
    let alert = NSAlert::new(window.mtm());
    alert.setAlertStyle(NSAlertStyle::Warning);
    alert.setMessageText(&ns(message));
    alert.setInformativeText(&ns(info));
    let delete = alert.addButtonWithTitle(&ns(action));
    delete.setHasDestructiveAction(true);
    let cancel = alert.addButtonWithTitle(&ns(t("Abbrechen")));
    cancel.setKeyEquivalent(&ns("\r"));
    delete.setKeyEquivalent(&ns(""));
    begin(&alert, window, move |agreed| {
        if agreed {
            then();
        }
    });
}

/// Offers to save the open documents `names` before a stash action, which sees only the files on disk; `then` runs
/// when the user chooses to save.
pub fn save_first(window: &NSWindow, names: &[String], then: impl FnOnce() + 'static) {
    let alert = NSAlert::new(window.mtm());
    alert.setMessageText(&ns(&match names {
        [one] => tf("„{}“ hat ungesicherte Änderungen.", &[one]),
        many => tf("{} Dateien haben ungesicherte Änderungen.", &[&many.len().to_string()]),
    }));
    alert.setInformativeText(&ns(t(
        "Der Stash sieht nur, was auf der Platte steht. Sichere zuerst, damit deine Änderungen dabei sind.",
    )));
    alert.addButtonWithTitle(&ns(t("Sichern")));
    alert.addButtonWithTitle(&ns(t("Abbrechen")));
    begin(&alert, window, move |agreed| {
        if agreed {
            then();
        }
    });
}

/// Asks for the message of a new stash entry and whether untracked files go along; `then` gets both.
pub fn stash_options(window: &NSWindow, then: impl FnOnce(String, bool) + 'static) {
    let mtm = window.mtm();
    let alert = NSAlert::new(mtm);
    alert.setMessageText(&ns(t("Änderungen stashen")));
    alert.setInformativeText(&ns(t(
        "Die Änderungen verlassen die Arbeitskopie und lassen sich später wieder anwenden.",
    )));
    alert.addButtonWithTitle(&ns(t("Stashen")));
    alert.addButtonWithTitle(&ns(t("Abbrechen")));
    let field = NSTextField::textFieldWithString(&ns(""), mtm);
    field.setPlaceholderString(Some(&ns(t("Nachricht (freiwillig)"))));
    ide_ui_common::util::set_label(&field, Some(t("Nachricht (freiwillig)")));
    // SAFETY: Without a target and an action the checkbox sends nothing; its state is read when the sheet ends.
    let untracked = unsafe {
        objc2_app_kit::NSButton::checkboxWithTitle_target_action(
            &ns(t("Mit nicht versionierten Dateien")),
            None,
            None,
            mtm,
        )
    };
    let stack = objc2_app_kit::NSStackView::new(mtm);
    stack.setOrientation(objc2_app_kit::NSUserInterfaceLayoutOrientation::Vertical);
    stack.setAlignment(objc2_app_kit::NSLayoutAttribute::Leading);
    stack.setSpacing(ide_ui_common::style::space::M);
    stack.addArrangedSubview(&field);
    stack.addArrangedSubview(&untracked);
    field.widthAnchor().constraintEqualToConstant(260.0).setActive(true);
    stack.setFrame(NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(260.0, 54.0)));
    alert.setAccessoryView(Some(&stack));
    alert.window().setInitialFirstResponder(Some(&field));
    let typed = field.clone();
    begin(&alert, window, move |agreed| {
        if agreed {
            let with = untracked.state() == objc2_app_kit::NSControlStateValueOn;
            then(typed.stringValue().to_string().trim().to_string(), with);
        }
    });
}

/// Asks once whether `provider`, which is not on this Mac, may get the request for a commit message, and says what
/// goes there (E101); `then` runs when the user allows it.
pub fn consent(window: &NSWindow, provider: &str, then: impl FnOnce() + 'static) {
    let alert = NSAlert::new(window.mtm());
    alert.setMessageText(&ns(&tf("Änderungen an {} senden?", &[provider])));
    alert.setInformativeText(&ns(&tf(
        "Für die Commit-Nachricht gehen an {}: der vorgemerkte Diff, ohne Vorgemerktes alle Änderungen, der Name des Branches und die letzten zehn Betreffzeilen. Vorher prüft gitleaks diesen Text auf Secrets; bei einem Fund geht nichts hinaus. Die Erlaubnis lässt sich in den Einstellungen unter „Agent“ zurücknehmen.",
        &[provider],
    )));
    alert.addButtonWithTitle(&ns(t("Erlauben")));
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

/// Asks for the name of a new branch that starts where `info` says; `then` gets it unless the user cancels or leaves
/// it empty.
pub fn branch_name(window: &NSWindow, info: &str, then: impl FnOnce(String) + 'static) {
    let mtm = window.mtm();
    let alert = NSAlert::new(mtm);
    alert.setMessageText(&ns(t("Neuer Branch")));
    alert.setInformativeText(&ns(info));
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
        ide_ui_common::style::SMALL,
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

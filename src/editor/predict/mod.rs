//! Edit prediction in the editor (B4, E92): after a short pause in typing the provider of the settings is asked what
//! comes next; the next keystroke cancels the question, and typing never waits for it (E25). A proposal is bound to
//! the document's revision (E24), the selection and its range, and dropped once any of them changed (Review C6). It
//! shows as ghost text after the caret (E81), with further lines in a box below, as before and after in the box for a
//! change of existing text, or as a hint "Tab: Zeile 42" for a change elsewhere. Tab takes it as one undo step, ⌥→ a
//! word of an insertion, Esc drops it; an open completion list and a snippet's tab stops come first. Only in trusted
//! folders, never for files on the exclusion list, and off per language from the status bar.

pub use ide_predict::guard;
pub mod model;
pub mod provider;
mod shown;

use std::ops::Range;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use ide_predict::{Cancel, Error, Indent, Prediction, Provider, Request};
use objc2::rc::Retained;
use objc2::runtime::{AnyObject, Sel};
use objc2::{DefinedClass, MainThreadMarker, MainThreadOnly, sel};
use objc2_app_kit::{NSControlStateValueOff, NSControlStateValueOn, NSMenuItem, NSTextInputClient};
use objc2_foundation::NSRange;
use splat_i18n::{t, tf};

use super::EditorTextView;
use crate::util::{OnDrop, main_soon, ns};
use model::{Kind, Proposal};
use provider::Blocked;

pub use provider::{Predictions, chosen, settings_changed};
pub use shown::{ask_consent, sign_in_sheet};
#[cfg(feature = "selftest")]
mod selftest_probe;
#[cfg(feature = "selftest")]
pub use selftest_probe::*;

/// How long typing pauses before the provider is asked.
const PAUSE: f64 = 0.25;

/// The tags of the menu items and the status bar's button, which all send `idePrediction:`.
const TAKE: isize = 1;
const TAKE_WORD: isize = 2;
const DROP: isize = 3;
const LANGUAGE: isize = 4;
const STATUS: isize = 5;

/// What edit prediction knows about one editor.
#[derive(Default)]
pub struct State {
    /// The proposal on view, with the provider that made it.
    shown: Option<(Proposal, Arc<dyn Provider>)>,
    /// The question under way: its id and how to stop it.
    asking: Option<(u64, Cancel)>,
    /// Set while the editor itself moves the caret or edits for a proposal, which then neither drops nor asks.
    quiet: bool,
    /// What the provider last said instead of proposals, for the status bar.
    note: Option<String>,
    /// What the secrets check said once, shown until the next edit.
    warning: Option<String>,
    parts: shown::Parts,
}

/// Where edit prediction stands for an editor.
enum Gate {
    Ready(Arc<dyn Provider>),
    Blocked(Blocked, PathBuf),
    /// The file never takes part, for the reason given.
    NotHere(&'static str),
    /// Its language is switched off.
    LanguageOff,
}

/// The language id of `view`'s file, as language servers know it, and its name for the user.
fn language(view: &EditorTextView) -> (String, String) {
    let document = view.document();
    if let Some((language, id)) = document.path().and_then(|path| ide_lsp::Language::detect(path, &[])) {
        return (id, language.name().to_string());
    }
    match document.language() {
        Some(language) => (language.name().to_lowercase(), language.name().to_string()),
        None => ("plaintext".to_string(), t("Text").to_string()),
    }
}

fn gate(view: &EditorTextView) -> Gate {
    let not_here = t("Vorschläge nur in vertrauten Ordnern");
    let Some(path) = view.document().path().map(Path::to_path_buf) else {
        return Gate::NotHere(not_here);
    };
    let Some(controller) = view.controller() else {
        return Gate::NotHere(not_here);
    };
    let root = controller.project_root().filter(|root| path.starts_with(root));
    let Some(root) = root.filter(|_| controller.is_trusted()) else {
        return Gate::NotHere(not_here);
    };
    let chosen = chosen();
    if model::excluded(&path, &chosen.exclude) {
        return Gate::NotHere(t("Vorschläge aus: Datei auf der Ausschlussliste"));
    }
    if chosen.off_for.contains(&language(view).0) {
        return Gate::LanguageOff;
    }
    match provider::provider(&root) {
        Ok(provider) => Gate::Ready(provider),
        Err(blocked) => Gate::Blocked(blocked, root),
    }
}

/// Whether a provider is chosen at all, cheap enough for every keystroke.
fn active() -> bool {
    provider::forced() || chosen().choice != model::Choice::Off
}

/// The selection of `view` in bytes of its text.
fn selection(view: &EditorTextView) -> Range<usize> {
    let range = view.selectedRange();
    let document = view.document();
    let text = document.text();
    let byte = |at: usize| text.utf16_to_byte(at.min(text.len_utf16())).unwrap_or(0);
    byte(range.location)..byte(range.location + range.length)
}

/// Byte `at` of `view`'s text in UTF-16 code units.
fn utf16(view: &EditorTextView, at: usize) -> usize {
    view.document().text().byte_to_utf16(at).unwrap_or(0)
}

/// Runs `f` while the editor's own moves and edits neither drop the proposal nor ask anew.
fn quietly<R>(view: &EditorTextView, f: impl FnOnce() -> R) -> R {
    view.ivars().predict.borrow_mut().quiet = true;
    let _quiet = OnDrop::new(|| view.ivars().predict.borrow_mut().quiet = false);
    f()
}

/// Takes the proposal away and stops the question under way.
fn forget(view: &EditorTextView) {
    let (had, asking) = {
        let mut state = view.ivars().predict.borrow_mut();
        (state.shown.take().is_some(), state.asking.take())
    };
    if let Some((_, cancel)) = asking {
        cancel.cancel();
    }
    if had {
        shown::clear(view);
    }
}

/// The document of `view` changed: the proposal goes, and once typing pauses the provider is asked.
pub fn changed(view: &EditorTextView) {
    let (had, asking) = match view.ivars().predict.try_borrow_mut() {
        Ok(mut state) if !state.quiet => {
            state.warning = None;
            (state.shown.take().is_some(), state.asking.take())
        }
        _ => return,
    };
    if let Some((_, cancel)) = asking {
        cancel.cancel();
    }
    if had {
        // TextKit is still processing the edit, which an attribute change of the insets must not disturb.
        let serial = view.ivars().serial;
        main_soon(move || {
            if let Some(view) = super::find_editor(serial)
                && view.ivars().predict.borrow().shown.is_none()
            {
                shown::clear(&view);
            }
        });
    }
    if active() {
        view.later(sel!(idePredict), PAUSE);
    }
}

/// Text is about to go into `view`: the proposal goes first. Typing after the kerned character of a gap costs AppKit
/// several milliseconds more than after a plain one, which the edit would otherwise pay before the proposal goes.
pub fn before_edit(view: &EditorTextView) {
    if view
        .ivars()
        .predict
        .try_borrow()
        .is_ok_and(|state| !state.quiet && state.shown.is_some())
    {
        forget(view);
    }
}

/// A command of the keyboard that no one took is about to run in `view`: one that deletes or inserts text drops the
/// proposal first, as typing does.
pub fn before_command(view: &EditorTextView, command: Sel) {
    let name = command.name().to_str().unwrap_or_default();
    if name.starts_with("delete") || name.starts_with("insert") {
        before_edit(view);
    }
}

/// The selection of `view` moved: a proposal made for another selection goes.
pub fn moved(view: &EditorTextView) {
    let stale = view.ivars().predict.try_borrow().is_ok_and(|state| {
        !state.quiet
            && state
                .shown
                .as_ref()
                .is_some_and(|(p, _)| p.selection != selection(view))
    });
    if stale {
        forget(view);
    }
}

/// Asks the provider about `view` once typing paused.
pub fn request(view: &EditorTextView) {
    if view.hasMarkedText() {
        view.later(sel!(idePredict), PAUSE);
        return;
    }
    if !super::multi::extra(view).is_empty() || view.ivars().predict.borrow().shown.is_some() {
        return;
    }
    let provider = match gate(view) {
        Gate::Ready(provider) => provider,
        Gate::Blocked(Blocked::Consent, _) => {
            ask_from_editor(view);
            return refresh_status(view);
        }
        Gate::Blocked(..) | Gate::NotHere(_) | Gate::LanguageOff => return refresh_status(view),
    };
    let (request, snapshot) = {
        let document = view.document();
        let Some(path) = document.path().map(Path::to_path_buf) else {
            return;
        };
        let indent = view.ivars().indent.borrow().clone();
        let request = Request {
            path,
            language: language(view).0,
            text: String::new(),
            revision: document.revision(),
            selection: selection(view),
            indent: if indent.starts_with('\t') {
                Indent::Tabs
            } else {
                Indent::Spaces(u8::try_from(indent.len()).unwrap_or(4))
            },
        };
        (request, document.text().clone())
    };
    let id = provider::next_question();
    let cancel = Cancel::new();
    if let Some((_, old)) = view.ivars().predict.borrow_mut().asking.replace((id, cancel.clone())) {
        old.cancel();
    }
    let serial = view.ivars().serial;
    let (revision, asked) = (request.revision, request.selection.clone());
    let from = provider.clone();
    let reply: ide_predict::Reply = Box::new(move |answer| {
        main_soon(move || arrived(serial, id, revision, asked, from, answer));
    });
    let outside = provider::outside();
    let (scans, scan) = (provider::scans(), provider::gitleaks());
    let checking = crate::util::spawn("prediction-request", move || {
        let request = Request {
            text: snapshot.to_string(),
            ..request
        };
        if !outside {
            return provider.predict(request, cancel, reply);
        }
        if let Some(verdict) = guard::send(request, true, &scans, &scan, &*provider, cancel, reply) {
            main_soon(move || checked(serial, &verdict));
        }
    });
    if let Err(error) = checking {
        crate::diag::report("asking for a prediction", error);
    }
}

/// The secrets check of a request of the editor `serial` said `verdict`: a missing gitleaks or a failed check shows in
/// the status bar, a finding pauses the file there.
fn checked(serial: ide_ui_lsp::EditorSerial, verdict: &guard::Verdict) {
    let Some(view) = super::find_editor(serial) else { return };
    let warning = match verdict {
        guard::Verdict::NotInstalled if !crate::context::app().predictions().told_missing.replace(true) => Some(tf(
            "Ohne gitleaks geht der Code ungeprüft hinaus. Installiere es mit „{}“.",
            &[ide_ui_git::message::INSTALL_GITLEAKS],
        )),
        guard::Verdict::Failed(problem) => Some(tf("Die Secrets-Prüfung ist fehlgeschlagen: {}", &[problem])),
        _ => None,
    };
    if warning.is_some() {
        view.ivars().predict.borrow_mut().warning = warning;
    }
    refresh_status(&view);
}

fn arrived(
    serial: ide_ui_lsp::EditorSerial,
    id: u64,
    revision: u64,
    asked: Range<usize>,
    provider: Arc<dyn Provider>,
    answer: Result<Vec<Prediction>, Error>,
) {
    let Some(view) = super::find_editor(serial) else { return };
    {
        let mut state = view.ivars().predict.borrow_mut();
        if state.asking.as_ref().map(|(asking, _)| *asking) != Some(id) {
            return;
        }
        state.asking = None;
        state.note = None;
    }
    match answer {
        Ok(predictions) => {
            if view.document().revision() != revision || selection(&view) != asked {
                return;
            }
            let text = view.document().text().to_string();
            if let Some(proposal) = Proposal::first(&text, revision, asked, predictions) {
                show(&view, proposal, provider);
            }
        }
        Err(Error::Cancelled) => {}
        Err(Error::SignIn) => view.ivars().predict.borrow_mut().note = Some(t("Copilot: Anmeldung nötig").to_string()),
        Err(Error::Unavailable(text)) => view.ivars().predict.borrow_mut().note = Some(text),
        Err(Error::Malformed(text)) => crate::diag::report("an edit prediction", text),
    }
    refresh_status(&view);
}

fn show(view: &EditorTextView, proposal: Proposal, provider: Arc<dyn Provider>) {
    shown::clear(view);
    shown::present(view, &proposal);
    view.ivars().predict.borrow_mut().shown = Some((proposal, provider));
}

/// The proposal on view, if it still fits the document; one that no longer does goes.
fn current(view: &EditorTextView) -> Option<(Proposal, Arc<dyn Provider>)> {
    let shown = view.ivars().predict.borrow().shown.clone()?;
    let holds = {
        let document = view.document();
        let text = document.text().to_string();
        shown.0.holds(document.revision(), &selection(view), &text)
    };
    if !holds {
        forget(view);
        return None;
    }
    Some(shown)
}

/// A command of the keyboard reached `view`: Tab, ⌥→ and Esc act on a proposal; whether one took it.
pub fn command(view: &EditorTextView, command: Sel) -> bool {
    let (tab, word, escape) = (
        command == sel!(insertTab:),
        command == sel!(moveWordRight:),
        command == sel!(cancelOperation:),
    );
    if !(tab || word || escape) {
        return false;
    }
    let Some((proposal, provider)) = current(view) else {
        return false;
    };
    if tab {
        take(view, proposal, &provider);
        true
    } else if word {
        take_word(view, &proposal, &provider)
    } else {
        forget(view);
        true
    }
}

/// Replaces UTF-16 `range` of `view` with `text` as one undoable step that flows into the document like typing, and
/// puts the caret after it in view, as typing does.
fn replace(view: &EditorTextView, range: Range<usize>, text: &str) {
    let Some(storage) = view.storage() else { return };
    super::undo_step(view, || {
        let range = NSRange::new(range.start, range.end - range.start);
        let text = ns(text);
        if view.shouldChangeTextInRange_replacementString(range, Some(&text)) {
            storage.replaceCharactersInRange_withString(range, &text);
            view.didChangeText();
        }
    });
    let caret = NSRange::new(range.start + text.encode_utf16().count(), 0);
    view.setSelectedRange(caret);
    // Scrolling has TextKit 2 lay the edited line out again; without it, an undo in the same turn of the run loop that
    // shortens the text trips UIFoundation's assertion "NSTextContainer range out of document range".
    view.scrollRangeToVisible(caret);
}

/// The first Tab on a proposal elsewhere goes there and shows it; any other Tab takes it.
fn take(view: &EditorTextView, proposal: Proposal, provider: &Arc<dyn Provider>) {
    if let Kind::Elsewhere { .. } = proposal.kind {
        let at = utf16(view, proposal.range.start);
        quietly(view, || {
            view.setSelectedRange(NSRange::new(at, 0));
            view.scrollRangeToVisible(NSRange::new(at, 0));
        });
        let text = view.document().text().to_string();
        let there = proposal.moved(&text, selection(view));
        show(view, there, provider.clone());
        return;
    }
    let range = utf16(view, proposal.range.start)..utf16(view, proposal.range.end);
    forget(view);
    quietly(view, || replace(view, range, &proposal.text));
    provider.accepted(&proposal.prediction);
    crate::window::announce(view.mtm(), t("Vorschlag übernommen"));
    view.later(sel!(idePredict), PAUSE);
}

/// ⌥→ takes the first word of an insertion and leaves the rest on view; whether there was one.
fn take_word(view: &EditorTextView, proposal: &Proposal, provider: &Arc<dyn Provider>) -> bool {
    let Some(word) = proposal.word().map(str::to_string) else {
        return false;
    };
    let at = utf16(view, proposal.range.start);
    forget(view);
    quietly(view, || replace(view, at..at, &word));
    let (text, revision) = {
        let document = view.document();
        (document.text().to_string(), document.revision())
    };
    match proposal.rest(word.len(), &text, revision) {
        Some(rest) => show(view, rest, provider.clone()),
        None => {
            provider.accepted(&proposal.prediction);
            crate::window::announce(view.mtm(), t("Vorschlag übernommen"));
            view.later(sel!(idePredict), PAUSE);
        }
    }
    true
}

/// Asks once per run, from the editor, whether the chosen provider outside the Mac may get text.
fn ask_from_editor(view: &EditorTextView) {
    let predictions = crate::context::app();
    if predictions.predictions().asked_consent.replace(true) {
        return;
    }
    let Some(window) = view.window() else { return };
    let name = provider::name(chosen().choice);
    let serial = view.ivars().serial;
    ask_consent(&window, name, move || {
        provider::consent();
        if let Some(view) = super::find_editor(serial) {
            request(&view);
        }
    });
}

/// The settings or a provider's state changed: proposals that may no longer show go, and the status bars follow.
pub fn follow_settings() {
    for view in super::all_editors() {
        if !matches!(gate(&view), Gate::Ready(..)) {
            forget(&view);
        }
        refresh_status(&view);
    }
}

/// Copilot became ready for `root`: its editors ask once more.
pub fn ask_again(root: &Path) {
    for view in super::all_editors() {
        if view.document().path().is_some_and(|p| p.starts_with(root)) {
            view.later(sel!(idePredict), PAUSE);
        }
    }
}

/// Turns proposals on or off for the language with id `id`.
pub fn toggle_language(id: &str) {
    let mut off = chosen().off_for;
    if off.iter().any(|l| l == id) {
        off.retain(|l| l != id);
    } else {
        off.push(id.to_string());
        off.sort();
    }
    let mut settings = ide_core::settings::Settings::load_user();
    if let Err(error) = settings.set(&model::OFF_FOR, &off) {
        crate::diag::report("saving the languages without edit prediction", error);
    }
    settings_changed();
    crate::settings::prediction_changed();
}

/// A menu item or the status bar's button with `idePrediction:` was used.
pub fn perform(view: &EditorTextView, sender: Option<&AnyObject>) {
    let tag = sender
        .and_then(|s| s.downcast_ref::<NSMenuItem>().map(NSMenuItem::tag))
        .or_else(|| sender.and_then(|s| s.downcast_ref::<objc2_app_kit::NSControl>().map(|c| c.tag())))
        .unwrap_or(0);
    match tag {
        TAKE | TAKE_WORD => {
            if let Some((proposal, provider)) = current(view) {
                if tag == TAKE {
                    take(view, proposal, &provider);
                } else {
                    take_word(view, &proposal, &provider);
                }
            }
        }
        DROP => forget(view),
        LANGUAGE => toggle_language(&language(view).0),
        STATUS => status_pressed(view),
        _ => {}
    }
}

/// The status bar's button: switches the language on or off where proposals can come, else leads to what is missing.
fn status_pressed(view: &EditorTextView) {
    match gate(view) {
        Gate::Ready(..) | Gate::LanguageOff => toggle_language(&language(view).0),
        Gate::Blocked(Blocked::Consent, _) => {
            crate::context::app().predictions().asked_consent.set(false);
            ask_from_editor(view);
        }
        Gate::Blocked(Blocked::Missing, root) => provider::install(&root),
        Gate::Blocked(..) => crate::settings::show_prediction(view.mtm()),
        Gate::NotHere(_) => {}
    }
}

/// The menu items of edit prediction: greyed out without a proposal, the language's named after it and ticked while
/// proposals come for it; `None` for every other item.
pub fn validate(view: &EditorTextView, item: &NSMenuItem) -> Option<bool> {
    if item.action()? != sel!(idePrediction:) {
        return None;
    }
    let shown = view.ivars().predict.borrow().shown.clone();
    Some(match item.tag() {
        TAKE | DROP => shown.is_some(),
        TAKE_WORD => shown.is_some_and(|(p, _)| p.word().is_some()),
        LANGUAGE => {
            let (id, name) = language(view);
            item.setTitle(&ns(&tf("Vorschläge für {}", &[&name])));
            let on = !chosen().off_for.contains(&id);
            item.setState(if on {
                NSControlStateValueOn
            } else {
                NSControlStateValueOff
            });
            active() && view.document().path().is_some()
        }
        _ => true,
    })
}

/// Adds edit prediction's commands to the Editor menu.
pub fn with_menu_items(mtm: MainThreadMarker, code_menu: Retained<NSMenuItem>) -> Retained<NSMenuItem> {
    let Some(menu) = code_menu.submenu() else {
        return code_menu;
    };
    let entry = |title: &str, tag: isize| crate::util::tagged(mtm, title, sel!(idePrediction:), "", tag);
    for item in [
        crate::util::separator(mtm),
        entry(t("Vorschlag übernehmen"), TAKE),
        entry(t("Wort des Vorschlags übernehmen"), TAKE_WORD),
        entry(t("Vorschlag verwerfen"), DROP),
        entry(&tf("Vorschläge für {}", &["…"]), LANGUAGE),
    ] {
        menu.addItem(&item);
    }
    code_menu
}

/// Adds the switch of edit prediction to the status bar's `stack` of `view`.
pub fn status_items(mtm: MainThreadMarker, view: &EditorTextView, stack: &objc2_app_kit::NSStackView) {
    shown::status_item(mtm, view, stack, STATUS);
    refresh_status(view);
}

/// What the status bar of `view` says about edit prediction, and whether its button does something.
fn status(view: &EditorTextView) -> (String, String, bool) {
    let (_, name) = language(view);
    let provider = if provider::forced() {
        "Stub"
    } else {
        provider::name(chosen().choice)
    };
    let (note, warning) = {
        let state = view.ivars().predict.borrow();
        (state.note.clone(), state.warning.clone())
    };
    let paused = provider::outside() && view.document().path().is_some_and(|p| provider::scans().paused(p));
    match gate(view) {
        Gate::Ready(..) if paused => {
            let why = t("Vorschläge pausiert: Die Datei enthält ein Secret");
            (why.to_string(), why.to_string(), false)
        }
        Gate::Ready(..) => match note.or(warning) {
            Some(note) => (note.clone(), note, true),
            None => (
                provider.to_string(),
                tf("Vorschläge für {} ausschalten", &[&name]),
                true,
            ),
        },
        Gate::LanguageOff => (
            tf("{}: aus für {}", &[provider, &name]),
            tf("Vorschläge für {} einschalten", &[&name]),
            true,
        ),
        Gate::Blocked(blocked, _) => {
            let text = match blocked {
                Blocked::Off => t("Vorschläge aus").to_string(),
                Blocked::Missing => t("Copilot fehlt – installieren?").to_string(),
                Blocked::SignIn => t("Copilot: Anmeldung nötig").to_string(),
                Blocked::Consent => tf("{}: Erlaubnis nötig", &[provider]),
                Blocked::Busy(text) | Blocked::Unavailable(text) => text,
            };
            (text, t("Einstellungen …").to_string(), true)
        }
        Gate::NotHere(why) => (why.to_string(), why.to_string(), false),
    }
}

fn refresh_status(view: &EditorTextView) {
    let (title, tip, enabled) = status(view);
    shown::set_status(view, &title, &tip, enabled);
}

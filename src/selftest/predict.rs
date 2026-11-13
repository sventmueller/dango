//! The `edit-prediction` scenario (B4, E92) with the stub provider: a pause in typing asks, the answer shows as ghost
//! text after the caret; ⌥→ takes a word, Tab the rest as one undo step, and the provider hears of it; Esc
//! drops a proposal; an answer that arrives after a further edit never shows; an open completion list and a snippet's
//! tab stops win over Tab; a replacement and a deletion show as before and after with their range marked; a change
//! elsewhere shows "Tab: Zeile 1", the first Tab goes there and the second takes it; further lines and an insertion at a
//! line's start go into the box under the line; a language switched off and a file on the exclusion list ask nothing;
//! a provider outside the Mac waits for consent; and a keystroke with a proposal on view stays within E25's budget.

use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use ide_predict::stub::Stub;
use ide_predict::{Prediction, Provider};
use objc2::rc::Retained;
use objc2::runtime::Sel;
use objc2::{MainThreadMarker, sel};
use objc2_app_kit::{NSStandardKeyBindingResponding, NSTextView, NSWindow};
use objc2_foundation::NSRange;

use super::{check, finish, snapshot};
use crate::editor::insets::{self, Layer};
use crate::editor::predict::{self, model::Kind};
use crate::editor::{self, EditorTextView};
use crate::util::main_after;
use crate::window;

const SCENARIO: &str = "edit-prediction";
const MAIN: &str = "fn main() {\n    let total = sc\n    let other = 1;\n}\n";
/// Long enough for the pause before asking and the stub's answer.
const SETTLE: Duration = Duration::from_millis(450);

/// The stub answering now.
static STUB: Mutex<Option<Arc<Stub>>> = Mutex::new(None);

fn folder() -> PathBuf {
    ide_core::paths::home().join("predict-project")
}

fn root() -> PathBuf {
    let folder = folder();
    folder.canonicalize().unwrap_or(folder)
}

fn open(file: &str) -> Option<Retained<EditorTextView>> {
    let controller = window::for_project(&root())?;
    let path = root().join(file);
    controller.open_file(&path);
    editor::editor_for(&controller, &path)
}

fn main_view() -> Option<Retained<EditorTextView>> {
    open("src/main.rs")
}

fn text(view: &EditorTextView) -> String {
    view.document().text().to_string()
}

fn picture(view: &EditorTextView, name: &str) {
    if let Some(window) = view.window() {
        window.displayIfNeeded();
        snapshot(&window, name);
    }
}

fn after(delay: Duration, next: fn()) {
    main_after(delay, next);
}

/// The stub's counts of questions and of proposals taken.
fn counts() -> (usize, usize) {
    STUB.lock()
        .ok()
        .and_then(|s| s.clone())
        .map_or((0, 0), |stub| (stub.asked().len(), stub.accepted_predictions().len()))
}

/// Has a new stub answer the next question with `range` of the text replaced by `new`, after `delay`.
fn answer(range: std::ops::Range<usize>, new: &str, delay: Duration) {
    let prediction = Prediction {
        range,
        text: new.into(),
        feedback: Some(serde_json::json!({"command": "taken"})),
    };
    let stub = Arc::new(Stub::new(vec![Ok(vec![prediction])], delay));
    if let Ok(mut current) = STUB.lock() {
        *current = Some(stub.clone());
    }
    predict::provider::force(Some(stub));
}

/// Byte offset of the end of the first `needle` in the view's text.
fn after_text(view: &EditorTextView, needle: &str) -> usize {
    let text = text(view);
    text.find(needle).map_or(0, |at| at + needle.len())
}

fn caret_at_byte(view: &EditorTextView, byte: usize) {
    let at = view.document().text().byte_to_utf16(byte).unwrap_or(0);
    view.setSelectedRange(NSRange::new(at, 0));
}

fn press(view: &EditorTextView, command: Sel) {
    let text_view: &NSTextView = view;
    // SAFETY: A selector of the standard key bindings, which the text view answers, as a key press would send it.
    unsafe { text_view.doCommandBySelector(command) };
    text_view.breakUndoCoalescing();
    super::helpers::post_event();
}

fn kind(view: &EditorTextView) -> Option<Kind> {
    predict::shown_proposal(view).map(|(_, _, kind)| kind)
}

/// Asks at once, as the end of a pause in typing does, in place of a pause an earlier step's edit left waiting.
fn ask(view: &EditorTextView) {
    use objc2_foundation::NSObjectNSDelayedPerforming;
    let object: &objc2::runtime::NSObject = view;
    // SAFETY: Cancels a delayed call of a selector without arguments that this view answers; nothing else is touched.
    unsafe {
        objc2::runtime::NSObject::cancelPreviousPerformRequestsWithTarget_selector_object(
            object,
            sel!(idePredict),
            None,
        );
    }
    predict::request(view);
}

pub fn run() {
    let folder = folder();
    let written = std::fs::create_dir_all(folder.join("src"))
        .and_then(|()| std::fs::write(folder.join("src/main.rs"), MAIN))
        .and_then(|()| std::fs::write(folder.join(".env.local"), "TOKEN=1\n"))
        .and_then(|()| {
            let secret = format!("fn token() -> &'static str {{\n    \"{}\"\n}}\n", token());
            std::fs::write(folder.join("src/secret.rs"), secret)
        })
        .and_then(|()| std::fs::write(folder.join("big.rs"), big()));
    if let Err(error) = written {
        check("the project is made", false, error.to_string());
        return finish(SCENARIO);
    }
    let Some(mtm) = MainThreadMarker::new() else { return };
    crate::project::trust(&folder);
    crate::app::open_project(mtm, folder);
    after(Duration::from_millis(400), ghost);
}

fn big() -> String {
    (0..10_000)
        .map(|n| format!("    let value_{n} = compute({n}, other_{n});\n"))
        .collect()
}

/// Typing pauses, the stub answers, and the answer shows as ghost text after the caret.
fn ghost() {
    let Some(view) = main_view() else {
        check("the file opens", false, "");
        return finish(SCENARIO);
    };
    let caret = after_text(&view, "total = sc");
    caret_at_byte(&view, caret);
    answer(caret + 1..caret + 1, "le(21, 2);", Duration::from_millis(10));
    editor::type_text(&view, "a");
    after(SETTLE, ghost_shown);
}

fn ghost_shown() {
    let Some(view) = main_view() else {
        return finish(SCENARIO);
    };
    let asked = STUB
        .lock()
        .ok()
        .and_then(|s| s.clone())
        .map(|s| s.asked())
        .unwrap_or_default();
    check(
        "a pause in typing asks the provider once, with the document's text, revision and caret",
        asked.len() == 1
            && asked[0].text == text(&view)
            && asked[0].revision == view.document().revision()
            && asked[0].language == "rust",
        format!("{asked:?}"),
    );
    let ghosts = insets::of(&view, Layer::Prediction);
    check(
        "the proposal shows as ghost text right after the caret at the line's end",
        kind(&view)
            == Some(Kind::Insert {
                first: "le(21, 2);".into(),
                rest: None,
            })
            && ghosts.len() == 1
            && ghosts[0].ghost
            && insets::shown(&view)
                .iter()
                .any(|s| s.layer == Layer::Prediction && !s.in_gap),
        format!("{ghosts:?}; {:?}", insets::shown(&view)),
    );
    check(
        "the status bar names the provider",
        predict::status_shown(&view).is_some_and(|(title, on)| title == "Stub" && on),
        format!("{:?}", predict::status_shown(&view)),
    );
    check(
        "the menu offers to take the proposal",
        menu_enabled(&view, 1) == Some(true),
        format!("{:?}", menu_enabled(&view, 1)),
    );
    picture(&view, "predict-ghost");
    press(&view, sel!(moveWordRight:));
    check(
        "⌥→ takes one word and leaves the rest on view",
        text(&view).contains("total = scale\n")
            && predict::shown_proposal(&view).is_some_and(|(_, rest, _)| rest == "(21, 2);"),
        format!("{:?} {:?}", text(&view), predict::shown_proposal(&view)),
    );
    press(&view, sel!(insertTab:));
    check(
        "Tab takes the rest and the provider hears of it",
        text(&view).contains("total = scale(21, 2);\n") && counts().1 == 1 && kind(&view).is_none(),
        format!("{:?}, {:?}", text(&view), counts()),
    );
    after(Duration::from_millis(50), undo_take);
}

/// The menu item of edit prediction with `tag`: whether it is enabled for `view`.
fn menu_enabled(view: &EditorTextView, tag: isize) -> Option<bool> {
    let mtm = MainThreadMarker::new()?;
    let main = objc2_app_kit::NSApplication::sharedApplication(mtm).mainMenu()?;
    let item = main
        .itemArray()
        .iter()
        .filter_map(|top| top.submenu())
        .find_map(|menu| {
            menu.itemArray()
                .iter()
                .find(|item| item.action() == Some(sel!(idePrediction:)) && item.tag() == tag)
        })?;
    use objc2_app_kit::NSMenuItemValidation;
    Some(view.validateMenuItem(&item))
}

fn undo_take() {
    let Some(view) = main_view() else {
        return finish(SCENARIO);
    };
    let Some(undo) = view.undoManager() else {
        check("the editor has an undo manager", false, "");
        return finish(SCENARIO);
    };
    undo.undo();
    let once = text(&view);
    undo.undo();
    check(
        "undo takes back what Tab took in one step, then the word",
        once.contains("total = scale\n") && text(&view).contains("total = sca\n"),
        format!("{once:?} then {:?}", text(&view)),
    );
    let caret = after_text(&view, "total = ");
    caret_at_byte(&view, caret);
    answer(caret..caret, "2 * ", Duration::from_millis(10));
    ask(&view);
    after(Duration::from_millis(150), escape);
}

fn escape() {
    let Some(view) = main_view() else {
        return finish(SCENARIO);
    };
    if let Some(window) = view.window() {
        window.displayIfNeeded();
    }
    let gap = insets::shown(&view)
        .into_iter()
        .find(|s| s.layer == Layer::Prediction && s.in_gap);
    let at = view.selectedRange().location;
    let caret = crate::lsp::rect_of(&view, at..at);
    check(
        "in the middle of a line the ghost text opens a gap, with the caret at its left edge",
        gap.as_ref()
            .zip(caret)
            .is_some_and(|(gap, caret)| (caret.origin.x - gap.rect.origin.x).abs() < 2.0),
        format!("{gap:?}, caret {caret:?}"),
    );
    picture(&view, "predict-gap");
    let before = text(&view);
    let had = kind(&view).is_some();
    press(&view, sel!(cancelOperation:));
    check(
        "Esc drops the proposal and changes nothing",
        had && kind(&view).is_none() && text(&view) == before && insets::of(&view, Layer::Prediction).is_empty(),
        format!("shown before: {had}"),
    );
    check(
        "without a proposal the menu item is greyed out",
        menu_enabled(&view, 1) == Some(false),
        "",
    );
    let caret = after_text(&view, "total = sca");
    caret_at_byte(&view, caret);
    answer(caret..caret, "le", Duration::from_millis(300));
    ask(&view);
    editor::type_text(&view, "l");
    after(Duration::from_millis(600), late);
}

fn late() {
    let Some(view) = main_view() else {
        return finish(SCENARIO);
    };
    check(
        "an answer that arrives after a further edit never shows",
        kind(&view).is_none() && counts().0 >= 1,
        format!("{:?}", predict::shown_proposal(&view)),
    );
    if let Some(undo) = view.undoManager() {
        undo.undo();
    }
    let caret = after_text(&view, "total = sca");
    caret_at_byte(&view, caret);
    answer(caret..caret, "le", Duration::from_millis(10));
    ask(&view);
    after(Duration::from_millis(150), list_first);
}

fn list_first() {
    let Some(view) = main_view() else {
        return finish(SCENARIO);
    };
    let had = kind(&view).is_some();
    crate::lsp::completion::show(&view, &["scale_factor"]);
    let listed = crate::lsp::completion::shown(&view);
    press(&view, sel!(insertTab:));
    check(
        "an open completion list takes Tab before the proposal",
        had && listed == vec!["scale_factor".to_string()]
            && text(&view).contains("total = scale_factor\n")
            && counts().1 == 0,
        format!("{listed:?}, {:?}", text(&view)),
    );
    if let Some(undo) = view.undoManager() {
        undo.undo();
    }
    let caret = after_text(&view, "total = sca");
    caret_at_byte(&view, caret);
    answer(caret..caret, "le", Duration::from_millis(10));
    ask(&view);
    after(Duration::from_millis(150), snippet_first);
}

fn snippet_first() {
    let Some(view) = main_view() else {
        return finish(SCENARIO);
    };
    let had = kind(&view).is_some();
    let before = text(&view);
    crate::lsp::completion::snippet(&view, std::iter::once(4..6).collect());
    press(&view, sel!(insertTab:));
    let selected = view.selectedRange();
    check(
        "a snippet's next tab stop takes Tab before the proposal",
        had && text(&view) == before && (selected.location, selected.length) == (4, 2) && counts().1 == 0,
        format!("{selected:?}"),
    );
    let caret = after_text(&view, "other = 1;");
    caret_at_byte(&view, caret);
    let start = after_text(&view, "let ");
    let start = text(&view)[start..].find("other").map_or(0, |i| start + i);
    answer(start..start + 5, "rest", Duration::from_millis(10));
    ask(&view);
    after(Duration::from_millis(150), replace);
}

fn replace() {
    let Some(view) = main_view() else {
        return finish(SCENARIO);
    };
    let boxed = predict::boxed_text(&view).unwrap_or_default();
    check(
        "a replacement shows before and after in the box, its range marked",
        kind(&view) == Some(Kind::Change)
            && boxed.contains("− other")
            && boxed.contains("+ rest")
            && predict::marked(&view).is_some_and(|r| r.len() == 5),
        format!("{boxed:?}, {:?}", predict::marked(&view)),
    );
    picture(&view, "predict-change");
    press(&view, sel!(insertTab:));
    let taken = text(&view).contains("let rest = 1;") && predict::marked(&view).is_none();
    if let Some(undo) = view.undoManager() {
        undo.undo();
    }
    check(
        "Tab replaces, and undo puts it back",
        taken && text(&view).contains("let other = 1;"),
        text(&view),
    );
    let caret = after_text(&view, "other = 1;");
    caret_at_byte(&view, caret);
    let from = after_text(&view, "let other");
    answer(from..from + 4, "", Duration::from_millis(10));
    ask(&view);
    after(Duration::from_millis(150), delete);
}

fn delete() {
    let Some(view) = main_view() else {
        return finish(SCENARIO);
    };
    let boxed = predict::boxed_text(&view).unwrap_or_default();
    let shown = kind(&view) == Some(Kind::Change) && boxed.contains("− ") && !boxed.contains('+');
    press(&view, sel!(insertTab:));
    let taken = text(&view).contains("let other;");
    if let Some(undo) = view.undoManager() {
        undo.undo();
    }
    check(
        "a deletion shows what goes, Tab deletes it, undo brings it back",
        shown && taken && text(&view).contains("let other = 1;"),
        format!("{boxed:?}"),
    );
    let caret = after_text(&view, "other = 1;");
    caret_at_byte(&view, caret);
    let main = text(&view).find("main").unwrap_or(0);
    answer(main..main + 4, "start", Duration::from_millis(10));
    ask(&view);
    after(Duration::from_millis(150), jump);
}

fn jump() {
    let Some(view) = main_view() else {
        return finish(SCENARIO);
    };
    let hint = insets::of(&view, Layer::Prediction);
    check(
        "a change elsewhere shows where it is",
        kind(&view) == Some(Kind::Elsewhere { line: 0 }) && hint.first().is_some_and(|h| h.text == "Tab: Zeile 1"),
        format!("{hint:?}"),
    );
    picture(&view, "predict-jump");
    press(&view, sel!(insertTab:));
    let caret = view.selectedRange().location;
    check(
        "the first Tab goes there and shows the change",
        kind(&view) == Some(Kind::Change) && caret == 3 && text(&view).contains("fn main()"),
        format!("caret {caret}, {:?}", kind(&view)),
    );
    press(&view, sel!(insertTab:));
    check(
        "the second Tab takes it",
        text(&view).starts_with("fn start()") && counts().1 == 1,
        text(&view),
    );
    if let Some(undo) = view.undoManager() {
        undo.undo();
    }
    let caret = after_text(&view, "other = 1;");
    caret_at_byte(&view, caret);
    answer(caret..caret, " // one\n    let more = 2;", Duration::from_millis(10));
    ask(&view);
    after(Duration::from_millis(150), lines);
}

fn lines() {
    let Some(view) = main_view() else {
        return finish(SCENARIO);
    };
    let ghost = insets::of(&view, Layer::Prediction);
    check(
        "the first line of an insertion goes into the text, the rest into the box below",
        ghost.first().is_some_and(|g| g.text == " // one" && g.ghost)
            && predict::boxed_text(&view).as_deref() == Some("    let more = 2;"),
        format!("{ghost:?}, {:?}", predict::boxed_text(&view)),
    );
    picture(&view, "predict-lines");
    press(&view, sel!(cancelOperation:));
    let start = after_text(&view, "sca\n");
    caret_at_byte(&view, start);
    answer(start..start, "// note", Duration::from_millis(10));
    ask(&view);
    after(Duration::from_millis(150), line_start);
}

fn line_start() {
    let Some(view) = main_view() else {
        return finish(SCENARIO);
    };
    check(
        "at a line's start, where no gap can open, the proposal shows in the box (E81)",
        insets::of(&view, Layer::Prediction).is_empty() && predict::boxed_text(&view).as_deref() == Some("// note"),
        format!("{:?}", predict::boxed_text(&view)),
    );
    press(&view, sel!(cancelOperation:));
    predict::toggle_language("rust");
    let off = predict::status_shown(&view);
    let asked = counts().0;
    ask(&view);
    predict::toggle_language("rust");
    check(
        "a language switched off in the status bar asks nothing",
        counts().0 == asked && off.as_ref().is_some_and(|(title, _)| title.contains("Rust")),
        format!("{off:?}"),
    );
    let Some(env) = open(".env.local") else {
        check("the excluded file opens", false, "");
        return finish(SCENARIO);
    };
    ask(&env);
    check(
        "a file on the exclusion list asks nothing and says why",
        counts().0 == asked && predict::status_shown(&env).is_some_and(|(_, on)| !on),
        format!("{:?}", predict::status_shown(&env)),
    );
    consent();
    after(Duration::from_millis(100), secrets);
}

/// A token of the shape gitleaks' rule `github-pat` finds, put together here so that this file holds none.
fn token() -> String {
    format!("ghp_{}{}", "Xk7Pq2Lm9Rt4Vw1Zb8", "Nc5Hd3Jf6Gs0Ya2Ue7")
}

/// Times the secrets check of the big file twice on a worker thread, then asks about a file with a secret through a
/// provider outside the Mac.
fn secrets() {
    let Some(big) = open("big.rs") else {
        check("the big file opens", false, "");
        return finish(SCENARIO);
    };
    let (text, folder) = (text(&big), root());
    let scan = predict::provider::gitleaks();
    let spawned = std::thread::Builder::new()
        .name("selftest-gitleaks".into())
        .spawn(move || {
            let mut times = Vec::new();
            let mut verdicts = Vec::new();
            for _ in 0..2 {
                let started = std::time::Instant::now();
                verdicts.push(scan(&folder, &text));
                times.push(started.elapsed());
            }
            crate::util::main_soon(move || secrets_timed(&times, &verdicts));
        });
    if spawned.is_err() {
        check("the secrets check starts", false, "");
        finish(SCENARIO);
    }
}

fn secrets_timed(times: &[Duration], verdicts: &[predict::guard::Verdict]) {
    let missing = verdicts.contains(&predict::guard::Verdict::NotInstalled);
    check(
        "the secrets check of a 10 000-line file runs once per revision, off the main thread",
        missing || verdicts.iter().all(|v| *v == predict::guard::Verdict::Clean),
        format!("{verdicts:?} in {times:?}"),
    );
    let Some(view) = open("src/secret.rs") else {
        check("the file with a secret opens", false, "");
        return finish(SCENARIO);
    };
    let end = after_text(&view, "\"\n");
    caret_at_byte(&view, end);
    let stub = Arc::new(Stub::new(vec![Ok(vec![]), Ok(vec![])], Duration::from_millis(10)));
    if let Ok(mut current) = STUB.lock() {
        *current = Some(stub.clone());
    }
    predict::provider::force_outside(stub);
    ask(&view);
    after(
        Duration::from_millis(1500),
        if missing { secrets_unchecked } else { secret_found },
    );
}

/// Without gitleaks the text goes out, and the status bar says so once.
fn secrets_unchecked() {
    let Some(view) = open("src/secret.rs") else {
        return finish(SCENARIO);
    };
    check(
        "skipped: gitleaks missing; the text goes out unchecked and the status bar says so",
        counts().0 == 1 && predict::status_shown(&view).is_some_and(|(title, _)| title.contains("gitleaks")),
        format!("{:?}", predict::status_shown(&view)),
    );
    predict::provider::force(None);
    budget();
}

fn secret_found() {
    let Some(view) = open("src/secret.rs") else {
        return finish(SCENARIO);
    };
    let status = predict::status_shown(&view);
    check(
        "a request for a provider outside the Mac with a secret in it sends nothing and pauses the file",
        counts().0 == 0
            && status
                .as_ref()
                .is_some_and(|(title, on)| title.contains("Secret") && !on),
        format!("{status:?}"),
    );
    picture(&view, "predict-secret");
    let token = token();
    let start = text(&view).find(&token).unwrap_or(0);
    let from = view.document().text().byte_to_utf16(start).unwrap_or(0);
    view.setSelectedRange(NSRange::new(from, token.len()));
    editor::type_text(&view, "none");
    let end = after_text(&view, "\"\n");
    caret_at_byte(&view, end);
    ask(&view);
    after(Duration::from_millis(1500), secret_gone);
}

fn secret_gone() {
    let Some(view) = open("src/secret.rs") else {
        return finish(SCENARIO);
    };
    let status = predict::status_shown(&view);
    check(
        "once a later revision checks clean, the request goes out and the pause ends",
        counts().0 == 1 && status.as_ref().is_some_and(|(title, _)| !title.contains("Secret")),
        format!("{status:?}"),
    );
    predict::provider::force(None);
    budget();
}

/// A local server outside the Mac waits for consent, which the settings take back.
fn consent() {
    let Some(view) = main_view() else { return };
    let mut settings = ide_core::settings::Settings::load_user();
    let remote = predict::model::Local {
        url: "http://192.0.2.1:11434".into(),
        model: "qwen2.5-coder:1.5b".into(),
    };
    let stored = settings
        .set(&predict::model::PROVIDER, &predict::model::Choice::Ollama)
        .and_then(|()| settings.set(&predict::model::OLLAMA, &remote));
    predict::provider::force(None);
    predict::settings_changed();
    let waiting = predict::status_shown(&view);
    predict::provider::consent();
    let allowed = predict::chosen().consented();
    if let Some(mtm) = MainThreadMarker::new() {
        crate::settings::show_prediction(mtm);
    }
    let tab = crate::settings::prediction_shown();
    predict::provider::revoke_all();
    let revoked = !predict::chosen().consented();
    let restored = settings.set(&predict::model::PROVIDER, &predict::model::Choice::Off);
    predict::settings_changed();
    check(
        "a provider outside the Mac waits for consent, which the settings show and take back",
        stored.is_ok()
            && restored.is_ok()
            && waiting.as_ref().is_some_and(|(title, _)| title.contains("Ollama"))
            && allowed
            && revoked
            && tab
                .as_ref()
                .is_some_and(|(provider, _, line, can)| provider == "Ollama" && line.contains("ollama") && *can),
        format!("{waiting:?}, {tab:?}"),
    );
    if let Some(window) = crate::settings::window() {
        picture_window(&window, "predict-settings");
        window.close();
    }
}

fn picture_window(window: &NSWindow, name: &str) {
    window.displayIfNeeded();
    snapshot(window, name);
}

/// E25 with predictions on: a proposal on view, which the next keystroke drops, adds under 3 ms to a keystroke's
/// median in 10 000 lines, in the middle of a line, where it opens a gap, and at its end. Keystrokes with and without a
/// proposal alternate, so that the load of this Mac weighs on both alike.
fn budget() {
    let Some(big) = open("big.rs") else {
        check("the big file opens", false, "");
        return finish(SCENARIO);
    };
    let middle = big.line_start(5000).unwrap_or(0) + 30;
    editor::scroll_to_byte(&big, big.document().text().utf16_to_byte(middle).unwrap_or(0));
    let stub: Arc<dyn Provider> = Arc::new(Stub::new(Vec::new(), Duration::from_secs(5)));
    let mut detail = Vec::new();
    let mut ok = true;
    for (place, line) in [("middle", 5000), ("end", 5010)] {
        let at = if place == "middle" {
            big.line_start(line).unwrap_or(0) + 30
        } else {
            big.line_start(line + 1).unwrap_or(1) - 1
        };
        big.setSelectedRange(NSRange::new(at, 0));
        let (with, without) = keystrokes(&big, &stub);
        ok &= with.0.saturating_sub(without.0) < Duration::from_millis(3);
        detail.push(format!(
            "{place}: with a proposal median {:?}, slowest {:?}; without median {:?}, slowest {:?}",
            with.0, with.1, without.0, without.1
        ));
    }
    check(
        "E25: a proposal on view adds under 3 ms to a keystroke's median in 10 000 lines",
        ok,
        detail.join("; "),
    );
    match main_view() {
        Some(view) => super::predict_menu::run(view),
        None => finish(SCENARIO),
    }
}

/// Sixty keystrokes in `view`, every other one after a proposal was put on view; the median and the slowest CPU time
/// of those with a proposal and of those without.
fn keystrokes(view: &EditorTextView, stub: &Arc<dyn Provider>) -> ((Duration, Duration), (Duration, Duration)) {
    let (mut with, mut without) = (Vec::new(), Vec::new());
    for n in 0..60 {
        let proposal = n % 2 == 0;
        predict::provider::force(proposal.then(|| stub.clone()));
        if proposal {
            let caret = view.selectedRange().location;
            let byte = view.document().text().utf16_to_byte(caret).unwrap_or(0);
            let prediction = Prediction {
                range: byte..byte,
                text: format!("ghost_{n}(1, 2)"),
                feedback: None,
            };
            predict::show_now(view, stub.clone(), prediction);
        }
        if let Some(window) = view.window() {
            window.displayIfNeeded();
        }
        let start = super::insets::thread_time();
        editor::type_text(view, "x");
        predict::settle_now(view);
        insets::settle(view);
        if let Some(window) = view.window() {
            window.displayIfNeeded();
        }
        let took = super::insets::thread_time().saturating_sub(start);
        if proposal { with.push(took) } else { without.push(took) }
    }
    let summary = |times: &mut Vec<Duration>| {
        times.sort();
        (
            times.get(times.len() / 2).copied().unwrap_or_default(),
            times.last().copied().unwrap_or_default(),
        )
    };
    (summary(&mut with), summary(&mut without))
}

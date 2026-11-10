//! The `conflict` scenario (E24, E43): files of an open folder change on disk while their editors hold unsaved text.
//! An editor without unsaved text takes the change at once; the others get the bar, while typing goes on and a further
//! change updates it. Its buttons show the differences in a tab, merge as one undo step, load the file's version and
//! keep the editor's; a deleted file's bar saves it anew or closes its editor after asking, as the editor may hold the
//! file's last copy, and closing the window, quitting and ⌘W ask the same. A window close or quit the user cancels
//! after giving up some editors' text keeps every text and its copy.

use std::path::{Path, PathBuf};
use std::time::Duration;

use ide_document::recovery::{Recovery, RecoveryId};
use objc2::runtime::AnyObject;
use objc2::{MainThreadMarker, msg_send};
use objc2_app_kit::{NSAlertFirstButtonReturn, NSAlertSecondButtonReturn, NSAlertThirdButtonReturn};
use objc2_foundation::NSRange;
use splat_i18n::{t, tf};

use super::helpers::{
    answer_alert_after, answer_alert_saying, editor, picture_alert_after, tab, text_of, type_at, utf16_len, wait_until,
    window_of,
};
use super::{check, finish, snapshot};
use crate::window::WindowController;

const SCENARIO: &str = "conflict";

const CLEAN: &str = "clean.txt";
const MERGED: &str = "merged.txt";
const KEPT: &str = "kept.txt";
const LOADED: &str = "loaded.txt";
const SAVED: &str = "saved.txt";
const CLOSED: &str = "closed.txt";
const DROPPED: &str = "dropped.txt";
const GONE: &str = "gone.txt";

fn folder() -> PathBuf {
    ide_core::paths::home().join("conflict-project")
}

fn root() -> PathBuf {
    folder().canonicalize().unwrap_or_else(|_| folder())
}

fn controller() -> Option<objc2::rc::Retained<WindowController>> {
    window_of(&folder())
}

fn write(name: &str, text: &str) {
    let _ = std::fs::write(root().join(name), text);
}

fn bar_of(name: &str) -> Option<(String, Vec<String>)> {
    controller()
        .and_then(|c| editor(&c, name))
        .and_then(|view| crate::editor::conflict_bar(&view))
}

fn text(name: &str) -> String {
    controller()
        .and_then(|c| editor(&c, name))
        .map(|view| text_of(&view))
        .unwrap_or_default()
}

fn dirty(name: &str) -> bool {
    controller()
        .and_then(|c| editor(&c, name))
        .is_some_and(|view| view.document().is_dirty())
}

fn press(name: &str, button: &str) -> bool {
    controller()
        .and_then(|c| editor(&c, name))
        .is_some_and(|view| crate::editor::press_conflict_button(&view, button))
}

pub fn run() {
    let Some(mtm) = MainThreadMarker::new() else { return };
    let folder = folder();
    let _ = std::fs::create_dir_all(&folder);
    for name in [CLEAN, KEPT, LOADED, SAVED, CLOSED, DROPPED, GONE] {
        let _ = std::fs::write(folder.join(name), format!("{name}\nzweite Zeile\n"));
    }
    let _ = std::fs::write(folder.join(MERGED), "eins\nzwei\ndrei\nvier\nfünf\n");
    crate::project::trust(&folder);
    crate::app::open_project(mtm, folder.clone());
    let Some(controller) = controller() else {
        check("the folder gets its window", false, "");
        finish(SCENARIO);
        return;
    };
    for name in [CLEAN, MERGED, KEPT, LOADED, SAVED, CLOSED, DROPPED, GONE] {
        controller.open_file(&root().join(name));
    }
    for name in [MERGED, KEPT, LOADED, CLOSED] {
        if let Some(view) = editor(&controller, name) {
            type_at(&view, NSRange::new(0, 0), ">");
        }
    }
    // The watcher reports only what changes after it started.
    crate::util::main_after(Duration::from_secs(1), || {
        write(CLEAN, "von außen\n");
        write(MERGED, "eins\nzwei\ndrei\nvier\nFÜNF\n");
        write(KEPT, "von außen\n");
        write(LOADED, "von außen\n");
        wait_until(
            || text(CLEAN) == "von außen\n" && [MERGED, KEPT, LOADED].iter().all(|n| bar_of(n).is_some()),
            changed_outside,
        );
    });
}

fn changed_outside(arrived: bool) {
    check(
        "an editor without unsaved text takes a change on disk at once",
        text(CLEAN) == "von außen\n" && !dirty(CLEAN) && bar_of(CLEAN).is_none(),
        text(CLEAN),
    );
    let expected = (
        tf("„{}“ wurde außerhalb geändert.", &[MERGED]),
        [
            t("Unterschiede zeigen"),
            t("Zusammenführen"),
            t("Version von der Platte laden"),
            t("Meine behalten"),
        ]
        .iter()
        .map(|b| (*b).to_string())
        .collect::<Vec<_>>(),
    );
    check(
        "an editor with unsaved text gets the bar instead",
        arrived && bar_of(MERGED) == Some(expected) && text(MERGED) == ">eins\nzwei\ndrei\nvier\nfünf\n",
        format!("{:?}, {:?}", bar_of(MERGED), text(MERGED)),
    );
    if let Some(controller) = controller() {
        if let Some(id) = tab(&controller, MERGED) {
            controller.select_tab_by_id(id);
        }
        if let Some(window) = controller.window() {
            snapshot(&window, "conflict-bar");
        }
        if let Some(view) = editor(&controller, MERGED) {
            type_at(&view, NSRange::new(utf16_len(">eins"), 0), "!");
        }
    }
    check(
        "typing goes on while the bar shows",
        text(MERGED) == ">eins!\nzwei\ndrei\nvier\nfünf\n" && bar_of(MERGED).is_some(),
        text(MERGED),
    );
    write(MERGED, "eins\nzwei\ndrei\nvier\n5\n");
    crate::util::main_after(Duration::from_secs(3), show_differences);
}

fn show_differences() {
    let Some(controller) = controller() else {
        finish(SCENARIO);
        return;
    };
    press(MERGED, t("Unterschiede zeigen"));
    let title = tf("{} (Platte ↔ Editor)", &[MERGED]);
    let shown = crate::editor::compared(&root().join(MERGED)).unwrap_or_default();
    check(
        "the differences open in a tab, against the latest version on disk",
        controller.tab_titles().contains(&title)
            && shown.contains("    1       − eins\n")
            && shown.contains("          1 + >eins!\n")
            && shown.contains("    5       − 5\n"),
        shown.clone(),
    );
    if let Some(window) = controller.window() {
        snapshot(&window, "conflict-differences");
    }
    if let Some(id) = tab(&controller, MERGED) {
        controller.select_tab_by_id(id);
    }
    press(MERGED, t("Zusammenführen"));
    let merged = text(MERGED);
    let bar = bar_of(MERGED);
    let undo = controller.window().and_then(|w| w.undoManager());
    if let Some(undo) = &undo {
        undo.undo();
    }
    let undone = text(MERGED);
    if let Some(undo) = &undo {
        undo.redo();
    }
    check(
        "merging takes both changes as one undo step and stays unsaved",
        merged == ">eins!\nzwei\ndrei\nvier\n5\n"
            && dirty(MERGED)
            && bar.is_none()
            && undone == ">eins!\nzwei\ndrei\nvier\nfünf\n",
        format!("{merged:?}, {bar:?}, undone {undone:?}"),
    );
    press(LOADED, t("Version von der Platte laden"));
    check(
        "loading the version on disk drops the editor's text",
        text(LOADED) == "von außen\n" && !dirty(LOADED) && bar_of(LOADED).is_none(),
        text(LOADED),
    );
    press(KEPT, t("Meine behalten"));
    let kept = text(KEPT);
    let still_dirty = dirty(KEPT);
    if let Some(view) = editor(&controller, KEPT) {
        // Should saving ask about the change on disk after all, the question is declined, so the check fails
        // instead of the run waiting for an answer.
        let question = tf("„{}“ wurde inzwischen außerhalb geändert.", &[KEPT]);
        answer_alert_saying(Duration::from_millis(800), question, NSAlertSecondButtonReturn);
        crate::editor::save_view(&view);
    }
    let on_disk = std::fs::read_to_string(root().join(KEPT)).unwrap_or_default();
    check(
        "keeping the editor's text saves without asking about the change on disk",
        kept == format!(">{KEPT}\nzweite Zeile\n") && still_dirty && on_disk == kept && !dirty(KEPT),
        format!("{kept:?}, on disk {on_disk:?}"),
    );
    let _ = std::fs::remove_file(root().join(SAVED));
    let _ = std::fs::remove_file(root().join(CLOSED));
    let _ = std::fs::remove_file(root().join(DROPPED));
    let _ = std::fs::remove_file(root().join(GONE));
    wait_until(
        || [SAVED, CLOSED, DROPPED, GONE].iter().all(|name| bar_of(name).is_some()),
        deleted,
    );
}

fn deleted(arrived: bool) {
    let expected = (
        tf("„{}“ wurde auf der Platte gelöscht.", &[SAVED]),
        vec![t("Sichern").to_string(), t("Schließen").to_string()],
    );
    check(
        "a deleted file gets the bar to save it anew or close it",
        arrived && bar_of(SAVED) == Some(expected),
        format!("{:?}", bar_of(SAVED)),
    );
    if let Some(controller) = controller()
        && let Some(id) = tab(&controller, SAVED)
    {
        controller.select_tab_by_id(id);
        if let Some(window) = controller.window() {
            snapshot(&window, "conflict-deleted");
        }
    }
    press(SAVED, t("Sichern"));
    let on_disk = std::fs::read_to_string(root().join(SAVED)).unwrap_or_default();
    check(
        "saving writes the file anew and the bar goes",
        on_disk == format!("{SAVED}\nzweite Zeile\n") && bar_of(SAVED).is_none() && !dirty(SAVED),
        on_disk,
    );
    close_unedited();
}

fn titles() -> Vec<String> {
    controller().map(|c| c.tab_titles()).unwrap_or_default()
}

/// What the question before the text of `name`'s editor goes says and offers.
fn question_of(name: &str) -> (String, String, Vec<(String, String)>) {
    controller()
        .and_then(|c| editor(&c, name))
        .map(|view| crate::editor::discard_question(&view))
        .unwrap_or_default()
}

/// The answers to that question, Return pressing the last.
fn close_answers() -> Vec<(String, String)> {
    vec![
        (t("Sichern").to_string(), String::new()),
        (t("Verwerfen").to_string(), String::new()),
        (t("Abbrechen").to_string(), "\r".to_string()),
    ]
}

/// Closes the editor of a deleted file without changes of its own and saves it from the question.
fn close_unedited() {
    let question = question_of(DROPPED);
    let expected = (
        tf("„{}“ wurde gelöscht. Inhalt verwerfen?", &[DROPPED]),
        t("Dieser Editor hält die letzte Kopie der Datei. „Sichern“ stellt sie wieder her.").to_string(),
        close_answers(),
    );
    check(
        "closing a deleted file's editor without changes of its own asks whether to let its last copy go, Return cancels",
        question == expected,
        format!("{question:?}"),
    );
    picture_alert_after(Duration::from_millis(250), "conflict-close-deleted");
    let saved = answer_alert_saying(Duration::from_millis(500), expected.0, NSAlertFirstButtonReturn);
    press(DROPPED, t("Schließen"));
    let on_disk = std::fs::read_to_string(root().join(DROPPED)).unwrap_or_default();
    let after = titles();
    check(
        "Sichern in that question writes the file anew and closes the editor",
        saved.get() && on_disk == format!("{DROPPED}\nzweite Zeile\n") && !after.iter().any(|title| title == DROPPED),
        format!("asked {}, on disk {on_disk:?}, {}", saved.get(), after.join(", ")),
    );
    close_edited();
}

/// Closes the editor of a deleted file with changes of its own: cancels first, then lets the text go.
fn close_edited() {
    let question = question_of(CLOSED);
    let expected = (
        tf("Ungesicherte Änderungen an „{}“ verwerfen?", &[CLOSED]),
        t("Ohne Sichern gehen deine Änderungen verloren.").to_string(),
        close_answers(),
    );
    check(
        "with changes of its own the question is about those, Return cancels",
        question == expected,
        format!("{question:?}"),
    );
    let copy = controller()
        .and_then(|c| editor(&c, CLOSED))
        .map(|view| crate::editor::recovery_of(&view));
    let stored = move || {
        let pending = Recovery::in_data_dir().pending().unwrap_or_default();
        copy.as_ref().is_some_and(|id| pending.iter().any(|p| &p.id == id))
    };
    picture_alert_after(Duration::from_millis(250), "conflict-close-edited");
    let cancelled = answer_alert_saying(Duration::from_millis(500), expected.0.clone(), NSAlertThirdButtonReturn);
    press(CLOSED, t("Schließen"));
    let after = titles();
    check(
        "Abbrechen keeps editor, text, bar and copy",
        cancelled.get()
            && after.iter().any(|title| title == CLOSED)
            && text(CLOSED) == format!(">{CLOSED}\nzweite Zeile\n")
            && dirty(CLOSED)
            && bar_of(CLOSED).is_some()
            && stored(),
        format!(
            "asked {}, copy kept {}, {}",
            cancelled.get(),
            stored(),
            after.join(", ")
        ),
    );
    let discarded = answer_alert_saying(Duration::from_millis(500), expected.0, NSAlertSecondButtonReturn);
    press(CLOSED, t("Schließen"));
    let answered = discarded.get();
    wait_until(
        move || !stored(),
        move |gone| {
            let after = titles();
            check(
                "Verwerfen closes it, drops its copy and leaves the file deleted",
                answered && gone && !after.iter().any(|title| title == CLOSED) && !root().join(CLOSED).exists(),
                after.join(", "),
            );
            cancelled_close();
        },
    );
}

/// Closes the window with two editors holding unsaved text and one of a deleted file: the first two are given up, the
/// last cancels the close.
fn cancelled_close() {
    let Some(controller) = controller() else {
        finish(SCENARIO);
        return;
    };
    if let Some(view) = editor(&controller, KEPT) {
        type_at(&view, NSRange::new(0, 0), "+");
    }
    let question = |name: &str| tf("Änderungen an „{}“ sichern?", &[name]);
    answer_alert_saying(Duration::from_millis(500), question(MERGED), NSAlertSecondButtonReturn);
    answer_alert_saying(Duration::from_millis(1500), question(KEPT), NSAlertSecondButtonReturn);
    let gone = answer_alert_saying(
        Duration::from_millis(2500),
        deleted_question(),
        NSAlertThirdButtonReturn,
    );
    if let Some(window) = controller.window() {
        window.performClose(None);
    }
    let asked = gone.get();
    let copies = copies_of(&[MERGED, KEPT, GONE]);
    wait_until(copies_kept(copies.clone()), move |ok| {
        let open = controller_is_open(&root());
        check(
            "a window close asks about a deleted file's editor too; cancelled there, it keeps every text and its copy",
            asked
                && ok
                && copies.len() == 3
                && open
                && dirty(MERGED)
                && dirty(KEPT)
                && titles().iter().any(|title| title == GONE),
            format!("asked {asked}, window open {open}, copies of {copies:?}"),
        );
        cancelled_quit();
    });
}

/// Quits with the editor of a deleted file and two more holding unsaved text: the first is given up, the question
/// about the other two cancels the quit.
fn cancelled_quit() {
    let Some(mtm) = MainThreadMarker::new() else { return };
    let gone = answer_alert_saying(
        Duration::from_millis(500),
        deleted_question(),
        NSAlertSecondButtonReturn,
    );
    let others = answer_alert_saying(
        Duration::from_millis(1500),
        tf("{} Dateien haben ungesicherte Änderungen.", &["2"]),
        NSAlertThirdButtonReturn,
    );
    let quits = crate::editor::confirm_quit(mtm);
    let asked = (gone.get(), others.get());
    let copies = copies_of(&[MERGED, KEPT, GONE]);
    wait_until(copies_kept(copies.clone()), move |ok| {
        check(
            "a quit asks about a deleted file's editor first, then about the others; cancelled, each keeps its copy",
            !quits && asked == (true, true) && ok && copies.len() == 3 && titles().iter().any(|title| title == GONE),
            format!("quits {quits}, asked {asked:?}, copies of {copies:?}"),
        );
        close_with_key();
    });
}

/// ⌘W on the editor of a deleted file, given up.
fn close_with_key() {
    let Some(controller) = controller() else {
        finish(SCENARIO);
        return;
    };
    if let Some(id) = tab(&controller, GONE) {
        controller.select_tab_by_id(id);
    }
    let copies = copies_of(&[GONE]);
    let discarded = answer_alert_saying(
        Duration::from_millis(500),
        deleted_question(),
        NSAlertSecondButtonReturn,
    );
    // SAFETY: The action takes its sender, which may be nil.
    let () = unsafe { msg_send![&*controller, ideCloseTab: None::<&AnyObject>] };
    let answered = discarded.get();
    let left = copies.clone();
    wait_until(
        move || {
            let pending = pending();
            left.iter().all(|id| !pending.contains(id))
        },
        move |gone| {
            let after = titles();
            check(
                "⌘W on a deleted file's editor asks the same; Verwerfen closes it and drops its copy",
                answered
                    && gone
                    && copies.len() == 1
                    && !after.iter().any(|title| title == GONE)
                    && !root().join(GONE).exists(),
                format!("asked {answered}, copies of {copies:?}, {}", after.join(", ")),
            );
            answer_alert_after(Duration::from_secs(1), NSAlertSecondButtonReturn);
            finish(SCENARIO);
        },
    );
}

fn deleted_question() -> String {
    tf("„{}“ wurde gelöscht. Inhalt verwerfen?", &[GONE])
}

fn copies_of(names: &[&str]) -> Vec<RecoveryId> {
    let Some(controller) = controller() else {
        return Vec::new();
    };
    names
        .iter()
        .filter_map(|name| editor(&controller, name).map(|view| crate::editor::recovery_of(&view)))
        .collect()
}

/// Whether each of `ids` has its copy.
fn copies_kept(ids: Vec<RecoveryId>) -> impl Fn() -> bool + Send + 'static {
    move || {
        let pending = pending();
        ids.iter().all(|id| pending.contains(id))
    }
}

fn pending() -> Vec<RecoveryId> {
    Recovery::in_data_dir()
        .pending()
        .map(|p| p.into_iter().map(|p| p.id).collect())
        .unwrap_or_default()
}

fn controller_is_open(root: &Path) -> bool {
    window_of(root).is_some_and(|c| c.window().is_some_and(|w| w.isVisible()))
}

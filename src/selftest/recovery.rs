//! The `recovery` scenario, in two launches (E24). The first types into three named files and an untitled tab, waits
//! for the copies against a crash and the record of the windows, leaves a damaged copy and a text without description
//! beside them, changes two of the files on disk and kills itself. The second launch checks that the window comes back
//! with its tabs, texts, marks of unsaved changes and the tab in front; that both changed files get the bar instead of
//! a merge without a word, and that merging them works, cleanly as one undo step and with conflict markers; that the
//! damaged copy is set aside and the text without description comes back untitled; and that saving and closing without
//! saving remove the copies.

use std::path::PathBuf;
use std::time::Duration;

use ide_document::recovery::{Recovery, RecoveryId};
use objc2::MainThreadMarker;
use objc2_app_kit::NSAlertSecondButtonReturn;
use objc2_foundation::NSRange;
use splat_i18n::{t, tf};

use super::helpers::{
    answer_alert_after, answer_alert_saying, editor, tab, text_of, type_at, utf16_len, wait_until, window_of,
};
use super::{check, finish, snapshot, write_report};
use crate::util::main_after;
use crate::window;

const SCENARIO: &str = "recovery";

const MAIN: &str = "src/main.rs";
const NOTES: &str = "notes.txt";
const MERGED: &str = "merge.txt";
const KEPT: &str = "kept.txt";
const MAIN_TYPED: &str = "fn main() {}\n// vor dem Absturz";
const NOTES_TYPED: &str = "eins\nZWEI\ndrei\n";
const NOTES_ON_DISK: &str = "eins\nzwo\ndrei\n";
const NOTES_MARKED: &str = "eins\n<<<<<<< {mine}\nZWEI\n=======\nzwo\n>>>>>>> {disk}\ndrei\n";
const MERGED_TYPED: &str = "A\nb\nc\nd\ne\n";
const MERGED_ON_DISK: &str = "a\nb\nc\nd\nE\n";
const MERGED_BOTH: &str = "A\nb\nc\nd\nE\n";
const UNTITLED_TYPED: &str = "Entwurf ohne Titel";
/// The name of a copy whose description is no JSON.
const DAMAGED: &str = "0bad-1-0";
/// The name of a copy whose description a crash kept from being written.
const ORPHAN: &str = "0abc-2-0";
const ORPHAN_TEXT: &str = "Text ohne Beschreibung";

pub fn run() {
    if std::env::var("IDE_SELFTEST_LAUNCH").as_deref() == Ok("crash") {
        before_crash();
    } else {
        wait_until(bars_shown, |shown| {
            main_after(Duration::from_millis(500), move || after_crash(shown));
        });
    }
}

fn folder() -> PathBuf {
    ide_core::paths::home().join("recovery-project")
}

fn before_crash() {
    let Some(mtm) = MainThreadMarker::new() else { return };
    let folder = folder();
    for (path, text) in [
        (MAIN, "fn main() {}\n"),
        (NOTES, "eins\nzwei\ndrei\n"),
        (MERGED, "a\nb\nc\nd\ne\n"),
        (KEPT, "unverändert\n"),
    ] {
        let file = folder.join(path);
        let _ = std::fs::create_dir_all(file.parent().unwrap_or(&folder));
        let _ = std::fs::write(file, text);
    }
    crate::project::trust(&folder);
    crate::app::open_project(mtm, folder.clone());
    let Some(controller) = window_of(&folder) else {
        check("the folder gets its window", false, "");
        crash();
        return;
    };
    let root = folder.canonicalize().unwrap_or(folder);
    for path in [MAIN, NOTES, MERGED, KEPT] {
        controller.open_file(&root.join(path));
    }
    crate::editor::new_untitled(&controller, 13.0);
    if let Some(view) = editor(&controller, "main.rs") {
        type_at(
            &view,
            NSRange::new(utf16_len("fn main() {}\n"), 0),
            "// vor dem Absturz",
        );
    }
    if let Some(view) = editor(&controller, NOTES) {
        type_at(&view, NSRange::new(utf16_len("eins\n"), utf16_len("zwei")), "ZWEI");
    }
    if let Some(view) = editor(&controller, MERGED) {
        type_at(&view, NSRange::new(0, 1), "A");
    }
    if let Some(view) = editor(&controller, t("Ohne Titel")) {
        type_at(&view, NSRange::new(0, 0), UNTITLED_TYPED);
    }
    controller.open_file(&root.join(MAIN));
    wait_until(copies_and_record_written, move |ok| {
        let pending = Recovery::in_data_dir().pending().map(|p| p.len()).unwrap_or(0);
        check(
            "the unsaved texts and the windows are written down before the crash",
            ok,
            format!("{pending} copies, {:?}", crate::session::recorded()),
        );
        let recovery = Recovery::in_data_dir();
        let _ = std::fs::write(recovery.dir().join(format!("{DAMAGED}.json")), "{ kaputt");
        let _ = std::fs::write(recovery.dir().join(format!("{ORPHAN}.text")), ORPHAN_TEXT);
        let _ = std::fs::write(root.join(NOTES), NOTES_ON_DISK);
        let _ = std::fs::write(root.join(MERGED), MERGED_ON_DISK);
        if let Some(window) = window_of(&root).and_then(|c| c.window()) {
            snapshot(&window, "recovery-before-crash");
        }
        crash();
    });
}

fn copies_and_record_written() -> bool {
    let copies = Recovery::in_data_dir().pending().map(|p| p.len()).unwrap_or(0);
    let recorded = crate::session::recorded();
    let tabs = recorded.first().map(|w| (w.tabs.len(), w.selected));
    copies == 4 && tabs == Some((6, Some(1)))
}

/// Reports what the first launch saw and ends it as a crash would: no quit, no clean-up.
fn crash() {
    write_report(SCENARIO);
    // SAFETY: Plain system calls on this process; SIGKILL ends it at once, before any more of its code runs.
    unsafe { libc::kill(libc::getpid(), libc::SIGKILL) };
}

fn bars_shown() -> bool {
    window_of(&folder()).is_some_and(|controller| {
        [NOTES, MERGED]
            .iter()
            .all(|name| editor(&controller, name).is_some_and(|view| crate::editor::conflict_bar(&view).is_some()))
    })
}

fn changed_bar(name: &str) -> (String, Vec<String>) {
    let message = tf("„{}“ wurde außerhalb geändert.", &[name]);
    let buttons = [
        t("Unterschiede zeigen"),
        t("Zusammenführen"),
        t("Version von der Platte laden"),
        t("Meine behalten"),
    ];
    (message, buttons.iter().map(|b| (*b).to_string()).collect())
}

fn after_crash(shown: bool) {
    let Some(controller) = window_of(&folder()) else {
        check(
            "the project window comes back",
            false,
            format!("{} windows", window::open_windows().len()),
        );
        finish(SCENARIO);
        return;
    };
    check("the project window comes back", true, "");
    let titles = controller.tab_titles();
    let expected = [t("Willkommen"), "main.rs", NOTES, MERGED, KEPT, t("Ohne Titel")];
    check(
        "its tabs come back in their order",
        titles == expected,
        titles.join(", "),
    );
    let selected = controller
        .selected_tab()
        .and_then(|id| controller.tab_ids().iter().position(|tab| *tab == id))
        .and_then(|index| titles.get(index).cloned())
        .unwrap_or_default();
    check("the tab in front comes back", selected == "main.rs", selected);
    let text = |title: &str| editor(&controller, title).map(|v| text_of(&v));
    check(
        "the unsaved texts come back",
        text("main.rs").as_deref() == Some(MAIN_TYPED)
            && text(NOTES).as_deref() == Some(NOTES_TYPED)
            && text(MERGED).as_deref() == Some(MERGED_TYPED)
            && text(t("Ohne Titel")).as_deref() == Some(UNTITLED_TYPED),
        format!(
            "{:?}",
            [text("main.rs"), text(NOTES), text(MERGED), text(t("Ohne Titel"))]
        ),
    );
    check(
        "a file without unsaved text reopens as it is on disk",
        text(KEPT).as_deref() == Some("unverändert\n"),
        text(KEPT).unwrap_or_default(),
    );
    let marks = controller.tab_dirty_marks();
    check(
        "the restored documents are marked unsaved",
        marks == [false, true, true, true, false, true],
        format!("{marks:?}"),
    );
    let notice = controller.notice().unwrap_or_default();
    check(
        "a note says how many documents came back",
        notice == tf("{} Dokumente wiederhergestellt", &["4"]),
        notice,
    );
    check_set_aside_and_orphan();
    let bars: Vec<_> = [NOTES, MERGED]
        .iter()
        .map(|name| editor(&controller, name).and_then(|view| crate::editor::conflict_bar(&view)))
        .collect();
    check(
        "files changed on disk since get the bar instead of a merge without a word",
        shown && bars[0] == Some(changed_bar(NOTES)) && bars[1] == Some(changed_bar(MERGED)),
        format!("{bars:?}"),
    );
    if let Some(id) = tab(&controller, NOTES) {
        controller.select_tab_by_id(id);
    }
    if let Some(window) = controller.window() {
        snapshot(&window, "recovery-restored");
    }
    merge_both(&controller);
}

fn check_set_aside_and_orphan() {
    let recovery = Recovery::in_data_dir();
    let set_aside = recovery.dir().join("damaged").join(format!("{DAMAGED}.json")).exists();
    let still_there = recovery.dir().join(format!("{DAMAGED}.json")).exists();
    check(
        "a damaged copy is set aside and kept",
        set_aside && !still_there,
        format!("set aside {set_aside}, still in place {still_there}"),
    );
    let orphan = window::open_windows().into_iter().find(|w| w.project().is_none());
    let back = orphan
        .as_ref()
        .and_then(|w| editor(w, t("Ohne Titel")).map(|v| text_of(&v)));
    let notice = orphan.as_ref().and_then(|w| w.notice()).unwrap_or_default();
    check(
        "a text without description comes back untitled in a window of its own",
        back.as_deref() == Some(ORPHAN_TEXT) && notice == t("Ein Dokument wiederhergestellt"),
        format!("{back:?}, {notice}"),
    );
}

/// Merges the file changed in other lines, which goes through as one undo step, then the one changed in the same
/// line, which brings conflict markers, and settles that conflict by hand.
fn merge_both(controller: &crate::window::WindowController) {
    if let Some(view) = editor(controller, MERGED) {
        crate::editor::press_conflict_button(&view, t("Zusammenführen"));
        let merged = text_of(&view);
        let dirty = view.document().is_dirty();
        let bar = crate::editor::conflict_bar(&view);
        let undo = view.undoManager();
        if let Some(undo) = &undo {
            undo.undo();
        }
        let undone = text_of(&view);
        if let Some(undo) = &undo {
            undo.redo();
        }
        check(
            "a clean merge takes both changes as one undo step and stays unsaved",
            merged == MERGED_BOTH && dirty && bar.is_none() && undone == MERGED_TYPED && text_of(&view) == MERGED_BOTH,
            format!("{merged:?}, dirty {dirty}, bar {bar:?}, undone {undone:?}"),
        );
    }
    let Some(view) = editor(controller, NOTES) else {
        finish(SCENARIO);
        return;
    };
    crate::editor::press_conflict_button(&view, t("Zusammenführen"));
    let marked = NOTES_MARKED
        .replace("{mine}", t("deine Fassung"))
        .replace("{disk}", t("Fassung auf der Platte"));
    let bar = crate::editor::conflict_bar(&view);
    let expected_bar = (
        tf("„{}“: ein Konflikt beim Zusammenführen", &[NOTES]),
        vec![t("Vorheriger Konflikt").to_string(), t("Nächster Konflikt").to_string()],
    );
    let at_marker = view.selectedRange().location == utf16_len("eins\n");
    check(
        "overlapping changes come in between conflict markers, counted on the bar",
        text_of(&view) == marked && bar == Some(expected_bar) && at_marker,
        format!("{:?}, {bar:?}, selection {:?}", text_of(&view), view.selectedRange()),
    );
    if let Some(window) = controller.window() {
        snapshot(&window, "recovery-merge-conflict");
    }
    let block_end = marked.find("\ndrei").unwrap_or(marked.len());
    let start = utf16_len("eins\n");
    type_at(
        &view,
        NSRange::new(start, utf16_len(&marked[..block_end]) - start),
        "ZWEI",
    );
    wait_until(
        || {
            window_of(&folder())
                .and_then(|c| editor(&c, NOTES))
                .is_some_and(|v| crate::editor::conflict_bar(&v).is_none())
        },
        |gone| {
            let text = window_of(&folder())
                .and_then(|c| editor(&c, NOTES))
                .map(|v| text_of(&v))
                .unwrap_or_default();
            check(
                "the bar goes once the last conflict is settled",
                gone && text == NOTES_TYPED,
                text,
            );
            save_and_close();
        },
    );
}

/// Saving one file and closing another without saving remove their copies; the untitled one keeps its copy.
fn save_and_close() {
    let Some(controller) = window_of(&folder()) else {
        finish(SCENARIO);
        return;
    };
    let id_of = |title: &str| editor(&controller, title).map(|view| crate::editor::recovery_of(&view));
    let (main, notes, untitled) = (id_of("main.rs"), id_of(NOTES), id_of(t("Ohne Titel")));
    if let Some(view) = editor(&controller, "main.rs") {
        crate::editor::save_view(&view);
    }
    if let Some(id) = tab(&controller, NOTES) {
        let question = tf("Änderungen an „{}“ sichern?", &[NOTES]);
        answer_alert_saying(Duration::from_millis(300), question, NSAlertSecondButtonReturn);
        controller.close_tab(id);
    }
    let gone = [main, notes];
    let kept = untitled.clone();
    wait_until(
        move || {
            let pending = pending_ids();
            gone.iter().flatten().all(|id| !pending.contains(id))
                && kept.as_ref().is_some_and(|id| pending.contains(id))
        },
        move |ok| {
            check(
                "saving and closing without saving remove their copies",
                ok,
                format!("{:?}, the untitled one is {untitled:?}", pending_ids()),
            );
            answer_alert_after(Duration::from_secs(1), NSAlertSecondButtonReturn);
            finish(SCENARIO);
        },
    );
}

fn pending_ids() -> Vec<RecoveryId> {
    Recovery::in_data_dir()
        .pending()
        .map(|p| p.into_iter().map(|p| p.id).collect())
        .unwrap_or_default()
}

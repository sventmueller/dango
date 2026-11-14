//! The `git-stash` scenario (E87): with a clean and an unsaved open document, "Stash …" first offers to save, then
//! asks for the message and untracked files; the entry shows in its section with its count, its files and a diff;
//! applying brings the changes back and the clean editors reload on their own; a pop that meets conflicts keeps its
//! entry and shows "Stash-Konflikt", whose "Abbrechen" puts back the state before; an apply with conflicts ends with
//! "Fertig"; only the staged changes go with "Nur Vorgemerktes stashen"; deleting asks with a sheet whose destructive
//! button is not the default. Pictures in light and dark appearance, and the accessibility audit of the window.

use std::path::{Path, PathBuf};

use ide_git::CommitOptions;
use objc2::sel;
use objc2_app_kit::{NSAlertFirstButtonReturn, NSButton, NSControlStateValueOn, NSTextField};
use splat_i18n::{t, tf};

use super::{answer_sheet, fixture_git, pictures, sheet_default_button, sheet_ready, sheet_views, window_of, write};
use crate::selftest::helpers::{editor, text_of, type_at, wait_on};
use crate::selftest::{check, finish, snapshot};
use crate::util::main_soon;
use crate::window::WindowController;
use ide_ui_git::probe::{self, StashAction};
use ide_ui_git::selftest as panel;

const SCENARIO: &str = "git-stash";
const README: &str = "README.md";
const LIBRARY: &str = "src/lib.rs";
const NOTES: &str = "notes.txt";
const COMMITTED_README: &str = "# Stash\n\nfirst\n";
const CHANGED_README: &str = "# Stash\n\nchanged in the working tree\n";
const COMMITTED_LIBRARY: &str = "pub fn one() {}\n";
const PARKED: &str = "Parked work";

pub fn run() {
    let folder = crate::selftest::helpers::folder("stash-project");
    std::thread::spawn(move || {
        let made = make_repository(&folder);
        main_soon(move || match made {
            Ok(()) => open(folder),
            Err(error) => {
                check("a test repository is made", false, error);
                finish(SCENARIO);
            }
        });
    });
}

fn make_repository(folder: &Path) -> Result<(), String> {
    let git = fixture_git()?;
    std::fs::create_dir_all(folder.join("src")).map_err(|e| e.to_string())?;
    let repo = git.init(folder).map_err(|e| e.to_string())?;
    super::append(
        &repo.git_dir().join("config"),
        "[user]\n\tname = Selftest\n\temail = selftest@example.invalid\n",
    )?;
    write(folder, README, COMMITTED_README)?;
    write(folder, LIBRARY, COMMITTED_LIBRARY)?;
    repo.stage_all().map_err(|e| e.to_string())?;
    repo.commit("First state", CommitOptions::default())
        .map_err(|e| e.to_string())?;
    write(folder, README, CHANGED_README)?;
    write(folder, NOTES, "untracked\n")
}

/// Runs `job` on the repository at `root` on a thread of its own, then `then` on the main thread.
fn job(
    root: PathBuf,
    job: impl FnOnce(&Path) -> Result<(), String> + Send + 'static,
    then: impl FnOnce(PathBuf) + Send + 'static,
) {
    std::thread::spawn(move || {
        let done = job(&root);
        main_soon(move || match done {
            Ok(()) => then(root),
            Err(error) => {
                check("a step of the test repository works", false, error);
                finish(SCENARIO);
            }
        });
    });
}

/// The texts of the labels of the sheet on the window of `root`, such as an alert's message.
pub(super) fn sheet_labels(root: &Path) -> String {
    sheet_views(root)
        .into_iter()
        .filter_map(|view| view.downcast::<NSTextField>().ok())
        .map(|field| field.stringValue().to_string())
        .collect::<Vec<_>>()
        .join("\n")
}

fn stash_count(controller: &WindowController) -> usize {
    probe::stashes(controller).len()
}

fn idle(controller: &WindowController) -> bool {
    panel::seen(controller).is_some_and(|s| !s.busy)
}

fn open(folder: PathBuf) {
    let Some((root, controller)) = super::open_repository(&folder, SCENARIO) else {
        return;
    };
    controller.open_file(&root.join(LIBRARY));
    controller.open_file(&root.join(README));
    wait_on(
        root,
        |c| {
            panel::seen(c).is_some_and(|s| s.changes.len() == 2)
                && editor(c, "lib.rs").is_some()
                && editor(c, "README.md").is_some()
        },
        listed,
    );
}

fn listed(root: PathBuf, ok: bool) {
    let Some(controller) = window_of(&root) else {
        return finish(SCENARIO);
    };
    check("the panel lists the changes and the editors are open", ok, "");
    check(
        "the menu bar offers Stash but not stashing what is staged, with nothing staged",
        probe::can(&controller, sel!(ideGitStash:)) && !probe::can(&controller, sel!(ideGitStashStaged:)),
        "",
    );
    let Some(library) = editor(&controller, "lib.rs") else {
        return finish(SCENARIO);
    };
    type_at(&library, objc2_foundation::NSRange::new(0, 0), "// unsaved\n");
    probe::stash(&controller);
    wait_on(root.clone(), sheet_ready, save_offered);
}

fn save_offered(root: PathBuf, ok: bool) {
    let says = sheet_labels(&root);
    check(
        "a stash first offers to save the unsaved open document",
        ok && says.contains(&tf("„{}“ hat ungesicherte Änderungen.", &[LIBRARY])),
        says,
    );
    super::sheet_pictures(root, "git-stash-save", |root| {
        if !answer_sheet(&root, NSAlertFirstButtonReturn, None) {
            return finish(SCENARIO);
        }
        wait_on(root.clone(), sheet_ready, options_asked);
    });
}

fn options_asked(root: PathBuf, ok: bool) {
    let saved = std::fs::read_to_string(root.join(LIBRARY)).unwrap_or_default();
    check(
        "Sichern saves the document, then the stash asks for its message",
        ok && saved.starts_with("// unsaved"),
        saved.lines().next().unwrap_or("").to_string(),
    );
    let untracked = sheet_views(&root)
        .into_iter()
        .filter_map(|view| view.downcast::<NSButton>().ok())
        .find(|button| button.title().to_string() == t("Mit nicht versionierten Dateien"));
    check("the stash sheet offers untracked files", untracked.is_some(), "");
    if let Some(button) = untracked {
        button.setState(NSControlStateValueOn);
    }
    super::sheet_pictures(root, "git-stash-options", |root| {
        if !answer_sheet(&root, NSAlertFirstButtonReturn, Some(PARKED)) {
            return finish(SCENARIO);
        }
        wait_on(
            root.clone(),
            |c| stash_count(c) == 1 && panel::seen(c).is_some_and(|s| s.changes.is_empty()),
            stashed,
        );
    });
}

fn stashed(root: PathBuf, ok: bool) {
    let Some(controller) = window_of(&root) else {
        return finish(SCENARIO);
    };
    let listed = probe::stashes(&controller);
    check(
        "the entry is listed with its message and the working tree is clean",
        ok && listed.first().is_some_and(|(_, m)| m.ends_with(PARKED)) && !root.join(NOTES).exists(),
        format!("{listed:?}"),
    );
    let pill = probe::stash_pill(&controller);
    check(
        "the section counts its entry",
        pill.as_deref() == Some(tf("{}, 1 Eintrag", &[t("Stashes")]).as_str()),
        format!("{pill:?}"),
    );
    wait_on(
        root.clone(),
        |c| {
            editor(c, "README.md").is_some_and(|v| text_of(&v) == COMMITTED_README)
                && editor(c, "lib.rs").is_some_and(|v| text_of(&v) == COMMITTED_LIBRARY)
        },
        reloaded,
    );
}

fn reloaded(root: PathBuf, ok: bool) {
    let Some(controller) = window_of(&root) else {
        return finish(SCENARIO);
    };
    let shown: Vec<(String, bool)> = ["README.md", "lib.rs"]
        .iter()
        .filter_map(|title| editor(&controller, title))
        .map(|v| (text_of(&v), v.document().is_dirty()))
        .collect();
    let dirty = shown.len() < 2 || shown.iter().any(|(_, dirty)| *dirty);
    check(
        "the clean editors take the files as the stash left them, without asking",
        ok && !dirty && !controller.has_sheet(),
        format!("waited {ok}, sheet {}, editors {shown:?}", controller.has_sheet()),
    );
    let hash = probe::stashes(&controller).remove(0).0;
    probe::open_stash(&controller, &hash);
    wait_on(
        root.clone(),
        move |c| probe::stash_rows(c, &hash).len() == 3,
        stash_opened,
    );
}

fn stash_opened(root: PathBuf, ok: bool) {
    let Some(controller) = window_of(&root) else {
        return finish(SCENARIO);
    };
    let hash = probe::stashes(&controller).remove(0).0;
    let rows = probe::stash_rows(&controller, &hash);
    check(
        "an open entry lists its files, untracked ones too",
        ok,
        format!("{rows:?}"),
    );
    probe::select_stash(&controller, &hash);
    check(
        "with an entry selected the menu bar offers applying and deleting it",
        probe::can(&controller, sel!(ideGitApplyStash:))
            && probe::can(&controller, sel!(ideGitDropStash:))
            && !probe::can(&controller, sel!(ideGitFinishStash:)),
        "",
    );
    probe::show_stash_diff(&controller, &hash, Path::new(README));
    wait_on(
        root.clone(),
        move |c| probe::stash_diff_counts(c, &hash, Path::new(README)).is_some_and(|(_, hunks)| hunks == 1),
        move |root, ok| {
            check("the diff of an entry's file opens in a tab", ok, "");
            pictures(root, "git-stash", apply);
        },
    );
}

fn apply(root: PathBuf) {
    let Some(controller) = window_of(&root) else {
        return finish(SCENARIO);
    };
    let hash = probe::stashes(&controller).remove(0).0;
    probe::stash_action(&controller, &hash, StashAction::Apply);
    wait_on(
        root.clone(),
        |c| idle(c) && panel::seen(c).is_some_and(|s| s.changes.len() == 3),
        applied,
    );
}

fn applied(root: PathBuf, ok: bool) {
    let Some(controller) = window_of(&root) else {
        return finish(SCENARIO);
    };
    check(
        "applying brings the changes back and keeps the entry",
        ok && stash_count(&controller) == 1 && root.join(NOTES).exists(),
        "",
    );
    wait_on(
        root.clone(),
        |c| {
            editor(c, "README.md").is_some_and(|v| text_of(&v) == CHANGED_README)
                && editor(c, "lib.rs").is_some_and(|v| text_of(&v).starts_with("// unsaved"))
        },
        |root, ok| {
            check("the clean editors follow the apply on their own", ok, "");
            job(root, conflicting_commit, pop_into_conflict);
        },
    );
}

/// Drops the applied changes and commits another change of the README, which the entry then conflicts with.
fn conflicting_commit(root: &Path) -> Result<(), String> {
    let repo = fixture_git()?.discover(root).map_err(|e| e.to_string())?;
    repo.destructive().reset_hard("HEAD").map_err(|e| e.to_string())?;
    std::fs::remove_file(root.join(NOTES)).map_err(|e| e.to_string())?;
    write(root, README, "# Stash\n\ncommitted meanwhile\n")?;
    repo.stage_all().map_err(|e| e.to_string())?;
    repo.commit("Second state", CommitOptions::default())
        .map(drop)
        .map_err(|e| e.to_string())
}

fn pop_into_conflict(root: PathBuf) {
    wait_on(
        root.clone(),
        |c| idle(c) && panel::seen(c).is_some_and(|s| s.changes.is_empty()),
        |root, _| {
            let Some(controller) = window_of(&root) else {
                return finish(SCENARIO);
            };
            let hash = probe::stashes(&controller).remove(0).0;
            probe::stash_action(&controller, &hash, StashAction::Pop);
            wait_on(root.clone(), |c| probe::stash_conflict(c).is_some() && idle(c), popped);
        },
    );
}

fn popped(root: PathBuf, ok: bool) {
    let Some(controller) = window_of(&root) else {
        return finish(SCENARIO);
    };
    let banner = probe::banner(&controller).unwrap_or_default();
    check(
        "a pop with conflicts shows Stash-Konflikt with Fertig and Abbrechen",
        ok && banner[0] == t("Stash-Konflikt") && banner[2] == t("Fertig") && banner[3] == t("Abbrechen …"),
        format!("{banner:?}"),
    );
    check(
        "the panel says that the entry stays, and it does",
        banner[1].contains(t("Der Stash bleibt erhalten, weil sein Anwenden Konflikte hatte."))
            && stash_count(&controller) == 1
            && panel::seen(&controller).is_some_and(|s| s.conflicts == [PathBuf::from(README)]),
        banner[1].clone(),
    );
    pictures(root, "git-stash-conflict", |root| {
        let Some(controller) = window_of(&root) else {
            return finish(SCENARIO);
        };
        probe::end_conflict(&controller, false);
        wait_on(root.clone(), sheet_ready, abort_asked);
    });
}

fn abort_asked(root: PathBuf, ok: bool) {
    let default = sheet_default_button(&root);
    check(
        "abandoning the stash apply asks, and Return cancels",
        ok && default.as_deref() == Some(t("Abbrechen")),
        format!("{default:?}"),
    );
    if !answer_sheet(&root, NSAlertFirstButtonReturn, None) {
        return finish(SCENARIO);
    }
    wait_on(
        root.clone(),
        |c| idle(c) && probe::stash_conflict(c).is_none() && panel::seen(c).is_some_and(|s| s.conflicts.is_empty()),
        aborted,
    );
}

fn aborted(root: PathBuf, ok: bool) {
    let Some(controller) = window_of(&root) else {
        return finish(SCENARIO);
    };
    let readme = std::fs::read_to_string(root.join(README)).unwrap_or_default();
    check(
        "Abbrechen puts back the state before the apply and keeps the entry",
        ok && readme == "# Stash\n\ncommitted meanwhile\n"
            && !root.join(NOTES).exists()
            && stash_count(&controller) == 1
            && probe::banner(&controller).is_none(),
        readme,
    );
    let hash = probe::stashes(&controller).remove(0).0;
    probe::stash_action(&controller, &hash, StashAction::Apply);
    wait_on(
        root.clone(),
        |c| probe::stash_conflict(c) == Some(false) && idle(c),
        |root, ok| {
            check("an apply with conflicts waits as well", ok, "");
            job(
                root,
                |root| write(root, README, "# Stash\n\nresolved\n"),
                finish_conflict,
            );
        },
    );
}

fn finish_conflict(root: PathBuf) {
    let Some(controller) = window_of(&root) else {
        return finish(SCENARIO);
    };
    probe::end_conflict(&controller, true);
    wait_on(
        root.clone(),
        |c| idle(c) && probe::stash_conflict(c).is_none() && panel::seen(c).is_some_and(|s| s.conflicts.is_empty()),
        finished,
    );
}

fn finished(root: PathBuf, ok: bool) {
    let Some(controller) = window_of(&root) else {
        return finish(SCENARIO);
    };
    let seen = panel::seen(&controller);
    check(
        "Fertig marks the conflicts resolved and ends the state",
        ok && seen.as_ref().is_some_and(|s| s.staged.contains(&PathBuf::from(README))),
        seen.map(|s| format!("staged {:?}", s.staged)).unwrap_or_default(),
    );
    check(
        "with something staged the menu bar offers stashing only that",
        probe::can(&controller, sel!(ideGitStashStaged:)),
        "",
    );
    probe::stash_staged(&controller);
    wait_on(
        root.clone(),
        |c| idle(c) && stash_count(c) == 2 && panel::seen(c).is_some_and(|s| s.staged.is_empty()),
        staged_stashed,
    );
}

fn staged_stashed(root: PathBuf, ok: bool) {
    let Some(controller) = window_of(&root) else {
        return finish(SCENARIO);
    };
    check(
        "Nur Vorgemerktes stashen parks the staged changes and leaves the others",
        ok && panel::seen(&controller).is_some_and(|s| !s.changes.is_empty()),
        "",
    );
    let hash = probe::stashes(&controller).remove(0).0;
    probe::stash_action(&controller, &hash, StashAction::Drop);
    wait_on(root.clone(), sheet_ready, delete_asked);
}

fn delete_asked(root: PathBuf, ok: bool) {
    let default = sheet_default_button(&root);
    let delete = sheet_views(&root)
        .into_iter()
        .filter_map(|view| view.downcast::<NSButton>().ok())
        .find(|button| button.title().to_string() == t("Löschen"));
    check(
        "deleting an entry asks; Löschen is destructive and not the default",
        ok && default.as_deref() == Some(t("Abbrechen")) && delete.is_some_and(|b| b.hasDestructiveAction()),
        format!("{default:?}"),
    );
    super::sheet_pictures(root, "git-stash-delete", |root| {
        if !answer_sheet(&root, NSAlertFirstButtonReturn, None) {
            return finish(SCENARIO);
        }
        wait_on(root.clone(), |c| idle(c) && stash_count(c) == 1, deleted);
    });
}

fn deleted(root: PathBuf, ok: bool) {
    check("Löschen deletes the entry", ok, "");
    let Some(window) = window_of(&root).and_then(|c| c.window()) else {
        return finish(SCENARIO);
    };
    snapshot(&window, "git-stash-end");
    let findings = crate::selftest::a11y::audit::findings(&crate::selftest::a11y::audit::elements(&window));
    check(
        "the accessibility audit finds nothing in the window with the stash",
        findings.is_empty(),
        findings
            .iter()
            .map(|f| format!("{}: {} ({})", f.problem, f.what, f.path))
            .collect::<Vec<_>>()
            .join("; "),
    );
    finish(SCENARIO);
}

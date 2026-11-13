//! The history of the `git` scenario past its first page: enough commits that the oldest is beyond it, a row that
//! reads more, a commit shown from outside the panel that is found page by page (E86), and the commit menu's copying
//! and new branch.

use ide_core::LockIgnoringPoison;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use objc2::sel;
use objc2_app_kit::{NSAlertFirstButtonReturn, NSPasteboard};

use super::{SCENARIO, answer_sheet, fixture_git, in_background, wait_for, window_of, write};
use crate::git_panel::selftest::{self as panel, HISTORY_PAGE};
use crate::selftest::{check, finish};
use crate::window::WindowController;

/// Commits made on top, so the first one is older than a page.
const ADDED: usize = HISTORY_PAGE + 5;
const OLDEST: &str = "First state";
const TAKEN_BACK: &str = "taken-back.txt";
const BRANCH: &str = "from-the-start";

/// The hash of the repository's first commit.
static ROOT_COMMIT: Mutex<String> = Mutex::new(String::new());

fn root_commit() -> String {
    ROOT_COMMIT.lock_ignoring_poison().clone()
}

pub fn start(root: PathBuf) {
    in_background(root, stage_one, |root| wait_for(root, |c| staged(c, true), take_back));
}

fn stage_one(root: &Path) -> Result<(), String> {
    write(root, TAKEN_BACK, "staged, then not\n")?;
    let repo = fixture_git()?.discover(root).map_err(|e| e.to_string())?;
    repo.stage(&[TAKEN_BACK]).map_err(|e| e.to_string())
}

/// Whether `TAKEN_BACK` is staged, or else among the changes.
fn staged(controller: &WindowController, staged: bool) -> bool {
    panel::seen(controller).is_some_and(|s| {
        let file = Path::new(TAKEN_BACK);
        !s.busy && s.staged.iter().any(|p| p == file) == staged && s.changes.iter().any(|p| p == file) != staged
    })
}

fn take_back(root: PathBuf, ok: bool) {
    let Some(controller) = window_of(&root) else {
        return finish(SCENARIO);
    };
    check("a staged file is listed as staged", ok, "");
    panel::staged_file_action(&controller, Path::new(TAKEN_BACK), sel!(ideGitUnstage:));
    wait_for(root, |c| staged(c, false), taken_back);
}

fn taken_back(root: PathBuf, ok: bool) {
    check(
        "\"Zurücknehmen\" in a staged file's menu moves it back to the changes",
        ok,
        "",
    );
    in_background(root, add_commits, |root| {
        wait_for(
            root,
            |c| panel::seen(c).is_some_and(|s| !s.busy && s.commits == HISTORY_PAGE) && panel::has_more_row(c),
            paged,
        );
    });
}

fn add_commits(root: &Path) -> Result<(), String> {
    let repo = fixture_git()?.discover(root).map_err(|e| e.to_string())?;
    for n in 0..ADDED {
        write(root, "history.txt", &format!("{n}\n"))?;
        repo.stage(&["history.txt"]).map_err(|e| e.to_string())?;
        repo.commit(&format!("History {n}"), ide_git::CommitOptions::default())
            .map_err(|e| e.to_string())?;
    }
    let hash = repo
        .log(ADDED + 100)
        .map_err(|e| e.to_string())?
        .into_iter()
        .find(|c| c.subject == OLDEST)
        .map(|c| c.hash)
        .ok_or("no first commit")?;
    *ROOT_COMMIT.lock_ignoring_poison() = hash;
    Ok(())
}

fn paged(root: PathBuf, ok: bool) {
    let Some(controller) = window_of(&root) else {
        return finish(SCENARIO);
    };
    let commits = panel::seen(&controller).map_or(0, |s| s.commits);
    check(
        "a long history shows one page of commits and a row that reads more",
        ok,
        format!("{commits} commits"),
    );
    crate::git_panel::reveal_commit(&controller, &root_commit());
    wait_for(
        root,
        |c| crate::git_panel::selected_commit(c).is_some_and(|hash| hash == root_commit()),
        revealed,
    );
}

fn revealed(root: PathBuf, ok: bool) {
    let Some(controller) = window_of(&root) else {
        return finish(SCENARIO);
    };
    let commits = panel::seen(&controller).map_or(0, |s| s.commits);
    check(
        "a commit older than the page shown is read page by page and selected",
        ok && commits > HISTORY_PAGE,
        format!(
            "{commits} commits, selected {:?}",
            crate::git_panel::selected_commit(&controller)
        ),
    );
    let hash = root_commit();
    panel::commit_action(&controller, &hash, sel!(ideGitCopyHash:));
    let hash_copied = pasted();
    panel::commit_action(&controller, &hash, sel!(ideGitCopyMessage:));
    let message_copied = pasted();
    check(
        "the commit menu copies the full hash and the message",
        hash_copied.as_deref() == Some(hash.as_str()) && message_copied.as_deref() == Some(OLDEST),
        format!("{hash_copied:?} {message_copied:?}"),
    );
    panel::commit_action(&controller, &hash, sel!(ideGitBranchHere:));
    wait_for(root, WindowController::has_sheet, name_branch);
}

fn pasted() -> Option<String> {
    // SAFETY: The string type is AppKit's constant.
    unsafe { NSPasteboard::generalPasteboard().stringForType(objc2_app_kit::NSPasteboardTypeString) }
        .map(|s| s.to_string())
}

fn name_branch(root: PathBuf, ok: bool) {
    check("a branch at a commit asks for its name", ok, "");
    if !ok || !answer_sheet(&root, NSAlertFirstButtonReturn, Some(BRANCH)) {
        return finish(SCENARIO);
    }
    wait_for(
        root,
        |c| panel::seen(c).is_some_and(|s| !s.busy && s.branch.as_deref() == Some(BRANCH)),
        branched,
    );
}

fn branched(root: PathBuf, ok: bool) {
    let top = window_of(&root).and_then(|c| panel::seen(&c)).and_then(|s| s.top);
    check(
        "the new branch starts at that commit and is checked out",
        ok && top.as_ref().is_some_and(|(subject, _)| subject == OLDEST),
        format!("{top:?}"),
    );
    super::open_plain_folder();
}

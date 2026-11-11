//! The second half of the `git` scenario, after the first push: a branch made and left through the branch popup, a
//! push the remote refuses, a diverged pull joined by merge and by rebase, a conflicting merge resolved and carried
//! on, a conflicting rebase abandoned, and a hanging fetch cancelled. The remote's branch moves through commits made
//! in the remote repository itself.

use std::cell::Cell;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use ide_git::CommitOptions;
use objc2_app_kit::{NSAlertFirstButtonReturn, NSAlertSecondButtonReturn};
use splat_i18n::t;

use super::{
    README, SCENARIO, answer_sheet, fixture_git, has, in_background, names, pictures, wait_for, window_of, write,
};
use crate::git_panel::selftest::{self as panel, Control, Seen};
use crate::selftest::{check, finish};
use crate::window::WindowController;

const BRANCH: &str = "feature/selftest";
/// The most a cancelled fetch may take to end.
const CANCEL_LIMIT: Duration = Duration::from_secs(10);

thread_local! {
    static FETCH_STARTED: Cell<Option<Instant>> = const { Cell::new(None) };
}

pub fn start(root: PathBuf) {
    let Some(controller) = window_of(&root) else {
        return finish(SCENARIO);
    };
    let offered = panel::choose_branch(&controller, t("Neuer Branch …"));
    check("the branch popup offers a new branch", offered, "");
    wait_for(root, WindowController::has_sheet, name_branch);
}

fn seen(controller: &WindowController) -> Option<Seen> {
    panel::seen(controller)
}

fn describe(controller: &WindowController) -> String {
    seen(controller).map_or_else(String::new, |s| {
        format!(
            "branch {:?} {:?}, top {:?}, operation {:?}, conflicts [{}], problem {:?}",
            s.branch,
            s.ahead_behind,
            s.top,
            s.operation,
            names(&s.conflicts),
            s.problem
        )
    })
}

fn on_branch(controller: &WindowController, name: &str) -> bool {
    seen(controller).is_some_and(|s| !s.busy && s.branch.as_deref() == Some(name))
}

fn name_branch(root: PathBuf, ok: bool) {
    check("a new branch asks for its name", ok, "");
    if !ok || !answer_sheet(&root, NSAlertFirstButtonReturn, Some(BRANCH)) {
        return finish(SCENARIO);
    }
    wait_for(root, |c| on_branch(c, BRANCH), branch_made);
}

fn branch_made(root: PathBuf, ok: bool) {
    let Some(controller) = window_of(&root) else {
        return finish(SCENARIO);
    };
    check("the new branch is checked out", ok, describe(&controller));
    let shown = crate::git_panel::toolbar_seen(&controller);
    check(
        "the toolbar names the branch beside the title and offers the others and a new one",
        !shown.is_empty()
            && shown.iter().all(|seen| {
                seen.as_ref().is_some_and(|(title, choices)| {
                    title == BRANCH
                        && choices.iter().any(|c| c == "master")
                        && choices.iter().any(|c| c == t("Neuer Branch …"))
                })
            }),
        format!("{shown:?}"),
    );
    let offered = crate::git_panel::toolbar_choose(&controller, "master");
    check("the toolbar's branch lists master", offered, "");
    wait_for(root, |c| on_branch(c, "master"), switched_back);
}

fn switched_back(root: PathBuf, ok: bool) {
    let Some(controller) = window_of(&root) else {
        return finish(SCENARIO);
    };
    check(
        "the toolbar's branch switches back to master",
        ok,
        describe(&controller),
    );
    in_background(
        root,
        |root| {
            diverge(
                root,
                ("remote.txt", "remote\n"),
                ("local.txt", "local\n"),
                "Local change",
            )
        },
        push_behind,
    );
}

/// Moves the remote's branch on with a commit of `theirs` and commits `mine` locally as `message`.
fn diverge(root: &Path, theirs: (&str, &str), mine: (&str, &str), message: &str) -> Result<(), String> {
    let git = fixture_git()?;
    let remote = git
        .discover(&root.with_file_name("git-remote"))
        .map_err(|e| e.to_string())?;
    // Pushes move the remote's branch without its index, which would otherwise undo them in the commit.
    remote.unstage_all().map_err(|e| e.to_string())?;
    write(remote.root(), theirs.0, theirs.1)?;
    remote.stage(&[theirs.0]).map_err(|e| e.to_string())?;
    remote
        .commit(&format!("Remote {}", theirs.0), CommitOptions::default())
        .map_err(|e| e.to_string())?;
    let local = git.discover(root).map_err(|e| e.to_string())?;
    write(root, mine.0, mine.1)?;
    local.stage(&[mine.0]).map_err(|e| e.to_string())?;
    local
        .commit(message, CommitOptions::default())
        .map_err(|e| e.to_string())?;
    Ok(())
}

fn push_behind(root: PathBuf) {
    let Some(controller) = window_of(&root) else {
        return finish(SCENARIO);
    };
    panel::click(&controller, Control::Push);
    wait_for(
        root,
        |c| seen(c).is_some_and(|s| !s.busy && s.problem.is_some()),
        push_refused,
    );
}

fn push_refused(root: PathBuf, ok: bool) {
    let Some(controller) = window_of(&root) else {
        return finish(SCENARIO);
    };
    let problem = seen(&controller).and_then(|s| s.problem);
    check(
        "a push behind the remote is refused with a message",
        ok && problem.as_deref()
            == Some(t(
                "Der Remote hat den Push abgelehnt. Hol zuerst seine neuen Commits (Pull).",
            )),
        format!("{problem:?}"),
    );
    pull_and_answer(root, asked_merge);
}

/// Pulls; a diverged branch then asks how to join, which `then` answers.
fn pull_and_answer(root: PathBuf, then: fn(PathBuf, bool)) {
    let Some(controller) = window_of(&root) else {
        return finish(SCENARIO);
    };
    panel::click(&controller, Control::Pull);
    wait_for(root, WindowController::has_sheet, then);
}

fn asked_merge(root: PathBuf, ok: bool) {
    check("a diverged pull asks whether to merge or rebase", ok, "");
    if !ok || !answer_sheet(&root, NSAlertFirstButtonReturn, None) {
        return finish(SCENARIO);
    }
    wait_for(root, |c| joined(c, 2), merged);
}

/// The pull is done, nothing is behind, and the newest commit has `parents` parents.
fn joined(controller: &WindowController, parents: usize) -> bool {
    seen(controller).is_some_and(|s| {
        !s.busy
            && s.operation.is_none()
            && s.ahead_behind.is_some_and(|(_, behind)| behind == 0)
            && s.top.as_ref().is_some_and(|(_, p)| *p == parents)
    })
}

fn merged(root: PathBuf, ok: bool) {
    let Some(controller) = window_of(&root) else {
        return finish(SCENARIO);
    };
    check(
        "Zusammenführen joins the branches in a merge commit",
        ok && root.join("remote.txt").exists(),
        describe(&controller),
    );
    push_in_sync(root, merged_pushed);
}

/// Pushes and waits until nothing is ahead.
fn push_in_sync(root: PathBuf, then: fn(PathBuf, bool)) {
    let Some(controller) = window_of(&root) else {
        return finish(SCENARIO);
    };
    panel::click(&controller, Control::Push);
    wait_for(
        root,
        |c| seen(c).is_some_and(|s| !s.busy && s.ahead_behind == Some((0, 0))),
        then,
    );
}

fn merged_pushed(root: PathBuf, ok: bool) {
    check(
        "the joined branch pushes",
        ok,
        window_of(&root).map(|c| describe(&c)).unwrap_or_default(),
    );
    in_background(
        root,
        |root| {
            diverge(
                root,
                ("remote2.txt", "remote\n"),
                ("local2.txt", "local\n"),
                "Local change two",
            )
        },
        |root| pull_and_answer(root, asked_rebase),
    );
}

fn asked_rebase(root: PathBuf, ok: bool) {
    if !ok || !answer_sheet(&root, NSAlertSecondButtonReturn, None) {
        check("a diverged pull asks again", false, "");
        return finish(SCENARIO);
    }
    wait_for(root, |c| joined(c, 1), rebased);
}

fn rebased(root: PathBuf, ok: bool) {
    let Some(controller) = window_of(&root) else {
        return finish(SCENARIO);
    };
    let top = seen(&controller).and_then(|s| s.top).map(|(subject, _)| subject);
    check(
        "Rebase puts the local commit on top of the remote's",
        ok && top.as_deref() == Some("Local change two") && root.join("remote2.txt").exists(),
        describe(&controller),
    );
    push_in_sync(root, |root, _| {
        in_background(
            root,
            |root| {
                diverge(
                    root,
                    (README, "# Selftest\n\nRemote line.\n"),
                    (README, "# Selftest\n\nLocal line.\n"),
                    "Local README",
                )
            },
            |root| pull_and_answer(root, asked_conflicting_merge),
        );
    });
}

fn asked_conflicting_merge(root: PathBuf, ok: bool) {
    if !ok || !answer_sheet(&root, NSAlertFirstButtonReturn, None) {
        check("a conflicting pull asks how to join", false, "");
        return finish(SCENARIO);
    }
    wait_for(root, |c| stopped_at_conflict(c, "Merge"), merge_conflicted);
}

fn stopped_at_conflict(controller: &WindowController, operation: &str) -> bool {
    seen(controller)
        .is_some_and(|s| !s.busy && s.operation.as_deref() == Some(operation) && has(&s.conflicts, README) && s.banner)
}

fn merge_conflicted(root: PathBuf, ok: bool) {
    let Some(controller) = window_of(&root) else {
        return finish(SCENARIO);
    };
    let can_resume = seen(&controller).is_some_and(|s| s.can_resume);
    check(
        "a conflicting merge lists the file under Konflikte and shows the banner",
        ok && !can_resume,
        describe(&controller),
    );
    pictures(root, "git-conflict", |root| {
        let written = write(&root, README, "# Selftest\n\nBoth lines.\n");
        if let (Ok(()), Some(controller)) = (written, window_of(&root)) {
            panel::resolve(&controller, Path::new(README));
        }
        wait_for(
            root,
            |c| seen(c).is_some_and(|s| s.conflicts.is_empty() && has(&s.staged, README) && s.can_resume),
            resolved,
        );
    });
}

fn resolved(root: PathBuf, ok: bool) {
    let Some(controller) = window_of(&root) else {
        return finish(SCENARIO);
    };
    check(
        "marking a file resolved stages it and allows going on",
        ok,
        describe(&controller),
    );
    let width = controller.inspect().sidebar_width;
    check(
        "the banner's text wraps instead of widening the sidebar",
        (width - 304.0).abs() < 2.0,
        format!("{width} pt"),
    );
    if let Some(window) = controller.window() {
        crate::selftest::snapshot(&window, "git-resolved");
    }
    panel::click(&controller, Control::Resume);
    wait_for(
        root,
        |c| seen(c).is_some_and(|s| !s.busy && s.operation.is_none()),
        continued,
    );
}

fn continued(root: PathBuf, ok: bool) {
    let Some(controller) = window_of(&root) else {
        return finish(SCENARIO);
    };
    let s = seen(&controller);
    check(
        "Fortsetzen finishes the merge with a merge commit and hides the banner",
        ok && s
            .as_ref()
            .is_some_and(|s| !s.banner && s.top.as_ref().is_some_and(|(_, p)| *p == 2)),
        describe(&controller),
    );
    push_in_sync(root, |root, _| {
        in_background(
            root,
            |root| {
                diverge(
                    root,
                    (README, "# Selftest\n\nRemote again.\n"),
                    (README, "# Selftest\n\nLocal again.\n"),
                    "Local README again",
                )
            },
            |root| pull_and_answer(root, asked_conflicting_rebase),
        );
    });
}

fn asked_conflicting_rebase(root: PathBuf, ok: bool) {
    if !ok || !answer_sheet(&root, NSAlertSecondButtonReturn, None) {
        check("a conflicting pull asks again", false, "");
        return finish(SCENARIO);
    }
    wait_for(root, |c| stopped_at_conflict(c, "Rebase"), rebase_conflicted);
}

fn rebase_conflicted(root: PathBuf, ok: bool) {
    let Some(controller) = window_of(&root) else {
        return finish(SCENARIO);
    };
    check("a conflicting rebase stops with the banner", ok, describe(&controller));
    panel::click(&controller, Control::Abort);
    wait_for(root, WindowController::has_sheet, asked_abort);
}

fn asked_abort(root: PathBuf, ok: bool) {
    check("abandoning a rebase asks first", ok, "");
    if !ok || !answer_sheet(&root, NSAlertFirstButtonReturn, None) {
        return finish(SCENARIO);
    }
    wait_for(
        root,
        |c| seen(c).is_some_and(|s| !s.busy && s.operation.is_none() && s.conflicts.is_empty()),
        aborted,
    );
}

fn aborted(root: PathBuf, ok: bool) {
    let Some(controller) = window_of(&root) else {
        return finish(SCENARIO);
    };
    let s = seen(&controller);
    let kept = s.as_ref().is_some_and(|s| {
        s.top
            .as_ref()
            .is_some_and(|(subject, _)| subject == "Local README again")
            && s.ahead_behind == Some((1, 1))
    });
    let text = std::fs::read_to_string(root.join(README)).unwrap_or_default();
    check(
        "an abandoned rebase leaves the local commit as it was",
        ok && kept && text.contains("Local again."),
        describe(&controller),
    );
    in_background(root, make_origin_hang, fetch_hanging);
}

/// Points origin at a server that never answers, as one that hangs would.
fn make_origin_hang(root: &Path) -> Result<(), String> {
    let config = root.join(".git/config");
    let text = std::fs::read_to_string(&config).map_err(|e| e.to_string())?;
    let remote = root.with_file_name("git-remote");
    let text = text.replace(
        &format!("url = {}", remote.display()),
        "url = ssh://example.invalid/repo.git",
    );
    let text = format!("{text}[core]\n\tsshCommand = sh -c 'sleep 30'\n");
    std::fs::write(&config, text).map_err(|e| e.to_string())
}

fn fetch_hanging(root: PathBuf) {
    let Some(controller) = window_of(&root) else {
        return finish(SCENARIO);
    };
    panel::click(&controller, Control::Fetch);
    FETCH_STARTED.with(|started| started.set(Some(Instant::now())));
    wait_for(root, |c| seen(c).is_some_and(|s| s.busy), fetching);
}

fn fetching(root: PathBuf, ok: bool) {
    let Some(controller) = window_of(&root) else {
        return finish(SCENARIO);
    };
    check("a fetch runs until it is cancelled", ok, "");
    panel::click(&controller, Control::Cancel);
    wait_for(root, |c| seen(c).is_some_and(|s| !s.busy), cancelled);
}

fn cancelled(root: PathBuf, ok: bool) {
    let Some(controller) = window_of(&root) else {
        return finish(SCENARIO);
    };
    let took = FETCH_STARTED.with(Cell::get).map(|started| started.elapsed());
    let problem = seen(&controller).and_then(|s| s.problem);
    check(
        "cancelling stops the fetch quietly",
        ok && problem.is_none() && took.is_some_and(|t| t < CANCEL_LIMIT),
        format!("after {took:?}, problem {problem:?}"),
    );
    super::sheets::start(root);
}

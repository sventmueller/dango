//! Within the `ci` scenario: re-running a single job and a whole run, starting a workflow, a manual job and a new
//! pipeline, and the open pull requests, merge requests and issues of both stubs.

use std::path::PathBuf;

use objc2_app_kit::NSAlertFirstButtonReturn;
use splat_i18n::t;

use super::super::git::{answer_sheet, pictures, wait_for, window_of};
use super::super::{check, finish};
use super::{SCENARIO, describe, posted_to, stubs};
use crate::ci_panel::selftest::{self as panel, Tab};
use crate::window::WindowController;

fn idle(controller: &WindowController) -> bool {
    panel::seen(controller).is_some_and(|s| !s.busy)
}

fn github_posted(path: &'static str) -> impl Fn(&WindowController) -> bool {
    move |c| posted_to(&stubs().github, path) && idle(c)
}

/// Starts with the GitHub repository's failed job 72 of run 9001.
pub(super) fn github(root: PathBuf) {
    let Some(controller) = window_of(&root) else {
        return finish(SCENARIO);
    };
    let menu = panel::menu_of_job(&controller, 9001, 72);
    check(
        "a finished job offers to start it again, and only a waiting one to approve its deployment",
        menu.contains(&(t("Job neu starten").to_string(), true))
            && menu.contains(&(t("Deployment freigeben").to_string(), false)),
        format!("{menu:?}"),
    );
    panel::retry_job(&controller, 9001, 72);
    wait_for(root, |c| github_posted("/actions/jobs/72/rerun")(c), job_retried);
}

fn job_retried(root: PathBuf, ok: bool) {
    let Some(controller) = window_of(&root) else {
        return finish(SCENARIO);
    };
    check("a single GitHub job starts again", ok, "");
    panel::rerun_all(&controller, 9001);
    wait_for(root, |c| github_posted("/actions/runs/9001/rerun")(c), run_rerun);
}

fn run_rerun(root: PathBuf, ok: bool) {
    let Some(controller) = window_of(&root) else {
        return finish(SCENARIO);
    };
    check("every job of a GitHub run starts again", ok, "");
    let workflows = panel::seen(&controller).map(|s| s.workflows).unwrap_or_default();
    check(
        "the checkout's workflow that names workflow_dispatch may be started",
        workflows == ["release.yml"] && panel::can_start_run(&controller),
        format!("{workflows:?}"),
    );
    panel::start_run(&controller);
    wait_for(root, WindowController::has_sheet, workflow_sheet);
}

fn workflow_sheet(root: PathBuf, ok: bool) {
    if !ok || !answer_sheet(&root, NSAlertFirstButtonReturn, None) {
        check("starting a workflow asks which one first", false, "");
        return finish(SCENARIO);
    }
    wait_for(
        root,
        |c| github_posted("/actions/workflows/release.yml/dispatches")(c),
        workflow_started,
    );
}

fn workflow_started(root: PathBuf, ok: bool) {
    let Some(controller) = window_of(&root) else {
        return finish(SCENARIO);
    };
    let body = stubs()
        .github
        .seen()
        .into_iter()
        .find(|s| s.target.ends_with("/dispatches"))
        .map(|s| s.body)
        .unwrap_or_default();
    check(
        "the chosen workflow starts on the branch",
        ok && body.contains("\"ref\""),
        body,
    );
    panel::choose_tab(&controller, Tab::Changes);
    wait_for(
        root,
        |c| panel::seen(c).is_some_and(|s| s.tab == Tab::Changes && s.items.len() == 2),
        pulls_listed,
    );
}

fn pulls_listed(root: PathBuf, ok: bool) {
    let Some(controller) = window_of(&root) else {
        return finish(SCENARIO);
    };
    let seen = panel::seen(&controller);
    let numbers: Vec<u64> = seen
        .as_ref()
        .map_or_else(Vec::new, |s| s.items.iter().map(|i| i.0).collect());
    check(
        "the open pull requests list newest first, one row each",
        ok && numbers == [31, 30] && seen.as_ref().is_some_and(|s| s.rows == 2 && s.note.is_none()),
        describe(&controller),
    );
    pictures(root, "ci-pulls", |root| {
        if let Some(controller) = window_of(&root) {
            panel::choose_tab(&controller, Tab::Issues);
        }
        wait_for(
            root,
            |c| panel::seen(c).is_some_and(|s| s.tab == Tab::Issues && !s.items.is_empty()),
            github_issues_listed,
        );
    });
}

fn github_issues_listed(root: PathBuf, ok: bool) {
    let Some(controller) = window_of(&root) else {
        return finish(SCENARIO);
    };
    let items = panel::seen(&controller).map(|s| s.items).unwrap_or_default();
    check(
        "GitHub's open issues leave out the pull requests among them",
        ok && items.iter().map(|i| i.0).collect::<Vec<_>>() == [28],
        format!("{items:?}"),
    );
    panel::choose_tab(&controller, Tab::Runs);
    wait_for(
        root,
        |c| panel::seen(c).is_some_and(|s| s.tab == Tab::Runs && s.rows > 0 && !s.busy),
        |root, _| super::hand_to_agent(root),
    );
}

fn gitlab_posted(path: &'static str) -> impl Fn(&WindowController) -> bool {
    move |c| posted_to(&stubs().gitlab, path) && idle(c)
}

/// Starts with the GitLab pipeline 501, whose job 3002 failed and whose job 3004 waits to be started by hand.
pub(super) fn gitlab(root: PathBuf) {
    let Some(controller) = window_of(&root) else {
        return finish(SCENARIO);
    };
    let menu = panel::menu_of_job(&controller, 501, 3004);
    check(
        "a manual GitLab job offers to start it",
        menu.contains(&(t("Job starten").to_string(), true)),
        format!("{menu:?}"),
    );
    panel::retry_job(&controller, 501, 3002);
    wait_for(root, |c| gitlab_posted("/jobs/3002/retry")(c), gitlab_retried);
}

fn gitlab_retried(root: PathBuf, ok: bool) {
    let Some(controller) = window_of(&root) else {
        return finish(SCENARIO);
    };
    check("a single GitLab job starts again", ok, "");
    panel::start_job(&controller, 501, 3004);
    wait_for(root, |c| gitlab_posted("/jobs/3004/play")(c), played);
}

fn played(root: PathBuf, ok: bool) {
    let Some(controller) = window_of(&root) else {
        return finish(SCENARIO);
    };
    check("a manual GitLab job starts", ok, "");
    panel::start_run(&controller);
    wait_for(root, WindowController::has_sheet, pipeline_sheet);
}

fn pipeline_sheet(root: PathBuf, ok: bool) {
    if !ok || !answer_sheet(&root, NSAlertFirstButtonReturn, None) {
        check("starting a pipeline asks first", false, "");
        return finish(SCENARIO);
    }
    wait_for(root, |c| gitlab_posted("/pipeline?ref=")(c), pipeline_started);
}

fn pipeline_started(root: PathBuf, ok: bool) {
    let Some(controller) = window_of(&root) else {
        return finish(SCENARIO);
    };
    check("a new GitLab pipeline starts for the branch", ok, "");
    panel::choose_tab(&controller, Tab::Changes);
    wait_for(
        root,
        |c| panel::seen(c).is_some_and(|s| s.tab == Tab::Changes && s.items.len() == 2),
        merge_requests_listed,
    );
}

fn merge_requests_listed(root: PathBuf, ok: bool) {
    let Some(controller) = window_of(&root) else {
        return finish(SCENARIO);
    };
    let items = panel::seen(&controller).map(|s| s.items).unwrap_or_default();
    check(
        "the open merge requests list newest first",
        ok && items.iter().map(|i| i.0).collect::<Vec<_>>() == [14, 13],
        format!("{items:?}"),
    );
    pictures(root, "ci-merge-requests", |root| {
        if let Some(controller) = window_of(&root) {
            panel::choose_tab(&controller, Tab::Runs);
        }
        wait_for(
            root,
            |c| panel::seen(c).is_some_and(|s| s.tab == Tab::Runs && s.rows > 0 && !s.busy),
            |root, _| super::ask_to_cancel(root),
        );
    });
}

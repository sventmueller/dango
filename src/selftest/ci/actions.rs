//! Within the `ci` scenario: re-running a single job and a whole run, starting a workflow, a manual job and a new
//! pipeline, and the open pull requests, merge requests and issues of both stubs.

use std::path::PathBuf;

use objc2::sel;
use objc2_app_kit::NSAlertFirstButtonReturn;
use splat_i18n::t;

use super::super::git::{answer_sheet, pictures, wait_for, window_of};
use super::super::{check, give_up};
use super::{SCENARIO, describe, posted_to, stubs};
use crate::ci_panel::selftest::{self as panel, Key, Tab};
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
        return give_up(SCENARIO, "the folder's window is open");
    };
    let menu = panel::menu_of_job(&controller, 9001, 72);
    check(
        "a finished job offers to start it again, and only a waiting one to approve its deployment",
        menu.contains(&(t("Job neu starten").to_string(), true))
            && menu.contains(&(t("Deployment freigeben").to_string(), false)),
        format!("{menu:?}"),
    );
    let item = |title: &'static str, enabled: bool| (t(title).to_string(), enabled);
    let failed = panel::menu_of_run(&controller, 9001);
    check(
        "a failed GitHub run offers its failed jobs or all of them again, but not to be cancelled",
        failed
            == [
                item("Fehlgeschlagene Jobs neu starten", true),
                item("Alle Jobs neu starten", true),
                item("Lauf abbrechen …", false),
                item("Im Browser öffnen", true),
            ],
        format!("{failed:?}"),
    );
    let account = panel::account_menu(&controller);
    check(
        "without a token the account menu says only public data is read and offers to sign in",
        account.get(1) == Some(&item("Ohne Anmeldung, nur öffentliche Daten", false))
            && account.last() == Some(&item("Anmelden …", true)),
        format!("{account:?}"),
    );
    panel::choose_in_menu(&controller, Key::Job(9001, 72), sel!(ideCiRetryJob:));
    wait_for(root, |c| github_posted("/actions/jobs/72/rerun")(c), job_retried);
}

fn job_retried(root: PathBuf, ok: bool) {
    let Some(controller) = window_of(&root) else {
        return give_up(SCENARIO, "the folder's window is open");
    };
    check("a single GitHub job starts again", ok, "");
    panel::choose_in_menu(&controller, Key::Run(9001), sel!(ideCiRerunAll:));
    wait_for(
        root,
        |c| github_posted("/actions/runs/9001/rerun")(c) && panel::can_start_run(c),
        run_rerun,
    );
}

fn run_rerun(root: PathBuf, ok: bool) {
    let Some(controller) = window_of(&root) else {
        return give_up(SCENARIO, "the folder's window is open");
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
        return give_up(SCENARIO, "starting a workflow asks which one first");
    }
    wait_for(
        root,
        |c| github_posted("/actions/workflows/release.yml/dispatches")(c),
        workflow_started,
    );
}

fn workflow_started(root: PathBuf, ok: bool) {
    let Some(controller) = window_of(&root) else {
        return give_up(SCENARIO, "the folder's window is open");
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
        return give_up(SCENARIO, "the folder's window is open");
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
    let menu = panel::menu_of_item(&controller, 31);
    check(
        "a pull request's row opens it in the browser or copies its link",
        menu == [
            (t("Im Browser öffnen").to_string(), true),
            (t("Link kopieren").to_string(), true),
        ],
        format!("{menu:?}"),
    );
    panel::choose_in_menu(&controller, Key::Item(31), sel!(ideCiCopyLink:));
    // SAFETY: The string type is AppKit's constant.
    let copied = unsafe {
        objc2_app_kit::NSPasteboard::generalPasteboard().stringForType(objc2_app_kit::NSPasteboardTypeString)
    }
    .map(|s| s.to_string());
    check(
        "\"Link kopieren\" puts the pull request's page on the pasteboard",
        copied.as_deref() == Some("https://github.com/acme/widget/pull/31"),
        format!("{copied:?}"),
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
        return give_up(SCENARIO, "the folder's window is open");
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
        return give_up(SCENARIO, "the folder's window is open");
    };
    let menu = panel::menu_of_job(&controller, 501, 3004);
    check(
        "a manual GitLab job offers to start it",
        menu.contains(&(t("Job starten").to_string(), true)),
        format!("{menu:?}"),
    );
    let running = panel::menu_of_run(&controller, 501);
    check(
        "a running GitLab pipeline can be cancelled, and GitLab has no re-run of all its jobs",
        running
            == [
                (t("Fehlgeschlagene Jobs neu starten").to_string(), false),
                (t("Lauf abbrechen …").to_string(), true),
                (t("Im Browser öffnen").to_string(), true),
            ],
        format!("{running:?}"),
    );
    panel::choose_in_menu(&controller, Key::Job(501, 3002), sel!(ideCiRetryJob:));
    wait_for(root, |c| gitlab_posted("/jobs/3002/retry")(c), gitlab_retried);
}

fn gitlab_retried(root: PathBuf, ok: bool) {
    let Some(controller) = window_of(&root) else {
        return give_up(SCENARIO, "the folder's window is open");
    };
    check("a single GitLab job starts again", ok, "");
    panel::choose_in_menu(&controller, Key::Job(501, 3004), sel!(ideCiStartJob:));
    wait_for(root, |c| gitlab_posted("/jobs/3004/play")(c), played);
}

fn played(root: PathBuf, ok: bool) {
    let Some(controller) = window_of(&root) else {
        return give_up(SCENARIO, "the folder's window is open");
    };
    check("a manual GitLab job starts", ok, "");
    panel::start_run(&controller);
    wait_for(root, WindowController::has_sheet, pipeline_sheet);
}

fn pipeline_sheet(root: PathBuf, ok: bool) {
    if !ok || !answer_sheet(&root, NSAlertFirstButtonReturn, None) {
        return give_up(SCENARIO, "starting a pipeline asks first");
    }
    wait_for(root, |c| gitlab_posted("/pipeline?ref=")(c), pipeline_started);
}

fn pipeline_started(root: PathBuf, ok: bool) {
    let Some(controller) = window_of(&root) else {
        return give_up(SCENARIO, "the folder's window is open");
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
        return give_up(SCENARIO, "the folder's window is open");
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

//! The `ci` scenario against two stub servers, one answering as GitHub, one as a self-managed GitLab under a path:
//! runs, jobs over several pages and steps, the badge, a running run read again until it ends, a log with the find
//! bar, re-running, a failure handed to an agent, a token asked for and sent only to its host, and cancelling.
//! `ci-login` signs in to both stubs through the browser (`login`); `ci-settings` changes the host rules and OAuth
//! apps in the settings window (`hosts`).
//! `ci-live` reads, and never changes, the runs of the owner's real repository when `gh` or `glab` are logged in.

mod hosts;
mod login;
pub(super) mod stub;

use std::path::{Path, PathBuf};
use std::sync::{Arc, OnceLock};
use std::time::Duration;

use ide_ci::State;
use ide_core::settings::Settings;
use objc2::MainThreadMarker;
use objc2_app_kit::NSAlertFirstButtonReturn;

use super::git::{answer_sheet, append, fixture_git, pictures, wait_for, window_of, write};
use super::{check, finish};
use crate::ci_panel::selftest as panel;
use crate::util::main_soon;
use crate::window::{Panel, WindowController};
use stub::{Service, Stub};

const SCENARIO: &str = "ci";
const LIVE: &str = "ci-live";
const TOKEN: &str = "glpat-selftest";

struct Stubs {
    github: Arc<Stub>,
    gitlab: Arc<Stub>,
    github_root: PathBuf,
    gitlab_root: PathBuf,
}

static STUBS: OnceLock<Stubs> = OnceLock::new();

fn stubs() -> &'static Stubs {
    STUBS.get().expect("the scenario starts the stubs first")
}

pub fn run() {
    begin(SCENARIO, |stubs| stubs.github_root.clone(), github_listed);
}

pub fn login() {
    begin(login::SCENARIO, |stubs| stubs.gitlab_root.clone(), login::start);
}

pub fn hosts() {
    begin(hosts::SCENARIO, |stubs| stubs.gitlab_root.clone(), hosts::start);
}

/// Starts the stubs, then opens the repository `first` names and goes on with `then` once it is read.
fn begin(scenario: &'static str, first: fn(&Stubs) -> PathBuf, then: fn(PathBuf, bool)) {
    std::thread::spawn(move || {
        let made = prepare();
        main_soon(move || match made {
            Ok(made) => {
                let _ = STUBS.set(made);
                open(first(stubs()), then);
            }
            Err(error) => {
                check("the stub servers and test repositories are made", false, error);
                finish(scenario);
            }
        });
    });
}

/// Two stubs, a repository whose origin is each, and host rules that tell the app what runs there.
fn prepare() -> Result<Stubs, String> {
    let github = Stub::start(Service::GitHub, None).map_err(|e| e.to_string())?;
    let gitlab = Stub::start(Service::GitLab, Some(TOKEN)).map_err(|e| e.to_string())?;
    let home = ide_core::paths::home();
    let github_root = repository(
        &home.join("ci-github"),
        &format!("http://127.0.0.1:{}/acme/widget.git", github.port),
        &github,
    )?;
    let gitlab_root = repository(
        &home.join("ci-gitlab"),
        &format!("http://127.0.0.1:{}/gitlab/group/app.git", gitlab.port),
        &gitlab,
    )?;
    let rules = vec![
        format!("127.0.0.1:{}=github", github.port),
        format!("127.0.0.1:{}/gitlab=gitlab", gitlab.port),
    ];
    Settings::load_user()
        .set(&crate::ci_panel::CI_HOSTS, &rules)
        .map_err(|e| e.to_string())?;
    Ok(Stubs {
        github,
        gitlab,
        github_root,
        gitlab_root,
    })
}

/// A repository with one commit whose origin is `url`; `stub` answers for its HEAD.
fn repository(folder: &Path, url: &str, stub: &Stub) -> Result<PathBuf, String> {
    let git = fixture_git()?;
    std::fs::create_dir_all(folder).map_err(|e| e.to_string())?;
    let repo = git.init(folder).map_err(|e| e.to_string())?;
    append(
        &repo.git_dir().join("config"),
        &format!("[user]\n\tname = Selftest\n\temail = selftest@example.invalid\n[remote \"origin\"]\n\turl = {url}\n"),
    )?;
    write(folder, "README.md", "# CI\n")?;
    repo.stage_all().map_err(|e| e.to_string())?;
    repo.commit("First state", ide_git::CommitOptions::default())
        .map_err(|e| e.to_string())?;
    let sha = repo.head().map_err(|e| e.to_string())?.unwrap_or_default();
    let branch = repo.current_branch().map_err(|e| e.to_string())?.unwrap_or_default();
    stub.set_head(&sha, &branch);
    Ok(folder.canonicalize().unwrap_or(folder.to_path_buf()))
}

fn open(root: PathBuf, then: fn(PathBuf, bool)) {
    let Some(mtm) = MainThreadMarker::new() else { return };
    crate::project::trust(&root);
    crate::app::open_project(mtm, root.clone());
    if let Some(controller) = window_of(&root) {
        controller.activate_panel(Panel::Ci);
    }
    wait_for(root, ready, then);
}

fn ready(controller: &WindowController) -> bool {
    panel::seen(controller).is_some_and(|s| s.phase != "Loading" && !s.runs.is_empty() || s.phase == "SignIn")
}

fn describe(controller: &WindowController) -> String {
    panel::seen(controller).map_or_else(String::new, |s| {
        let runs: Vec<String> = s
            .runs
            .iter()
            .map(|(name, _, state, jobs)| {
                let jobs: Vec<String> = jobs
                    .iter()
                    .map(|(n, _, st, steps)| format!("{n} {st:?} {steps}"))
                    .collect();
                format!("{name} {state:?} [{}]", jobs.join(", "))
            })
            .collect();
        format!(
            "{} {:?}: {}; account {:?} {:?}; problem {:?}; {} rows",
            s.phase,
            s.forge,
            runs.join("; "),
            s.account,
            s.source,
            s.problem,
            s.rows
        )
    })
}

fn github_listed(root: PathBuf, ok: bool) {
    let Some(controller) = window_of(&root) else {
        return finish(SCENARIO);
    };
    let seen = panel::seen(&controller);
    let runs = seen.as_ref().map(|s| s.runs.clone()).unwrap_or_default();
    let listed = runs.len() == 2
        && runs[0].2 == State::Failed
        && runs[0].3.iter().map(|j| (j.0.as_str(), j.2, j.3)).collect::<Vec<_>>()
            == [("build", State::Success, 2), ("test", State::Failed, 3)]
        && runs[1].2 == State::Running;
    check(
        "the panel lists HEAD's runs, their jobs over two pages and their steps",
        ok && listed,
        describe(&controller),
    );
    let badge = controller.badge(Panel::Ci);
    check(
        "the CI symbol shows a dot for the failed run",
        badge.as_deref() == Some(""),
        format!("{badge:?}"),
    );
    let unsent = stubs().github.seen().iter().all(|s| s.authorization.is_none());
    check(
        "no token goes to a host nobody gave one for",
        unsent,
        format!("{} requests", stubs().github.seen().len()),
    );
    pictures(root, "ci-runs", |root| wait_for(root, lint_done, lint_finished));
}

fn lint_done(controller: &WindowController) -> bool {
    panel::seen(controller).is_some_and(|s| s.runs.get(1).is_some_and(|r| r.2 == State::Success))
}

fn lint_finished(root: PathBuf, ok: bool) {
    let Some(controller) = window_of(&root) else {
        return finish(SCENARIO);
    };
    check("a running run is read again until it ends", ok, describe(&controller));
    panel::press_return_on(&controller, 9001, 72);
    wait_for(
        root,
        |c| panel::log(c, 72, false).is_some_and(|(lines, _)| !lines.is_empty()),
        log_shown,
    );
}

fn log_shown(root: PathBuf, ok: bool) {
    let Some(controller) = window_of(&root) else {
        return finish(SCENARIO);
    };
    let (lines, found) = panel::log(&controller, 72, true).unwrap_or_default();
    let text = ide_ci::log::text(&lines);
    let error = lines
        .iter()
        .any(|l| l.kind == ide_ci::log::LineKind::Error && l.text.contains("exit code 101"));
    check(
        "Return on a failed job opens its log in a tab without escape sequences and with its error marked",
        ok && error && !text.contains('\u{1b}') && text.contains("panicked at src/parser.rs:42:9"),
        format!("{} lines", lines.len()),
    );
    check("⌘F shows the find bar of the log", found, "");
    pictures(root, "ci-log", |root| {
        if let Some(controller) = window_of(&root) {
            panel::rerun(&controller, 9001);
        }
        wait_for(root, |_| posted("/rerun-failed-jobs"), rerun_sent);
    });
}

fn posted(path: &str) -> bool {
    stubs()
        .github
        .seen()
        .iter()
        .any(|s| s.method == "POST" && s.target.ends_with(path))
}

fn rerun_sent(root: PathBuf, ok: bool) {
    let Some(controller) = window_of(&root) else {
        return finish(SCENARIO);
    };
    check("re-running asks the service to start the failed jobs again", ok, "");
    panel::send_to_agent(&controller, 9001, 72);
    wait_for(
        root,
        |c| panel::agent_draft(c).is_some_and(|d| d.contains("panicked at")),
        drafted,
    );
}

fn drafted(root: PathBuf, ok: bool) {
    let Some(controller) = window_of(&root) else {
        return finish(SCENARIO);
    };
    let draft = panel::agent_draft(&controller).unwrap_or_default();
    let marked = panel::agent_has_outside(&controller);
    check(
        "a failed job's log goes to a new AI tab as an attachment marked as outside text",
        ok && marked && draft.contains("exit code 101"),
        format!("{} characters", draft.chars().count()),
    );
    open(stubs().gitlab_root.clone(), gitlab_asks);
}

fn gitlab_asks(root: PathBuf, ok: bool) {
    let Some(controller) = window_of(&root) else {
        return finish(SCENARIO);
    };
    let phase = panel::seen(&controller).map(|s| s.phase).unwrap_or_default();
    check(
        "a GitLab that wants a token asks for one",
        ok && phase == "SignIn",
        describe(&controller),
    );
    pictures(root, "ci-sign-in", |root| {
        if let Some(controller) = window_of(&root) {
            panel::sign_in(&controller);
        }
        wait_for(root, WindowController::has_sheet, token_sheet);
    });
}

fn token_sheet(root: PathBuf, ok: bool) {
    if !ok || !answer_sheet(&root, NSAlertFirstButtonReturn, Some(TOKEN)) {
        check("the token sheet opens", false, "");
        return finish(SCENARIO);
    }
    wait_for(
        root,
        |c| panel::seen(c).is_some_and(|s| s.phase == "Ready" && s.account.is_some()),
        gitlab_listed,
    );
}

fn gitlab_listed(root: PathBuf, ok: bool) {
    let Some(controller) = window_of(&root) else {
        return finish(SCENARIO);
    };
    let seen = panel::seen(&controller);
    let jobs: Vec<String> = seen
        .as_ref()
        .and_then(|s| s.runs.first())
        .map(|r| r.3.iter().map(|j| j.0.clone()).collect())
        .unwrap_or_default();
    check(
        "with the token the GitLab pipeline shows its jobs from two pages and the account",
        ok && jobs == ["build", "test", "lint", "deploy"]
            && seen
                .as_ref()
                .is_some_and(|s| s.account.as_deref() == Some("lab-selftest")),
        describe(&controller),
    );
    let bearer = format!("Bearer {TOKEN}");
    let only_there = stubs()
        .github
        .seen()
        .iter()
        .all(|s| s.authorization.as_deref() != Some(&bearer))
        && stubs()
            .gitlab
            .seen()
            .iter()
            .any(|s| s.authorization.as_deref() == Some(&bearer));
    check("the token goes to its own host only", only_there, "");
    pictures(root, "ci-gitlab", |root| {
        if let Some(controller) = window_of(&root) {
            panel::cancel(&controller, 501);
        }
        wait_for(root, WindowController::has_sheet, cancel_sheet);
    });
}

fn cancel_sheet(root: PathBuf, ok: bool) {
    check("cancelling a run asks first", ok, "");
    if !ok || !answer_sheet(&root, NSAlertFirstButtonReturn, None) {
        return finish(SCENARIO);
    }
    wait_for(
        root,
        |c| panel::seen(c).is_some_and(|s| s.runs.first().is_some_and(|r| r.2 == State::Cancelled)),
        cancelled,
    );
}

fn cancelled(root: PathBuf, ok: bool) {
    let Some(controller) = window_of(&root) else {
        return finish(SCENARIO);
    };
    check("a cancelled pipeline shows as cancelled", ok, describe(&controller));
    finish(SCENARIO);
}

/// The owner's repository `ci-live` reads: `IDE_CI_LIVE_REPO`, else `~/Repos/splat`.
fn live_repository() -> Option<PathBuf> {
    let path = std::env::var_os("IDE_CI_LIVE_REPO")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join("Repos/splat")))?;
    path.join(".git").exists().then(|| path.canonicalize().unwrap_or(path))
}

pub fn live() {
    let Some(root) = live_repository() else {
        check(
            "reading a real repository's runs",
            true,
            "skipped: no repository at IDE_CI_LIVE_REPO or ~/Repos/splat",
        );
        return finish(LIVE);
    };
    let Some(mtm) = MainThreadMarker::new() else { return };
    crate::project::trust(&root);
    crate::app::open_project(mtm, root.clone());
    if let Some(controller) = window_of(&root) {
        controller.activate_panel(Panel::Ci);
    }
    super::git::poll(
        root,
        std::time::Instant::now() + Duration::from_secs(60),
        |c| panel::seen(c).is_some_and(|s| s.phase != "Loading"),
        live_read,
    );
}

fn live_read(root: PathBuf, ok: bool) {
    let Some(controller) = window_of(&root) else {
        return finish(LIVE);
    };
    let seen = panel::seen(&controller);
    let phase = seen.as_ref().map(|s| s.phase.clone()).unwrap_or_default();
    match phase.as_str() {
        "Ready" => check("reading a real repository's runs", ok, describe(&controller)),
        "SignIn" | "Unknown" => check(
            "reading a real repository's runs",
            true,
            format!("skipped: {phase}, no usable login for {}", root.display()),
        ),
        _ => check("reading a real repository's runs", false, describe(&controller)),
    }
    pictures(root, "ci-live", |_| finish(LIVE));
}

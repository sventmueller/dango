//! GitHub: Actions' workflow runs, their jobs and steps, job logs, re-running, starting, approving deployments and
//! cancelling; open pull requests and issues.

use serde::Deserialize;

use crate::client::{Client, encode};
use crate::error::Error;
use crate::error::Result;
use crate::http;
use crate::model::{Item, Job, Run, State, Step, seconds};

#[derive(Deserialize)]
struct Runs {
    workflow_runs: Vec<RawRun>,
}

#[derive(Deserialize)]
struct RawRun {
    id: u64,
    name: Option<String>,
    display_title: Option<String>,
    run_number: u64,
    event: Option<String>,
    status: Option<String>,
    conclusion: Option<String>,
    head_branch: Option<String>,
    head_sha: String,
    html_url: String,
    created_at: Option<String>,
    updated_at: Option<String>,
}

#[derive(Deserialize)]
struct Jobs {
    jobs: Vec<RawJob>,
}

#[derive(Deserialize)]
struct RawJob {
    id: u64,
    run_id: u64,
    name: String,
    status: Option<String>,
    conclusion: Option<String>,
    html_url: Option<String>,
    started_at: Option<String>,
    completed_at: Option<String>,
    #[serde(default)]
    steps: Vec<RawStep>,
}

#[derive(Deserialize)]
struct RawItem {
    number: u64,
    title: String,
    user: Option<RawUser>,
    html_url: String,
    updated_at: Option<String>,
    #[serde(default)]
    draft: bool,
    head: Option<RawHead>,
    pull_request: Option<serde_json::Value>,
}

#[derive(Deserialize)]
struct RawUser {
    login: String,
}

#[derive(Deserialize)]
struct RawHead {
    #[serde(rename = "ref")]
    reference: String,
}

#[derive(Deserialize)]
struct RawPending {
    environment: RawEnvironment,
    #[serde(default)]
    current_user_can_approve: bool,
}

#[derive(Deserialize)]
struct RawEnvironment {
    id: u64,
}

#[derive(Deserialize)]
struct RawStep {
    name: String,
    number: u32,
    status: Option<String>,
    conclusion: Option<String>,
}

/// The state of `status` and `conclusion`, as runs, jobs and steps report them.
fn state(status: Option<&str>, conclusion: Option<&str>) -> State {
    match (status, conclusion) {
        (Some("in_progress"), _) => State::Running,
        (Some("waiting"), _) => State::Manual,
        (Some("completed"), Some("success")) => State::Success,
        (Some("completed"), Some("failure" | "timed_out" | "startup_failure")) => State::Failed,
        (Some("completed"), Some("cancelled")) => State::Cancelled,
        (Some("completed"), Some("skipped")) => State::Skipped,
        (Some("completed"), Some("action_required")) => State::Manual,
        (Some("completed"), _) => State::Neutral,
        _ => State::Queued,
    }
}

fn repo(client: &Client) -> String {
    format!("{}/repos/{}", client.forge.api, client.forge.project)
}

pub(crate) fn runs(client: &Client, branch: Option<&str>, sha: &str) -> Result<Vec<Run>> {
    let by_commit = format!("{}/actions/runs?head_sha={}&per_page=30", repo(client), encode(sha));
    let mut runs = read_runs(client, &by_commit)?;
    if runs.is_empty()
        && let Some(branch) = branch
    {
        let by_branch = format!("{}/actions/runs?branch={}&per_page=10", repo(client), encode(branch));
        runs = read_runs(client, &by_branch)?;
    }
    Ok(runs)
}

fn read_runs(client: &Client, url: &str) -> Result<Vec<Run>> {
    let response = client.get(url, "workflow runs")?;
    let runs: Runs = http::json(&response)?;
    Ok(runs
        .workflow_runs
        .into_iter()
        .map(|r| Run {
            id: r.id,
            name: r.name.or(r.display_title).unwrap_or_default(),
            state: state(r.status.as_deref(), r.conclusion.as_deref()),
            url: r.html_url,
            branch: r.head_branch,
            sha: r.head_sha,
            event: r.event,
            number: r.run_number,
            created: r.created_at.as_deref().and_then(seconds),
            updated: r.updated_at.as_deref().and_then(seconds),
        })
        .collect())
}

pub(crate) fn jobs(client: &Client, run: &Run) -> Result<Vec<Job>> {
    let url = format!(
        "{}/actions/runs/{}/jobs?filter=latest&per_page=100",
        repo(client),
        run.id
    );
    client.pages(&url, "jobs", |response| {
        let jobs: Jobs = http::json(response)?;
        Ok(jobs.jobs.into_iter().map(job).collect())
    })
}

fn job(raw: RawJob) -> Job {
    Job {
        id: raw.id,
        run: raw.run_id,
        state: state(raw.status.as_deref(), raw.conclusion.as_deref()),
        name: raw.name,
        stage: None,
        url: raw.html_url.unwrap_or_default(),
        started: raw.started_at.as_deref().and_then(seconds),
        finished: raw.completed_at.as_deref().and_then(seconds),
        steps: raw
            .steps
            .into_iter()
            .map(|s| Step {
                number: s.number,
                state: state(s.status.as_deref(), s.conclusion.as_deref()),
                name: s.name,
            })
            .collect(),
    }
}

pub(crate) fn log_url(client: &Client, job: &Job) -> String {
    format!("{}/actions/jobs/{}/logs", repo(client), job.id)
}

pub(crate) fn rerun_url(client: &Client, run: &Run) -> String {
    format!("{}/actions/runs/{}/rerun-failed-jobs", repo(client), run.id)
}

pub(crate) fn cancel_url(client: &Client, run: &Run) -> String {
    format!("{}/actions/runs/{}/cancel", repo(client), run.id)
}

pub(crate) fn rerun_all_url(client: &Client, run: &Run) -> String {
    format!("{}/actions/runs/{}/rerun", repo(client), run.id)
}

pub(crate) fn retry_job_url(client: &Client, job: &Job) -> String {
    format!("{}/actions/jobs/{}/rerun", repo(client), job.id)
}

pub(crate) fn dispatch_url(client: &Client, workflow: &str) -> String {
    format!("{}/actions/workflows/{}/dispatches", repo(client), encode(workflow))
}

/// Approves the deployments of `job`'s run that wait for this account.
pub(crate) fn approve_deployments(client: &Client, job: &Job) -> Result<()> {
    let url = format!("{}/actions/runs/{}/pending_deployments", repo(client), job.run);
    let pending: Vec<RawPending> = http::json(&client.get(&url, "pending deployments")?)?;
    let ids: Vec<u64> = pending
        .iter()
        .filter(|p| p.current_user_can_approve)
        .map(|p| p.environment.id)
        .collect();
    if ids.is_empty() {
        return Err(Error::Forbidden {
            message: "no deployment of this run waits for your approval".into(),
        });
    }
    let body = serde_json::json!({ "environment_ids": ids, "state": "approved", "comment": "" });
    client.post_json(&url, "pending deployments", &body).map(drop)
}

pub(crate) fn pulls(client: &Client) -> Result<Vec<Item>> {
    let url = format!(
        "{}/pulls?state=open&sort=updated&direction=desc&per_page=50",
        repo(client)
    );
    items(client, &url, "pull requests")
}

/// The open issues; GitHub lists pull requests among them, which are left out.
pub(crate) fn issues(client: &Client) -> Result<Vec<Item>> {
    let url = format!(
        "{}/issues?state=open&sort=updated&direction=desc&per_page=50",
        repo(client)
    );
    items(client, &url, "issues")
}

fn items(client: &Client, url: &str, what: &str) -> Result<Vec<Item>> {
    let raw: Vec<RawItem> = http::json(&client.get(url, what)?)?;
    let pulls = url.contains("/pulls?");
    Ok(raw
        .into_iter()
        .filter(|i| pulls || i.pull_request.is_none())
        .map(|i| Item {
            number: i.number,
            title: i.title,
            author: i.user.map(|u| u.login),
            url: i.html_url,
            updated: i.updated_at.as_deref().and_then(seconds),
            draft: i.draft,
            branch: i.head.map(|h| h.reference),
        })
        .collect())
}

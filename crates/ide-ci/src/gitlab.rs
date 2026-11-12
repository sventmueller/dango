//! GitLab: pipelines, their jobs, job traces, retrying, playing and cancelling; open merge requests and issues.

use serde::Deserialize;

use crate::client::{Client, encode};
use crate::error::Result;
use crate::http::{self, Response};
use crate::model::{Item, Job, Run, State, seconds};

#[derive(Deserialize)]
struct RawPipeline {
    id: u64,
    sha: String,
    #[serde(rename = "ref")]
    reference: Option<String>,
    status: String,
    source: Option<String>,
    name: Option<String>,
    web_url: String,
    created_at: Option<String>,
    updated_at: Option<String>,
}

#[derive(Deserialize)]
struct RawJob {
    id: u64,
    name: String,
    stage: Option<String>,
    status: String,
    web_url: Option<String>,
    started_at: Option<String>,
    finished_at: Option<String>,
    pipeline: Option<RawPipelineRef>,
}

#[derive(Deserialize)]
struct RawItem {
    iid: u64,
    title: String,
    author: Option<RawUser>,
    web_url: String,
    updated_at: Option<String>,
    #[serde(default)]
    draft: bool,
    #[serde(default)]
    work_in_progress: bool,
    source_branch: Option<String>,
}

#[derive(Deserialize)]
struct RawUser {
    username: String,
}

#[derive(Deserialize)]
struct RawPipelineRef {
    id: u64,
}

fn state(status: &str) -> State {
    match status {
        "running" => State::Running,
        "success" => State::Success,
        "failed" => State::Failed,
        "canceled" | "canceling" => State::Cancelled,
        "skipped" => State::Skipped,
        "manual" => State::Manual,
        _ => State::Queued,
    }
}

pub(crate) fn project_url(client: &Client) -> String {
    format!("{}/projects/{}", client.forge.api, encode(&client.forge.project))
}

pub(crate) fn runs(client: &Client, branch: Option<&str>, sha: &str) -> Result<Vec<Run>> {
    let by_commit = format!("{}/pipelines?sha={}&per_page=20", project_url(client), encode(sha));
    let mut runs = read_pipelines(client, &by_commit)?;
    if runs.is_empty()
        && let Some(branch) = branch
    {
        let by_branch = format!(
            "{}/pipelines?ref={}&per_page=10&order_by=id&sort=desc",
            project_url(client),
            encode(branch)
        );
        runs = read_pipelines(client, &by_branch)?;
    }
    Ok(runs)
}

fn read_pipelines(client: &Client, url: &str) -> Result<Vec<Run>> {
    let response = client.get(url, "pipelines")?;
    let pipelines: Vec<RawPipeline> = http::json(&response)?;
    Ok(pipelines
        .into_iter()
        .map(|p| Run {
            id: p.id,
            name: p.name.unwrap_or_else(|| "Pipeline".into()),
            state: state(&p.status),
            url: p.web_url,
            branch: p.reference,
            sha: p.sha,
            event: p.source,
            number: p.id,
            created: p.created_at.as_deref().and_then(seconds),
            updated: p.updated_at.as_deref().and_then(seconds),
        })
        .collect())
}

pub(crate) fn jobs(client: &Client, run: &Run) -> Result<Vec<Job>> {
    let url = format!("{}/pipelines/{}/jobs?per_page=100", project_url(client), run.id);
    client.pages(&url, "jobs", |response| {
        let jobs: Vec<RawJob> = http::json(response)?;
        Ok(jobs
            .into_iter()
            .map(|j| Job {
                id: j.id,
                run: j.pipeline.map_or(run.id, |p| p.id),
                name: j.name,
                stage: j.stage,
                state: state(&j.status),
                url: j.web_url.unwrap_or_default(),
                started: j.started_at.as_deref().and_then(seconds),
                finished: j.finished_at.as_deref().and_then(seconds),
                steps: Vec::new(),
            })
            .collect())
    })
}

pub(crate) fn log_url(client: &Client, job: &Job) -> String {
    format!("{}/jobs/{}/trace", project_url(client), job.id)
}

pub(crate) fn retry_url(client: &Client, run: &Run) -> String {
    format!("{}/pipelines/{}/retry", project_url(client), run.id)
}

pub(crate) fn retry_job_url(client: &Client, job: &Job) -> String {
    format!("{}/jobs/{}/retry", project_url(client), job.id)
}

pub(crate) fn play_url(client: &Client, job: &Job) -> String {
    format!("{}/jobs/{}/play", project_url(client), job.id)
}

pub(crate) fn new_pipeline_url(client: &Client, branch: &str) -> String {
    format!("{}/pipeline?ref={}", project_url(client), encode(branch))
}

pub(crate) fn merge_requests(client: &Client) -> Result<Vec<Item>> {
    items(client, "merge_requests", "merge requests")
}

pub(crate) fn issues(client: &Client) -> Result<Vec<Item>> {
    items(client, "issues", "issues")
}

fn items(client: &Client, path: &str, what: &str) -> Result<Vec<Item>> {
    let url = format!(
        "{}/{path}?state=opened&order_by=updated_at&sort=desc&per_page=50",
        project_url(client)
    );
    let response = client.get(&url, what)?;
    let raw: Vec<RawItem> = http::json(&response)?;
    Ok(raw
        .into_iter()
        .map(|i| Item {
            number: i.iid,
            title: i.title,
            author: i.author.map(|a| a.username),
            url: i.web_url,
            updated: i.updated_at.as_deref().and_then(seconds),
            draft: i.draft || i.work_in_progress,
            branch: i.source_branch,
        })
        .collect())
}

pub(crate) fn cancel_url(client: &Client, run: &Run) -> String {
    format!("{}/pipelines/{}/cancel", project_url(client), run.id)
}

/// The next page by GitLab's `x-next-page` header, for a list that sends no `Link`.
pub(crate) fn next_page(url: &str, response: &Response) -> Option<String> {
    let page: u32 = response.header("x-next-page")?.trim().parse().ok()?;
    let mut parts: Vec<String> = Vec::new();
    let (base, query) = url.split_once('?').unwrap_or((url, ""));
    for part in query.split('&').filter(|p| !p.is_empty() && !p.starts_with("page=")) {
        parts.push(part.to_string());
    }
    parts.push(format!("page={page}"));
    Some(format!("{base}?{}", parts.join("&")))
}

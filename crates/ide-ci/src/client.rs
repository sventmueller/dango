//! One client for both services: runs of a commit or branch, their jobs and logs, re-running, starting and cancelling;
//! open merge requests and issues, and one issue or a label's issues with their text.

use std::sync::Arc;

use crate::error::{Error, Result};
use crate::forge::{Forge, Kind};
use crate::http::{self, MAX_PAGES, Method, Request, Response, Transport};
use crate::model::{Issue, Item, Job, Run};
use crate::{github, gitlab};

/// The API of one repository, with the token of its host if there is one.
#[derive(Clone)]
pub struct Client {
    pub(crate) forge: Forge,
    token: Option<String>,
    transport: Arc<dyn Transport>,
}

impl std::fmt::Debug for Client {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Client")
            .field("forge", &self.forge)
            .field("token", &self.token.as_ref().map(|_| "…"))
            .finish_non_exhaustive()
    }
}

impl Client {
    /// A client for `forge` that sends `token`, which must belong to the forge's host.
    pub fn new(forge: Forge, token: Option<String>, transport: Arc<dyn Transport>) -> Client {
        Client {
            forge,
            token,
            transport,
        }
    }

    /// The repository it talks to.
    pub fn forge(&self) -> &Forge {
        &self.forge
    }

    /// The runs of the commit `sha`, newest first; when it has none, the latest runs of `branch`.
    ///
    /// ```
    /// use std::sync::Arc;
    ///
    /// use ide_ci::{Client, Method, Recorded, State, candidates};
    ///
    /// let forge = candidates("git@github.com:acme/widget.git", &[]).remove(0);
    /// let recorded = Recorded::new().answer(
    ///     Method::Get,
    ///     "https://api.github.com/repos/acme/widget/actions/runs?head_sha=abc123&per_page=30",
    ///     200,
    ///     r#"{"workflow_runs": [{"id": 1, "name": "CI", "run_number": 7, "status": "completed",
    ///         "conclusion": "failure", "head_sha": "abc123", "html_url": "https://github.com/acme/widget"}]}"#,
    ///     &[],
    /// );
    /// let client = Client::new(forge, None, Arc::new(recorded));
    /// let runs = client.runs(Some("main"), "abc123").unwrap();
    /// assert_eq!((runs[0].name.as_str(), runs[0].number, runs[0].state), ("CI", 7, State::Failed));
    /// ```
    pub fn runs(&self, branch: Option<&str>, sha: &str) -> Result<Vec<Run>> {
        match self.forge.kind {
            Kind::GitHub => github::runs(self, branch, sha),
            Kind::GitLab => gitlab::runs(self, branch, sha),
        }
    }

    /// The jobs of `run`, with their steps where the service lists them.
    pub fn jobs(&self, run: &Run) -> Result<Vec<Job>> {
        match self.forge.kind {
            Kind::GitHub => github::jobs(self, run),
            Kind::GitLab => gitlab::jobs(self, run),
        }
    }

    /// The log of `job` as the service gives it, escape sequences and all; a very long one keeps its end.
    pub fn log(&self, job: &Job) -> Result<String> {
        let url = match self.forge.kind {
            Kind::GitHub => github::log_url(self, job),
            Kind::GitLab => gitlab::log_url(self, job),
        };
        let response = self.get(&url, "log")?;
        Ok(response.text())
    }

    /// Starts the failed jobs of `run` again.
    pub fn rerun_failed(&self, run: &Run) -> Result<()> {
        let url = match self.forge.kind {
            Kind::GitHub => github::rerun_url(self, run),
            Kind::GitLab => gitlab::retry_url(self, run),
        };
        self.post(&url, "run").map(drop)
    }

    /// Starts every job of `run` again; GitHub only, GitLab starts a new pipeline instead ([`Client::start_run`]).
    pub fn rerun_all(&self, run: &Run) -> Result<()> {
        match self.forge.kind {
            Kind::GitHub => self.post(&github::rerun_all_url(self, run), "run").map(drop),
            Kind::GitLab => Err(Error::NotFound {
                what: "re-running a whole pipeline".into(),
            }),
        }
    }

    /// Starts the finished `job` again; on GitHub the jobs that depend on it run again too.
    pub fn retry_job(&self, job: &Job) -> Result<()> {
        let url = match self.forge.kind {
            Kind::GitHub => github::retry_job_url(self, job),
            Kind::GitLab => gitlab::retry_job_url(self, job),
        };
        self.post(&url, "job").map(drop)
    }

    /// Starts `job`, which waits for someone: plays a manual job on GitLab; on GitHub approves the deployments of its
    /// run that wait for this account.
    pub fn start_job(&self, job: &Job) -> Result<()> {
        match self.forge.kind {
            Kind::GitHub => github::approve_deployments(self, job),
            Kind::GitLab => self.post(&gitlab::play_url(self, job), "job").map(drop),
        }
    }

    /// Starts a new run for the remote's `branch`: a pipeline on GitLab, the workflow file `workflow` on GitHub.
    pub fn start_run(&self, branch: &str, workflow: Option<&str>) -> Result<()> {
        match (self.forge.kind, workflow) {
            (Kind::GitLab, _) => self.post(&gitlab::new_pipeline_url(self, branch), "pipeline").map(drop),
            (Kind::GitHub, Some(workflow)) => {
                let body = serde_json::json!({ "ref": branch });
                self.post_json(&github::dispatch_url(self, workflow), "workflow", &body)
                    .map(drop)
            }
            (Kind::GitHub, None) => Err(Error::NotFound {
                what: "workflow".into(),
            }),
        }
    }

    /// The open merge requests, pull requests on GitHub, last changed first.
    pub fn changes(&self) -> Result<Vec<Item>> {
        match self.forge.kind {
            Kind::GitHub => github::pulls(self),
            Kind::GitLab => gitlab::merge_requests(self),
        }
    }

    /// The open issues, last changed first.
    pub fn issues(&self) -> Result<Vec<Item>> {
        match self.forge.kind {
            Kind::GitHub => github::issues(self),
            Kind::GitLab => gitlab::issues(self),
        }
    }

    /// The issue `number` with its text and labels, open or not; never its comments.
    pub fn issue(&self, number: u64) -> Result<Issue> {
        match self.forge.kind {
            Kind::GitHub => github::issue(self, number),
            Kind::GitLab => gitlab::issue(self, number),
        }
    }

    /// The open issues that carry `label`, oldest first, with their text and labels.
    pub fn labelled_issues(&self, label: &str) -> Result<Vec<Issue>> {
        match self.forge.kind {
            Kind::GitHub => github::labelled_issues(self, label),
            Kind::GitLab => gitlab::labelled_issues(self, label),
        }
    }

    /// The page where a merge request or pull request of `branch` starts.
    pub fn new_change_page(&self, branch: &str) -> String {
        match self.forge.kind {
            Kind::GitHub => format!("{}/compare/{}?expand=1", self.forge.page(), encode(branch)),
            Kind::GitLab => format!(
                "{}/-/merge_requests/new?merge_request%5Bsource_branch%5D={}",
                self.forge.page(),
                encode(branch)
            ),
        }
    }

    /// The page where a new issue starts.
    pub fn new_issue_page(&self) -> String {
        match self.forge.kind {
            Kind::GitHub => format!("{}/issues/new", self.forge.page()),
            Kind::GitLab => format!("{}/-/issues/new", self.forge.page()),
        }
    }

    /// Cancels `run`.
    pub fn cancel(&self, run: &Run) -> Result<()> {
        let url = match self.forge.kind {
            Kind::GitHub => github::cancel_url(self, run),
            Kind::GitLab => gitlab::cancel_url(self, run),
        };
        self.post(&url, "run").map(drop)
    }

    /// The name of the account the token belongs to.
    pub fn account(&self) -> Result<String> {
        let response = self.get(&format!("{}/user", self.forge.api), "account")?;
        let user: serde_json::Value = http::json(&response)?;
        user.get("login")
            .or(user.get("username"))
            .and_then(serde_json::Value::as_str)
            .map(str::to_string)
            .ok_or_else(|| Error::Decode("no account name".into()))
    }

    pub(crate) fn get(&self, url: &str, what: &str) -> Result<Response> {
        self.call(Method::Get, url, what, None)
    }

    pub(crate) fn post(&self, url: &str, what: &str) -> Result<Response> {
        self.call(Method::Post, url, what, None)
    }

    pub(crate) fn post_json(&self, url: &str, what: &str, body: &serde_json::Value) -> Result<Response> {
        self.call(Method::Post, url, what, Some(body))
    }

    fn call(&self, method: Method, url: &str, what: &str, body: Option<&serde_json::Value>) -> Result<Response> {
        let mut headers = match self.forge.kind {
            Kind::GitHub => vec![
                ("Accept".to_string(), "application/vnd.github+json".to_string()),
                ("X-GitHub-Api-Version".to_string(), "2022-11-28".to_string()),
            ],
            Kind::GitLab => vec![("Accept".to_string(), "application/json".to_string())],
        };
        if let Some(token) = &self.token {
            headers.push(("Authorization".to_string(), format!("Bearer {token}")));
        }
        if body.is_some() {
            headers.push(("Content-Type".to_string(), "application/json".to_string()));
        }
        let request = Request {
            method,
            url: url.to_string(),
            headers,
            body: body.map(|b| b.to_string().into_bytes()).unwrap_or_default(),
        };
        http::checked(self.transport.send(&request)?, &self.forge.host, what)
    }

    /// Every page of a list from `url` on, up to `MAX_PAGES`, each read by `read`.
    pub(crate) fn pages<T>(&self, url: &str, what: &str, read: impl Fn(&Response) -> Result<Vec<T>>) -> Result<Vec<T>> {
        let mut items = Vec::new();
        let mut next = Some(url.to_string());
        for _ in 0..MAX_PAGES {
            let Some(url) = next.take() else { break };
            let response = self.get(&url, what)?;
            items.extend(read(&response)?);
            next = http::next_page(&response).or_else(|| gitlab::next_page(&url, &response));
        }
        Ok(items)
    }
}

/// The first of `candidates` whose API knows the project, for a GitLab instance whose prefix the remote does not tell;
/// a single candidate is taken as it is.
pub fn locate(candidates: &[Forge], token: Option<&str>, transport: Arc<dyn Transport>) -> Result<Forge> {
    let [first, ..] = candidates else {
        return Err(Error::NotFound {
            what: "CI service".into(),
        });
    };
    if candidates.len() == 1 || first.kind != Kind::GitLab {
        return Ok(first.clone());
    }
    for forge in candidates {
        let client = Client::new(forge.clone(), token.map(str::to_string), transport.clone());
        match client.get(&gitlab::project_url(&client), "project") {
            Ok(_) => return Ok(forge.clone()),
            Err(Error::NotFound { .. } | Error::Network(_) | Error::Decode(_)) => {}
            Err(Error::Http { status, .. }) if status < 500 => {}
            Err(other) => return Err(other),
        }
    }
    Err(Error::NotFound { what: "project".into() })
}

/// `text` with every character but the unreserved ones percent-encoded, for a path segment or query value.
pub(crate) fn encode(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for byte in text.bytes() {
        if byte.is_ascii_alphanumeric() || b"-._~".contains(&byte) {
            out.push(char::from(byte));
        } else {
            out.push_str(&format!("%{byte:02X}"));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn paths_and_branch_names_are_encoded_whole() {
        assert_eq!(encode("group/sub/app"), "group%2Fsub%2Fapp");
        assert_eq!(encode("feature/ä b"), "feature%2F%C3%A4%20b");
        assert_eq!(encode("main"), "main");
    }
}

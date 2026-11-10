//! One client for both services: runs of a commit or branch, their jobs and logs, re-running and cancelling.

use std::sync::Arc;

use crate::error::{Error, Result};
use crate::forge::{Forge, Kind};
use crate::http::{self, MAX_PAGES, Method, Request, Response, Transport};
use crate::model::{Job, Run};
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

    /// Whether it sends a token.
    pub fn has_token(&self) -> bool {
        self.token.is_some()
    }

    /// The runs of the commit `sha`, newest first; when it has none, the latest runs of `branch`.
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
        self.call(Method::Get, url, what)
    }

    fn post(&self, url: &str, what: &str) -> Result<Response> {
        self.call(Method::Post, url, what)
    }

    fn call(&self, method: Method, url: &str, what: &str) -> Result<Response> {
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
        let request = Request {
            method,
            url: url.to_string(),
            headers,
            body: Vec::new(),
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

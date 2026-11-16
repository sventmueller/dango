//! What the app writes to a service on the user's click (E177): a merge request (a pull request on GitHub) from a
//! branch and a comment on an issue, each found again by a marker in its text, and the head a branch has there.
//! Comments are read only to find the marker; their text never leaves this module.

use serde::Deserialize;

use crate::client::{Client, encode};
use crate::error::{Error, Result};
use crate::forge::Kind;
use crate::http;
use crate::model::{Item, seconds};

#[derive(Deserialize)]
struct RawUser {
    #[serde(alias = "username")]
    login: String,
}

/// A pull request or merge request as both services send it.
#[derive(Deserialize)]
struct RawChange {
    #[serde(alias = "iid")]
    number: u64,
    title: String,
    #[serde(alias = "description")]
    body: Option<String>,
    #[serde(alias = "author")]
    user: Option<RawUser>,
    #[serde(alias = "web_url")]
    html_url: String,
    updated_at: Option<String>,
    #[serde(default)]
    draft: bool,
    head: Option<RawHead>,
    source_branch: Option<String>,
}

#[derive(Deserialize)]
struct RawHead {
    #[serde(rename = "ref")]
    reference: String,
}

impl RawChange {
    fn item(self) -> Item {
        Item {
            number: self.number,
            title: self.title,
            author: self.user.map(|u| u.login),
            url: self.html_url,
            updated: self.updated_at.as_deref().and_then(seconds),
            draft: self.draft,
            branch: self.head.map(|h| h.reference).or(self.source_branch),
        }
    }
}

/// A comment on GitHub, a note on GitLab.
#[derive(Deserialize)]
struct RawComment {
    id: u64,
    #[serde(default)]
    body: String,
    html_url: Option<String>,
}

#[derive(Deserialize)]
struct RawRef {
    object: RawObject,
}

#[derive(Deserialize)]
struct RawObject {
    sha: String,
}

#[derive(Deserialize)]
struct RawBranch {
    commit: RawCommit,
}

#[derive(Deserialize)]
struct RawCommit {
    id: String,
}

impl Client {
    /// Opens a merge request (pull request) of `branch` into `base` with `title` and the Markdown `body`.
    pub fn open_change(&self, branch: &str, base: &str, title: &str, body: &str) -> Result<Item> {
        let (url, payload) = match self.forge.kind {
            Kind::GitHub => (
                format!("{}/pulls", self.repo()),
                serde_json::json!({ "title": title, "head": branch, "base": base, "body": body }),
            ),
            Kind::GitLab => (
                format!("{}/merge_requests", self.project()),
                serde_json::json!({
                    "source_branch": branch,
                    "target_branch": base,
                    "title": title,
                    "description": body,
                }),
            ),
        };
        let response = self.post_json(&url, "merge request", &payload)?;
        Ok(http::json::<RawChange>(&response)?.item())
    }

    /// The merge request (pull request) of `branch` whose text holds `marker`, in any state.
    pub fn find_change(&self, branch: &str, marker: &str) -> Result<Option<Item>> {
        let url = match self.forge.kind {
            Kind::GitHub => {
                let owner = self.forge.project.split('/').next().unwrap_or_default();
                format!(
                    "{}/pulls?state=all&head={}&per_page=100",
                    self.repo(),
                    encode(&format!("{owner}:{branch}"))
                )
            }
            Kind::GitLab => format!(
                "{}/merge_requests?state=all&source_branch={}&per_page=100",
                self.project(),
                encode(branch)
            ),
        };
        let changes = self.pages(&url, "merge requests", http::json::<Vec<RawChange>>)?;
        Ok(changes
            .into_iter()
            .find(|c| c.body.as_deref().is_some_and(|b| b.contains(marker)))
            .map(RawChange::item))
    }

    /// Comments `body` on the issue `number`; the comment's page.
    pub fn comment_issue(&self, number: u64, body: &str) -> Result<String> {
        let url = self.comments_url(number);
        let response = self.post_json(&url, "comment", &serde_json::json!({ "body": body }))?;
        let comment: RawComment = http::json(&response)?;
        Ok(self.comment_page(number, &comment))
    }

    /// The page of the comment on the issue `number` that holds `marker`, if there is one.
    pub fn find_comment(&self, number: u64, marker: &str) -> Result<Option<String>> {
        let url = match self.forge.kind {
            Kind::GitHub => format!("{}?per_page=100", self.comments_url(number)),
            Kind::GitLab => format!("{}?sort=desc&per_page=100", self.comments_url(number)),
        };
        let comments = self.pages(&url, "comments", http::json::<Vec<RawComment>>)?;
        Ok(comments
            .iter()
            .find(|c| c.body.contains(marker))
            .map(|c| self.comment_page(number, c)))
    }

    /// The commit `branch` points at on the service; `None` when it has no such branch.
    pub fn branch_head(&self, branch: &str) -> Result<Option<String>> {
        let read = match self.forge.kind {
            Kind::GitHub => {
                let path: Vec<String> = branch.split('/').map(encode).collect();
                let url = format!("{}/git/ref/heads/{}", self.repo(), path.join("/"));
                self.get(&url, "branch")
                    .and_then(|r| http::json::<RawRef>(&r))
                    .map(|r| r.object.sha)
            }
            Kind::GitLab => {
                let url = format!("{}/repository/branches/{}", self.project(), encode(branch));
                self.get(&url, "branch")
                    .and_then(|r| http::json::<RawBranch>(&r))
                    .map(|b| b.commit.id)
            }
        };
        match read {
            Ok(commit) => Ok(Some(commit)),
            Err(Error::NotFound { .. }) => Ok(None),
            Err(error) => Err(error),
        }
    }

    fn repo(&self) -> String {
        format!("{}/repos/{}", self.forge.api, self.forge.project)
    }

    fn project(&self) -> String {
        format!("{}/projects/{}", self.forge.api, encode(&self.forge.project))
    }

    fn comments_url(&self, number: u64) -> String {
        match self.forge.kind {
            Kind::GitHub => format!("{}/issues/{number}/comments", self.repo()),
            Kind::GitLab => format!("{}/issues/{number}/notes", self.project()),
        }
    }

    /// GitHub names a comment's page; GitLab's note is an anchor on its issue's page.
    fn comment_page(&self, number: u64, comment: &RawComment) -> String {
        comment
            .html_url
            .clone()
            .unwrap_or_else(|| format!("{}/-/issues/{number}#note_{}", self.forge.page(), comment.id))
    }
}

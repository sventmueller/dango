//! Bringing an accepted order to GitHub or GitLab, on the user's click only (E177, O2, E164): its branch pushed, a
//! merge request (a pull request on GitHub) with a summary of the evidence, and a comment on the issue it came from.
//! The app words the texts in the user's language ([`Outgoing`]); the factory appends the order's [`marker`], has the
//! whole outgoing text scanned for secrets with the frozen rules first (I4, F75), and runs each step as an operation
//! with a target state a restart can check (C66): the branch at the accepted commit on the service, a merge request of
//! the branch with the marker in its text, a comment with the marker on the issue.

use serde::{Deserialize, Serialize};

use crate::ids::OrderId;

/// Where the comment's text names the merge request; the factory puts the merge request's page there.
pub const CHANGE_PLACEHOLDER: &str = "{change}";

/// What goes out, in the user's words: the merge request's title and Markdown body, and the comment on the issue,
/// which names the merge request as [`CHANGE_PLACEHOLDER`]. No session transcripts, no file contents.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Outgoing {
    title: String,
    body: String,
    comment: String,
}

impl Outgoing {
    /// The texts for a merge request titled `title` with `body`, and `comment` for the order's issue.
    pub fn new(title: &str, body: &str, comment: &str) -> Outgoing {
        Outgoing {
            title: title.trim().to_string(),
            body: body.trim().to_string(),
            comment: comment.trim().to_string(),
        }
    }

    /// The merge request's title.
    pub fn title(&self) -> &str {
        &self.title
    }

    /// The merge request's body as the user's words have it, without the marker.
    pub fn body(&self) -> &str {
        &self.body
    }

    /// The comment's text as the user's words have it, without the marker.
    pub fn comment(&self) -> &str {
        &self.comment
    }
}

/// The line that marks a merge request or comment as the order's, so a restart finds what it made (C66); an HTML
/// comment, which neither service shows.
pub fn marker(order: &OrderId) -> String {
    format!("<!-- {}-order: {order} -->", ide_product::COMMAND)
}

/// `text` with the order's marker on a line of its own at its end.
pub(crate) fn marked(text: &str, order: &OrderId) -> String {
    format!("{}\n\n{}", text.trim_end(), marker(order))
}

/// A merge request or pull request the order opened or found.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChangeRef {
    number: u64,
    url: String,
    reused: bool,
}

impl ChangeRef {
    /// The merge request `number` at `url`; `reused` when it was there already.
    pub fn new(number: u64, url: &str, reused: bool) -> ChangeRef {
        ChangeRef {
            number,
            url: url.to_string(),
            reused,
        }
    }

    /// The number the project shows, such as 12 in `!12` or `#12`.
    pub fn number(&self) -> u64 {
        self.number
    }

    /// Its page.
    pub fn url(&self) -> &str {
        &self.url
    }

    /// Whether it was found rather than opened.
    pub fn reused(&self) -> bool {
        self.reused
    }
}

/// The step of the publication that failed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PublishStep {
    /// Pushing the branch.
    Push,
    /// Opening the merge request.
    Change,
    /// Commenting the issue.
    Comment,
}

/// Why nothing more went out.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "failure")]
pub enum PublishFailure {
    /// gitleaks is missing, so the outgoing text could not be checked; here that is a failure (F75).
    ScanUnavailable,
    /// The outgoing text holds secrets.
    SecretsFound {
        /// How many.
        count: u32,
    },
    /// The log names no commit that G2 accepted, so there is nothing to push.
    NoAcceptedCommit,
    /// A step failed, with the app's words for it.
    Step {
        /// Which step.
        step: PublishStep,
        /// What went wrong.
        detail: String,
    },
}

/// Where the order's way to GitHub or GitLab stands, once the user asked for it.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct Publication {
    pub(crate) texts: Outgoing,
    pub(crate) pushed: bool,
    pub(crate) change: Option<ChangeRef>,
    pub(crate) comment: Option<String>,
    pub(crate) failure: Option<PublishFailure>,
}

impl Publication {
    /// The texts the user's last click sent.
    pub fn texts(&self) -> &Outgoing {
        &self.texts
    }

    /// Whether the branch is on the service at the accepted commit.
    pub fn pushed(&self) -> bool {
        self.pushed
    }

    /// The merge request, once opened or found.
    pub fn change(&self) -> Option<&ChangeRef> {
        self.change.as_ref()
    }

    /// The page of the comment on the issue, once made or found.
    pub fn comment(&self) -> Option<&str> {
        self.comment.as_deref()
    }

    /// Why it stopped, until the user tries again.
    pub fn failure(&self) -> Option<&PublishFailure> {
        self.failure.as_ref()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_marker_names_the_order_on_a_line_of_its_own() {
        let order = OrderId("o-7".into());
        assert_eq!(marker(&order), format!("<!-- {}-order: o-7 -->", ide_product::COMMAND));
        assert_eq!(marked("Body\n\n", &order), format!("Body\n\n{}", marker(&order)));
        assert!(!marker(&OrderId("o-70".into())).contains(&marker(&order)));
    }
}

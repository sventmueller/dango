//! Branches and the history of the current one.

use crate::error::{Error, Result};
use crate::remote::{overwritten_paths, word};
use crate::repo::Repository;

/// A local or remote-tracking branch.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Branch {
    /// The short name, such as `main` or `origin/main`.
    pub name: String,
    /// The full ref, such as `refs/heads/main`.
    pub full_name: String,
    /// Whether it is a remote-tracking branch.
    pub remote: bool,
    /// Whether HEAD is on it.
    pub current: bool,
    /// The commit it points at.
    pub commit: String,
    /// The upstream's short name.
    pub upstream: Option<String>,
    /// Commits ahead of the upstream.
    pub ahead: u32,
    /// Commits behind the upstream.
    pub behind: u32,
    /// Whether the upstream is set but gone from the remote.
    pub upstream_gone: bool,
    /// The subject of its commit.
    pub subject: String,
    /// When its commit was made, in seconds since 1970.
    pub time: i64,
}

/// A commit in the history.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Commit {
    /// The full hash.
    pub hash: String,
    /// The parents' hashes.
    pub parents: Vec<String>,
    /// The first line of the message.
    pub subject: String,
    /// The author's name.
    pub author_name: String,
    /// The author's email address.
    pub author_email: String,
    /// When it was authored, in seconds since 1970.
    pub author_time: i64,
    /// The refs that point at it, such as `HEAD -> main`, `origin/main` or `tag: v1`.
    pub refs: Vec<String>,
}

impl Repository {
    /// The local branches, then the remote-tracking ones.
    pub fn branches(&self) -> Result<Vec<Branch>> {
        let format = "--format=%(refname)%00%(refname:short)%00%(objectname)%00%(HEAD)%00%(upstream:short)%00\
                      %(upstream:track,nobracket)%00%(symref)%00%(committerdate:unix)%00%(contents:subject)";
        let output = self
            .git()
            .ok(self.call(["for-each-ref", format, "refs/heads", "refs/remotes"]))?;
        Ok(String::from_utf8_lossy(&output.stdout)
            .lines()
            .filter_map(parse_branch)
            .collect())
    }

    /// The current branch's short name; `None` when HEAD is detached.
    pub fn current_branch(&self) -> Result<Option<String>> {
        let output = self
            .git()
            .run(self.call(["symbolic-ref", "--quiet", "--short", "HEAD"]))?;
        Ok((output.code == 0).then(|| output.text()))
    }

    /// Creates the branch `name` at `start`, or at HEAD; with `switch`, also switches to it.
    pub fn create_branch(&self, name: &str, start: Option<&str>, switch: bool) -> Result<()> {
        self.check_branch_name(name)?;
        let mut args = if switch {
            vec!["switch", "--create", name]
        } else {
            vec!["branch", name]
        };
        if let Some(start) = start {
            args.push(word(start)?);
        }
        self.switching(self.call(args))
    }

    /// Switches to the branch `name`. A remote-tracking branch such as `origin/x` gets a local branch that tracks it,
    /// as does a name only one remote has.
    pub fn switch_branch(&self, name: &str) -> Result<()> {
        word(name)?;
        let local = self.resolve(&format!("refs/heads/{name}"))?.is_some();
        let remote = !local && self.resolve(&format!("refs/remotes/{name}"))?.is_some();
        let args = if remote {
            vec!["switch", "--track", name]
        } else {
            vec!["switch", name]
        };
        self.switching(self.call(args))
    }

    /// Deletes the branch `name` when its commits are merged; otherwise `Error::NotMerged`. Forcing it is
    /// [`Destructive::delete_branch`](crate::Destructive::delete_branch).
    pub fn delete_branch(&self, name: &str) -> Result<()> {
        word(name)?;
        let output = self.git().run(self.call(["branch", "--delete", name]))?;
        if output.code == 0 {
            return Ok(());
        }
        let said = output.said();
        if said.contains("not fully merged") {
            return Err(Error::NotMerged {
                branch: name.to_string(),
                output: said,
            });
        }
        Err(crate::run::failure(&output))
    }

    /// The last `limit` commits of HEAD, newest first; empty before the first commit.
    pub fn log(&self, limit: usize) -> Result<Vec<Commit>> {
        if self.head()?.is_none() {
            return Ok(Vec::new());
        }
        let count = format!("--max-count={limit}");
        let output = self.git().ok(self.call([
            "-c",
            "log.showSignature=false",
            "log",
            "-z",
            "--no-color",
            "--decorate=short",
            &count,
            "--format=%H%x1f%P%x1f%an%x1f%ae%x1f%at%x1f%D%x1f%s",
            "HEAD",
            "--",
        ]))?;
        Ok(crate::path::nul_fields(&output.stdout)
            .filter_map(parse_commit)
            .collect())
    }

    pub(crate) fn check_branch_name(&self, name: &str) -> Result<()> {
        word(name)?;
        let output = self.git().run(self.call(["check-ref-format", "--branch", name]))?;
        if output.code == 0 {
            Ok(())
        } else {
            Err(Error::InvalidName { name: name.to_string() })
        }
    }

    /// Runs a command that changes the checked-out branch, sorting out why it failed.
    fn switching(&self, call: crate::run::Call<'_>) -> Result<()> {
        let (output, failed_hook) = crate::hooks::run(self.git(), call.unlimited())?;
        if output.code == 0 {
            return Ok(());
        }
        if let Some(hook) = failed_hook {
            return Err(Error::HookFailed {
                hook,
                output: output.said(),
            });
        }
        let said = output.said();
        if said.contains("would be overwritten") {
            return Err(Error::LocalChanges {
                paths: overwritten_paths(&said),
                output: said,
            });
        }
        Err(crate::run::failure(&output))
    }
}

fn parse_branch(line: &str) -> Option<Branch> {
    let fields: Vec<&str> = line.splitn(9, '\0').collect();
    let [full_name, name, commit, head, upstream, track, symref, time, subject] = fields[..] else {
        return None;
    };
    if !symref.is_empty() {
        return None;
    }
    let count = |word: &str| -> u32 {
        track
            .split(", ")
            .find_map(|part| part.strip_prefix(word))
            .and_then(|n| n.trim().parse().ok())
            .unwrap_or(0)
    };
    Some(Branch {
        name: name.to_string(),
        full_name: full_name.to_string(),
        remote: full_name.starts_with("refs/remotes/"),
        current: head == "*",
        commit: commit.to_string(),
        upstream: (!upstream.is_empty()).then(|| upstream.to_string()),
        ahead: count("ahead "),
        behind: count("behind "),
        upstream_gone: track == "gone",
        subject: subject.to_string(),
        time: time.parse().unwrap_or(0),
    })
}

fn parse_commit(record: &[u8]) -> Option<Commit> {
    let text = String::from_utf8_lossy(record);
    let fields: Vec<&str> = text.trim_start_matches('\n').splitn(7, '\u{1f}').collect();
    let [hash, parents, author_name, author_email, time, refs, subject] = fields[..] else {
        return None;
    };
    Some(Commit {
        hash: hash.to_string(),
        parents: parents.split_whitespace().map(String::from).collect(),
        subject: subject.to_string(),
        author_name: author_name.to_string(),
        author_email: author_email.to_string(),
        author_time: time.parse().unwrap_or(0),
        refs: refs.split(", ").filter(|r| !r.is_empty()).map(String::from).collect(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn branches_carry_their_upstream_and_distance() {
        let line = [
            "refs/heads/main",
            "main",
            "abc",
            "*",
            "origin/main",
            "ahead 2, behind 1",
            "",
            "1794247537",
            "Fix it",
        ];
        let branch = parse_branch(&line.join("\0")).unwrap();
        assert!(branch.current && !branch.remote);
        assert_eq!((branch.ahead, branch.behind), (2, 1));
        assert_eq!(branch.upstream.as_deref(), Some("origin/main"));
        assert_eq!(branch.subject, "Fix it");
        let gone = ["refs/heads/x", "x", "abc", " ", "origin/x", "gone", "", "0", "s"];
        let gone = parse_branch(&gone.join("\0")).unwrap();
        assert!(gone.upstream_gone);
        let symbolic = [
            "refs/remotes/origin/HEAD",
            "origin",
            "abc",
            " ",
            "",
            "",
            "refs/remotes/origin/main",
            "0",
            "s",
        ];
        assert!(parse_branch(&symbolic.join("\0")).is_none());
    }

    #[test]
    fn commits_are_read_with_their_refs() {
        let record =
            "abc\u{1f}p1 p2\u{1f}Ann\u{1f}ann@example.org\u{1f}1794247537\u{1f}HEAD -> main, tag: v1\u{1f}Merge";
        let commit = parse_commit(record.as_bytes()).unwrap();
        assert_eq!(commit.parents, ["p1", "p2"]);
        assert_eq!(commit.refs, ["HEAD -> main", "tag: v1"]);
        assert_eq!(commit.author_time, 1_794_247_537);
        assert!(parse_commit(b"short").is_none());
    }
}

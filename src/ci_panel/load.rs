//! Reading what the panel shows, on the CI lane: the repository's remote, the service and token for it, and the runs
//! of HEAD with their jobs.

use std::path::Path;

use ide_ci::{Client, Error, Forge, Item, Job, Kind, Run, Source, Workflow};
use ide_git::{StatusOptions, Untracked};

use super::model::Tab;
use super::work;

/// How many runs get their jobs read.
const RUNS_WITH_JOBS: usize = 10;

/// A run with its jobs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunRow {
    pub run: Run,
    pub jobs: Vec<Job>,
    /// Why its jobs could not be read.
    pub problem: Option<String>,
}

/// The commit the runs belong to.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Head {
    pub sha: String,
    pub branch: Option<String>,
}

/// What a read found.
pub enum Loaded {
    NoGit,
    NoRepository,
    NoCommit,
    NoRemote,
    /// The remote's host does not say which service it is.
    Unknown(String),
    /// The service wants a token, or refused the one it got.
    SignIn {
        forge: Forge,
        refused: bool,
    },
    Failed(Error, Option<Forge>),
    Ready(Box<Snapshot>),
}

/// The runs of HEAD on its service.
pub struct Snapshot {
    pub head: Head,
    pub client: Client,
    pub source: Option<Source>,
    pub account: Option<String>,
    pub runs: Vec<RunRow>,
    /// The tab the open merge requests or issues were read for.
    pub tab: Tab,
    pub items: Vec<Item>,
    pub items_problem: Option<String>,
    pub workflows: Vec<Workflow>,
}

/// HEAD's commit and branch, or why there is none.
pub fn head(root: &Path) -> Result<(ide_git::Repository, Head), Box<Loaded>> {
    let Some(git) = crate::git_panel::git() else {
        return Err(Box::new(Loaded::NoGit));
    };
    let repo = match git.discover(root) {
        Ok(repo) => repo,
        Err(ide_git::Error::NotARepository { .. }) => return Err(Box::new(Loaded::NoRepository)),
        Err(ide_git::Error::GitMissing) => return Err(Box::new(Loaded::NoGit)),
        Err(error) => return Err(Box::new(Loaded::Failed(Error::Decode(error.to_string()), None))),
    };
    let Ok(Some(sha)) = repo.head() else {
        return Err(Box::new(Loaded::NoCommit));
    };
    let branch = repo.current_branch().ok().flatten();
    Ok((repo, Head { sha, branch }))
}

/// Everything the panel shows for `root` with `tab` open; `known` is the account name known for the token found last
/// time.
pub fn load(root: &Path, known: Option<(String, Source, String)>, tab: Tab) -> Loaded {
    let (repo, head) = match head(root) {
        Ok(found) => found,
        Err(loaded) => return *loaded,
    };
    let Some(url) = remote_url(&repo) else {
        return Loaded::NoRemote;
    };
    let candidates = ide_ci::candidates(&url, &work::host_rules());
    let Some(first) = candidates.first() else {
        let host = ide_ci::RemoteUrl::parse(&url).map_or(url, |r| r.host);
        return Loaded::Unknown(host);
    };
    let Some(auth) = work::auth() else { return Loaded::NoGit };
    let mut credential = auth.fresh(first, &*work::network());
    let mut renewed = false;
    let RunsRead { forge, client, runs } = loop {
        let token = credential.as_ref().map(|c| c.token.clone());
        let (error, forge) = match read_runs(&candidates, token.clone(), &head) {
            Ok(read) => break read,
            Err(failed) => *failed,
        };
        let own = credential.as_ref().is_some_and(|c| c.source == Source::Keychain);
        if !(matches!(error, Error::Unauthorized { .. }) && own && !renewed) {
            return refused(error, forge.unwrap_or_else(|| first.clone()), token.is_some());
        }
        renewed = true;
        match auth.renew(first, &*work::network(), token.as_deref().unwrap_or_default()) {
            Some(fresh) => credential = Some(fresh),
            None => return refused(error, forge.unwrap_or_else(|| first.clone()), true),
        }
    };
    let kept = credential.as_ref().and_then(|c| c.account.clone());
    let source = credential.map(|c| c.source);
    let account = match (&known, &source) {
        _ if kept.is_some() => kept,
        (Some((host, known_source, name)), Some(found)) if *host == forge.host && known_source == found => {
            Some(name.clone())
        }
        (_, Some(_)) => client.account().ok(),
        _ => None,
    };
    let runs = runs
        .into_iter()
        .enumerate()
        .map(|(i, run)| with_jobs(&client, run, i < RUNS_WITH_JOBS))
        .collect();
    let listed = match tab {
        Tab::Runs => Ok(Vec::new()),
        Tab::Changes => client.changes(),
        Tab::Issues => client.issues(),
    };
    let (items, items_problem) = match listed {
        Ok(items) => (items, None),
        Err(error) => (Vec::new(), Some(super::text::problem(&error))),
    };
    let workflows = match forge.kind {
        Kind::GitHub => ide_ci::manual_workflows(repo.root()),
        Kind::GitLab => Vec::new(),
    };
    Loaded::Ready(Box::new(Snapshot {
        head,
        client,
        source,
        account,
        runs,
        tab,
        items,
        items_problem,
        workflows,
    }))
}

/// The runs of a commit on the forge that answered.
struct RunsRead {
    forge: Forge,
    client: Client,
    runs: Vec<Run>,
}

/// Why reading the runs failed, with the forge once it was found.
type ReadFailed = Box<(Error, Option<Forge>)>;

/// The runs of `head` on whichever of `candidates` answers.
fn read_runs(candidates: &[Forge], token: Option<String>, head: &Head) -> Result<RunsRead, ReadFailed> {
    let forge = ide_ci::locate(candidates, token.as_deref(), work::network()).map_err(|e| Box::new((e, None)))?;
    let client = Client::new(forge.clone(), token, work::network());
    match client.runs(head.branch.as_deref(), &head.sha) {
        Ok(runs) => Ok(RunsRead { forge, client, runs }),
        Err(error) => Err(Box::new((error, Some(forge)))),
    }
}

fn with_jobs(client: &Client, run: Run, read: bool) -> RunRow {
    let (jobs, problem) = if read {
        match client.jobs(&run) {
            Ok(jobs) => (jobs, None),
            Err(error) => (Vec::new(), Some(super::text::problem(&error))),
        }
    } else {
        (Vec::new(), None)
    };
    RunRow { run, jobs, problem }
}

/// A missing or refused token asks to sign in; a repository that is not found without a token may be private.
fn refused(error: Error, forge: Forge, had_token: bool) -> Loaded {
    match error {
        Error::Unauthorized { .. } => Loaded::SignIn {
            forge,
            refused: had_token,
        },
        Error::NotFound { .. } if !had_token => Loaded::SignIn { forge, refused: false },
        other => Loaded::Failed(other, Some(forge)),
    }
}

/// The URL of the remote the current branch tracks, else `origin`'s, else the first one's.
fn remote_url(repo: &ide_git::Repository) -> Option<String> {
    let remotes = repo.remotes().ok()?;
    let tracked = repo
        .status(StatusOptions {
            untracked: Untracked::No,
            ignored: false,
        })
        .ok()
        .and_then(|s| s.branch.upstream)
        .and_then(|u| u.split_once('/').map(|(remote, _)| remote.to_string()));
    let remote = tracked
        .and_then(|name| remotes.iter().find(|r| r.name == name))
        .or_else(|| remotes.iter().find(|r| r.name == "origin"))
        .or_else(|| remotes.first())?;
    remote.fetch_url.clone()
}

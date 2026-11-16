//! Reading what the panel shows, on the CI lane: the repository's remote, the service and token for it, and the runs
//! of HEAD with their jobs.

use std::path::Path;

use ide_ci::{Client, Error, Forge, Kind, Run, Source};
use ide_git::{StatusOptions, Untracked};

use super::work;
use ide_ci::panel::{Head, Loaded, RunRow, Snapshot, Tab};

/// How many runs get their jobs read.
const RUNS_WITH_JOBS: usize = 10;

/// HEAD's commit and branch, or why there is none.
pub(crate) fn head(root: &Path) -> Result<(ide_git::Repository, Head), Box<Loaded>> {
    let Some(git) = ide_ui_git::git() else {
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
    Ok((
        repo,
        Head {
            sha: sha.to_string(),
            branch,
        },
    ))
}

/// Everything the panel shows for `root` with `tab` open; `known` is the account name known for the token found last
/// time.
pub(crate) fn load(root: &Path, known: Option<(String, Source, String)>, tab: Tab) -> Loaded {
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
            return Loaded::refused(error, forge.unwrap_or_else(|| first.clone()), token.is_some());
        }
        renewed = true;
        match auth.renew(first, &*work::network(), token.as_deref().unwrap_or_default()) {
            Some(fresh) => credential = Some(fresh),
            None => return Loaded::refused(error, forge.unwrap_or_else(|| first.clone()), true),
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

/// The client of `root`'s remote with the token of its host, as the panel finds them, for work beside the panel such
/// as the Leitstand's intake from issues (E176); blocks on the network.
pub(crate) fn client(root: &Path) -> Result<Client, Error> {
    remote_client(root).map(|(_, client)| client)
}

/// The name of `root`'s remote the panel reads, with the client of its service and the token of its host, for
/// publishing an order there (E177); blocks on the network.
pub(crate) fn remote_client(root: &Path) -> Result<(String, Client), Error> {
    let (repo, _) = head(root).map_err(|_| Error::NotFound {
        what: "a git repository with a commit".into(),
    })?;
    let (name, url) = remote(&repo).ok_or_else(|| Error::NotFound { what: "remote".into() })?;
    let candidates = ide_ci::candidates(&url, &work::host_rules());
    let first = candidates.first().ok_or_else(|| Error::NotFound {
        what: "CI service".into(),
    })?;
    let auth = work::auth().ok_or_else(|| Error::Network("no login shell environment".into()))?;
    let token = auth.fresh(first, &*work::network()).map(|c| c.token);
    let forge = ide_ci::locate(&candidates, token.as_deref(), work::network())?;
    Ok((name, Client::new(forge, token, work::network())))
}

/// Whether `root` has a remote on a service the app knows and a token for its host, without asking the service;
/// blocks on `gh` or `glab` (E177).
pub(crate) fn access(root: &Path) -> ide_ui_factory::model::detail::ForgeAccess {
    use ide_ui_factory::model::detail::ForgeAccess;
    let Ok((repo, _)) = head(root) else {
        return ForgeAccess::NoRemote;
    };
    let Some((_, url)) = remote(&repo) else {
        return ForgeAccess::NoRemote;
    };
    let candidates = ide_ci::candidates(&url, &work::host_rules());
    let Some(first) = candidates.first() else {
        return ForgeAccess::NoRemote;
    };
    let token = work::auth().and_then(|auth| auth.fresh(first, &*work::network()));
    match token {
        Some(_) => ForgeAccess::Ready,
        None => ForgeAccess::NoToken {
            host: first.host.clone(),
        },
    }
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

/// The URL of the remote the current branch tracks, else `origin`'s, else the first one's.
fn remote_url(repo: &ide_git::Repository) -> Option<String> {
    remote(repo).map(|(_, url)| url)
}

/// The name and fetch URL of the remote the current branch tracks, else `origin`'s, else the first one's.
fn remote(repo: &ide_git::Repository) -> Option<(String, String)> {
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
    Some((remote.name.clone(), remote.fetch_url.clone()?))
}

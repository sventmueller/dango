//! The orchestrator's git (E192, B45a, E167): every call through `Git::unattended`, without the repository's hooks,
//! with E186's environment and a deadline; worktrees made, merged, taken over and removed through
//! `ide_git::thread_trees::order`; and the checks a restart makes before it repeats a side effect. Everything here
//! blocks and runs on the Leitstand's git lane.

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use ide_core::explain::Explain;
use ide_factory::event::{Action, OpResult, TargetState};
use ide_factory::gates::SecretScan;
use ide_factory::relevance::{Change, ChangeKind};
use ide_factory::{OPERATION_TRAILER, OperationId, OrderId, TakeOverBlocker};
use ide_git::thread_trees::{self, order};
use ide_git::{CheckpointThread, Checkpoints, CommitId, CommitOptions, Git, Repository, ThreadTree};
use ide_process::Supervisor;

/// How long one effect's git calls may take altogether.
const DEADLINE: Duration = Duration::from_secs(10 * 60);

/// The user's repository as the orchestrator's git sees it, with that git for its other folders.
pub(super) struct Repos {
    pub(super) git: Git,
    pub(super) main: Repository,
}

impl std::ops::Deref for Repos {
    type Target = Repository;

    fn deref(&self) -> &Repository {
        &self.main
    }
}

/// The orchestrator's git for the folder at `root`, from the login shell's environment `login`.
pub(super) fn open(root: &Path, login: &[(String, String)]) -> ide_git::Result<Repos> {
    let env = ide_process::order_env::for_orders(login, &[]);
    let git = Git::new(Supervisor::global(), login.to_vec()).unattended(env, Instant::now() + DEADLINE);
    let main = git.discover(root)?;
    Ok(Repos { git, main })
}

/// The same git in another folder of the repository, such as an order's worktree.
pub(super) fn open_in(repos: &Repos, folder: &Path) -> ide_git::Result<Repository> {
    repos.git.discover(folder)
}

/// What failed, in the user's words, as the factory records it.
pub(super) fn failed(error: &impl Explain) -> OpResult {
    OpResult::Failed {
        detail: error.explain(),
    }
}

/// The order's worktree as the app keeps it beside the folder of worktrees.
pub(super) fn tree_of(root: &Path, order: &OrderId) -> Option<ThreadTree> {
    thread_trees::list(root)
        .into_iter()
        .find(|tree| tree.order.as_deref() == Some(order.0.as_str()))
}

/// The tree of `commit`.
fn tree(repo: &Repository, branch: &str) -> ide_git::Result<Option<String>> {
    Ok(repo.branch_head(branch)?.map(|head| head.tree))
}

/// The branch name below `refs/heads/`.
fn short(branch: &str) -> &str {
    branch.strip_prefix("refs/heads/").unwrap_or(branch)
}

/// Makes the order's worktree on `branch` from `base_branch`, or finds the one a run before made (E188).
pub(super) fn make_worktree(
    main: &Repository,
    root: &Path,
    order: &OrderId,
    branch: &str,
    base_branch: &str,
) -> Result<ThreadTree, Box<OpResult>> {
    if let Some(tree) = tree_of(root, order).filter(|t| t.branch == branch && t.path.is_dir()) {
        return Ok(tree);
    }
    let made = order::create(main, root, &order.0, "", &order.0, Some(base_branch)).map_err(|error| {
        Box::new(match error {
            thread_trees::TreeError::Git(error) => failed(&error),
            thread_trees::TreeError::Io(error) => failed(&error),
            thread_trees::TreeError::NoCommit | thread_trees::TreeError::NoBranch(_) => OpResult::Failed {
                detail: splat_i18n::tf("Den Branch „{}“ gibt es nicht.", &[base_branch]),
            },
        })
    })?;
    if made.branch != branch {
        return Err(Box::new(OpResult::Failed {
            detail: splat_i18n::tf("Der Branch „{}“ ist schon vergeben.", &[branch]),
        }));
    }
    exclude_from_backup(&made.path);
    Ok(made)
}

/// Keeps Time Machine away from a worktree, whose build folder can take gigabytes (E188, R68).
fn exclude_from_backup(folder: &Path) {
    use objc2::runtime::AnyObject;
    use objc2_foundation::{NSNumber, NSString, NSURL, NSURLIsExcludedFromBackupKey};
    let url = NSURL::fileURLWithPath_isDirectory(&NSString::from_str(&folder.to_string_lossy()), true);
    let yes = NSNumber::new_bool(true);
    let value: &AnyObject = &yes;
    // SAFETY: The key is the framework's own constant, which lives as long as the process, and it takes a boolean
    // NSNumber as its value.
    let set = unsafe { url.setResourceValue_forKey_error(Some(value), NSURLIsExcludedFromBackupKey) };
    if let Err(error) = set {
        ide_core::diag::report("excluding a worktree from backups", error.localizedDescription());
    }
}

/// Commits what the worktree holds with `message` and the operation in its trailer, without hooks, if its head is
/// still `parent` (E163, E192, C66).
pub(super) fn commit(copy: &Repository, branch: &str, op: &OperationId, message: &str, parent: &str) -> OpResult {
    match copy.head() {
        Ok(Some(head)) if head.as_str() == parent => {}
        Ok(_) => {
            return OpResult::Failed {
                detail: splat_i18n::t("Der Worktree steht nicht mehr auf dem erwarteten Commit.").into(),
            };
        }
        Err(error) => return failed(&error),
    }
    if let Err(error) = copy.stage_all() {
        return failed(&error);
    }
    let message = format!("{}\n\n{OPERATION_TRAILER}: {op}\n", message.trim_end());
    match copy.commit(&message, CommitOptions::default()) {
        Ok(commit) => match tree(copy, short(branch)) {
            Ok(Some(tree)) => OpResult::Committed {
                commit: commit.to_string(),
                tree,
            },
            Ok(None) => OpResult::Failed {
                detail: splat_i18n::tf("Den Branch „{}“ gibt es nicht.", &[branch]),
            },
            Err(error) => failed(&error),
        },
        Err(ide_git::Error::NothingToCommit) => OpResult::NothingToCommit,
        Err(error) => failed(&error),
    }
}

/// The paths the diff from `base` to `head` changes, as the factory classifies them (E172).
pub(super) fn diff_names(repo: &Repository, base: &str, head: &str) -> ide_git::Result<Vec<Change>> {
    let files = repo.changes_between(base, head)?;
    Ok(files
        .into_iter()
        .map(|file| {
            let path = file.path.to_string_lossy().into_owned();
            let from = || {
                file.from
                    .as_ref()
                    .map(|p| p.to_string_lossy().into_owned())
                    .unwrap_or_default()
            };
            let kind = match file.change {
                ide_git::Change::Added => ChangeKind::Added,
                ide_git::Change::Deleted => ChangeKind::Deleted,
                ide_git::Change::Modified | ide_git::Change::Unmerged => ChangeKind::Modified,
                ide_git::Change::Renamed { .. } => ChangeKind::Renamed(from()),
                ide_git::Change::Copied { .. } => ChangeKind::Copied(from()),
            };
            Change::new(&path, kind)
        })
        .collect())
}

/// The added lines of the worktree's changes since `since`, as one text for gitleaks, with where each line is.
fn added_text(copy: &Repository, since: &str) -> ide_git::Result<(String, Vec<String>)> {
    let mut text = String::new();
    let mut places = Vec::new();
    for file in copy.diff_since(since, &[] as &[PathBuf])? {
        let path = file.path().to_string_lossy().into_owned();
        for line in file.hunks.iter().flat_map(|h| &h.lines) {
            if line.kind == ide_git::LineKind::Added {
                text.push_str(&line.text());
                text.push('\n');
                places.push(format!("{path}:{}", line.new_number.unwrap_or_default()));
            }
        }
    }
    Ok((text, places))
}

/// The secrets scan of the worktree's changes since `since` with the rules frozen from the main checkout (C52).
pub(super) fn scan(
    repos: &Repos,
    worktree: &Path,
    since: &str,
    rules: &ide_git::FrozenRules,
) -> ide_git::Result<SecretScan> {
    let copy = open_in(repos, worktree)?;
    let (text, places) = added_text(&copy, since)?;
    if text.is_empty() {
        return Ok(SecretScan::Clean);
    }
    Ok(match repos.git.scan_text_frozen(&text, rules)? {
        ide_git::SecretScan::NotInstalled => SecretScan::NotInstalled,
        ide_git::SecretScan::Clean => SecretScan::Clean,
        ide_git::SecretScan::Found(findings) => SecretScan::Found {
            count: u32::try_from(findings.len()).unwrap_or(u32::MAX),
            places: findings
                .iter()
                .map(|f| {
                    let index = usize::try_from(f.line).unwrap_or(1).saturating_sub(1);
                    places.get(index).cloned().unwrap_or_else(|| format!("{}", f.line))
                })
                .collect(),
        },
    })
}

/// Merges the order's base into its branch in the worktree (F49); on conflict the merge is undone (C56).
pub(super) fn merge_target(main: &Repos, tree: &ThreadTree) -> OpResult {
    let copy = match open_in(main, &tree.path) {
        Ok(copy) => copy,
        Err(error) => return failed(&error),
    };
    let base = tree.base_branch.clone().unwrap_or_default();
    match order::merge_base(&copy, tree) {
        Ok(merged) => {
            let moved = matches!(merged, order::BaseMerge::Merged(_));
            if let order::BaseMerge::Conflicts(paths) = merged {
                if let Err(error) = copy.destructive().abort_operation() {
                    return failed(&error);
                }
                return OpResult::Conflict {
                    paths: paths.iter().map(|p| p.to_string_lossy().into_owned()).collect(),
                };
            }
            let (Ok(Some(head)), Ok(Some(base_tip))) = (copy.head(), copy.resolve(&format!("refs/heads/{base}")))
            else {
                return OpResult::Failed {
                    detail: splat_i18n::tf("Den Branch „{}“ gibt es nicht.", &[&base]),
                };
            };
            match tree_of_commit(&copy, &tree.branch) {
                Ok(tree) => OpResult::Merged {
                    head: head.to_string(),
                    tree,
                    base: base_tip.to_string(),
                    moved,
                },
                Err(error) => failed(&error),
            }
        }
        Err(error) => failed(&error),
    }
}

fn tree_of_commit(repo: &Repository, branch: &str) -> ide_git::Result<String> {
    tree(repo, short(branch))?.ok_or_else(|| ide_git::Error::Unexpected {
        what: format!("{branch} has no commit"),
    })
}

/// Takes the order over into its base in the user's working copy (E164, F49).
pub(super) fn take_over(main: &Repository, tree: &ThreadTree, head: &str) -> OpResult {
    let Some(accepted) = CommitId::parse(head) else {
        return OpResult::Failed {
            detail: splat_i18n::t("Der abgenommene Stand ist kein Commit.").into(),
        };
    };
    match order::take_over(main, tree, &accepted) {
        Ok(commit) => OpResult::TakenOver {
            commit: commit.to_string(),
        },
        Err(order::TakeOverError::Refused(refusal)) => match refusal {
            order::Refusal::OtherBranch { current, .. } => OpResult::Blocked {
                blocker: TakeOverBlocker::OtherBranch(current.unwrap_or_default()),
            },
            order::Refusal::NotClean { .. } | order::Refusal::InOperation(_) => OpResult::Blocked {
                blocker: TakeOverBlocker::Dirty,
            },
            order::Refusal::BaseAhead { .. } => OpResult::TargetMoved,
            order::Refusal::NoBase | order::Refusal::Moved { .. } => OpResult::Failed {
                detail: splat_i18n::t("Der Branch des Auftrags steht nicht mehr auf dem abgenommenen Stand.").into(),
            },
        },
        Err(order::TakeOverError::Git(error)) => failed(&error),
    }
}

/// The checkpoint threads by their names; one git refuses names nothing.
fn threads(names: &[String]) -> Vec<CheckpointThread> {
    names
        .iter()
        .filter_map(|name| CheckpointThread::new(name.clone()).ok())
        .collect()
}

/// Removes the order's checkpoints, through the worktree while it is there, else through the main repository (C70).
pub(super) fn remove_checkpoints(main: &Repos, worktree: Option<&Path>, names: &[String]) -> OpResult {
    let copy = worktree.filter(|p| p.is_dir()).and_then(|p| open_in(main, p).ok());
    let checkpoints = Checkpoints::for_repository(copy.as_ref().unwrap_or(&main.main));
    for thread in threads(names) {
        if let Err(error) = checkpoints.delete_thread(&thread) {
            return failed(&error);
        }
    }
    OpResult::Removed
}

/// Removes the order's worktree and, unless `keep_branch`, its branch.
pub(super) fn remove_worktree(
    main: &Repository,
    root: &Path,
    order: &OrderId,
    path: &str,
    branch: &str,
    keep_branch: bool,
) -> OpResult {
    let tree = tree_of(root, order).unwrap_or_else(|| ThreadTree {
        path: PathBuf::from(path),
        branch: short(branch).to_string(),
        title: order.0.clone(),
        base: unreachable_commit(),
        base_branch: None,
        agent: String::new(),
        order: Some(order.0.clone()),
    });
    match order::remove(main, &tree, &[], !keep_branch) {
        Ok(_) => OpResult::Removed,
        Err(error) => failed(&error),
    }
}

/// A commit id no repository has; only for a worktree record rebuilt to remove it.
fn unreachable_commit() -> CommitId {
    CommitId::parse(&"0".repeat(40)).expect("forty zeros name a commit")
}

/// Whether the target state of an operation holds, with what it achieved if so (E167, C66).
pub(super) fn verify(
    main: &Repos,
    root: &Path,
    order: &OrderId,
    action: &Action,
    target: &TargetState,
) -> ide_git::Result<Option<OpResult>> {
    Ok(match target {
        TargetState::WorktreeOn { branch } => {
            let on = main
                .worktrees()?
                .into_iter()
                .find(|w| w.branch.as_deref().map(short) == Some(short(branch)));
            match on {
                Some(worktree) => Some(OpResult::Prepared {
                    path: worktree.path.to_string_lossy().into_owned(),
                    tree: tree(main, short(branch))?.unwrap_or_default(),
                    setup: None,
                    copied: Vec::new(),
                }),
                None => None,
            }
        }
        TargetState::CommitWithTrailer {
            branch,
            parent,
            operation,
        } => {
            let trailer = format!("{OPERATION_TRAILER}: {operation}");
            main.branch_head(short(branch))?
                .filter(|head| head.parents.iter().any(|p| p.as_str() == parent))
                .filter(|head| head.message.lines().any(|line| line.trim() == trailer))
                .map(|head| OpResult::Committed {
                    commit: head.commit.to_string(),
                    tree: head.tree,
                })
        }
        TargetState::BranchContains { branch, other } => {
            let Some(tip) = main.resolve(&format!("refs/heads/{}", short(branch)))? else {
                return Ok(None);
            };
            let Some(other_commit) = main.resolve(other)? else {
                return Ok(None);
            };
            if main.merge_base(tip.as_str(), other_commit.as_str())?.as_ref() != Some(&other_commit) {
                return Ok(None);
            }
            Some(match action {
                Action::TakeOver { .. } => OpResult::TakenOver {
                    commit: tip.to_string(),
                },
                _ => OpResult::Merged {
                    head: tip.to_string(),
                    tree: tree(main, short(branch))?.unwrap_or_default(),
                    base: other_commit.to_string(),
                    moved: true,
                },
            })
        }
        TargetState::CheckpointsGone { threads: names } => {
            let tree = tree_of(root, order);
            let copy = tree
                .as_ref()
                .filter(|t| t.path.is_dir())
                .and_then(|t| open_in(main, &t.path).ok());
            let checkpoints = Checkpoints::for_repository(copy.as_ref().unwrap_or(&main.main));
            let mut left = 0;
            for thread in threads(names) {
                left += checkpoints.list(Some(&thread))?.len();
            }
            (left == 0).then_some(OpResult::Removed)
        }
        TargetState::WorktreeGone { path } => (!Path::new(path).exists()).then_some(OpResult::Removed),
    })
}

/// The diff of the order and the messages of its commits since `base`, for the reviewer (E179).
pub(super) fn review_input(copy: &Repository, base: &str, head: &str) -> ide_git::Result<(String, String)> {
    let diff = String::from_utf8_lossy(&copy.full_diff(base, head)?).into_owned();
    let mut messages = Vec::new();
    for commit in copy.history(200, None)? {
        if commit.hash.as_str() == base {
            break;
        }
        let body = commit
            .body
            .lines()
            .filter(|l| !l.starts_with(OPERATION_TRAILER))
            .collect::<Vec<_>>()
            .join("\n");
        messages.push(format!("{}\n{}", commit.subject, body).trim_end().to_string());
    }
    Ok((diff, messages.join("\n\n")))
}

/// Whether the order's diff against the new base is byte for byte the one against the old (C56).
pub(super) fn same_diff(repo: &Repository, old: (&str, &str), new: (&str, &str)) -> ide_git::Result<bool> {
    Ok(repo.full_diff(old.0, old.1)? == repo.full_diff(new.0, new.1)?)
}

/// Reverses the hunks the user rejected at G2 in the worktree (C55): each names a file and the hunk's place in the
/// diff of the order since `base`, as `path#index`.
pub(super) fn reject_hunks(copy: &Repository, base: &str, hunks: &[ide_factory::HunkId]) -> ide_git::Result<()> {
    let mut by_path: std::collections::BTreeMap<String, Vec<usize>> = std::collections::BTreeMap::new();
    for hunk in hunks {
        if let Some((path, index)) = hunk.0.rsplit_once('#')
            && let Ok(index) = index.parse::<usize>()
        {
            by_path.entry(path.to_string()).or_default().push(index);
        }
    }
    for (path, indices) in by_path {
        for diff in copy.diff_since(base, &[PathBuf::from(&path)])? {
            copy.discard_hunks(&diff, &indices)?;
        }
    }
    Ok(())
}

/// The main repository's refs now (E173).
pub(super) fn refs(main: &Repository) -> ide_git::Result<ide_git::RefSnapshot> {
    main.ref_snapshot()
}

/// What changed between two snapshots, with what Dango made itself marked: checkpoints of every session, and the
/// branches of other orders (E173).
pub(super) fn ref_changes(
    before: &ide_git::RefSnapshot,
    after: &ide_git::RefSnapshot,
    own_branch: &str,
) -> Vec<ide_factory::input::RefChange> {
    let own = format!("refs/heads/{}", short(own_branch));
    before
        .changes_to(after)
        .all()
        .map(|change| ide_factory::input::RefChange {
            name: change.name.clone(),
            before: change.before.clone(),
            after: change.after.clone(),
            by_dango: change.kind == ide_git::RefKind::Checkpoint
                || (change.name.starts_with("refs/heads/order/") && change.name != own),
        })
        .collect()
}

#[cfg(test)]
#[path = "git_tests.rs"]
mod tests;

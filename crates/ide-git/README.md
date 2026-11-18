# ide-git

git for the app through the `git` program: repositories, status, diffs, staging down to single hunks, commits,
remotes, branches, history, the stash, worktrees, merge and cherry-pick, checkpoints of a folder before an agent's
turn, a secrets check with gitleaks before commits, and the request for a commit message, which gitleaks reads from
standard input before anything may send it. Every call blocks until its process ends, so callers run it on a worker
thread, and only in folders the user trusts (E22).

```mermaid
flowchart TB
    callers["worker threads of the app: git panel lane, worktrees, CI panel, agent checkpoints"] -->|"Repository methods"| repo["Repository: diff, status, staging, commit, remote, ..."]
    callers -->|"Repository::destructive, after the user approved"| destructive["Destructive: force push, reset --hard, clean, delete branch, drop stash, ..."]
    destructive --> repo
    repo -->|"Git::run(Call)"| run["Git: login-shell environment, limits"]
    repo -->|"SecretScan and RequestScan via run_program, the request on stdin"| run
    run -->|"Supervisor::spawn_awaited as Kind::Git or Kind::Tool"| process(["git or gitleaks"])
    run -->|"Pipes::step on the calling thread, no thread of its own"| pipes["input written, output read up to a limit, progress lines"]
    pipes --> loop["calling thread: waits for the pipes, the time limit, 50 ms while a Cancel may come, 250 ms for the end; gives the pipes up 2 s after the end"]
    loop -->|"Cancel or timeout: Supervisor::stop"| process
    supervisor["Supervisor::wait"] -->|"the end the reaper reported, kept until the call takes it"| loop
    loop -->|"Output parsed: FileDiff, Status, Commit, ..."| callers
    orchestrator["the orchestrator of work orders"] -->|"Repository::unattended: -c core.hooksPath=/dev/null, its environment, a deadline"| repo
    orchestrator -->|"thread_trees::order: create, merge_base, take_over into the base, remove with checkpoints"| trees["thread_trees: worktrees of threads and orders, JSON beside each"]
    callers -->|"thread_trees: create, list, read, take_over, keep_branch, remove"| trees
    trees --> repo
    orchestrator -->|"ref_snapshot, full_diff, changes_between, branch_head"| repo
    orchestrator -->|"Git::scan_text_frozen: --config and ignore file from the user's copy, --ignore-gitleaks-allow"| run
```

`panel` holds what the app's Git panel decides without AppKit (E98): its rows and sections, the commit graph's
lanes, and its words for git's errors; `ThreadTree` and `TreeState` describe the worktree of an AI thread (I1) or a
work order (F50), and `thread_trees` makes, lists, reads, takes over and removes them (F49), so the orchestrator of
work orders, which may not depend on a UI crate (E99), uses the same logic as the Git panel.

For that orchestrator (phase 13) the crate also has git without the repository's hooks, remotes, signing and
editors, with a given environment and a deadline, which finds a worktree only through the shared git folder's records
and names its git folder in every call (`Git::unattended`, `Git::bound_to`, E192); the diff a secrets check reads,
binary files included and the working tree's attributes ignored (`Repository::diff_since_as_text`); snapshots of every ref for the ref watch (`Repository::ref_snapshot`, E173);
the secrets check with rules frozen from the user's working copy (`Git::scan_text_frozen`, C52); a byte-exact diff
between two commits (`Repository::full_diff`, C56), the files changed between them (`changes_between`, C54) and a
branch's head with parents, tree and message (`branch_head`, C66).

The API is documented in [`src/lib.rs`](src/lib.rs) and its modules.

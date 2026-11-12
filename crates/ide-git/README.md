# ide-git

git for the app through the `git` program: repositories, status, diffs, staging down to single hunks, commits,
remotes, branches, history, worktrees, merge and cherry-pick, checkpoints of a folder before an agent's turn, and a
secrets check with gitleaks before commits. Every call blocks until its process ends, so callers run it on a worker
thread, and only in folders the user trusts (E22).

```mermaid
flowchart TB
    callers["worker threads of the app: git panel lane, worktrees, CI panel, agent checkpoints"] -->|"Repository methods"| repo["Repository: diff, status, staging, commit, remote, ..."]
    callers -->|"Repository::destructive, after the user approved"| destructive["Destructive: force push, reset --hard, clean, delete branch, ..."]
    destructive --> repo
    repo -->|"Git::run(Call)"| run["Git: login-shell environment, limits"]
    repo -->|"SecretScan via run_program"| run
    run -->|"Supervisor::spawn as Kind::Git or Kind::Tool"| process(["git or gitleaks"])
    run -->|"threads git-stdin, git-stdout, git-stderr"| pipes["input written, output read up to a limit"]
    pipes -->|"Done and progress lines over a channel"| loop["calling thread: polls every 50 ms"]
    loop -->|"Cancel or timeout: Supervisor::stop"| process
    supervisor["Supervisor events"] -->|"Ended"| exits["Endings, kept until the call takes them"]
    exits --> loop
    loop -->|"Output parsed: FileDiff, Status, Commit, ..."| callers
```

The API is documented in [`src/lib.rs`](src/lib.rs) and its modules.

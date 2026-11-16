# ide-factory

The Leitstand's orchestrator (docs/phase-13.md): work orders that agents of several vendors build and check in
worktrees of their own. A state machine without AppKit and without an async runtime: the app hands it what happened
and carries out what it asks for, so every rule is testable with a set clock.

```mermaid
flowchart TB
    input["Input: user command, turn ended, gate result, git result, tick, sleep/wake"] -->|"Factory::handle"| machine["state machine per order"]
    machine -->|"appends"| log["event log (JSON lines, operation ids)"]
    machine -->|"returns"| effects["Effect: prepare worktree, start session, send, run gate, commit, take over, notify"]
    effects -->|"the app runs them on its lanes"| input
    workflow[".dango/factory.toml + user defaults"] -->|"tightened only"| machine
    log -->|"replay after a restart"| machine
```

The API is documented in [`src/lib.rs`](src/lib.rs).

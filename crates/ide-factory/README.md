# ide-factory

The Leitstand's orchestrator (docs/phase-13.md, E160): work orders that agents of several vendors build and check in
worktrees of their own. A functional core without AppKit, threads, an async runtime, file or process I/O or clock
reads: the app hands `Factory::handle` what happened, appends the returned log entries to each order's JSON-lines file,
and only then carries out the returned effects on its lanes. Every rule is testable with a set clock.

```mermaid
flowchart TB
    user["user: new order, G1, G2, back to builder, criteria, by hand, abort, resume"] -->|"Input"| handle
    session["session driver (ide-session), mapped: turn ended, activity, request, usage, gap"] -->|"Input"| handle
    results["effect results: worktree, commit, diff, gate, scan, refs, merge, take-over, verify"] -->|"Input"| handle
    clock["Tick, Sleep, Wake, Battery, Windows, Quit"] -->|"Input"| handle
    rules["Rules: UserDefaults + .dango/factory.toml, tightened, loosenings confirmed per Mac"] -->|"InputKind::Rules"| handle
    handle["Factory::handle"] -->|"events applied"| order["Order: station, condition, phase"]
    handle -->|"Step.log first"| log[("order log, JSON lines, versioned")]
    handle -->|"Step.effects after the log is written"| app["the app's lanes: git, supervisor, sessions, notifications, wake lock"]
    app -->|"results with operation ids"| results
    log -->|"Factory::restore after a restart: paused, open operations verified"| handle
    order -->|"Panel, Board, Fleet, Metrics"| views["ide-ui-factory"]
```

What the app keeps for the factory: each order's log, the daily account (`DailyAccount`, across projects), the quota
book (`QuotaBook`), the confirmation hashes per project (`Confirmations`) and the user defaults (`UserDefaults`).

The API is documented in [`src/lib.rs`](src/lib.rs) and its modules.

# ide-session

One driver over every agent session (docs/phase-13.md, B37a, E165): Claude, Codex, OpenCode, ACP agents and the own agent, with or without a tab. `Registry::start` starts a `Launch` under a `SessionId` the app keeps in its context; the `Driver` sends prompts after a checkpoint, answers requests by the `RequestRule` it is handed, fans the events out to any number of subscribers and ends every turn with a typed `TurnEnd`. An order's contract becomes each path's own bounds (E173); OpenCode and ACP agents take no role in orders.

```mermaid
flowchart TB
    launch["Launch: path, folder, environment, model, resume, contract"] -->|"Registry::start"| start["start: the contract in the path's terms, or LaunchError"]
    start --> session["one of the five sessions, or the scripted one"]
    session -->|"events on its own threads"| inbox["inbox: control events in order, invalidations coalesced"]
    checkpoint["checkpoint worker"] -->|"result with the turn id"| inbox
    inbox -->|"Wake: the app's hop to the main thread"| driver["Driver::flush on the main thread"]
    driver -->|"Driver::send after the checkpoint"| session
    driver -->|"each request once"| rule["request rule: user or order"]
    rule -->|"allow, deny with a reason"| session
    rule -->|"left to the user: Signal::Waiting"| feeds
    driver -->|"gap in the stream: resync, or NeedsPause"| session
    driver -->|"signals and coalesced changes"| feeds["one feed per subscriber"]
    feeds -->|"Driver::take: Batch, a redraw after a backlog"| views["views: AI tabs, Leitstand, orchestrator"]
    driver -->|"Signal::TurnEnded: succeeded, failed, cancelled, connection lost"| views
```

The API is documented in [`src/lib.rs`](src/lib.rs).

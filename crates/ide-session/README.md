# ide-session

One driver over every agent session (docs/phase-13.md, B37a): Claude, Codex, OpenCode, ACP agents and the own agent,
with or without a tab. It starts the session, sends prompts after a checkpoint, answers requests by the rule it is
handed, fans the events out to any number of views and ends every turn with a typed result.

```mermaid
flowchart TB
    launch["Launch: path, folder, environment, order contract"] -->|"Driver::start"| session["one of the five sessions"]
    session -->|"events on its own threads"| driver["driver on the main thread"]
    driver -->|"control events in order, coalesced invalidations"| views["views: AI tab, Leitstand"]
    driver -->|"Ended: succeeded, failed, cancelled, connection lost"| orchestrator["orchestrator"]
    driver -->|"requests"| rule["request rule: user or order contract"]
    rule -->|"answer"| session
```

The API is documented in [`src/lib.rs`](src/lib.rs).

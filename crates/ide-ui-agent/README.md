# ide-ui-agent

The AI tab: one agent session per tab with its timeline, composer and status, as the official Claude Code extension
shows it, drawn natively; the Agents panel with its sessions; and the own agent's settings (E6). Claude runs through
`ide_claude::Session`, other agents over ACP through `ide_acp::Session`, the own agent through `ide_agent::Session`.
The crate knows a window only by its id (`ide_ui_common::window::Window`) and reaches the app's terminals, editors,
settings and activity view through its `Host` (E99).

```mermaid
flowchart LR
    app["app: menu, Agents panel, restored layout"] -->|"open_agent(window, agent)"| tab["AgentTab"]
    tab -->|"start"| session["Claude, ACP or own session"]
    session -->|"events, main_soon by tab key"| model["ide_thread::Thread"]
    model --> timeline["Timeline: cards"]
    tab -->|"composer: send, mentions"| session
    tab -->|"WindowHost: add_tab, tint, status"| window["the app's window"]
    tab -->|"Host: terminal, settings, activity view"| host["the app"]
```

The API is documented in [`src/lib.rs`](src/lib.rs).

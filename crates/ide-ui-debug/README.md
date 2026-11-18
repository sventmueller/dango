# ide-ui-debug

The debugger (B1, E94): the Debug panel with sessions, threads, call stacks and breakpoints, the inspector with the
selected frame's variables and watch expressions, the Debug menu and the debug console as a tab. The protocol and the
adapters are `ide_dap`'s. The crate knows a window only by its id (`ide_ui_common::window::Window`) and reaches the
app's editors and terminals through its `Host`; the app draws the breakpoints and the paused line in its editors from
what `gutter` and `DebugPanel` say (E99).

```mermaid
flowchart LR
    app["app: window opens, gutter click, Debug menu"] -->|"attach(window), gutter::click"| panel["DebugPanel"]
    panel -->|"Model (ide_dap)"| model["sessions, breakpoints, variables"]
    panel -->|"start_config"| session["ide_dap::Session on a worker thread"]
    session -->|"events, main_soon by panel id"| panel
    panel -->|"WindowHost: panel page, inspector, console tab"| window["the app's window"]
    panel -->|"Host::refresh_editors, reveal_line"| editors["the app's editors"]
    session -->|"runInTerminal: Host::open_running"| terminal["terminal tab"]
```

The API is documented in [`src/lib.rs`](src/lib.rs).

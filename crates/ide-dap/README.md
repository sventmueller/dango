# ide-dap

Debugging through the Debug Adapter Protocol (B1, E93): a session per program with its adapter, the adapters the app
knows (lldb-dap through `xcrun`, Delve, vscode-js-debug pinned and checked, debugpy from the project's Python), launch
configurations from `.vscode/launch.json` or the file in the editor, and who owns which process (Review C7). Like the
language server client it runs on the transport of `ide-rpc` with threads of its own; requests answer through callbacks and events arrive through
one callback with the session's id. Rust programs are built in the Cargo profile `ide-debug`, the dev profile with full
debug information, defined through the environment and built into `target/ide-debug`, so the project's files and its
dev build stay untouched. Nothing runs for a folder the user has not trusted. The module `model` is what the
debugger panel decides without a window (E98): sessions, selection, breakpoints, inspector, console and commands;
`kept` is what a project keeps of it between launches, as a part of its layout file beside the window tabs.

```mermaid
flowchart TB
    app["the app: debugger panel, inspector, console"] -->|"thread of the caller: ide_dap::start"| prepare["trust check, cargo build in the profile ide-debug for a cargo configuration, Adapters::find"]
    prepare -->|"ide_process::tool: which, Helper::run of xcrun --find, go env, python -c, cargo"| helpers(["short-lived tools"])
    app -->|"launch_configs, current_program, processes"| configs["LaunchConfig"]
    configs --> prepare
    prepare -->|"Session::start"| session["Session"]
    session -->|"Supervisor::spawn as Kind::DebugAdapter"| adapter(["lldb-dap, dlv dap, node dapDebugServer.js, python -m debugpy.adapter"])
    session --> writer["thread dap-write"]
    writer -->|"stdin or a loopback port, Content-Length frames"| adapter
    adapter -->|"stdout or the port"| reader["thread dap-read: responses, events, runInTerminal, startDebugging"]
    session --> timer["ide_core::timer, the app's one timer thread: expires requests"]
    reader -->|"Reply callbacks and Event with SessionId"| sink["events callback, main_soon in the app"]
    reader -->|"startDebugging: a second connection to the same port"| child["child Session"]
    adapter -->|"launches"| program(["launched program: belongs to the session"])
    attached(["attached program: never signalled"]) -.->|"debugged through"| adapter
    app -->|"Session::stop: thread dap-stop"| stop["terminate, then disconnect, then Supervisor::stop on the adapter's group"]
    stop --> adapter
    waiter["thread dap-wait: Supervisor::wait on the adapter"] -->|"adapter ended: crash or stop"| finish["Event::Ended; a launched program left over goes with its group"]
    quit["app quits"] -->|"stop_all, then Supervisor::shutdown with each session's graceful step"| stop
    app -->|"kept::load, kept::save: breakpoints, watches, exception filters"| kept[("the project's layout file, its part debugging, through ide_core::layout")]
    fake["fake::Script and replay: adapters on threads, for tests and the self-test"] -.-> session
```

The API is documented in [`src/lib.rs`](src/lib.rs) and its modules.

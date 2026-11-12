# ide-process

The one register of every process the app starts or adopts: each runs in a process group of its own, is listed for
the activity view, and is ended in order — graceful step, SIGTERM, SIGKILL — when it is stopped or the app quits. It
also reads the login shell's environment, which every other child process is started with, and keeps the tail of a
process's output.

```mermaid
flowchart TB
    callers["ide-git, ide-lsp, ide-acp, ide-claude, ide-agent, ide-ci"] -->|"Supervisor::spawn"| spawn["Command with process_group(0)"]
    spawn --> child(["child process, leader of its own group"])
    spawn -->|"one thread per child: wait-PID"| waiter["Child::wait"]
    waiter -->|"Event::Ended to every subscriber, on that thread"| listeners["subscribe listeners"]
    terminal["ide-terminal"] -->|"Supervisor::register, forget"| register[("register of running processes")]
    spawn --> register
    caller2["any owner"] -->|"Supervisor::stop: thread stop"| stopping["on_stop step, then kill(-group, SIGTERM), then SIGKILL"]
    quit["app quits"] -->|"Supervisor::shutdown, blocking"| stopping
    stopping --> child
    app["the app at launch"] -->|"shell_env::capture"| env["capture"]
    env -->|"Supervisor::spawn"| shell(["login shell -l -i -c env -0"])
    env -->|"reader thread, recv_timeout"| parsed["variables between the marks"]
    tail["tail::collect: thread tail"] -->|"keeps the last bytes"| output["Tail"]
```

The API is documented in [`src/lib.rs`](src/lib.rs) and its modules.

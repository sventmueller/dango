# ide-process

The one register of every process the app starts or adopts: each runs in a process group of its own, is listed for
the activity view, and is ended in order — graceful step, SIGTERM, SIGKILL — when it is stopped or the app quits. One
reaper thread hears through kqueue when any of them ends, so no child costs a thread of its own. It also reads the
login shell's environment, which every other child process is started with, serves a process's pipes from the thread
that waits for it, keeps the tail of a process's output, frames the messages language servers and debug adapters
exchange on their standard streams, and runs the short-lived helper programs others need (`tool`): found on the login
shell's PATH, started with that environment only, with a time limit, a way to cancel and the last mebibyte of each
stream kept. Ending a process ends its whole group, also what is left of it once the leader is gone.

For work orders (phase 13) it runs gate commands as `Kind::Gate` (`gate`): with a deadline and a cap on their output,
past which the whole process group ends, and a run that returns only once that end is confirmed, also for what a
gate left behind in its group. `sandbox` builds the `sandbox-exec` profiles of the own agent's shell tool and of gates
(E187): writing only in given folders and a per-run temp folder, the network off or on (`sandbox-exec` cannot limit it
to domains), the home folder hidden except the tool folders, Unix sockets closed except a list. `order_env` narrows
the login shell's environment for orders (E186): a list of names, no SSH agent, nothing named like a credential, and
an empty `credential.helper` for git.

```mermaid
flowchart TB
    callers["ide-git, ide-lsp, ide-dap, ide-acp, ide-claude, ide-agent, ide-ci"] -->|"Supervisor::spawn"| spawn["Command with process_group(0)"]
    spawn --> child(["child process, leader of its own group"])
    spawn -->|"reaper::reap: EVFILT_PROC NOTE_EXIT on one kqueue"| waiter["thread reaper: waitpid once the child ended"]
    waiter -->|"Event::Ended to every subscriber, on that thread"| listeners["subscribe listeners, until their Subscription drops"]
    waiter -->|"the end kept for Supervisor::wait, beyond the last 256 for spawn_awaited"| register
    terminal["ide-terminal"] -->|"Supervisor::register, forget"| register[("register of running processes")]
    spawn --> register
    caller2["any owner"] -->|"Supervisor::stop: thread stop"| stopping["on_stop step, then kill(-group, SIGTERM), then SIGKILL, until the leader and the rest of its group are gone"]
    stopping -->|"reaper::wait_until: woken when a member of the group ends"| waiter
    terminal -->|"reaper::group_ends_within for the job in front"| waiter
    quit["app quits"] -->|"Supervisor::shutdown, blocking"| stopping
    stopping --> child
    app["the app at launch"] -->|"shell_env::capture"| env["capture"]
    env -->|"Supervisor::spawn"| shell(["login shell -l -i -c env -0"])
    env -->|"pipes::Pipes on the calling thread, until its time limit"| parsed["variables between the marks"]
    tail["tail::collect: thread tail"] -->|"keeps the last bytes"| output["Tail"]
    protocols["ide-rpc, for ide-lsp and ide-dap"] -->|"framing::read, framing::frame"| framing["Content-Length frames, stray output skipped"]
    lines["ide-claude, ide-agent"] -->|"framing::read_line"| capped["lines up to a limit, a longer one dropped"]
    helpers["ide-lsp, ide-dap, ide-acp, ide-predict"] -->|"tool::which, tool::var, tool::Helper::run"| tool["helper with timeout and Cancel"]
    tool -->|"Supervisor::spawn_awaited, Kind::Tool"| spawn
    tool -->|"pipes::Pipes on the calling thread: stderr lines up to a mebibyte, stdout's tail"| output
    orchestrator["the orchestrator of work orders"] -->|"order_env::for_orders, sandbox::Profile::gate"| gate["gate::Gate::run"]
    gate -->|"sandbox-exec -p profile, Supervisor::spawn_awaited, Kind::Gate, nice 20"| spawn
    gate -->|"deadline, output cap or Cancel: Supervisor::stop, then wait for the end"| stopping
    gate -->|"leader ended: what is left of its group gets SIGTERM, SIGKILL, until reaper::group_ends_within"| waiter
```

The API is documented in [`src/lib.rs`](src/lib.rs) and its modules.

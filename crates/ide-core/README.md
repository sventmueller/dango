# ide-core

What the app knows and keeps without a window: its data folders, settings in a user and a project layer, the
shortcut table, each project's window tabs, and the lanes that move blocking work off the main thread. It also holds
what every crate shares: locks that survive a panic, a request to cancel, threads started for one piece of work that
hand it back when no thread starts, one timer thread for every deadline, and the diagnostics hook the app installs,
limited to a few reports of a kind per minute. Nothing here starts a process; the only threads are a lane's workers and
the timer, which start with their first job.

```mermaid
flowchart LR
    main["main thread of the app"] -->|"Lane::ask, Lane::try_run"| queue[["bounded queue of a lane"]]
    queue -.->|"Err(Full) once capacity jobs wait"| main
    queue --> workers["lane threads: DISK, WORK and the app's own lanes"]
    workers -->|"contained(work), None after a panic"| answer["answer(result) on the lane thread"]
    answer -->|"main_soon, in the app"| main
    main -->|"Settings, State: load, get, set"| store["store: versioned JSON with migrations"]
    main -->|"layout::load, layout::save"| store
    store -->|"write beside, then rename"| files[("settings.json, state.json, layouts in paths::data_dir, settings.json in paths::project_dir")]
    main -->|"Keymap::load"| keys[("keybindings.json")]
    crates["the crates"] -->|"diag::report"| limit["at most BURST a minute per kind"]
    limit -->|"the hook diag::install set, else stderr"| log[("the system log, through the app")]
    crates -->|"timer::after(deadline, fire)"| timer["thread timer"]
    crates -->|"thread::detach, detach_answering"| own["a thread of its own, or the work back to answer"]
```

The API is documented in [`src/lib.rs`](src/lib.rs) and its modules.

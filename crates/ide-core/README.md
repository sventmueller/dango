# ide-core

What the app knows and keeps without a window: its data folders, settings in a user and a project layer, the shortcut table, each project's window tabs with the rules of their panes, and the lanes that move blocking work off the main thread. A document of a layout is a kind's name and what the feature of that kind keeps of it; the app's features name their kinds and read them back, and keep parts of their own beside the tabs, such as the debugger its breakpoints. Each setting's key says whether it follows the user to their other Macs, and each crate lists its keys as `SETTINGS`, which the app gathers into one registry for the sync. It also holds what every crate shares: locks that survive a panic, a request to cancel that tasks poll or hook into, threads started for one piece of work that hand it back when no thread starts, one timer thread for every deadline, the diagnostics hook the app installs, limited to a few reports of a kind per minute, files replaced as a whole and flushed to the disk, and answers about a file kept while its stamp stays the same. Nothing here starts a process; the only threads are a lane's workers and the timer, which start with their first job.

```mermaid
flowchart LR
    main["main thread of the app"] -->|"Lane::ask, Lane::try_run"| queue[["bounded queue of a lane"]]
    queue -.->|"Err(Refused::Full) once capacity jobs wait, Err(Refused::Stopped) when no thread starts"| main
    queue --> workers["lane threads: DISK, WORK and the app's own lanes"]
    workers -->|"contained(work), None after a panic"| answer["answer(result) on the lane thread"]
    answer -->|"main_soon, in the app"| main
    main -->|"Settings, State: load, get, set"| store["store: versioned JSON with migrations"]
    main -->|"PaneTree: split, close, move, to_layout, build"| panes["panes::PaneTree of a window tab"]
    panes -->|"layout::Node of version 4"| main
    main -->|"layout::load, layout::save: Tab of a kind a feature names, Tab::new and Tab::read"| store
    features["features beside the tabs: ide-dap kept"] -->|"layout::load_part, layout::save_part"| store
    store -->|"stamp::ByStamp: one stat while the file is unchanged"| files
    store -->|"atomic_write: scratch file of its own, fsync, rename, fsync of the folder"| files[("settings.json, state.json, layouts in paths::data_dir, settings.json in paths::project_dir")]
    others["ide-project trust, ide-thread index, ide-agent conversations, ide-acp installs"] -->|"atomic_write::write, write_json"| own_files[("their files")]
    main -->|"Keymap::load"| keys[("keybindings.json")]
    crates["the crates"] -->|"diag::report"| limit["at most BURST a minute per kind"]
    limit -->|"the hook diag::install set, else stderr"| log[("the system log, through the app")]
    crates -->|"timer::after(deadline, fire)"| timer["thread timer"]
    crates -->|"thread::detach, detach_answering"| own["a thread of its own, or the work back to answer"]
    asker["whoever stops a task"] -->|"Cancel::cancel"| hooks["the flag the task polls, and the hooks it set with on_cancel"]
```

The API is documented in [`src/lib.rs`](src/lib.rs) and its modules.

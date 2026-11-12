# ide-core

What the app knows and keeps without a window: its data folders, settings in a user and a project layer, the
shortcut table, each project's window tabs, and the lanes that move blocking work off the main thread. Nothing here
starts a process; the only threads are a lane's workers, which start with its first job.

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
```

The API is documented in [`src/lib.rs`](src/lib.rs) and its modules.

# ide-ui-common

What the app's AppKit parts share (E99): helpers that build controls and menu items, hops to the main thread,
callbacks that contain a panic, reports to the system log, one undoable step in a text view, and the AI tab's sizes,
fonts and colours with the contrast rules of agent accents (E66). It holds no state of its own beyond the colour
caches; the other `ide-ui-*` crates and the app build on it.

```mermaid
flowchart LR
    worker["worker thread"] -->|"main_soon, main_after, main_on_run_loop"| main["main thread"]
    appkit["AppKit callback"] -->|"guarded: a panic gives the fallback"| code["app code"]
    code -->|"item, tagged, submenu, symbol_button, set_action"| controls["NSMenuItem, NSButton"]
    code -->|"diag::report"| log["system log (Console.app)"]
    code -->|"undo_step"| undo["NSUndoManager group"]
    code -->|"style::accent(base)"| accent{"contrast with the ground"}
    accent -->|"3:1, with Increase Contrast 4.5:1"| color["dynamic NSColor per appearance"]
```

The API is documented in [`src/lib.rs`](src/lib.rs).

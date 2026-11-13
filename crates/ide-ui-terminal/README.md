# ide-ui-terminal

A terminal tab's view and how it draws its screen (E8, E9, E76). `TerminalView` is the layer-backed view that hands
an `ide-terminal` session keys, input-method text, mouse, pastes and drops, reads its screen to VoiceOver and shows
banners for a dev server and short notices above it. Drawing covers the monospaced faces and the cell they fill, the
colours of the light and dark appearance, line and block characters as shapes, and one pass over a snapshot that
paints backgrounds, selection, text, underlines, cursor and marked text onto either Core Graphics or Metal quads.
Besides that it holds what a ⌘-click points at, Mac key codes as terminal keys, and the view's decisions that need no
AppKit (`model`).

The tab, starting the shell with the login-shell environment and what runs in a terminal stay in the app. The app
installs a `Host` once: it keeps the views' registry in the app's state, shows what a view opened (a web address, a
file at a line, a new terminal in a folder) and names tabs. A view knows its window only by id.

```mermaid
flowchart LR
    app["app: tab, shell start, what runs"] -->|"TerminalView::new(window id, folder)"| view["TerminalView"]
    view -->|"Host::opened(window, Opened)"| app
    view -->|"Host::set_tab_title"| app
    session["ide-terminal Session"] -->|"events, on the main thread"| view
    view -->|"keys, mouse, pastes"| session
    view --> paint["render::paint / render::draw"]
    snapshot["ide-terminal Snapshot"] --> paint
    theme["palette::theme(dark, high_contrast)"] --> paint
    paint --> gpu{"gpu::Surface::new()"}
    gpu -->|"Metal device and shader build"| quads["sprites::QuadCanvas: quads over a sprite atlas"]
    quads --> layer["CAMetalLayer, presented with the window's transaction"]
    gpu -->|"no Metal, shader fails, IDE_TERMINAL_CPU"| cg["render::draw: Core Graphics onto the view"]
    paint -->|"line and block characters"| boxdraw["boxdraw: shapes filling the cell"]
    click["⌘-click"] --> links["links::url_at, links::path_at"]
    key["key code"] --> keys["keys::named, keys::modifier, keys::option_is_meta"]
```

The API is documented in [`src/lib.rs`](src/lib.rs).

# ide-ui-terminal

How a terminal tab draws its screen (E8, E76): the monospaced faces and the cell they fill, the colours of the light
and dark appearance, line and block characters as shapes, and one pass over a snapshot of `ide-terminal` that paints
backgrounds, selection, text, underlines, cursor and marked text onto either Core Graphics or Metal quads. Besides
drawing it holds what a ⌘-click points at, Mac key codes as terminal keys, and the view's decisions that need no
AppKit (`model`). The view, its input and its session stay in the app.

```mermaid
flowchart LR
    snapshot["ide-terminal Snapshot"] --> paint["render::paint / render::draw"]
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

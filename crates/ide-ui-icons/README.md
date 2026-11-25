# ide-ui-icons

The icon of a file in lists and on tabs (E66): the logo of a common language from Simple Icons, vendored in `assets/simple-icons` at the repository's root, or an SF Symbol for every other kind of file, each in a system colour that keeps its contrast in every appearance. The logos are parsed from their SVG path once and drawn by Core Graphics as template images.

```mermaid
flowchart LR
    name["file name"] -->|"icon_of: name, prefix, extensions"| icon["Icon: shape + tint"]
    icon -->|"Shape::Logo"| svg["vendored SVG path"] -->|"kurbo, Core Graphics"| template["template NSImage (cached)"]
    icon -->|"Shape::Symbol"| symbol["SF Symbol at 12 pt"] --> template
    icon -->|"Tint"| accent["ide_ui_common::style::accent"]
    template --> view["image_view: NSImageView, hidden from VoiceOver"]
    accent --> view
    template --> image["image: tinted NSImage for a tab title"]
    accent --> image
```

The API is documented in [`src/lib.rs`](src/lib.rs).

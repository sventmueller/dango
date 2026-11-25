# ide-ui-export

A Markdown document as PDF, a self-contained HTML page or a Word file (E99), through splat's crates. Formulas and diagrams go in as the hidden renderer drew them, pictures come from the document's folder and from the web only in a trusted folder (E45). The file is built on a thread of its own; it comes to the main thread only for AppKit's pictures and text, while a sheet on the window says what runs and offers to cancel it. The app hands in what to export as a `Source` and holds the running exports in its context through `Host`; the crate knows no editor.

```mermaid
flowchart LR
    app["app: File > Exportieren"] -->|"ask(mtm, window, Source, Kind)"| panel["NSSavePanel"]
    panel -->|"start"| wait{"formulas and diagrams drawn?"}
    wait -->|"no: renderer, at most 20 s"| wait
    wait -->|"yes"| go["go"]
    go -->|"Html, Word"| thread["export thread: to_html, to_docx"]
    go -->|"Pdf"| pdf["pdf: pages a step at a time on the main thread"]
    thread -->|"on_main: pictures, figures"| main["main thread"]
    thread --> finish["finish: file written, sheet closes"]
    pdf --> finish
    sheet["sheet: Abbrechen"] -->|"cancel"| finish
    finish -->|"Failed"| failed["sheet shows the reason until OK"]
```

The API is documented in [`src/lib.rs`](src/lib.rs).

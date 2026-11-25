# ide-project

A project folder without a window: its tree read one folder at a time, changes on disk, quick open, project search and replace, the `.editorconfig` settings of its files and whether the user trusts the folder. The watcher and quick open run threads of their own and answer through callbacks; the rest blocks on the calling thread. No process is started.

```mermaid
flowchart TB
    fsevents(["FSEvents through notify"]) -->|"bounded channel, overflow means rescan"| gather["thread ide-watch: batches events"]
    gather -->|"on_event(WatchEvent)"| callback["the app's watcher callback"]
    callback -->|"Tree::apply"| tree["Tree: folders read on demand"]
    callback -->|"QuickOpen::changed"| index["thread ide-index: FileIndex build and patch"]
    main["the app's main thread"] -->|"Tree::expand when a folder opens"| tree
    tree -->|"read_dir with the ignore rules"| disk[("project folder")]
    index -->|"on_indexed(Indexed) on that thread"| indexed["the caller's callback"]
    main -->|"QuickOpen::query"| answer["thread ide-quick-open: latest query wins"]
    answer -->|"fuzzy match on scoped threads"| hits["answer(hits) on that thread"]
    worker["a search thread of the app"] -->|"search_with"| search["parallel walk with ripgrep's searcher"]
    search -->|"on_file(FileResult) on the calling thread"| worker
    main -->|"replacements"| replace["the edits of a replace"]
    main -->|"TrustStore::is_trusted, trust, revoke"| trust[("trusted folders in the data folder")]
```

`search_panel` and `files_panel` hold what the app's search and files panels decide without AppKit (E98): the current search with its matches per file and status line, and what a click, the context menu and a new name mean in the tree.

The API is documented in [`src/lib.rs`](src/lib.rs) and its modules.

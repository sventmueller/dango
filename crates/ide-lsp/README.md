# ide-lsp

Language servers for the editor: the JSON-RPC client, the languages with their servers (Go, Rust, TypeScript, common servers on the PATH and the user's own), finding a project's server and installing a missing one. The client runs on the transport of `ide-rpc` with threads of its own, so no call blocks and no async runtime runs beside AppKit's run loop; every position counts UTF-16 code units whatever the server chose. Servers run only for folders the user trusts.

```mermaid
flowchart TB
    app["the app: src/lsp"] -->|"thread lsp-find: Registry::find, Registry::config"| registry["Registry"]
    registry -->|"ide_process::tool: which on the login PATH, Helper::run of rustup which, go env"| helpers(["short-lived tools"])
    installer -->|"ide_process::tool::Helper::run with a Cancel"| helpers
    app -->|"thread lsp-install: Installer"| installer["go install, npm install, or a checked download"]
    installer --> tools[("tools folder")]
    app -->|"Server::start(ServerConfig)"| server["Server"]
    server -->|"Supervisor::spawn as Kind::LanguageServer"| process(["language server"])
    server --> writer["thread lsp-write"]
    writer -->|"stdin, Content-Length frames"| process
    process -->|"stdout"| reader["thread lsp-read: dispatch"]
    server --> timer["ide_core::timer, the app's one timer thread: expires requests"]
    app -->|"open, change, hover, completion, ... with a callback"| server
    reader -->|"reply callback, or timeout from the timer"| callback["callback on that thread"]
    reader -->|"Event: diagnostics, status, requests from the server"| sink["events sink, main_soon in the app"]
    reader -->|"pipe closed"| restart["thread lsp-restart: RestartPolicy, then launch again"]
    restart --> server
```

Window-free parts of the app's language features live here too: `workspace_edit` checks and carries out a server's workspace edit against the folder and the open editors, `offsets` converts between the document model's UTF-16 offsets and protocol positions, `file_watch` matches changed files against the watchers servers registered, and `symbol_search` plans the servers a project-wide symbol search needs and ranks what they found.

The API is documented in [`src/lib.rs`](src/lib.rs) and its modules.

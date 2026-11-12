# ide-lsp

Language servers for the editor: the JSON-RPC client, the languages with their servers (Go, Rust, TypeScript, common
servers on the PATH and the user's own), finding a project's server and installing a missing one. The client runs on
threads of its own, so no call blocks and no async runtime runs beside AppKit's run loop; every position counts
UTF-16 code units whatever the server chose. Servers run only for folders the user trusts.

```mermaid
flowchart TB
    app["the app: src/lsp"] -->|"thread lsp-find: Registry::find, Registry::config"| registry["Registry"]
    registry -->|"helpers such as rustup which, go env, through the supervisor"| helpers(["short-lived tools"])
    app -->|"thread lsp-install: Installer"| installer["go install, npm install, or a checked download"]
    installer --> tools[("tools folder")]
    app -->|"Server::start(ServerConfig)"| server["Server"]
    server -->|"Supervisor::spawn as Kind::LanguageServer"| process(["language server"])
    server --> writer["thread lsp-write"]
    writer -->|"stdin, Content-Length frames"| process
    process -->|"stdout"| reader["thread lsp-read: dispatch"]
    server --> timer["thread lsp-timer: expires requests"]
    app -->|"open, change, hover, completion, ... with a callback"| server
    reader -->|"reply callback, or timeout from lsp-timer"| callback["callback on that thread"]
    reader -->|"Event: diagnostics, status, requests from the server"| sink["events sink, main_soon in the app"]
    reader -->|"pipe closed"| restart["thread lsp-restart: RestartPolicy, then launch again"]
    restart --> server
```

The API is documented in [`src/lib.rs`](src/lib.rs) and its modules.

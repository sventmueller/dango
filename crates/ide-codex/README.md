# ide-codex

OpenAI Codex over its own app-server protocol (E147, "SDK first, ACP as fallback"): `codex app-server` on stdio, a session that fills the shared thread model (`ide-thread`) as an ACP session does, and the management calls Codex offers beyond ACP. An older Codex is refused, so the app takes `codex-acp` through `ide-acp` instead; `docs/codex.md` lays down the framing, the handshake and how each notification changes a thread.

```mermaid
flowchart TB
    app["the app's agent tab"] -->|"version::probe: codex --version"| probe(["codex --version"])
    app -->|"Session::start(Launch)"| spawn["Supervisor::spawn as Kind::Agent"]
    spawn --> server(["codex app-server, JSON lines over stdio without jsonrpc"])
    server -->|"stderr"| tail["tail of ide-process"]
    writer["thread codex-writer"] -->|"stdin"| server
    server -->|"stdout"| reader["thread codex-reader: wire::read"]
    opener["thread codex-open"] -->|"initialize, initialized, account/read, thread/start or thread/resume, model/list"| writer
    reader -->|"item/*, turn/*, thread/*: Mapper, Thread::apply_all"| thread[("Thread")]
    reader -->|"requestApproval, requestUserInput, elicitation: asks kept for answer"| thread
    reader -->|"sink(Event)"| app
    app -->|"send, answer, set_mode, set_option"| writer
    app -->|"manage: models, config, MCP, skills, account, limits, feedback"| reply["Reply: then on a thread of its own"]
    reply --> writer
```

The API is documented in [`src/lib.rs`](src/lib.rs) and its modules.

# ide-acp

Agents over the Agent Client Protocol (E4, E5, E19), such as Codex or Gemini CLI: a session that fills the shared thread model (`ide-thread`), the register of agents the app knows, and installing them with pinned versions. The protocol runs through the `agent-client-protocol` crate, whose types stay inside this one; `docs/acp.md` lays down how each update changes a thread.

```mermaid
flowchart TB
    app["the app's agent tab"] -->|"Session::start(Launch)"| spawn["Supervisor::spawn as Kind::Agent"]
    spawn --> agent(["agent process, JSON-RPC over stdio"])
    agent -->|"stderr"| tail["thread tail"]
    app -->|"Session::start"| conn["thread acp-AGENT: the connection under block_on"]
    conn <-->|"stdin, stdout"| agent
    conn -->|"initialize, then session/new or session/load"| agent
    conn -->|"session/update: Mapper, Thread::apply_all"| thread[("Thread")]
    conn -->|"session/request_permission: asks kept for answer"| thread
    conn -->|"sink(Event), on the connection thread"| app
    app -->|"send, set_mode, set_option, ..."| request["a task of the connection per request: initialize, session/new, session/load and settings with a deadline from the timer of ide-core"]
    request --> conn
    app -->|"Register::cached, Register::fetch"| net["ide-net: Net, HTTPS"]
    net --> registry[("ACP registry")]
    app -->|"Installer: Plan of a preset or entry"| installer["download with checksum, npm, uv"]
    installer -->|"ide_process::tool: which, Helper::run"| helpers(["npm, uv"])
    installer --> installs[("agents folder in the data folder")]
```

The API is documented in [`src/lib.rs`](src/lib.rs) and its modules.

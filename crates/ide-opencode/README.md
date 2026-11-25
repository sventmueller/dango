# ide-opencode

OpenCode over its own v2 HTTP API (E147, "SDK first, ACP as fallback"): `opencode serve --stdio` on 127.0.0.1 with a password of its own per start, a session that fills the shared thread model (`ide-thread`) as an ACP session does, and the management calls OpenCode offers beyond ACP. An older OpenCode is refused, so the app takes `opencode acp` through `ide-acp` instead. The schema of OpenCode 2.2.6 in `tests/fixtures` is the source of every request and answer shape, and the tests check the client's requests and the stand-in server's answers against it.

```mermaid
flowchart TB
    app["the app's agent tab"] -->|"version::probe: opencode --version"| probe(["opencode --version"])
    app -->|"Session::start(Launch)"| spawn["Supervisor::spawn as Kind::Agent, OPENCODE_SERVER_PASSWORD"]
    spawn --> server(["opencode serve --stdio --port 0, HTTP on 127.0.0.1"])
    server -->|"stdout: {url}, then EOF when it ends"| stdout["thread opencode-stdout"]
    server -->|"stderr"| tail["tail of ide-process"]
    stdout -->|"url"| opener["thread opencode-open"]
    opener -->|"GET info, GET event, agents, models, providers, POST session or GET session + messages"| http["http.rs: HTTP/1.1, basic auth"]
    http --> server
    server -->|"GET /api/event: server-sent events"| events["thread opencode-events: Mapper, Thread::apply_all"]
    events -->|"permission.asked, form.created: asks kept for answer"| thread[("Thread")]
    events --> thread
    events -->|"sink(Event)"| app
    app -->|"send, cancel, answer, set_mode, set_option"| call["thread opencode-call per request"]
    app -->|"manage: MCP, sessions, fork, export, revert, credentials, limits"| reply["Reply: wait or then"]
    reply --> call
    call --> http
    app -->|"stop: stdin closes"| server
```

The API is documented in [`src/lib.rs`](src/lib.rs) and its modules.

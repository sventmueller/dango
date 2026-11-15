# ide-agent

The app's own agent (E6): one loop for Anthropic, OpenAI, Gemini, OpenRouter, Ollama, LM Studio and servers that speak
OpenAI's or Anthropic's protocol, with keys from the keychain and prices for what it spends. A session fills the
shared thread model (`ide-thread`) and is driven like an ACP session; its tools read, search, edit and write files,
run commands, fetch pages and read the language servers' diagnostics under a policy that decides what runs, asks or
is refused (E27).

```mermaid
flowchart TB
    app["the app: ide-ui-agent own_agent"] -->|"Keychain: ApiKey per endpoint"| provider["Provider: Endpoint, key, Http"]
    app -->|"Session::start or resume(Config)"| session["Session: conversation, Thread, Policy"]
    app -->|"send, compact"| turn["thread own-agent-turn or own-agent-compact"]
    app -->|"Session::cancel: closes the connections the turn has to itself, ending a silent stream at once"| provider
    app -->|"Session::cancel: the Cancel's hook closes a running web_fetch's own connections"| tools
    turn -->|"compact::run when the context is full"| provider
    turn -->|"Provider::stream(Request)"| provider
    provider -->|"HTTPS, server-sent events"| remote(["provider endpoint"])
    provider -->|"Delta per part, then Reply"| turn
    turn -->|"Thread::apply_all, then sink(Event::Changed) on the turn thread"| app
    turn -->|"Policy::decide per tool call"| ask["ask: permission entry, waits for Session::answer"]
    app -->|"Session::answer"| ask
    turn -->|"read-only calls in parallel on scoped threads"| tools["tools: files, grep and glob, shell, web_fetch, diagnostics"]
    tools -->|"ide_project::search"| project[("project folder")]
    tools -->|"Supervisor::spawn, sandbox-exec with the sandbox on"| shell(["the user's shell"])
    tools -->|"Diagnostics trait"| lsp["the app's language servers"]
    turn -->|"store::save as the conversation grows"| stored[("the thread's conversation on disk")]
```

The API is documented in [`src/lib.rs`](src/lib.rs) and its modules.

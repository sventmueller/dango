# ide-claude

The Claude connection (E21): the installed `claude` CLI in stream-json mode, one JSON frame per line each way. Frames
are read leniently and mapped into the shared thread model (`ide-thread`); control requests in both directions
carry permissions, plan approvals, questions, model and mode changes. `docs/claude-mapping.md` lays down how each
frame changes the thread. Past sessions are read from the CLI's own transcripts, and a recording can be replayed
without a process.

```mermaid
flowchart TB
    app["the app's agent tab"] -->|"Session::start(Launch)"| start["Supervisor::spawn as Kind::Agent"]
    start --> cli(["claude with stream-json input and output"])
    app -->|"send, answer, interrupt, set_model, ..."| queue[["outgoing lines"]]
    queue --> writer["thread claude-writer"]
    writer -->|"stdin"| cli
    cli -->|"stdout"| reader["thread claude-reader"]
    cli -->|"stderr"| tail["thread tail: last 64 KiB"]
    reader -->|"protocol::parse per line"| handle["control frames to Reply, other frames to Mapper"]
    handle -->|"Mapper::map, Thread::apply_all"| thread[("Thread")]
    handle -->|"sink(Event::Changed), on the reader thread"| app
    app -->|"Reply::then: waits on a thread of its own"| waiter["answer to a control request"]
    app -->|"history::list, history::rebuild"| transcripts[("the CLI transcripts in its config folder")]
    tests["tests, self-test"] -->|"Replay"| thread
```

The API is documented in [`src/lib.rs`](src/lib.rs) and its modules.

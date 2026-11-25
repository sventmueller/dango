# ide-thread

The one thread model every agent connection fills, shaped like ACP v2: a thread with its timeline, the subagents it started with timelines of their own, and the work it keeps running in the background, plus the index of threads the session manager lists. It holds data only: no threads, no processes, no I/O except the index file. A connection changes a thread only through `Thread::apply`, which reports each change with a stable entry id.

```mermaid
flowchart LR
    claude["ide-claude: Mapper"] -->|"Update"| apply["Thread::apply, apply_all"]
    acp["ide-acp: Mapper"] -->|"Update"| apply
    agent["ide-agent: turn"] -->|"Update"| apply
    apply -->|"under the session's lock, on the connection's thread"| thread[("Thread: header, lanes of entries, subagents, background")]
    apply -->|"Vec of Change: Header, Inserted, Updated, Removed, ..."| sink["session sink, Event::Changed"]
    sink -->|"main_soon in the app"| tab["agent tab: redraws only the entries named"]
    tab -->|"with_thread, snapshot"| thread
    tab -->|"ThreadIndex::record, list, remove: each change read again and written under index.lock"| index[("threads index in the data folder")]
```

The API is documented in [`src/lib.rs`](src/lib.rs) and its modules.

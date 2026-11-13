# ide-rpc

The transport language servers (`ide-lsp`) and debug adapters (`ide-dap`) share: `Content-Length` frames over a
pair of byte streams, calls that wait for an answer until a deadline, and requests from the other side that are
always answered, even when their handler forgets them. What a frame's body means is left to the protocol on top.

```mermaid
flowchart TB
    caller["ide-lsp, ide-dap"] -->|"Transport::call(id, frame, timeout, call)"| table[("calls waiting for an answer")]
    caller -->|"Transport::send, end_input"| queue[["bounded writer queue: Congested when full"]]
    table --> queue
    queue -->|"thread PROTOCOL-write-LABEL"| output(["the other side's input"])
    input(["the other side's output"]) -->|"thread PROTOCOL-read-LABEL: framing::read"| receiver["Receiver::receive(body)"]
    receiver -->|"Transport::take(id)"| table
    receiver -->|"Responder: ok, fail, or the refusal on drop"| queue
    timer["thread PROTOCOL-timer-LABEL"] -->|"deadline passed"| expired["Call::expired"]
    table --> timer
    input -->|"end or broken frame: diag::report"| close["Transport::close: Call::closed for each, then Receiver::ended"]
    close --> table
```

The API is documented in [`src/lib.rs`](src/lib.rs) and its modules.

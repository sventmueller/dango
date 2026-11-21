## Speed
- What an agent starts, such as builds, tests and shells, runs at the lowest priority instead of in macOS's
  background state. Dango still comes first and the agents before what they start, but their builds use the
  performance cores and the full speed of the disk once more; the background state had cut their throughput to
  about a tenth even on an otherwise idle Mac.

## Agents
- An AI tab's dot stays green while a background command of its session still runs, and the notice that a session
  finished comes only once its background work has ended too.

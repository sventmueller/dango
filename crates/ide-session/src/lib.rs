//! One driver over every agent session (docs/phase-13.md, B37a, E165): it starts a session of any of the five paths,
//! sends prompts after a checkpoint, answers requests by a rule it is handed, fans the session's events out to any
//! number of views and ends every turn with a typed result. It knows no AppKit; the app hands it the hop to the main
//! thread.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

//! The Leitstand's orchestrator (docs/phase-13.md, E160): work orders that agents build and check in worktrees of
//! their own. The crate is a state machine without AppKit and without an async runtime: the app hands it what happened
//! (an `Input`) and carries out what it asks for (an `Effect`) on its lanes, so every rule here is testable with a set
//! clock and scripted sessions.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

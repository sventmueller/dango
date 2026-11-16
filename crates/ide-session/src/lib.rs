//! One driver over every agent session (docs/phase-13.md, B37a, E165): it starts a session of any of the five paths,
//! sends prompts after a checkpoint, answers requests by a rule it is handed, fans the session's events out to any
//! number of views and ends every turn with a typed result. It knows no AppKit; the app hands it the hop to the main
//! thread.
//!
//! # Starting
//!
//! A [`Launch`] names the [`AgentPath`] (Claude, Codex, OpenCode, an ACP preset, the own agent, or the scripted
//! session of the tests), the folder, the environment (the app filters it for an order, E186), model, effort, the
//! session to resume, git for checkpoints and the [`Contract`]: the user's [`UserChoices`], as the AI tab makes them
//! today, or an [`Order`] with its [`Role`], worktree, build folder, readable folders and allowed domains.
//! [`Registry::start`] starts it under a [`SessionId`] the app keeps in its context, with a [`RequestRule`];
//! [`rule::for_contract`] gives the contract's own.
//!
//! An order's bounds per path (E173):
//!
//! | Path | Builder | Readers |
//! |---|---|---|
//! | Claude | `ide_claude::OrderBounds`: `dontAsk` (or `default` without O1), edits allowed in worktree and build folder, git beyond the worktree denied, sandbox with `failIfUnavailable` and no unsandboxed commands, both read blocks, `network.allowedDomains`, no MCP, no web tools, no bypass | the same without edit rules, the writing tools denied and both folders closed to commands' writes |
//! | Codex | `ide_codex::Confinement`: a server of its own, `never`, `workspace-write` with the build folder as write root, no network, apps, plugins, web search, MCP and subagents off | `read-only` |
//! | Own agent | `ide_agent::Policy::order`: sandbox on, nothing asks but destructive git | the "Nur fragen" profile |
//! | OpenCode, ACP | refused: [`LaunchError::NoRole`] (C48, C72) | refused |
//!
//! An allow-list of domains is refused with [`LaunchError::Unsupported`] on Codex and the own agent, which can only
//! switch the network off (C59).
//!
//! # Driving
//!
//! [`Driver::send`] queues a [`Prompt`]; it goes out when no turn runs, after the checkpoint
//! (`ide_git::Checkpoints`, thread `<order>-<role>` for an order, the thread's id otherwise). For an order, a
//! failed checkpoint fails the turn instead. [`Driver::stop_turn`], [`Driver::stop_background`],
//! [`Driver::clear_queue`] and [`Driver::stop`] stop what they say. Requests go to the rule once: [`Verdict::Allow`],
//! [`Verdict::Deny`] with a reason the agent reads where the path carries one, or [`Verdict::User`], after which the
//! user answers through [`Driver::answer_permission`], [`Driver::answer_questions`] and [`Driver::answer_plan`].
//! [`OrderRule`] refuses what reaches beyond an order's bounds (or leaves it to the user without O1), leaves
//! questions to the user and refuses plans.
//!
//! # Events
//!
//! Sessions report on their own threads into the driver's inbox, which asks [`Wake`] once for a hop to the main
//! thread; there the app calls [`Registry::flush`]. Each subscriber ([`Driver::subscribe`]) has a feed: control
//! events ([`Signal`]) complete and in order, invalidations coalesced, a redraw from the snapshot after a backlog of
//! more than 4096; a new subscriber's first [`Batch`] is the snapshot. `flush` returns the subscribers whose feed
//! just filled, which [`Driver::take`] empties; [`Driver::unsubscribe`] ends a feed without ending the session.
//!
//! Every turn ends with [`Signal::TurnEnded`]: its [`TurnId`], an [`Outcome`] (succeeded, failed with the path's
//! words, cancelled, connection lost), its checkpoint and how much work still runs (E189). Every path ends a turn in
//! `Idle`, failed ones too; the outcome comes from the last interruption or error in the turn that no output of the
//! agent follows, and from the process's end.
//!
//! When OpenCode's event stream breaks off, [`Signal::StreamGap`] follows; once it is back, and before any further
//! prompt, the driver reads messages, waiting requests and usage from the server again ([`Signal::Resynced`]), or,
//! when that fails, reports [`Signal::NeedsPause`] and sends nothing until [`Driver::resync`] succeeds (C62).
//!
//! The scripted session (feature `selftest`, F61) plays a [`Script`] per prompt, writes files in the folder and
//! asks for permissions, so self-tests can run an order without an agent's CLI.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

mod backend;
mod checkpoint;
mod driver;
mod feed;
mod id;
mod inbox;
pub mod launch;
mod registry;
pub mod rule;
#[cfg(any(test, feature = "selftest"))]
pub mod script;
mod start;
pub mod turn;

pub use backend::PlanReply;
pub use driver::{AnswerError, Driver, Prompt, SendError, StreamState};
pub use feed::{Batch, Signal};
pub use id::{SessionId, SubscriberId, TurnId};
pub use launch::{AgentPath, Contract, Launch, LaunchError, Order, PathKind, Role, UserChoices};
pub use registry::{Registry, Wake};
pub use rule::{OrderRule, Request, RequestKind, RequestRule, UserRule, Verdict};
#[cfg(any(test, feature = "selftest"))]
pub use script::{End, Script, ScriptTurn, Step};
pub use turn::{Outcome, TurnEnd};

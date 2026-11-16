//! The Leitstand's orchestrator (docs/phase-13.md, E160): work orders that agents build and check in worktrees of
//! their own. The crate is a functional core without AppKit, threads, an async runtime, file or process I/O or clock
//! reads: the app hands it what happened and carries out what it asks for on its lanes, so every rule is testable with
//! a set clock and scripted sessions.
//!
//! - [`Factory`] holds the orders of one project folder. [`Factory::handle`] takes an [`Input`] (a user command, a
//!   session event mapped into [`SessionEvent`], an effect's result, sleep and wake, [`InputKind::Tick`]) with its
//!   [`Stamp`] (monotonic milliseconds for limits, wall clock for display) and returns a [`Step`]: [`log`] entries
//!   first, then [`Effect`]s, then refused commands. The app appends `step.log` to each order's JSON-lines file
//!   before it runs any effect of that order; if the write fails it reports [`InputKind::LogFailed`] and runs none
//!   (E167). [`Factory::restore`] rebuilds the factory from the log texts after a restart: running orders come back
//!   paused with "app ended", and operations begun without their end come back as [`Effect::Verify`].
//! - [`order::Order`] is an order's state, nothing but its [`event::Event`]s applied in turn; its station
//!   ([`Station`]) and condition ([`Condition`]) follow E166, and only the factory changes them: a station is done
//!   when its gate passes.
//! - [`limits`] holds E168's limits and the stall rule, [`budget`] the budgets in three units and the daily account
//!   the app keeps across projects, [`quota`] the quotas per account, service and window (C65).
//! - [`routing`] picks the entry of a role's ranked list (E162, E170) with vendors by model (E171) and what each path
//!   may do in orders ([`routing::Capabilities`], E173).
//! - [`workflow`] reads and edits `.dango/factory.toml` (E161) and merges it with the user's defaults
//!   ([`workflow::UserDefaults`], E183) into [`workflow::rules::Rules`], which only tighten and apply loosenings and
//!   gate commands only with the confirmations the app stores (E190).
//! - [`intake`] takes orders from issues (E176): the frozen origin, the task list as suggested criteria, and which
//!   issues automatic intake takes; such orders always wait for G1, and their text goes into prompts as foreign text.
//! - [`relevance`] classifies a diff's paths (E172), [`gates`] judges the review row (E163), [`verdict`] reads the
//!   reviewer's structured answer (E179), [`evidence`] binds the evidence to its fingerprint with the deviations
//!   first, [`options`] checks the sheet "Neuer Auftrag" (E185), [`prompt`] renders the templates.
//! - [`panel`] is what the panel, the Leitstand tab's station strip and table, and the fleet line show (E175, E98);
//!   [`metrics`] the figures of E180.
//!
//! Errors and reasons are typed values with the data a sentence in the user's language needs (E141); the app words
//! them.
//!
//! ```
//! use ide_factory::workflow::rules::Rules;
//! use ide_factory::{Factory, Input, InputKind, Stage, Stamp};
//!
//! let mut factory = Factory::new(Stage::One, Rules::default());
//! let step = factory.handle(Input::new(Stamp::new(0, 0), InputKind::Tick));
//! assert!(step.log.is_empty() && step.effects.is_empty());
//! ```

#![forbid(unsafe_code)]
#![warn(missing_docs)]

pub mod budget;
pub mod criteria;
pub mod effect;
pub mod event;
pub mod evidence;
mod factory;
pub mod gates;
mod hash;
pub mod ids;
pub mod input;
pub mod intake;
pub mod limits;
pub mod log;
pub mod metrics;
pub mod options;
pub mod order;
pub mod panel;
pub mod prompt;
pub mod quota;
pub mod relevance;
pub mod routing;
pub mod spec;
pub mod station;
pub mod time;
pub mod verdict;
pub mod workflow;

pub use effect::Effect;
pub use factory::{Factory, Refusal, Refused, RestoreProblem, Restored, Step};
pub use ids::{HunkId, OPERATION_TRAILER, OperationId, OrderId, PromptId, RequestId, SessionKey};
pub use input::{Input, InputKind, SessionEvent};
pub use order::Order;
pub use station::{Condition, FailReason, PauseReason, RiskClass, Role, Stage, Station, TakeOverBlocker, WaitReason};
pub use time::{Millis, Stamp};

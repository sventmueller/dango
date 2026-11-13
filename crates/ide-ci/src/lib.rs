//! CI for the app (E12): GitHub Actions and GitLab CI of a repository's remote, on github.com, gitlab.com, GitHub
//! Enterprise Server and self-managed GitLab, also under a path or on another port.
//!
//! - [`candidates`] reads which service a remote URL belongs to and where its API is, with the user's [`HostRule`]s
//!   for hosts whose name does not tell; [`locate`] asks the API which split of a self-managed GitLab's path is the
//!   instance and which the project.
//! - [`Auth`] finds a token strictly per host: the app's keychain entry, then `gh` or `glab`, then the login shell's
//!   environment; [`Auth::save`] keeps one the user typed, [`Auth::save_grant`] one from signing in through the
//!   browser ([`oauth`]: GitHub's device flow, GitLab's authorization code with PKCE), per host and account, and
//!   [`Auth::fresh`] renews it before it expires.
//! - [`panel`] is what the app's CI panel decides without a window (E98): its phase, the rows, and when to read again.
//! - [`Client`] reads runs of a commit or branch, their jobs, steps and logs, re-runs failed jobs, single jobs or whole
//!   runs, starts manual jobs (GitHub: approves waiting deployments) and new runs, cancels, and lists open merge
//!   requests and issues ([`Item`]), with paging, rate limits and typed [`Error`]s; [`log`] makes logs readable and
//!   cuts out the failure for an agent. [`manual_workflows`] finds the GitHub workflows a checkout may start by hand.
//!
//! Every call blocks until the network answers and so belongs on a worker thread. Calls go through a [`Transport`]:
//! [`Network`] in the app, [`Recorded`] answers in tests, which never touch the network or the keychain. The project's
//! remote decides where requests go, so the app uses this crate only in folders the user trusts (E22).

#![warn(missing_docs, unreachable_pub)]

mod auth;
mod client;
mod error;
mod forge;
mod github;
mod gitlab;
mod http;
pub mod log;
mod model;
pub mod oauth;
pub mod panel;

pub use auth::{
    Auth, Commands, Credential, Keychain, Memory, Printed, SaveError, SecretError, SecretStore, Source, Supervised,
    is_public_host, product_client,
};
pub use client::{Client, locate};
pub use error::{Error, Result};
pub use forge::{Forge, HostRule, Kind, RemoteUrl, candidates};
pub use http::{Method, Network, Recorded, Request, Response, Transport, now};
pub use model::{Item, Job, Run, Stage, State, Step, Workflow, manual_workflows, seconds, stages};

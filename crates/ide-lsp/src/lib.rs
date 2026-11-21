//! Language servers: the protocol client, the languages with their servers (Go, Rust, TypeScript, common servers
//! found on the PATH, and the user's own), finding a project's server and installing a missing one.
//!
//! The client speaks JSON-RPC over a server's standard input and output on threads of its own, so no call blocks
//! and no async runtime is needed beside AppKit's run loop: requests answer through a callback on a background
//! thread and can be cancelled through their `Ticket`; events arrive the same way. Every position the API takes or
//! gives counts UTF-16 code units, as the editor does, whatever unit the server chose.
//!
//! Servers run only for trusted folders: the caller decides about trust before it uses `Registry` or `Server`.
//!
//! Window-free parts of the app's language features: [`workspace_edit`] carries out a server's workspace edit,
//! [`offsets`] converts between document offsets and protocol positions, [`file_watch`] picks the file events each
//! server registered for, and [`symbol_search`] plans and ranks a project-wide symbol search.

#![forbid(unsafe_code)]

mod capabilities;
mod convert;
mod documents;
mod encoding;
mod error;
mod features;
pub mod file_watch;
#[cfg(feature = "fuzzing")]
#[doc(hidden)]
pub mod fuzzing;
mod incoming;
mod install;
mod language;
mod navigation;
pub mod offsets;
mod registry;
mod rpc;
mod semantic;
mod server;
pub mod snippet;
pub mod symbol_search;
mod text;
pub mod uri;
pub mod workspace_edit;

pub use documents::Documents;
pub use encoding::Encoding;
pub use error::Error;
pub use features::{Completions, Hover, Insertion, insert_range, insertion};
pub use ide_net::{Cancel, Download, Http, Net};
pub use install::{InstallError, Installer, Progress, Step};
pub use language::{BUILT_IN, CustomServer, Language, companions, root};
pub use lsp_types;
pub use navigation::{Calls, HierarchyItem, Neighbour, ProjectSymbol, Types};
pub use registry::{Install, Launch, NotFound, Registry, Source, VTSLS, default_tools, eslint_settings};
pub use semantic::{SemanticTokens, Token};
pub use server::{Event, Refresh, Reply, RestartPolicy, Server, ServerConfig, Status, Ticket, wait};

//! Language servers: the protocol client, the languages with their servers (Go, Rust, TypeScript, common servers
//! found on the PATH, and the user's own), finding a project's server and installing a missing one.
//!
//! The client speaks JSON-RPC over a server's standard input and output on threads of its own, so no call blocks
//! and no async runtime is needed beside AppKit's run loop: requests answer through a callback on a background
//! thread and can be cancelled through their `Ticket`; events arrive the same way. Every position the API takes or
//! gives counts UTF-16 code units, as the editor does, whatever unit the server chose.
//!
//! Servers run only for trusted folders: the caller decides about trust before it uses `Registry` or `Server`.

#![forbid(unsafe_code)]
#![warn(missing_docs, unreachable_pub)]

mod capabilities;
mod convert;
mod documents;
mod encoding;
mod error;
mod features;
mod framing;
mod incoming;
mod install;
mod language;
mod registry;
mod rpc;
mod semantic;
mod server;
pub mod snippet;
mod text;
mod tool;
pub mod uri;

pub use documents::Documents;
pub use encoding::Encoding;
pub use error::Error;
pub use features::{Completions, Hover, Insertion, insertion};
pub use install::{Download, Http, InstallError, Installer, Net, Progress, Step};
pub use language::{BUILT_IN, CustomServer, Language, root};
pub use lsp_types;
pub use registry::{Install, Launch, NotFound, Registry, Source, VTSLS, default_tools};
pub use semantic::{Legend, SemanticTokens, Token};
pub use server::{Event, Refresh, Reply, RestartPolicy, Server, ServerConfig, Status, Ticket, wait};
pub use tool::Cancel;

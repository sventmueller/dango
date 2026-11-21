//! The one way onto the network: HTTPS checked against the system's certificate store, a [`Net`] that tests stand in
//! for, downloads that hash what arrives while reporting progress and listening for [`Cancel`], what the installers
//! built on them share, and the client for the providers of language models with their failures ([`api`]).
//!
//! ```
//! use std::io::{self, Read};
//!
//! use ide_net::{Cancel, Download, Net};
//!
//! struct Canned;
//!
//! impl Net for Canned {
//!     fn get(&self, _url: &str, _accept: &str) -> io::Result<Download> {
//!         Ok(Download { length: Some(5), body: Box::new(io::Cursor::new(b"hello".to_vec())) })
//!     }
//! }
//!
//! let path = std::env::temp_dir().join(format!("ide-net-doc-{}", std::process::id()));
//! let mut seen = Vec::new();
//! let digest = ide_net::download(&Canned, "https://example.com/x", &path, &Cancel::new(), None, &mut |done, _| {
//!     seen.push(done)
//! })?;
//! assert_eq!(digest, "2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824");
//! assert_eq!(seen, [5]);
//! # std::fs::remove_file(&path)?;
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```

#![forbid(unsafe_code)]

pub mod api;
mod download;
pub mod install;
mod net;

pub use download::{DownloadError, download};
pub use ide_core::Cancel;
pub use net::{Answer, Download, Http, Net, Service};

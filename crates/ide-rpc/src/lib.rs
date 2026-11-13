//! The transport language servers and debug adapters share: `Content-Length` frames over a pair of byte streams,
//! calls that wait for an answer until a deadline, and requests from the other side that are always answered.
//!
//! Three threads per transport: one reads and hands each frame's body to a [`Receiver`], one writes from a bounded
//! queue so a caller never waits on a full pipe, one expires calls whose deadline passed. What a body means — JSON-RPC
//! for `ide-lsp`, the Debug Adapter Protocol for `ide-dap` — is the receiver's business.
//!
//! ```
//! use std::io::Write;
//! use std::sync::{Arc, mpsc};
//! use std::time::Duration;
//!
//! use ide_rpc::{Call, Receiver, Transport};
//!
//! struct Answer(mpsc::Sender<Option<Vec<u8>>>);
//!
//! impl Call for Answer {
//!     fn closed(self) {
//!         let _ = self.0.send(None);
//!     }
//!     fn expired(self, _: i64, _: &Transport<Self>) {
//!         let _ = self.0.send(None);
//!     }
//! }
//!
//! struct Echo;
//!
//! impl Receiver<Answer> for Echo {
//!     fn receive(&self, transport: &Arc<Transport<Answer>>, body: &[u8]) {
//!         let id = i64::from(body[0]);
//!         if let Some(answer) = transport.take(id) {
//!             let _ = answer.0.send(Some(body.to_vec()));
//!         }
//!     }
//!     fn ended(&self) {}
//! }
//!
//! let (input, mut peer_out) = std::io::pipe()?;
//! let (_peer_in, output) = std::io::pipe()?;
//! let transport = Transport::start(input, output, Arc::new(Echo), "rpc", "doc")?;
//! let (tx, rx) = mpsc::channel();
//! let id = transport.next_id();
//! transport.call(id, ide_process::framing::frame(b"ask"), Duration::from_secs(5), Answer(tx)).unwrap();
//! peer_out.write_all(&ide_process::framing::frame(&[u8::try_from(id).unwrap(), b'!']))?;
//! assert_eq!(rx.recv().unwrap(), Some(vec![1, b'!']));
//! # Ok::<(), std::io::Error>(())
//! ```

#![forbid(unsafe_code)]
#![warn(missing_docs, unreachable_pub)]

mod responder;
mod transport;

pub use responder::Responder;
pub use transport::{Call, Receiver, SendError, Transport, contained};

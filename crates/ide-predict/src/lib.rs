//! Edit predictions (B4 in docs/phase-11.md, E91 and E92): what a provider proposes to write next while the user
//! types, shown in grey and taken with Tab.
//!
//! The editor builds a [`Request`] from the document after a pause in typing and hands it to the [`Provider`] the
//! settings name. The provider answers once, on a thread of its own, with the [`Prediction`]s it has; the editor binds
//! each to the document, revision, selection and target range it was asked for and drops it when any of them changed
//! (Review C6). Offsets are byte offsets into [`Request::text`], which is UTF-8.
//!
//! Providers: GitHub Copilot through its language server, and fill-in-the-middle models under Ollama and LM Studio;
//! [`stub::Stub`] answers from a script, for tests and the self-test.

use std::ops::Range;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

pub mod stub;

/// What a provider is asked for: the whole text of one document and where the caret is in it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Request {
    /// The file on disk.
    pub path: PathBuf,
    /// The editor's language id, such as `rust`, as the language servers know it.
    pub language: String,
    /// The document's text as the editor holds it, saved or not.
    pub text: String,
    /// The document's revision the text belongs to (E24).
    pub revision: u64,
    /// The caret, or the selection, in bytes of `text`.
    pub selection: Range<usize>,
    /// Spaces or a tab, and how wide an indent is, so that a proposal indents as the file does.
    pub indent: Indent,
}

/// How the document indents.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Indent {
    Spaces(u8),
    Tabs,
}

/// One proposal: replace `range` of the request's text with `text`. An insertion at the caret has an empty range at
/// the caret; a next edit elsewhere, as Copilot proposes them, replaces or deletes text away from it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Prediction {
    /// The bytes of [`Request::text`] it replaces.
    pub range: Range<usize>,
    /// What it puts there.
    pub text: String,
    /// What the provider wants to hear once the user took it, such as the command Copilot sends along; handed back to
    /// [`Provider::accepted`].
    pub feedback: Option<serde_json::Value>,
}

/// Why a provider has no proposals.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Error {
    /// The request was cancelled before the answer came.
    Cancelled,
    /// The provider needs the user to sign in first, as Copilot does.
    SignIn,
    /// The provider is not set up or not reachable; the text says what to do, for the status bar.
    Unavailable(String),
    /// The provider answered with something it should not have.
    Malformed(String),
}

/// Stops a request; the provider drops its answer and replies [`Error::Cancelled`].
///
/// ```
/// use std::sync::mpsc::channel;
/// use std::time::Duration;
///
/// use ide_predict::stub::Stub;
/// use ide_predict::{Cancel, Error, Indent, Provider, Request};
///
/// let stub = Stub::new(vec![Ok(vec![])], Duration::from_millis(10));
/// let request = Request {
///     path: "/p/a.rs".into(),
///     language: "rust".into(),
///     text: String::new(),
///     revision: 1,
///     selection: 0..0,
///     indent: Indent::Spaces(4),
/// };
/// let (tx, rx) = channel();
/// let cancel = Cancel::new();
/// stub.predict(request, cancel.clone(), Box::new(move |answer| tx.send(answer).unwrap()));
/// cancel.cancel();
/// assert_eq!(rx.recv().unwrap(), Err(Error::Cancelled));
/// ```
#[derive(Debug, Clone, Default)]
pub struct Cancel(Arc<AtomicBool>);

impl Cancel {
    pub fn new() -> Cancel {
        Cancel::default()
    }

    pub fn cancel(&self) {
        self.0.store(true, Ordering::SeqCst);
    }

    pub fn is_cancelled(&self) -> bool {
        self.0.load(Ordering::SeqCst)
    }
}

/// Where an answer goes: called once, on a thread of the provider's, never on the caller's.
pub type Reply = Box<dyn FnOnce(Result<Vec<Prediction>, Error>) + Send>;

/// A source of predictions. Implementations do their work off the caller's thread and must never block it.
pub trait Provider: Send + Sync {
    /// The name the status bar and the settings show.
    fn name(&self) -> &str;

    /// Asks for proposals for `request`; `reply` gets them, or why there are none.
    fn predict(&self, request: Request, cancel: Cancel, reply: Reply);

    /// The user took `prediction`; sends what the provider asked to hear, if anything.
    fn accepted(&self, prediction: &Prediction);
}

//! What a server reports to the app, and the answers the app owes it.

use std::time::Duration;

use ide_process::Ending;
use lsp_types::{
    ApplyWorkspaceEditResponse, Diagnostic, MessageActionItem, MessageType, NumberOrString, Range, ShowDocumentResult,
    Uri, WorkDoneProgress, WorkspaceEdit,
};
use serde::Serialize;
use serde_json::Value;

use crate::rpc::Responder;

/// Where a server is in its life.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    /// Started; `initialize` has not been answered yet.
    Starting,
    /// Answering requests.
    Running,
    /// Crashed and waiting `delay` before restart number `attempt`.
    Restarting {
        /// The restart's number in the current run of crashes; 0 for a restart someone asked for.
        attempt: u32,
        /// The wait.
        delay: Duration,
    },
    /// Asked to stop and not yet gone.
    Stopping,
    /// Stopped on request.
    Stopped,
    /// Crashed too often in a row and stays down until restarted by hand.
    Failed,
}

/// What a server reported or asks of the app; delivered on a background thread.
#[derive(Debug)]
pub enum Event {
    /// The server's status changed.
    Status(Status),
    /// The process ended without being asked to.
    Crashed {
        /// How it ended, if the supervisor told in time.
        ending: Option<Ending>,
        /// The end of what it wrote to standard error.
        stderr: String,
        /// Whether a restart follows.
        restarting: bool,
    },
    /// The diagnostics of a document, replacing earlier ones.
    Diagnostics {
        /// The document.
        uri: Uri,
        /// The document version they belong to, if the server said.
        version: Option<i32>,
        /// With UTF-16 columns.
        diagnostics: Vec<Diagnostic>,
    },
    /// Progress of work the server does on its own, such as indexing.
    Progress {
        /// Tells apart simultaneous pieces of work.
        token: NumberOrString,
        /// Begin, report or end.
        progress: WorkDoneProgress,
    },
    /// A line for the server's log.
    Log {
        /// How important it is.
        level: MessageType,
        /// The text.
        message: String,
    },
    /// A message to show the user.
    Message {
        /// How important it is.
        level: MessageType,
        /// The text.
        message: String,
    },
    /// A message to show the user with buttons; the reply is the chosen one, or `None` when dismissed.
    MessageRequest {
        /// How important it is.
        level: MessageType,
        /// The text.
        message: String,
        /// The buttons.
        actions: Vec<MessageActionItem>,
        /// Must be answered; dropping it answers `None`.
        reply: Reply<Option<MessageActionItem>>,
    },
    /// The server wants the app to apply edits, for example after a code action's command.
    ApplyEdit {
        /// What the user may read in the undo menu.
        label: Option<String>,
        /// With UTF-16 columns.
        edit: WorkspaceEdit,
        /// Must be answered; dropping it answers that nothing was applied.
        reply: Reply<ApplyWorkspaceEditResponse>,
    },
    /// The server wants a document or URL shown.
    ShowDocument {
        /// What to show.
        uri: Uri,
        /// Whether it is meant for the system's browser.
        external: bool,
        /// Whether the editor should take focus.
        take_focus: bool,
        /// What to select, with UTF-16 columns.
        selection: Option<Range>,
        /// Dropping it answers that nothing was shown.
        reply: Reply<ShowDocumentResult>,
    },
    /// The server registered or unregistered capabilities; features may have come or gone.
    CapabilitiesChanged,
    /// Results of this kind are out of date and should be asked for again.
    Refresh(Refresh),
}

/// A kind of result the server declared out of date.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Refresh {
    /// Semantic tokens.
    SemanticTokens,
    /// Inlay hints.
    InlayHints,
    /// Pulled diagnostics.
    Diagnostics,
    /// Code lenses.
    CodeLenses,
    /// Folding ranges.
    FoldingRanges,
}

/// The answer the server waits for; sent once, or the fallback when dropped.
pub struct Reply<T: Serialize> {
    responder: Option<Responder>,
    fallback: Option<T>,
}

impl<T: Serialize> Reply<T> {
    pub(crate) fn new(responder: Responder, fallback: T) -> Reply<T> {
        Reply {
            responder: Some(responder),
            fallback: Some(fallback),
        }
    }

    /// Answers with `answer`.
    pub fn send(mut self, answer: T) {
        if let Some(responder) = self.responder.take() {
            responder.ok(serde_json::to_value(answer).unwrap_or(Value::Null));
        }
    }
}

impl<T: Serialize> Drop for Reply<T> {
    fn drop(&mut self) {
        if let (Some(responder), Some(fallback)) = (self.responder.take(), self.fallback.take()) {
            responder.ok(serde_json::to_value(fallback).unwrap_or(Value::Null));
        }
    }
}

impl<T: Serialize> std::fmt::Debug for Reply<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(if self.responder.is_some() {
            "Reply(waiting)"
        } else {
            "Reply(sent)"
        })
    }
}

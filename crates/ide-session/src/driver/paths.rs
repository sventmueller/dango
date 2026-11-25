//! What only one path offers to the user's session (B37b), as Claude's calls are reached through `Driver::claude`:
//! the sessions of Codex, OpenCode, an ACP agent and the own agent for their overlays (E146), models, modes,
//! management and the own agent's profiles, and permission answers with any option a request offers.

use ide_thread::EntryKind;

use super::{AnswerError, Driver};

impl Driver {
    /// The Codex session, for what only Codex offers, such as its overlays, its settings, its stored threads and
    /// Remote Control over the shared server (E148); `None` on the other paths.
    pub fn codex(&self) -> Option<&ide_codex::Session> {
        self.backend.codex()
    }

    /// The OpenCode session, for what only OpenCode offers, such as its overlays, agents, models and reverting its
    /// stored session; `None` on the other paths.
    pub fn opencode(&self) -> Option<&ide_opencode::Session> {
        self.backend.opencode()
    }

    /// The ACP session, for what only an ACP agent offers, such as its sign-in, settings and overlays; `None` on the
    /// other paths.
    pub fn acp(&self) -> Option<&ide_acp::Session> {
        self.backend.acp()
    }

    /// The own agent's session, for its profile, approval mode, sandbox, model and effort; `None` on the other paths.
    pub fn own(&self) -> Option<&ide_agent::Session> {
        self.backend.own()
    }

    /// Answers the permission request `request` with the option at `option` in its list, such as allowing a command
    /// for the whole session, or with none, which dismisses it; on Codex, OpenCode, ACP and the own agent.
    pub fn answer_option(&mut self, request: &str, option: Option<usize>) -> Result<(), AnswerError> {
        let Some(EntryKind::PermissionRequest(asked)) = self.pending(request) else {
            return Err(AnswerError::NoRequest(request.into()));
        };
        self.backend.option(&asked, option).map_err(AnswerError::Path)
    }
}

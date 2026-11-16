//! The self-test's hooks of the Leitstand: the scripted session its orders start instead of an agent's CLI (F61).

use std::cell::RefCell;

/// What the self-test set.
#[derive(Default)]
pub(crate) struct Probe {
    script: RefCell<Option<ide_session::Script>>,
}

impl Probe {
    /// The script every new session of an order plays, while one is set.
    pub(super) fn script(&self) -> Option<ide_session::Script> {
        self.script.borrow().clone()
    }
}

#[allow(dead_code, reason = "the self-test scenarios of B42 set it")]
impl super::Leitstand {
    /// Lets every session an order starts from now on play `script`, or start the agent's CLI again with `None`.
    pub(crate) fn use_script(&self, script: Option<ide_session::Script>) {
        *self.probe.script.borrow_mut() = script;
    }
}

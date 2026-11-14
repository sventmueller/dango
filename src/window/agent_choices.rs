//! Which agents the "+" menu offers and the menus enable, read on a thread of its own: finding an agent looks along
//! the login shell's PATH and the own agent's keys in the keychain, which would hold up a menu as it opens. The menus
//! take what was read last; each opening of the "+" menu and each change of the agents reads again for the next.

use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::time::Duration;

use ide_ui_agent::Agent;
use objc2::MainThreadMarker;

use crate::context::AppContext;
use crate::util::main_soon;

/// How long a read waits for the login shell's environment, which names the PATH the agents are looked for on.
const ENVIRONMENT: Duration = Duration::from_secs(15);

/// What the menus know of the agents.
#[derive(Debug, Clone)]
pub struct AgentChoices {
    /// Claude's own program is there.
    pub claude: bool,
    /// The own agent has an endpoint to offer.
    pub own: bool,
    /// Each ACP preset's name and whether it can be opened: it is there or can be installed.
    pub presets: Vec<(String, bool)>,
    /// The ACP presets that are there, with their place among all presets.
    pub installed_presets: Vec<(usize, String)>,
    /// The agents of a thread in a worktree of its own, whether each can be opened now and whether it is there.
    pub worktree: Vec<(Agent, bool, bool)>,
}

impl AgentChoices {
    fn read() -> AgentChoices {
        let worktree = crate::worktrees::agents()
            .into_iter()
            .map(|(agent, usable)| {
                let installed = ide_ui_agent::installed(&agent);
                (agent, usable, installed)
            })
            .collect();
        AgentChoices {
            claude: ide_ui_agent::installed(&Agent::Claude),
            own: ide_ui_agent::installed(&Agent::Own),
            presets: ide_ui_agent::preset_menu(),
            installed_presets: ide_ui_agent::installed_presets(),
            worktree,
        }
    }
}

/// The agents as read last, and whether a read runs.
#[derive(Default)]
pub struct AgentChoicesCache {
    last: RefCell<Option<Rc<AgentChoices>>>,
    reading: Cell<bool>,
    /// The agents changed while a read ran, so another follows it.
    again: Cell<bool>,
}

/// What the menus offer now: what was read last, or, before the first read came, a read right here.
pub fn now(context: &AppContext) -> Rc<AgentChoices> {
    let cache = &context.agent_choices;
    if let Some(last) = cache.last.borrow().as_ref() {
        return last.clone();
    }
    let read = Rc::new(AgentChoices::read());
    *cache.last.borrow_mut() = Some(read.clone());
    read
}

/// Reads the agents again on a thread of its own, for the menus' next opening.
pub fn refresh(context: &AppContext) {
    let cache = &context.agent_choices;
    if cache.reading.replace(true) {
        cache.again.set(true);
        return;
    }
    let started = crate::util::spawn("agent-choices", || {
        crate::environment::wait(ENVIRONMENT);
        let read = AgentChoices::read();
        main_soon(move || {
            let Some(mtm) = MainThreadMarker::new() else { return };
            let context = AppContext::get(mtm);
            let cache = &context.agent_choices;
            *cache.last.borrow_mut() = Some(Rc::new(read));
            cache.reading.set(false);
            if cache.again.replace(false) {
                refresh(&context);
            }
        });
    });
    if let Err(why) = started {
        cache.reading.set(false);
        crate::diag::report("reading the agents", why);
    }
}

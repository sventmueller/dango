//! Where CI work runs: a lane for reading the services, threads for what may take long, and the app-wide token
//! keeper and network.

use std::sync::{Arc, OnceLock};
use std::time::Duration;

use ide_ci::{Auth, HostRule, Memory, Network, SecretStore, Supervised, Transport};
use ide_core::lane::Lane;
use ide_core::settings::{Key, Settings};
use ide_process::Supervisor;

use crate::util::main_soon;

/// Hosts whose name does not say which service runs there, as `host[:port][/prefix]=github|gitlab`. Only the user
/// sets them: a project could otherwise send a token elsewhere.
pub(crate) const CI_HOSTS: Key<Vec<String>> = Key::user("ciHosts", ide_core::settings::Sharing::Shared, Vec::new);

/// How long `gh` or `glab` may take to say their token.
const TOOL_TIMEOUT: Duration = Duration::from_secs(8);

/// How long the first job waits for the login shell's environment.
const ENVIRONMENT_WAIT: Duration = Duration::from_secs(30);

static LANE: Lane = Lane::with_start("ide-ci", 2, 32, ide_process::priority::utility);
static NETWORK: OnceLock<Arc<dyn Transport>> = OnceLock::new();
static AUTH: OnceLock<Auth> = OnceLock::new();

/// The network every CI call goes through.
pub(crate) fn network() -> Arc<dyn Transport> {
    NETWORK.get_or_init(|| Arc::new(Network::new())).clone()
}

/// The app-wide token keeper; waits for the login shell's environment, so only for worker threads.
pub(crate) fn auth() -> Option<Auth> {
    if let Some(auth) = AUTH.get() {
        return Some(auth.clone());
    }
    let environment = crate::environment::wait(ENVIRONMENT_WAIT)?.to_vec();
    let commands = Arc::new(Supervised::new(Supervisor::global(), environment.clone(), TOOL_TIMEOUT));
    Some(AUTH.get_or_init(|| Auth::new(store(), commands, environment)).clone())
}

static STORE: OnceLock<Arc<dyn SecretStore>> = OnceLock::new();

/// Keeps the tokens in `store`, the app's one store of secrets; once, before the first token is read.
pub(crate) fn use_secrets(store: Arc<dyn SecretStore>) {
    let _ = STORE.set(store);
}

/// The store the app handed over; before that one that refuses, or memory in unit tests.
fn store() -> Arc<dyn SecretStore> {
    if cfg!(test) {
        return STORE.get_or_init(|| Arc::new(Memory::default())).clone();
    }
    STORE.get().cloned().unwrap_or_else(|| {
        crate::diag::report("reading a CI token", "the store of secrets was not handed over yet");
        Arc::new(ide_agent::NotReady)
    })
}

/// The host rules of the user's settings.
pub(crate) fn host_rules() -> Vec<HostRule> {
    Settings::load_user()
        .get(&CI_HOSTS)
        .iter()
        .filter_map(|rule| HostRule::parse(rule))
        .collect()
}

/// Runs `work`, which may wait for minutes, on a thread of its own and hands its result to `done` on the main thread,
/// `None` if it panicked.
pub(crate) fn long<T: Send + 'static>(
    work: impl FnOnce() -> T + Send + 'static,
    done: impl FnOnce(Option<T>) + Send + 'static,
) {
    let started = crate::util::spawn("ide-ci-login", move || {
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(work)).ok();
        main_soon(move || done(result));
    });
    if let Err(error) = started {
        crate::diag::report("starting the browser login", error);
    }
}

/// Runs `work` on the CI lane and hands its result to `done` on the main thread, `None` if it panicked; false when
/// the lane is full.
pub(crate) fn run<T: Send + 'static>(
    work: impl FnOnce() -> T + Send + 'static,
    done: impl FnOnce(Option<T>) + Send + 'static,
) -> bool {
    LANE.ask(work, move |result| main_soon(move || done(result))).is_ok()
}

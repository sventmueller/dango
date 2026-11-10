//! Where CI work runs: a lane for reading the services, threads for what may take long, and the app-wide token
//! keeper and network.

use std::sync::{Arc, OnceLock};
use std::time::Duration;

use ide_ci::{Auth, HostRule, Keychain, Memory, Network, SecretStore, Supervised, Transport};
use ide_core::lane::Lane;
use ide_core::settings::{Key, Settings};
use ide_process::Supervisor;

use crate::util::main_soon;

/// Hosts whose name does not say which service runs there, as `host[:port][/prefix]=github|gitlab`. Only the user
/// sets them: a project could otherwise send a token elsewhere.
pub const CI_HOSTS: Key<Vec<String>> = Key::user("ciHosts", Vec::new);

/// How long `gh` or `glab` may take to say their token.
const TOOL_TIMEOUT: Duration = Duration::from_secs(8);

/// How long the first job waits for the login shell's environment.
const ENVIRONMENT_WAIT: Duration = Duration::from_secs(30);

static LANE: Lane = Lane::new("ide-ci", 2, 32);
static NETWORK: OnceLock<Arc<dyn Transport>> = OnceLock::new();
static AUTH: OnceLock<Auth> = OnceLock::new();

/// The network every CI call goes through.
pub fn network() -> Arc<dyn Transport> {
    NETWORK.get_or_init(|| Arc::new(Network::new())).clone()
}

/// The app-wide token keeper; waits for the login shell's environment, so only for worker threads.
pub fn auth() -> Option<Auth> {
    if let Some(auth) = AUTH.get() {
        return Some(auth.clone());
    }
    let environment = crate::environment::wait(ENVIRONMENT_WAIT)?.to_vec();
    let commands = Arc::new(Supervised::new(Supervisor::global(), environment.clone(), TOOL_TIMEOUT));
    Some(AUTH.get_or_init(|| Auth::new(store(), commands, environment)).clone())
}

/// The keychain under the app's bundle identifier; the self-test keeps tokens in memory instead.
fn store() -> Arc<dyn SecretStore> {
    if cfg!(feature = "selftest") && std::env::var_os("IDE_SELFTEST").is_some() {
        return Arc::new(Memory::default());
    }
    let service = objc2_foundation::NSBundle::mainBundle()
        .bundleIdentifier()
        .map_or_else(|| ide_product::BUNDLE_ID.to_string(), |id| id.to_string());
    Arc::new(Keychain::new(&service))
}

/// The host rules of the user's settings.
pub fn host_rules() -> Vec<HostRule> {
    Settings::load_user()
        .get(&CI_HOSTS)
        .iter()
        .filter_map(|rule| HostRule::parse(rule))
        .collect()
}

/// Runs `work`, which may wait for minutes, on a thread of its own and hands its result to `done` on the main thread,
/// `None` if it panicked.
pub fn long<T: Send + 'static>(
    work: impl FnOnce() -> T + Send + 'static,
    done: impl FnOnce(Option<T>) + Send + 'static,
) {
    let started = std::thread::Builder::new().name("ide-ci-login".into()).spawn(move || {
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(work)).ok();
        main_soon(move || done(result));
    });
    if let Err(error) = started {
        crate::diag::report("starting the browser login", error);
    }
}

/// Runs `work` on the CI lane and hands its result to `done` on the main thread, `None` if it panicked; false when
/// the lane is full.
pub fn run<T: Send + 'static>(
    work: impl FnOnce() -> T + Send + 'static,
    done: impl FnOnce(Option<T>) + Send + 'static,
) -> bool {
    LANE.ask(work, move |result| main_soon(move || done(result))).is_ok()
}

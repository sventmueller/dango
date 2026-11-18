//! GitHub Copilot through its official language server, `@github/copilot-language-server` from npm (E91): inline
//! completions at the caret, next edits elsewhere through `textDocument/copilotInlineEdit`, and the sign-in with
//! GitHub's device flow, which the server runs itself. The code goes to GitHub (R15).
//!
//! The server runs through `ide_lsp::Server`, so the supervisor owns its process and it gets the login shell's
//! environment; the provider keeps it told about the documents it asks for, so the editor needs no extra sync.

mod account;
pub(crate) mod answer;
mod position;

use ide_core::LockIgnoringPoison;
use std::collections::VecDeque;
use std::io;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Sender, channel};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use ide_lsp::lsp_types::{ShowDocumentResult, Uri};
use ide_lsp::{Event, Install, Server, ServerConfig, Status};
use ide_process::{Kind, Supervisor, tool};
use serde_json::{Value, json};

pub use account::{Account, DeviceCode, SignIn};

use crate::{Cancel, Error, Request, on_own_thread, texts};

/// The program the package brings.
const PROGRAM: &str = "copilot-language-server";

/// What the app installs on request, with `ide_lsp::Installer`, into its tools folder.
pub const INSTALL: Install = Install::Npm {
    package: "@github/copilot-language-server",
    program: PROGRAM,
};

/// How many documents the server is kept told about; the one asked about longest ago is closed first.
const OPEN_DOCUMENTS: usize = 10;

/// How often the account is asked for while the user signs in on GitHub's page.
const POLL: Duration = Duration::from_secs(3);

/// The command that ends the device flow, when `signIn` did not name one.
const FINISH_DEVICE_FLOW: &str = "github.copilot.finishDeviceFlow";

/// The server's program: installed into `tools` (the app's tools folder), else on the login shell's PATH in `env`.
pub fn find(tools: &Path, env: &[(String, String)]) -> Option<PathBuf> {
    let installed = tools.join("node_modules/.bin").join(PROGRAM);
    if tool::is_executable(&installed) {
        return Some(installed);
    }
    tool::which(PROGRAM, tool::var(env, "PATH"))
}

/// What the server's events and the provider's threads share.
struct Shared {
    account: Mutex<Account>,
    on_account: Box<dyn Fn(Account) + Send + Sync>,
    /// The command the last `signIn` sent along, which ends the device flow.
    finish: Mutex<Option<Value>>,
    supervisor: Supervisor,
    env: Vec<(String, String)>,
    closed: AtomicBool,
}

impl Shared {
    fn account(&self) -> Account {
        self.account.lock_ignoring_poison().clone()
    }

    fn set(&self, account: Account) {
        let changed = {
            let mut current = self.account.lock_ignoring_poison();
            let changed = *current != account;
            *current = account.clone();
            changed
        };
        if changed {
            (self.on_account)(account);
        }
    }

    fn event(&self, event: Event) {
        match event {
            Event::Status(Status::Failed) => self.set(Account::Unavailable(texts::copilot_down())),
            Event::ShowDocument {
                uri,
                external: true,
                reply,
                ..
            } => reply.send(ShowDocumentResult {
                success: self.open_url(&uri),
            }),
            _ => {}
        }
    }

    /// Opens GitHub's sign-in page in the user's browser, where the server asks for it during the device flow.
    fn open_url(&self, url: &Uri) -> bool {
        if !matches!(url.scheme().map(|s| s.as_str()), Some("https" | "http")) {
            return false;
        }
        let mut command = Command::new("/usr/bin/open");
        command
            .arg(url.as_str())
            .env_clear()
            .envs(self.env.iter().map(|(k, v)| (k, v)))
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        self.supervisor.spawn(command, Kind::Tool, "open").is_ok()
    }
}

/// Copilot for one project root, which must be trusted (E22); dropping it stops the server.
pub struct Copilot {
    core: Arc<Core>,
    /// Predictions in the order they were asked for, on a thread of the provider's: telling the server the text
    /// costs time with a large document, which the caller does not wait for.
    jobs: Sender<Job>,
}

type Job = Box<dyn FnOnce() + Send>;

struct Core {
    server: Server,
    shared: Arc<Shared>,
    /// The documents the server knows, with the version it was told, the one asked about last at the back.
    documents: Mutex<VecDeque<(Uri, i32)>>,
}

impl Copilot {
    /// Starts `program` (see [`find`]) through `supervisor` with `env`, the login shell's environment, for the
    /// project at `root`; `on_account` hears every change of the account, on a thread of the provider's.
    pub fn start(
        supervisor: &Supervisor,
        program: &Path,
        root: &Path,
        env: Vec<(String, String)>,
        on_account: impl Fn(Account) + Send + Sync + 'static,
    ) -> io::Result<Copilot> {
        let shared = Arc::new(Shared {
            account: Mutex::new(Account::Starting),
            on_account: Box::new(on_account),
            finish: Mutex::new(None),
            supervisor: supervisor.clone(),
            env: env.clone(),
            closed: AtomicBool::new(false),
        });
        let mut config = ServerConfig::new("Copilot", program, root, env);
        config.args = vec!["--stdio".into()];
        let version = env!("CARGO_PKG_VERSION");
        config.initialization_options = Some(json!({
            "editorInfo": {"name": ide_product::NAME, "version": version},
            "editorPluginInfo": {"name": ide_product::NAME, "version": version},
        }));
        let events = shared.clone();
        let server = Server::start(supervisor, config, move |event| events.event(event))?;
        let (jobs, queue) = channel::<Job>();
        std::thread::Builder::new()
            .name("copilot-predict".into())
            .spawn(move || {
                ide_process::priority::set_qos(ide_process::priority::Qos::UserInitiated);
                queue.into_iter().for_each(|job| job());
            })?;
        let starting = (server.clone(), shared.clone());
        let started = ide_core::thread::detach("copilot-start", move || {
            let (server, shared) = starting;
            if server.wait_until_running(Duration::from_secs(60)) {
                check(&server, &shared);
            }
        });
        if let Err(unstarted) = started {
            ide_core::diag::report("thread copilot-start", &unstarted.error);
        }
        let core = Arc::new(Core {
            server,
            shared,
            documents: Mutex::new(VecDeque::new()),
        });
        Ok(Copilot { core, jobs })
    }

    /// The account as last reported.
    pub fn account(&self) -> Account {
        self.core.shared.account()
    }

    /// Asks the server for the account again; the answer arrives through `on_account` if it changed.
    pub fn check(&self) {
        check(&self.core.server, &self.core.shared);
    }

    /// Starts the device flow: `reply` gets the code to show, or the user already signed in.
    pub fn sign_in(&self, reply: impl FnOnce(Result<SignIn, Error>) + Send + 'static) {
        self.core.sign_in(reply);
    }

    /// Ends the device flow once the user entered the code: the server opens GitHub's page, and the account is asked
    /// for until the user is signed in, the code expired or `cancel` is cancelled; changes arrive through
    /// `on_account`.
    pub fn finish_sign_in(&self, cancel: Cancel) {
        self.core.finish_sign_in(cancel);
    }

    /// Signs the user out of Copilot on this Mac.
    pub fn sign_out(&self) {
        self.core.sign_out();
    }
}

impl Core {
    fn sign_in(&self, reply: impl FnOnce(Result<SignIn, Error>) + Send + 'static) {
        let reply = Arc::new(Mutex::new(Some(reply)));
        let answer = reply.clone();
        let shared = self.shared.clone();
        let down = self.is_down();
        let sent = self.server.request_value("signIn", json!({}), move |result| {
            let outcome = result
                .map_err(|e| error_of(e, down))
                .and_then(|value| account::sign_in(&value).ok_or_else(|| Error::Malformed(value.to_string())));
            let outcome = outcome.map(|(sign_in, command)| {
                match &sign_in {
                    SignIn::Already { user } => shared.set(Account::SignedIn { user: user.clone() }),
                    SignIn::Device(_) => *shared.finish.lock_ignoring_poison() = command,
                }
                sign_in
            });
            if let Some(reply) = answer.lock_ignoring_poison().take() {
                reply(outcome);
            }
        });
        if let Err(error) = sent {
            let error = error_of(error, self.is_down());
            if let Some(reply) = reply.lock_ignoring_poison().take() {
                on_own_thread(move || reply(Err(error)));
            }
        }
    }

    fn finish_sign_in(&self, cancel: Cancel) {
        let command = self
            .shared
            .finish
            .lock_ignoring_poison()
            .take()
            .unwrap_or_else(|| json!({"command": FINISH_DEVICE_FLOW}));
        let params = json!({
            "command": command.get("command").cloned().unwrap_or_else(|| json!(FINISH_DEVICE_FLOW)),
            "arguments": command.get("arguments").cloned().unwrap_or_else(|| json!([])),
        });
        let shared = self.shared.clone();
        let _ = self
            .server
            .request_value("workspace/executeCommand", params, move |answer| {
                if let Ok(value) = answer
                    && value.get("status").is_some()
                {
                    shared.set(account::status(&value));
                }
            });
        let server = self.server.clone();
        let shared = self.shared.clone();
        if let Err(unstarted) = ide_core::thread::detach("copilot-sign-in", move || poll(&server, &shared, &cancel)) {
            ide_core::diag::report("thread copilot-sign-in", &unstarted.error);
        }
    }

    fn sign_out(&self) {
        let shared = self.shared.clone();
        let _ = self.server.request_value("signOut", json!({}), move |answer| {
            if answer.is_ok() {
                shared.set(Account::NeedsSignIn);
            }
        });
    }

    fn is_down(&self) -> bool {
        matches!(
            self.server.status(),
            Status::Failed | Status::Stopped | Status::Stopping
        )
    }

    /// Tells the server the request's text, opening or changing the document, and returns its version.
    fn sync(&self, request: &Request, uri: &Uri) -> i32 {
        let version = i32::try_from(request.revision).unwrap_or(i32::MAX);
        let mut documents = self.documents.lock_ignoring_poison();
        let known = documents
            .iter()
            .position(|(u, _)| u == uri)
            .and_then(|i| documents.remove(i));
        let changed = match (self.server.text(uri), known) {
            (Some(old), Some((_, told))) if old == request.text && told == version => Ok(()),
            (Some(old), Some(_)) => self
                .server
                .change(uri, version, &[position::change(&old, &request.text)]),
            _ => Err(ide_lsp::Error::NotOpen),
        };
        if changed.is_err() {
            self.server.open(uri, &request.language, version, &request.text);
        }
        documents.push_back((uri.clone(), version));
        while documents.len() > OPEN_DOCUMENTS {
            if let Some((old, _)) = documents.pop_front() {
                self.server.close(&old);
            }
        }
        version
    }
}

impl Drop for Copilot {
    fn drop(&mut self) {
        self.core.shared.closed.store(true, Ordering::SeqCst);
    }
}

fn check(server: &Server, shared: &Arc<Shared>) {
    let shared = shared.clone();
    let _ = server.request_value("checkStatus", json!({}), move |answer| {
        if let Ok(value) = answer {
            shared.set(account::status(&value));
        }
    });
}

/// Asks for the account until the user signed in, the device code expired or the sign-in was cancelled.
fn poll(server: &Server, shared: &Shared, cancel: &Cancel) {
    let deadline = Instant::now() + Duration::from_secs(15 * 60);
    while Instant::now() < deadline {
        let next = Instant::now() + POLL;
        while Instant::now() < next {
            if cancel.is_cancelled() || shared.closed.load(Ordering::SeqCst) {
                return;
            }
            std::thread::sleep(Duration::from_millis(100));
        }
        if let Ok(value) = ide_lsp::wait(|reply| server.request_value("checkStatus", json!({}), reply)) {
            let account = account::status(&value);
            let done = matches!(account, Account::SignedIn { .. } | Account::NoSubscription { .. });
            shared.set(account);
            if done {
                return;
            }
        }
    }
}

/// What a failed request means for the user; `down` says whether the server stopped for good.
fn error_of(error: ide_lsp::Error, down: bool) -> Error {
    match error {
        ide_lsp::Error::Cancelled => Error::Cancelled,
        ide_lsp::Error::NotRunning | ide_lsp::Error::ServerGone if down => Error::Unavailable(texts::copilot_down()),
        ide_lsp::Error::NotRunning | ide_lsp::Error::ServerGone => Error::Unavailable(texts::copilot_starting()),
        ide_lsp::Error::Malformed(message) => Error::Malformed(message),
        ide_lsp::Error::Server { message, .. } => {
            let lower = message.to_ascii_lowercase();
            if ["not signed in", "notsignedin", "sign in", "not authenticated"]
                .iter()
                .any(|words| lower.contains(words))
            {
                Error::SignIn
            } else {
                Error::Unavailable(texts::failed("Copilot", &message))
            }
        }
        other => Error::Unavailable(texts::failed("Copilot", &other.to_string())),
    }
}

mod predict;

#[cfg(test)]
mod tests {
    use super::*;

    fn program(path: &Path, mode: u32) {
        use std::os::unix::fs::PermissionsExt;
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, "#!/bin/sh\n").unwrap();
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(mode)).unwrap();
    }

    #[test]
    fn the_installed_server_comes_before_one_on_the_path_and_only_programs_count() {
        let dir = ide_core::testing::scratch("predict-find");
        let (tools, plain, bin) = (dir.join("tools"), dir.join("plain"), dir.join("bin"));
        program(&plain.join(PROGRAM), 0o644);
        program(&bin.join(PROGRAM), 0o755);
        let path = vec![("PATH".to_string(), format!("::{}:{}", plain.display(), bin.display()))];
        assert_eq!(
            find(&tools, &path),
            Some(bin.join(PROGRAM)),
            "a file that cannot run is passed over"
        );
        assert_eq!(find(&tools, &[]), None);
        program(&tools.join("node_modules/.bin").join(PROGRAM), 0o755);
        assert_eq!(find(&tools, &path), Some(tools.join("node_modules/.bin").join(PROGRAM)));
    }

    #[test]
    fn a_failed_request_tells_the_user_whether_to_wait_sign_in_or_restart() {
        let server = |message: &str| ide_lsp::Error::Server {
            code: 1000,
            message: message.into(),
        };
        assert_eq!(error_of(ide_lsp::Error::Cancelled, false), Error::Cancelled);
        assert_eq!(
            error_of(ide_lsp::Error::NotRunning, false),
            Error::Unavailable(texts::copilot_starting())
        );
        assert_eq!(
            error_of(ide_lsp::Error::ServerGone, true),
            Error::Unavailable(texts::copilot_down())
        );
        assert_eq!(
            error_of(ide_lsp::Error::Malformed("no items".into()), false),
            Error::Malformed("no items".into())
        );
        for words in [
            "Not signed in",
            "NotSignedIn",
            "Please sign in",
            "User not authenticated",
        ] {
            assert_eq!(error_of(server(words), false), Error::SignIn, "{words}");
        }
        assert_eq!(
            error_of(server("rate limited"), false),
            Error::Unavailable(texts::failed("Copilot", "rate limited"))
        );
        assert_eq!(
            error_of(ide_lsp::Error::Timeout, false),
            Error::Unavailable(texts::failed("Copilot", &ide_lsp::Error::Timeout.to_string()))
        );
    }
}

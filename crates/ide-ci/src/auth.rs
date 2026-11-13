//! Tokens, strictly per host: the app's keychain entry first, then what `gh` or `glab` are logged in with, then the
//! login shell's environment. A token is only ever sent to the host it was found for.
//!
//! Signing in through the browser (GitHub's device flow, GitLab's OAuth with PKCE) needs an OAuth app registered for
//! the product, which waits for the owner's approval (E12); `browser_login` says whether it is there.

use ide_core::LockIgnoringPoison;
use std::io::Read;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use ide_process::{Kind as ProcessKind, Supervisor};
pub use ide_secrets::{Keychain, Memory, SecretError, SecretStore};

use serde::{Deserialize, Serialize};

use crate::forge::{Forge, Kind};
use crate::http::{Transport, now};
use crate::oauth::Grant;

/// Where a token came from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Source {
    /// The app's keychain entry for the host.
    Keychain,
    /// `gh auth token`.
    Gh,
    /// `glab auth status --show-token`.
    Glab,
    /// This variable of the login shell's environment.
    Environment(&'static str),
}

/// A token for one host.
#[derive(Clone, PartialEq, Eq)]
pub struct Credential {
    /// The host it belongs to.
    pub host: String,
    /// The token.
    pub token: String,
    /// Where it came from.
    pub source: Source,
    /// The account it belongs to, where the app keeps it.
    pub account: Option<String>,
}

/// What the keychain keeps for one account of a host.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
struct Kept {
    token: String,
    #[serde(default)]
    refresh: Option<String>,
    #[serde(default)]
    expires: Option<i64>,
    /// The OAuth client the token came from, which renews it.
    #[serde(default)]
    client: Option<String>,
}

/// How long before it expires a kept token is renewed.
const RENEW_BEFORE: i64 = 300;

impl std::fmt::Debug for Credential {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Credential")
            .field("host", &self.host)
            .field("source", &self.source)
            .finish_non_exhaustive()
    }
}

/// Why a token was not kept.
#[derive(Debug)]
pub enum SaveError {
    /// The keychain refused it.
    Store(SecretError),
    /// It could not be written as JSON.
    Encode(serde_json::Error),
}

impl std::fmt::Display for SaveError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SaveError::Store(error) => write!(f, "{error}"),
            SaveError::Encode(error) => write!(f, "the token could not be written: {error}"),
        }
    }
}

impl std::error::Error for SaveError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            SaveError::Store(error) => Some(error),
            SaveError::Encode(error) => Some(error),
        }
    }
}

impl From<SecretError> for SaveError {
    fn from(error: SecretError) -> SaveError {
        SaveError::Store(error)
    }
}

/// What a program printed.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Printed {
    /// Its standard output.
    pub stdout: String,
    /// Its standard error.
    pub stderr: String,
}

/// Runs the command-line tools of the services.
pub trait Commands: Send + Sync {
    /// What `program` with `args` printed; `None` when it is not installed or did not end in time.
    fn run(&self, program: &str, args: &[&str]) -> Option<Printed>;
}

/// Programs from the login shell's PATH, started through the supervisor in the home folder, so no repository's
/// configuration takes part.
pub struct Supervised {
    supervisor: Supervisor,
    environment: Vec<(String, String)>,
    timeout: Duration,
}

impl Supervised {
    /// Programs run with the login shell's `environment`, stopped after `timeout`.
    pub fn new(supervisor: &Supervisor, environment: Vec<(String, String)>, timeout: Duration) -> Supervised {
        Supervised {
            supervisor: supervisor.clone(),
            environment,
            timeout,
        }
    }

    fn variable(&self, name: &str) -> Option<&str> {
        self.environment
            .iter()
            .find(|(n, _)| n == name)
            .map(|(_, v)| v.as_str())
    }

    fn find(&self, program: &str) -> Option<PathBuf> {
        std::env::split_paths(self.variable("PATH")?)
            .map(|dir| dir.join(program))
            .find(|path| path.is_file())
    }
}

impl Commands for Supervised {
    fn run(&self, program: &str, args: &[&str]) -> Option<Printed> {
        let path = self.find(program)?;
        let mut command = Command::new(&path);
        command
            .args(args)
            .env_clear()
            .envs(self.environment.iter().map(|(k, v)| (k, v)))
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        if let Some(home) = self.variable("HOME") {
            command.current_dir(home);
        }
        let spawned = self.supervisor.spawn(command, ProcessKind::Tool, program).ok()?;
        let read = |pipe: Option<Box<dyn Read + Send>>| {
            std::thread::spawn(move || {
                let mut text = Vec::new();
                if let Some(mut pipe) = pipe {
                    let _ = pipe.by_ref().take(64 * 1024).read_to_end(&mut text);
                }
                String::from_utf8_lossy(&text).into_owned()
            })
        };
        let out = read(spawned.stdout.map(|p| Box::new(p) as Box<dyn Read + Send>));
        let err = read(spawned.stderr.map(|p| Box::new(p) as Box<dyn Read + Send>));
        let deadline = Instant::now() + self.timeout;
        while !(out.is_finished() && err.is_finished()) {
            if Instant::now() >= deadline {
                self.supervisor.stop(spawned.id, Duration::from_millis(300));
                return None;
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        Some(Printed {
            stdout: out.join().unwrap_or_default(),
            stderr: err.join().unwrap_or_default(),
        })
    }
}

/// Finds and keeps the tokens of hosts.
#[derive(Clone)]
pub struct Auth {
    store: Arc<dyn SecretStore>,
    commands: Arc<dyn Commands>,
    environment: Vec<(String, String)>,
    /// GitLab replaces the refresh token on every renewal, so two at once would leave one with a dead token.
    renewing: Arc<Mutex<()>>,
}

impl Auth {
    /// Tokens from `store`, the tools `commands` runs and the variables of `environment`.
    pub fn new(store: Arc<dyn SecretStore>, commands: Arc<dyn Commands>, environment: Vec<(String, String)>) -> Auth {
        Auth {
            store,
            commands,
            environment,
            renewing: Arc::new(Mutex::new(())),
        }
    }

    /// The token for the host of `forge`: the app's own, then the command-line tool's, then the environment's.
    pub fn find(&self, forge: &Forge) -> Option<Credential> {
        let credential = |token: String, source: Source| Credential {
            host: forge.host.clone(),
            token,
            source,
            account: None,
        };
        if let Some((name, kept)) = self.kept(forge) {
            return Some(Credential {
                account: name,
                ..credential(kept.token, Source::Keychain)
            });
        }
        let from_tool = match forge.kind {
            Kind::GitHub => self
                .commands
                .run("gh", &["auth", "token", "--hostname", &forge.host])
                .and_then(|p| gh_token(&p))
                .map(|t| (t, Source::Gh)),
            Kind::GitLab => self
                .commands
                .run(
                    "glab",
                    &[
                        "auth",
                        "status",
                        "--hostname",
                        &forge.host,
                        "--show-token",
                        "--output",
                        "json",
                    ],
                )
                .and_then(|p| glab_token(&p))
                .map(|t| (t, Source::Glab)),
        };
        if let Some((token, source)) = from_tool {
            return Some(credential(token, source));
        }
        self.environment_token(forge)
            .map(|(token, name)| credential(token, Source::Environment(name)))
    }

    /// The token for the host of `forge` as `find` gives it, renewed first when the app keeps it with a refresh token
    /// and it expires soon; a renewal that fails leaves the old one, which the service then refuses.
    pub fn fresh(&self, forge: &Forge, transport: &dyn Transport) -> Option<Credential> {
        let _renewing = self.renewing.lock_ignoring_poison();
        self.renewed(forge, transport, false)
    }

    /// The app's own token for the host of `forge` in place of `refused`, which the service turned down; `None` when
    /// there is nothing to renew it with or the service refuses that too.
    pub fn renew(&self, forge: &Forge, transport: &dyn Transport, refused: &str) -> Option<Credential> {
        let _renewing = self.renewing.lock_ignoring_poison();
        let found = self.find(forge)?;
        if found.token != refused {
            return Some(found);
        }
        let after = self.renewed(forge, transport, true)?;
        (after.token != refused).then_some(after)
    }

    fn renewed(&self, forge: &Forge, transport: &dyn Transport, now_anyway: bool) -> Option<Credential> {
        let found = self.find(forge)?;
        let Some((Some(name), kept)) = self.kept(forge).filter(|_| found.source == Source::Keychain) else {
            return Some(found);
        };
        let due = now_anyway || kept.expires.is_some_and(|at| at - now() < RENEW_BEFORE);
        let (true, Some(refresh), Some(client)) = (due, &kept.refresh, &kept.client) else {
            return Some(found);
        };
        let redirect = crate::oauth::redirect_uri(crate::oauth::CALLBACK_PORT);
        match crate::oauth::refresh_grant(transport, &forge.web, client, refresh, &redirect) {
            Ok(grant) => {
                let _ = self.save_grant(forge, &name, &grant, client);
                Some(Credential {
                    token: grant.token,
                    ..found
                })
            }
            Err(_) => Some(found),
        }
    }

    /// Keeps `token`, which `account` typed in, as the app's own for the host of `forge`.
    pub fn save(&self, forge: &Forge, account: &str, token: &str) -> Result<(), SaveError> {
        self.keep(
            forge,
            account,
            &Kept {
                token: token.trim().to_string(),
                refresh: None,
                expires: None,
                client: None,
            },
        )
    }

    /// Keeps what signing in as `account` through the OAuth client `client` gave, for the host of `forge`.
    pub fn save_grant(&self, forge: &Forge, account: &str, grant: &Grant, client: &str) -> Result<(), SaveError> {
        self.keep(
            forge,
            account,
            &Kept {
                token: grant.token.clone(),
                refresh: grant.refresh.clone(),
                expires: grant.expires,
                client: Some(client.to_string()),
            },
        )
    }

    /// Forgets the app's own token for the host of `forge`; tokens of the tools and the environment stay theirs.
    pub fn forget(&self, forge: &Forge) -> Result<(), SecretError> {
        if let Ok(Some(name)) = self.store.get(&host_account(forge)) {
            self.store.delete(&secret_account(forge, &name))?;
        }
        self.store.delete(&host_account(forge))
    }

    /// The account the app signed in with on the host of `forge`, and what it keeps for it. Before accounts, the
    /// host's item held the token itself; such a token still counts, without an account.
    fn kept(&self, forge: &Forge) -> Option<(Option<String>, Kept)> {
        let name = self.store.get(&host_account(forge)).ok()??;
        let Some(secret) = self.store.get(&secret_account(forge, &name)).ok()? else {
            let token = Kept {
                token: name,
                refresh: None,
                expires: None,
                client: None,
            };
            return Some((None, token));
        };
        let kept = serde_json::from_str(&secret).ok()?;
        Some((Some(name), kept))
    }

    fn keep(&self, forge: &Forge, account: &str, kept: &Kept) -> Result<(), SaveError> {
        let text = serde_json::to_string(kept).map_err(SaveError::Encode)?;
        self.store.set(&secret_account(forge, account), &text)?;
        Ok(self.store.set(&host_account(forge), account)?)
    }

    /// The variables the services' tools read, each only for the host it is meant for.
    fn environment_token(&self, forge: &Forge) -> Option<(String, &'static str)> {
        let get = |name: &str| {
            self.environment
                .iter()
                .find(|(n, v)| n == name && !v.trim().is_empty())
                .map(|(_, v)| v.trim().to_string())
        };
        let host_of = |name: &str| get(name).map(|h| bare_host(&h));
        let names: &[&'static str] = match forge.kind {
            Kind::GitHub if forge.host == "github.com" => &["GH_TOKEN", "GITHUB_TOKEN"],
            Kind::GitHub if host_of("GH_HOST").as_deref() == Some(forge.host.as_str()) => {
                &["GH_ENTERPRISE_TOKEN", "GITHUB_ENTERPRISE_TOKEN"]
            }
            Kind::GitLab if host_of("GITLAB_HOST").unwrap_or_else(|| "gitlab.com".into()) == forge.host => {
                &["GITLAB_TOKEN"]
            }
            _ => &[],
        };
        names.iter().find_map(|name| get(name).map(|token| (token, *name)))
    }
}

/// The product's own OAuth client for `forge`: on github.com and gitlab.com, once registered. Other hosts need the
/// id of an OAuth app registered there, which the user enters.
pub fn product_client(forge: &Forge) -> Option<&'static str> {
    let id = match (forge.kind, forge.host.as_str()) {
        (Kind::GitHub, "github.com") => ide_product::GITHUB_OAUTH_CLIENT_ID,
        (Kind::GitLab, "gitlab.com") => ide_product::GITLAB_COM_OAUTH_APPLICATION_ID,
        _ => return None,
    };
    (!id.is_empty()).then_some(id)
}

/// Whether `forge` is on a host where the product's OAuth app lives, rather than one the user names an app for.
pub fn is_public_host(forge: &Forge) -> bool {
    matches!(forge.host.as_str(), "github.com" | "gitlab.com")
}

/// The keychain account that names which account of the host of `forge` the app signed in with.
fn host_account(forge: &Forge) -> String {
    format!("ci:{}:{}", forge.kind.word(), forge.host)
}

/// The keychain account that keeps the token of `account` on the host of `forge`.
fn secret_account(forge: &Forge, account: &str) -> String {
    format!("ci:{}:{}:{account}", forge.kind.word(), forge.host)
}

/// `https://gitlab.example.com/` as `gitlab.example.com`.
fn bare_host(text: &str) -> String {
    let text = text.split_once("://").map_or(text, |(_, rest)| rest);
    text.trim_end_matches('/').to_ascii_lowercase()
}

/// The token `gh auth token` printed.
fn gh_token(printed: &Printed) -> Option<String> {
    let token = printed.stdout.trim();
    (!token.is_empty() && !token.contains(char::is_whitespace)).then(|| token.to_string())
}

/// The token `glab auth status --show-token` printed, as JSON or as text.
fn glab_token(printed: &Printed) -> Option<String> {
    if let Ok(value) = serde_json::from_str::<serde_json::Value>(printed.stdout.trim()) {
        let token = value
            .get("hosts")
            .and_then(|h| h.as_array())
            .and_then(|hosts| hosts.iter().find_map(|h| h.get("token")?.as_str()));
        return token.filter(|t| usable(t)).map(str::to_string);
    }
    format!("{}\n{}", printed.stdout, printed.stderr)
        .lines()
        .find_map(|line| line.split_once("Token:").map(|(_, t)| t.trim().to_string()))
        .filter(|t| usable(t))
}

/// Whether a printed token is one, not a masked placeholder.
fn usable(token: &str) -> bool {
    !token.is_empty() && !token.contains(char::is_whitespace) && token.chars().any(|c| c != '*')
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    use crate::forge::candidates;

    /// Answers each program with what it printed, and keeps what ran.
    #[derive(Default)]
    struct Tools {
        answers: HashMap<&'static str, Printed>,
        ran: Mutex<Vec<String>>,
    }

    impl Commands for Tools {
        fn run(&self, program: &str, args: &[&str]) -> Option<Printed> {
            self.ran.lock().unwrap().push(format!("{program} {}", args.join(" ")));
            self.answers.get(program).cloned()
        }
    }

    fn forge(url: &str) -> Forge {
        candidates(url, &[]).remove(0)
    }

    fn env(pairs: &[(&str, &str)]) -> Vec<(String, String)> {
        pairs.iter().map(|(k, v)| ((*k).into(), (*v).into())).collect()
    }

    #[test]
    fn the_apps_own_token_comes_first_then_the_tool_then_the_environment() {
        let store = Arc::new(Memory::default());
        let tools = Arc::new(Tools {
            answers: HashMap::from([(
                "gh",
                Printed {
                    stdout: "gho_tool\n".into(),
                    stderr: String::new(),
                },
            )]),
            ..Tools::default()
        });
        let auth = Auth::new(store.clone(), tools.clone(), env(&[("GH_TOKEN", "from_env")]));
        let github = forge("git@github.com:acme/widget.git");
        let found = auth.find(&github).unwrap();
        assert_eq!((found.token.as_str(), &found.source), ("gho_tool", &Source::Gh));
        assert_eq!(
            tools.ran.lock().unwrap().as_slice(),
            ["gh auth token --hostname github.com"]
        );
        auth.save(&github, "octo", " own ").unwrap();
        assert_eq!(auth.find(&github).unwrap().source, Source::Keychain);
        assert_eq!(auth.find(&github).unwrap().token, "own");
        assert_eq!(auth.find(&github).unwrap().account.as_deref(), Some("octo"));
        auth.forget(&github).unwrap();
        store.set("ci:github:github.com", "plain").unwrap();
        let earlier = auth.find(&github).unwrap();
        assert_eq!(
            (earlier.token.as_str(), earlier.account, earlier.source),
            ("plain", None, Source::Keychain),
            "a token kept before accounts still counts"
        );
        auth.forget(&github).unwrap();
        let quiet = Auth::new(store, Arc::new(Tools::default()), env(&[("GH_TOKEN", "from_env")]));
        assert_eq!(quiet.find(&github).unwrap().source, Source::Environment("GH_TOKEN"));
        assert!(format!("{:?}", quiet.find(&github).unwrap()).find("from_env").is_none());
    }

    #[test]
    fn environment_tokens_go_only_to_the_host_they_are_meant_for() {
        let auth = Auth::new(
            Arc::new(Memory::default()),
            Arc::new(Tools::default()),
            env(&[
                ("GH_TOKEN", "public"),
                ("GITLAB_TOKEN", "lab"),
                ("GITLAB_HOST", "https://gitlab.example.com/"),
            ]),
        );
        assert_eq!(auth.find(&forge("https://github.corp.example/a/b.git")), None);
        assert_eq!(auth.find(&forge("https://gitlab.com/a/b.git")), None);
        let own = auth.find(&forge("git@gitlab.example.com:group/app.git")).unwrap();
        assert_eq!((own.token.as_str(), own.host.as_str()), ("lab", "gitlab.example.com"));
        let other = Auth::new(
            Arc::new(Memory::default()),
            Arc::new(Tools::default()),
            env(&[("GITLAB_TOKEN", "lab")]),
        );
        assert_eq!(other.find(&forge("https://gitlab.com/a/b.git")).unwrap().token, "lab");
    }

    #[test]
    fn glab_tokens_are_read_from_json_or_text_and_masks_are_no_tokens() {
        let json = Printed {
            stdout: r#"{"hosts":[{"host":"gitlab.com","token":"glpat-abc"}]}"#.into(),
            stderr: String::new(),
        };
        assert_eq!(glab_token(&json).as_deref(), Some("glpat-abc"));
        let text = Printed {
            stdout: String::new(),
            stderr: "gitlab.com\n  ✓ Logged in to gitlab.com as someone\n  ✓ Token: glpat-xyz\n".into(),
        };
        assert_eq!(glab_token(&text).as_deref(), Some("glpat-xyz"));
        let masked = Printed {
            stdout: String::new(),
            stderr: "  ✓ Token: **************************\n".into(),
        };
        assert_eq!(glab_token(&masked), None);
        assert_eq!(gh_token(&Printed::default()), None);
        let github = candidates("git@github.com:a/b.git", &[]).remove(0);
        assert_eq!(product_client(&github), None, "empty until the app is registered");
        assert!(is_public_host(&github));
    }

    /// A folder with the program `fake-tool`, whose shell `body` runs, and a home folder.
    fn tool_folder(name: &str, body: &str) -> (ide_core::testing::Scratch, Vec<(String, String)>) {
        use std::os::unix::fs::PermissionsExt;
        let dir = ide_core::testing::scratch(&format!("ci-tool-{name}"));
        std::fs::create_dir_all(dir.join("home")).unwrap();
        let program = dir.join("fake-tool");
        std::fs::write(&program, format!("#!/bin/sh\n{body}\n")).unwrap();
        std::fs::set_permissions(&program, std::fs::Permissions::from_mode(0o755)).unwrap();
        let path = format!("/nonexistent:{}:/usr/bin:/bin", dir.display());
        let home = dir.join("home").to_string_lossy().into_owned();
        (dir, env(&[("PATH", &path), ("HOME", &home)]))
    }

    #[test]
    fn a_tool_on_the_path_runs_in_the_home_folder_and_one_that_is_not_there_gives_nothing() {
        let (dir, environment) = tool_folder("prints", "pwd; echo \"$1\"; echo warned >&2");
        let tools = Supervised::new(&Supervisor::new(), environment, Duration::from_secs(20));
        let printed = tools.run("fake-tool", &["auth"]).unwrap();
        assert_eq!(printed.stdout, format!("{}\nauth\n", dir.join("home").display()));
        assert_eq!(printed.stderr, "warned\n");
        assert_eq!(tools.run("not-installed-anywhere", &[]), None);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn a_tool_that_does_not_end_in_time_is_stopped_and_gives_nothing() {
        let (dir, environment) = tool_folder("hangs", "exec sleep 30");
        let tools = Supervised::new(&Supervisor::new(), environment, Duration::from_millis(300));
        let started = Instant::now();
        assert_eq!(tools.run("fake-tool", &[]), None);
        assert!(started.elapsed() < Duration::from_secs(10), "{:?}", started.elapsed());
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn signing_out_forgets_the_apps_token_and_an_old_token_without_account_still_counts() {
        let store = Arc::new(Memory::default());
        let auth = Auth::new(store.clone(), Arc::new(Tools::default()), Vec::new());
        let lab = forge("https://gitlab.com/a/b.git");
        auth.save(&lab, "someone", " glpat-typed \n").unwrap();
        let found = auth.find(&lab).unwrap();
        assert_eq!(
            (found.token.as_str(), found.account.as_deref()),
            ("glpat-typed", Some("someone"))
        );
        auth.forget(&lab).unwrap();
        assert_eq!(auth.find(&lab), None);
        store.set(&host_account(&lab), "glpat-from-before").unwrap();
        let old = auth.find(&lab).unwrap();
        assert_eq!(
            (old.token.as_str(), old.account.as_deref()),
            ("glpat-from-before", None)
        );
        auth.forget(&lab).unwrap();
        assert_eq!(store.get(&host_account(&lab)), Ok(None));
    }
}

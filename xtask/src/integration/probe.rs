//! What the steps ask of this Mac: programs on the login shell's PATH, its variables, files, sign-ins and hosts.

use std::cell::RefCell;
use std::collections::HashMap;
use std::net::{TcpStream, ToSocketAddrs};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use ide_process::Supervisor;
use ide_process::shell_env::{capture, user_shell};

/// How long a look at a host or a helper such as `gh auth status` may take.
const PATIENCE: Duration = Duration::from_secs(20);

/// The questions a step's needs ask; the tests answer them without the machine.
pub(crate) trait Probe {
    /// A program on the login shell's PATH.
    fn program(&self, name: &str) -> Option<PathBuf>;
    /// A variable of the login shell.
    fn var(&self, name: &str) -> Option<String>;
    /// A variable of this process, which the tests the runner starts inherit.
    fn process_var(&self, name: &str) -> Option<String>;
    fn home(&self) -> PathBuf;
    fn exists(&self, path: &Path) -> bool;
    /// Whether `host` takes connections on port 443.
    fn reachable(&self, host: &str) -> bool;
    /// Whether `program` with `args` ends well, run with the login shell's environment and its output dropped.
    fn succeeds(&self, program: &Path, args: &[&str]) -> bool;
    /// The URL of the `origin` remote of the clone at `repository`.
    fn origin(&self, repository: &Path) -> Option<String>;
    /// The clones in `~/Repos`, by name.
    fn clones(&self) -> Vec<PathBuf>;
}

/// This Mac, seen with the login shell's environment as the app sees it.
pub(crate) struct Machine {
    env: Vec<(String, String)>,
    reached: RefCell<HashMap<String, bool>>,
}

impl Machine {
    pub(crate) fn new() -> Machine {
        let home = std::env::var_os("HOME").map_or_else(|| PathBuf::from("/"), PathBuf::from);
        let env = capture(&Supervisor::new(), &user_shell(), &home, Duration::from_secs(30)).unwrap_or_else(|e| {
            eprintln!("the login shell's environment could not be read ({e:?}); using this process's");
            std::env::vars().collect()
        });
        Machine {
            env,
            reached: RefCell::new(HashMap::new()),
        }
    }
}

impl Probe for Machine {
    fn program(&self, name: &str) -> Option<PathBuf> {
        self.var("PATH")?
            .split(':')
            .filter(|dir| !dir.is_empty())
            .map(|dir| Path::new(dir).join(name))
            .find(|candidate| is_executable(candidate))
    }

    fn var(&self, name: &str) -> Option<String> {
        self.env
            .iter()
            .find(|(key, _)| key == name)
            .map(|(_, value)| value.clone())
    }

    fn process_var(&self, name: &str) -> Option<String> {
        std::env::var(name).ok()
    }

    fn home(&self) -> PathBuf {
        self.var("HOME")
            .or_else(|| self.process_var("HOME"))
            .map_or_else(|| PathBuf::from("/"), PathBuf::from)
    }

    fn exists(&self, path: &Path) -> bool {
        path.exists()
    }

    fn reachable(&self, host: &str) -> bool {
        if let Some(known) = self.reached.borrow().get(host) {
            return *known;
        }
        let reached = (host, 443)
            .to_socket_addrs()
            .ok()
            .and_then(|mut addresses| addresses.next())
            .is_some_and(|address| TcpStream::connect_timeout(&address, Duration::from_secs(5)).is_ok());
        self.reached.borrow_mut().insert(host.to_string(), reached);
        reached
    }

    fn succeeds(&self, program: &Path, args: &[&str]) -> bool {
        let child = Command::new(program)
            .args(args)
            .current_dir(std::env::temp_dir())
            .env_clear()
            .envs(self.env.iter().map(|(key, value)| (key, value)))
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn();
        let Ok(mut child) = child else { return false };
        let started = Instant::now();
        loop {
            match child.try_wait() {
                Ok(Some(status)) => return status.success(),
                Ok(None) if started.elapsed() < PATIENCE => std::thread::sleep(Duration::from_millis(50)),
                _ => {
                    let _ = child.kill();
                    let _ = child.wait();
                    return false;
                }
            }
        }
    }

    fn origin(&self, repository: &Path) -> Option<String> {
        let output = Command::new(self.program("git")?)
            .arg("-C")
            .arg(repository)
            .args(["remote", "get-url", "origin"])
            .env_clear()
            .envs(self.env.iter().map(|(key, value)| (key, value)))
            .stdin(Stdio::null())
            .stderr(Stdio::null())
            .output()
            .ok()?;
        output
            .status
            .success()
            .then(|| String::from_utf8_lossy(&output.stdout).trim().to_string())
    }

    fn clones(&self) -> Vec<PathBuf> {
        let mut clones: Vec<PathBuf> = std::fs::read_dir(self.home().join("Repos"))
            .map(|entries| {
                entries
                    .filter_map(Result::ok)
                    .map(|entry| entry.path())
                    .filter(|path| path.join(".git").exists())
                    .collect()
            })
            .unwrap_or_default();
        clones.sort();
        clones
    }
}

fn is_executable(path: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    std::fs::metadata(path).is_ok_and(|m| m.is_file() && m.permissions().mode() & 0o111 != 0)
}

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
        Machine::with(env)
    }

    /// This Mac with `env` as the login shell's environment.
    fn with(env: Vec<(String, String)>) -> Machine {
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

#[cfg(test)]
mod tests {
    use std::os::unix::fs::PermissionsExt;

    use super::*;
    use crate::shell::fake::scratch;

    fn machine(home: &Path, path: &str) -> Machine {
        Machine::with(vec![
            ("HOME".to_string(), home.display().to_string()),
            ("PATH".to_string(), path.to_string()),
        ])
    }

    #[test]
    fn programs_are_the_executable_files_on_the_login_shells_path() {
        let dir = scratch("probe-programs");
        let (first, second) = (dir.join("first"), dir.join("second"));
        std::fs::create_dir_all(&first).unwrap();
        std::fs::create_dir_all(&second).unwrap();
        std::fs::write(first.join("tool"), "not executable").unwrap();
        std::fs::write(second.join("tool"), "#!/bin/sh\n").unwrap();
        std::fs::set_permissions(second.join("tool"), std::fs::Permissions::from_mode(0o755)).unwrap();
        std::fs::create_dir_all(first.join("folder")).unwrap();
        let mac = machine(&dir, &format!("{}::{}", first.display(), second.display()));
        assert_eq!(mac.program("tool"), Some(second.join("tool")));
        assert_eq!(mac.program("folder"), None);
        assert_eq!(mac.program("missing"), None);
        assert_eq!(mac.var("HOME"), Some(dir.display().to_string()));
        assert_eq!(mac.var("NOT_SET"), None);
        assert_eq!(mac.home(), dir);
        assert!(mac.exists(&second) && !mac.exists(&dir.join("gone")));
        assert_eq!(Machine::with(Vec::new()).program("sh"), None, "no PATH, no programs");
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn without_a_home_in_the_login_shell_the_processs_home_counts() {
        let mac = Machine::with(Vec::new());
        assert_eq!(
            mac.home(),
            std::env::var_os("HOME").map_or_else(|| PathBuf::from("/"), PathBuf::from)
        );
        assert_eq!(mac.process_var("HOME"), std::env::var("HOME").ok());
    }

    #[test]
    fn a_program_succeeds_with_the_login_shells_environment_only() {
        let dir = scratch("probe-succeeds");
        let mac = Machine::with(vec![("IDE_PROBE".to_string(), "set".to_string())]);
        let sh = Path::new("/bin/sh");
        assert!(mac.succeeds(sh, &["-c", "test \"$IDE_PROBE\" = set"]));
        assert!(
            !mac.succeeds(sh, &["-c", "test -n \"$HOME\""]),
            "the process's own variables are left out"
        );
        assert!(!mac.succeeds(&dir.join("missing"), &[]));
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn a_hosts_reachability_is_asked_once() {
        let mac = Machine::with(Vec::new());
        mac.reached.borrow_mut().insert("example.invalid".to_string(), true);
        assert!(
            mac.reachable("example.invalid"),
            "the remembered answer, without a connection"
        );
    }

    #[test]
    fn the_clones_are_the_git_folders_in_repos_and_their_origin_is_read_with_git() {
        let dir = scratch("probe-clones");
        let repos = dir.join("Repos");
        for name in ["b", "a"] {
            std::fs::create_dir_all(repos.join(name)).unwrap();
            let status = Command::new("git")
                .args(["init", "-q"])
                .current_dir(repos.join(name))
                .status()
                .unwrap();
            assert!(status.success());
        }
        std::fs::create_dir_all(repos.join("plain")).unwrap();
        let path = std::env::var("PATH").unwrap_or_default();
        let mac = machine(&dir, &path);
        assert_eq!(mac.clones(), [repos.join("a"), repos.join("b")]);
        let added = Command::new("git")
            .args(["remote", "add", "origin", "https://example.org/a.git"])
            .current_dir(repos.join("a"))
            .status()
            .unwrap();
        assert!(added.success());
        assert_eq!(
            mac.origin(&repos.join("a")).as_deref(),
            Some("https://example.org/a.git")
        );
        assert_eq!(mac.origin(&repos.join("b")), None, "no origin");
        assert_eq!(machine(&dir, "").origin(&repos.join("a")), None, "no git");
        assert!(machine(&dir.join("nowhere"), &path).clones().is_empty());
        std::fs::remove_dir_all(dir).unwrap();
    }
}

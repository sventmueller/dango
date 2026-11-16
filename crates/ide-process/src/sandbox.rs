//! `sandbox-exec` profiles for commands the app runs on an agent's behalf: the own agent's shell tool (E19) and the
//! gates of a work order (E187), which run code the agent wrote, such as `build.rs`, tests or npm scripts.
//!
//! A profile limits writing to given folders and keeps the network off or on. A gate's profile also hides the home
//! folder except the tool folders builds read, and refuses Unix sockets except the listed ones, so `~/.ssh`, the
//! keychains, other programs' configuration and agents such as `ssh-agent` stay out of reach (C50). `sandbox-exec`
//! filters by path, address and port only, so an allowed network cannot be limited to domains.

use std::path::{Path, PathBuf};
use std::process::Command;

/// The program that runs a command under a profile.
pub const SANDBOX_EXEC: &str = "/usr/bin/sandbox-exec";

/// The folders below the home folder a gate may read, where toolchains, their caches and package stores live.
pub const TOOL_FOLDERS: &[&str] = &[
    ".cargo",
    ".rustup",
    "go",
    ".nvm",
    ".npm",
    ".bun",
    ".deno",
    ".pyenv",
    ".rbenv",
    ".volta",
    ".sdkman",
    ".gradle",
    ".m2",
    ".local/bin",
    ".local/share/pnpm",
    "Library/pnpm",
    "Library/Caches/go-build",
    "Library/Caches/pip",
    "Library/Caches/Yarn",
    "Library/Developer/CommandLineTools",
];

/// Whether a sandboxed command reaches the network.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Network {
    /// No connections and no listening, except the Unix sockets the profile allows.
    Off,
    /// Any host; `sandbox-exec` cannot limit it to a list of domains.
    On,
}

/// A `sandbox-exec` profile.
///
/// ```
/// use std::path::PathBuf;
///
/// use ide_process::sandbox::Profile;
///
/// let profile = Profile::writes_only(&[PathBuf::from("/work/shop")]).render();
/// assert!(profile.contains("(deny network*)"));
/// assert!(profile.contains("(allow file-write* (subpath \"/work/shop\")"));
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Profile {
    writable: Vec<PathBuf>,
    network: Network,
    /// The home folder whose reading is refused, and what below it stays readable.
    hidden_home: Option<(PathBuf, Vec<PathBuf>)>,
    /// The Unix sockets that may be reached; `None` for all of them.
    sockets: Option<Vec<PathBuf>>,
    loopback: bool,
}

impl Profile {
    /// Writing only in `folders` and the terminal devices, the network off except all Unix sockets, reading open: the
    /// own agent's shell tool (E19).
    pub fn writes_only(folders: &[PathBuf]) -> Profile {
        Profile {
            writable: folders.to_vec(),
            network: Network::Off,
            hidden_home: None,
            sockets: None,
            loopback: false,
        }
    }

    /// A gate's profile (E187): writing only in `folders`, such as the worktree and its build folder, and in the run's
    /// own `temp` folder; the network off; the home folder `home` hidden except [`TOOL_FOLDERS`] and the writable
    /// folders; no Unix socket. The paths are resolved as far as they exist, as the sandbox compares resolved paths.
    pub fn gate(folders: &[PathBuf], temp: &Path, home: &Path) -> Profile {
        let home = resolved(home);
        let tools = TOOL_FOLDERS.iter().map(|folder| home.join(folder)).collect();
        let mut writable: Vec<PathBuf> = folders.iter().map(|folder| resolved(folder)).collect();
        writable.push(resolved(temp));
        Profile {
            writable,
            network: Network::Off,
            hidden_home: Some((home, tools)),
            sockets: Some(Vec::new()),
            loopback: false,
        }
    }

    /// The same with the network `network`.
    #[must_use]
    pub fn network(mut self, network: Network) -> Profile {
        self.network = network;
        self
    }

    /// The same with connections to and listening on the loopback address, for tests that start a local server.
    #[must_use]
    pub fn loopback(mut self) -> Profile {
        self.loopback = true;
        self
    }

    /// The same with `paths` below a hidden home folder readable too, such as path dependencies beside the project.
    #[must_use]
    pub fn readable(mut self, paths: &[PathBuf]) -> Profile {
        if let Some((_, readable)) = self.hidden_home.as_mut() {
            readable.extend(paths.iter().map(|path| resolved(path)));
        }
        self
    }

    /// The same with the Unix sockets at `paths` reachable, besides those in the writable folders.
    #[must_use]
    pub fn sockets(mut self, paths: &[PathBuf]) -> Profile {
        self.sockets
            .get_or_insert_with(Vec::new)
            .extend(paths.iter().map(|path| resolved(path)));
        self
    }

    /// `program` under this profile; its arguments follow.
    pub fn command(&self, program: &Path) -> Command {
        let mut command = Command::new(SANDBOX_EXEC);
        command.arg("-p").arg(self.render()).arg(program);
        command
    }

    /// The profile's text.
    pub fn render(&self) -> String {
        let mut text = String::from("(version 1)\n(allow default)\n");
        text.push_str(&self.network_rules());
        if let Some((home, readable)) = &self.hidden_home {
            text.push_str(&home_rules(home, readable, &self.writable));
        }
        let folders: Vec<String> = self
            .writable
            .iter()
            .map(|folder| format!("(subpath \"{}\")", quoted(folder)))
            .collect();
        text.push_str(&format!(
            "(deny file-write*)\n\
             (allow file-write* {} (literal \"/dev/null\") (regex #\"^/dev/tty\") (regex #\"^/dev/fd/\"))\n",
            folders.join(" ")
        ));
        text
    }

    fn network_rules(&self) -> String {
        let mut text = String::new();
        match (self.network, &self.sockets) {
            (Network::Off, None) => text.push_str("(deny network*)\n(allow network* (remote unix-socket))\n"),
            (Network::On, None) => {}
            (Network::Off, Some(_)) => text.push_str("(deny network*)\n"),
            (Network::On, Some(_)) => text.push_str("(deny network* (remote unix-socket) (local unix-socket))\n"),
        }
        if let Some(sockets) = &self.sockets {
            for socket in sockets {
                text.push_str(&format!(
                    "(allow network-outbound (remote unix-socket (path-literal \"{}\")))\n",
                    quoted(socket)
                ));
            }
            for folder in &self.writable {
                let inside = format!("^{}/", regex(folder));
                text.push_str(&format!(
                    "(allow network* (remote unix-socket (path-regex #\"{inside}\")) \
                     (local unix-socket (path-regex #\"{inside}\")))\n"
                ));
            }
        }
        if self.loopback && self.network == Network::Off {
            text.push_str("(allow network* (local ip \"localhost:*\") (remote ip \"localhost:*\"))\n");
        }
        text
    }
}

/// Refuses reading below `home` except `readable` and `writable`, whose folders on the way down stay visible to
/// `stat`, as resolving a path looks at each.
fn home_rules(home: &Path, readable: &[PathBuf], writable: &[PathBuf]) -> String {
    let open: Vec<&PathBuf> = readable
        .iter()
        .chain(writable)
        .filter(|path| path.starts_with(home))
        .collect();
    let mut ancestors: Vec<PathBuf> = vec![home.to_path_buf()];
    for path in &open {
        let mut at = path.parent();
        while let Some(folder) = at.filter(|folder| folder.starts_with(home) && *folder != home) {
            ancestors.push(folder.to_path_buf());
            at = folder.parent();
        }
    }
    ancestors.sort();
    ancestors.dedup();
    let literals: Vec<String> = ancestors
        .iter()
        .map(|folder| format!("(literal \"{}\")", quoted(folder)))
        .collect();
    let mut text = format!(
        "(deny file-read* (subpath \"{}\"))\n(allow file-read-metadata {})\n",
        quoted(home),
        literals.join(" ")
    );
    if !open.is_empty() {
        let subpaths: Vec<String> = open
            .iter()
            .map(|path| format!("(subpath \"{}\")", quoted(path)))
            .collect();
        text.push_str(&format!("(allow file-read* {})\n", subpaths.join(" ")));
    }
    text
}

/// `path` inside a profile's string.
fn quoted(path: &Path) -> String {
    path.display().to_string().replace('\\', "\\\\").replace('"', "\\\"")
}

/// `path` as a regular expression that matches only itself, inside a profile's `#"…"` string.
fn regex(path: &Path) -> String {
    let mut out = String::new();
    for c in path.display().to_string().chars() {
        if "\\.^$|?*+()[]{}".contains(c) {
            out.push('\\');
        }
        if c == '"' {
            out.push('\\');
        }
        out.push(c);
    }
    out
}

/// `path` with symbolic links resolved through its longest existing part, such as `/tmp` to `/private/tmp`.
fn resolved(path: &Path) -> PathBuf {
    let mut existing = path.to_path_buf();
    let mut rest = Vec::new();
    while !existing.exists() {
        let Some(name) = existing.file_name().map(std::ffi::OsStr::to_os_string) else {
            break;
        };
        rest.push(name);
        if !existing.pop() {
            break;
        }
    }
    let mut out = existing.canonicalize().unwrap_or(existing);
    for name in rest.into_iter().rev() {
        out.push(name);
    }
    out
}

#[cfg(test)]
mod tests {
    use std::os::unix::net::UnixListener;
    use std::process::Stdio;

    use super::*;

    #[test]
    fn the_own_agents_profile_is_what_it_always_was() {
        let profile = Profile::writes_only(&[PathBuf::from("/w/my \"shop\""), PathBuf::from("/tmp/x")]).render();
        assert_eq!(
            profile,
            "(version 1)\n(allow default)\n(deny network*)\n(allow network* (remote unix-socket))\n\
             (deny file-write*)\n(allow file-write* (subpath \"/w/my \\\"shop\\\"\") (subpath \"/tmp/x\") \
             (literal \"/dev/null\") (regex #\"^/dev/tty\") (regex #\"^/dev/fd/\"))\n"
        );
    }

    #[test]
    fn a_gate_hides_the_home_folder_but_its_tools_and_its_own_folders() {
        let profile = Profile::gate(
            &[PathBuf::from("/Users/a/Repos/shop-order")],
            Path::new("/nonexistent-temp/run-1"),
            Path::new("/Users/a"),
        )
        .readable(&[PathBuf::from("/Users/a/Repos/splat")])
        .render();
        assert!(
            profile.contains("(deny file-read* (subpath \"/Users/a\"))"),
            "{profile}"
        );
        assert!(profile.contains("(subpath \"/Users/a/.cargo\")"), "{profile}");
        assert!(profile.contains("(subpath \"/Users/a/Repos/splat\")"), "{profile}");
        assert!(
            profile.contains("(allow file-read-metadata (literal \"/Users/a\") (literal \"/Users/a/.local\")"),
            "{profile}"
        );
        assert!(profile.contains("(literal \"/Users/a/Repos\")"), "{profile}");
        assert!(profile.contains("(deny network*)\n"), "{profile}");
        assert!(
            !profile.contains("(allow network* (remote unix-socket))"),
            "no socket is open: {profile}"
        );
        assert!(profile.contains("(subpath \"/nonexistent-temp/run-1\")"), "{profile}");
    }

    #[test]
    fn with_the_network_on_unix_sockets_stay_closed_except_the_listed() {
        let profile = Profile::gate(&[PathBuf::from("/w")], Path::new("/t"), Path::new("/h"))
            .network(Network::On)
            .sockets(&[PathBuf::from("/var/run/x.sock")])
            .render();
        assert!(
            profile.contains("(deny network* (remote unix-socket) (local unix-socket))"),
            "{profile}"
        );
        assert!(profile.contains("(path-literal \"/private/var/run/x.sock\")"), "{profile}");
        assert!(profile.contains("(path-regex #\"^/w/\")"), "{profile}");
        assert!(!profile.contains("(deny network*)\n"), "{profile}");
    }

    #[test]
    fn paths_in_a_regex_match_only_themselves() {
        assert_eq!(regex(Path::new("/a.b/c+d")), "/a\\.b/c\\+d");
    }

    /// Runs `script` with `/bin/sh` under `profile`; whether it exited with 0.
    fn sandboxed(profile: &Profile, script: &str) -> bool {
        profile
            .command(Path::new("/bin/sh"))
            .args(["-c", script])
            .env_clear()
            .env("PATH", "/usr/bin:/bin")
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .unwrap()
            .success()
    }

    #[test]
    fn a_gate_reads_neither_secrets_in_the_home_folder_nor_a_socket_outside_its_list() {
        let scratch = ide_core::testing::scratch("sandbox-gate");
        let home = scratch.join("home");
        for folder in ["home/.ssh", "home/.cargo", "home/Repos/shop", "temp", "outside"] {
            std::fs::create_dir_all(scratch.join(folder)).unwrap();
        }
        std::fs::write(home.join(".ssh/id_ed25519"), "secret").unwrap();
        std::fs::write(home.join("Repos/.env"), "TOKEN=x").unwrap();
        std::fs::write(home.join(".cargo/config.toml"), "").unwrap();
        let worktree = home.join("Repos/shop");
        let profile = Profile::gate(std::slice::from_ref(&worktree), &scratch.join("temp"), &home);
        let path = |p: &Path| p.display().to_string();
        let read = |p: &Path| format!("cat '{}'", path(p));
        assert!(!sandboxed(&profile, &read(&home.join(".ssh/id_ed25519"))));
        assert!(!sandboxed(&profile, &read(&home.join("Repos/.env"))));
        assert!(sandboxed(&profile, &read(&home.join(".cargo/config.toml"))));
        assert!(sandboxed(
            &profile,
            &format!("cd '{}' && echo x > made", path(&worktree))
        ));
        assert!(sandboxed(
            &profile,
            &format!("echo x > '{}/made'", path(&scratch.join("temp")))
        ));
        assert!(!sandboxed(
            &profile,
            &format!("echo x > '{}/made'", path(&scratch.join("outside")))
        ));

        let socket = scratch.join("outside/agent.sock");
        let _listener = UnixListener::bind(&socket).unwrap();
        let connect = format!(
            "/usr/bin/perl -MIO::Socket::UNIX -e 'IO::Socket::UNIX->new(Peer => $ARGV[0]) or exit 3' '{}'",
            path(&socket)
        );
        assert!(!sandboxed(&profile, &connect), "a socket outside the list stays closed");
        assert!(sandboxed(
            &profile.clone().sockets(std::slice::from_ref(&socket)),
            &connect
        ));
    }
}

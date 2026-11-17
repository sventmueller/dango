//! `sandbox-exec` profiles for commands the app runs on an agent's behalf: the own agent's shell tool (E19) and the
//! gates of a work order (E187), which run code the agent wrote, such as `build.rs`, tests or npm scripts.
//!
//! A profile limits writing to given folders and keeps the network off or on. A gate's profile also hides the home
//! folder except the tool folders builds read, other users' folders, mounted volumes, the user's temporary folder and
//! Homebrew's configuration; keeps credential files unreadable even inside the tool folders; refuses Unix sockets
//! except the listed ones and those the gate makes in its own temporary folder; keeps the `.git` of its folders
//! unwritable; and closes the ways to hand work to a process outside the sandbox: Apple events, LaunchServices, every
//! Mach service but [`MACH_SERVICES`], and the programs of [`REFUSED_PROGRAMS`]. So `~/.ssh`, the keychains, other
//! programs' configuration and agents such as `ssh-agent` stay out of reach (C50). `sandbox-exec` filters by path,
//! address and port only, so an allowed network cannot be limited to domains.

use std::ffi::OsStr;
use std::os::unix::ffi::OsStrExt;
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

/// The files below the home folder that hold credentials, which a gate never reads, also inside [`TOOL_FOLDERS`].
pub const CREDENTIAL_FILES: &[&str] = &[
    ".cargo/credentials",
    ".cargo/credentials.toml",
    ".m2/settings.xml",
    ".m2/settings-security.xml",
    ".gradle/gradle.properties",
    ".npmrc",
    ".netrc",
];

/// Where Homebrew keeps its configuration, in `etc`, on Apple silicon and on Intel.
const HOMEBREW_PREFIXES: &[&str] = &["/opt/homebrew", "/usr/local"];

/// What a toolchain reads of Homebrew's `etc`: certificates, OpenSSL's, clang's, rustup's and git's configuration.
const HOMEBREW_CONFIG: &[&str] = &[
    "ca-certificates",
    "openssl@1.1",
    "openssl@3",
    "openssl@4",
    "clang",
    "rustup",
    "gitconfig",
];

/// The Mach services every gate reaches: notifications, user and group lookups, logging, the helper that hands out the
/// per-user folders, fonts and file events. Any other service, such as the keychain's (`com.apple.SecurityServer`,
/// `com.apple.securityd.xpc`), LaunchServices, preferences, the pasteboard or the window server, does work outside the
/// sandbox for whoever asks, so it stays closed.
pub const MACH_SERVICES: &[&str] = &[
    "com.apple.system.notification_center",
    "com.apple.system.opendirectoryd.libinfo",
    "com.apple.system.opendirectoryd.membership",
    "com.apple.system.DirectoryService.libinfo_v1",
    "com.apple.system.DirectoryService.membership_v1",
    "com.apple.system.logger",
    "com.apple.logd",
    "com.apple.bsd.dirhelper",
    "com.apple.FSEvents",
    "com.apple.fonts",
];

/// The Mach services a gate with the network on reaches as well: name resolution, the network configuration and
/// certificate trust, which TLS clients such as Go's ask the system for. Each reaches the network on the gate's behalf,
/// so with the network off they stay closed.
pub const NETWORK_SERVICES: &[&str] = &[
    "com.apple.dnssd.service",
    "com.apple.SystemConfiguration.DNSConfiguration",
    "com.apple.SystemConfiguration.configd",
    "com.apple.trustd",
    "com.apple.trustd.agent",
    "com.apple.TrustEvaluationAgent",
];

/// The socket the system resolves host names through, which a profile with the network on and closed Unix sockets
/// still opens.
const NAME_RESOLVER: &str = "/private/var/run/mDNSResponder";

/// Programs a gate may not start, which hand work to processes outside the sandbox: opening documents and addresses,
/// jobs for launchd, scripts for other apps, the keychain, the pasteboard and the screen. A copy of one still fails, as
/// the Mach services it needs are closed.
pub const REFUSED_PROGRAMS: &[&str] = &[
    "/usr/bin/open",
    "/bin/launchctl",
    "/usr/bin/osascript",
    "/usr/bin/security",
    "/usr/bin/automator",
    "/usr/bin/shortcuts",
    "/usr/bin/pbcopy",
    "/usr/bin/pbpaste",
    "/usr/sbin/screencapture",
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
    /// What a gate's profile adds; `None` for the own agent's.
    confined: Option<Confined>,
    /// The Unix sockets that may be reached; `None` for all of them.
    sockets: Option<Vec<PathBuf>>,
    loopback: bool,
}

/// The rules a gate's profile adds to the writing and network rules.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Confined {
    /// The folders whose reading is refused.
    hidden: Vec<PathBuf>,
    /// What below them stays readable.
    readable: Vec<PathBuf>,
    /// Files and folders that stay unreadable even inside readable folders.
    unreadable: Vec<PathBuf>,
    /// Links below them that a path to a readable folder passes through.
    links: Vec<PathBuf>,
    /// The `.git` of each folder the gate writes in, which stays unwritable.
    pointers: Vec<PathBuf>,
    /// The run's own temporary folder, the one folder where the gate makes and reaches Unix sockets.
    temp: PathBuf,
}

impl Profile {
    /// Writing only in `folders` and the terminal devices, the network off except all Unix sockets, reading open: the
    /// own agent's shell tool (E19).
    pub fn writes_only(folders: &[PathBuf]) -> Profile {
        Profile {
            writable: folders.to_vec(),
            network: Network::Off,
            confined: None,
            sockets: None,
            loopback: false,
        }
    }

    /// A gate's profile (E187): writing only in `folders`, such as the worktree and its build folder, except their
    /// `.git`, and in the run's own `temp` folder; the network off; the home folder `home` hidden except
    /// [`TOOL_FOLDERS`] and the writable folders, and [`CREDENTIAL_FILES`] hidden in any case; no Unix socket but
    /// those in `temp`; see the module for the rest. The paths are resolved as far as they exist, as the sandbox
    /// compares resolved paths.
    pub fn gate(folders: &[PathBuf], temp: &Path, home: &Path) -> Profile {
        let home = resolved(home);
        let folders: Vec<PathBuf> = folders.iter().map(|folder| resolved(folder)).collect();
        let temp = resolved(temp);
        let mut hidden = vec![home.clone(), PathBuf::from("/Users"), PathBuf::from("/Volumes")];
        hidden.extend(user_folder());
        let mut readable: Vec<PathBuf> = TOOL_FOLDERS.iter().map(|folder| home.join(folder)).collect();
        let mut unreadable: Vec<PathBuf> = CREDENTIAL_FILES.iter().map(|file| home.join(file)).collect();
        for prefix in HOMEBREW_PREFIXES {
            let etc = resolved(&Path::new(prefix).join("etc"));
            if etc.is_dir() {
                readable.extend(HOMEBREW_CONFIG.iter().map(|name| etc.join(name)));
                unreadable.extend(HOMEBREW_CONFIG.iter().map(|name| etc.join(name).join("private")));
                hidden.push(etc);
            }
        }
        let pointers = folders.iter().map(|folder| folder.join(".git")).collect();
        let mut writable = folders;
        writable.push(temp.clone());
        Profile {
            writable,
            network: Network::Off,
            confined: Some(Confined {
                hidden,
                readable,
                unreadable,
                links: Vec::new(),
                pointers,
                temp,
            }),
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

    /// The same with `paths` below a hidden folder readable too, such as path dependencies beside the project.
    #[must_use]
    pub fn readable(mut self, paths: &[PathBuf]) -> Profile {
        if let Some(confined) = self.confined.as_mut() {
            confined.readable.extend(paths.iter().map(|path| resolved(path)));
        }
        self
    }

    /// The same with the links at `paths` below a hidden folder followable, such as those beside a worktree to its
    /// path dependencies: the sandbox refuses a path through a link it may not read, whatever the link leads to.
    #[must_use]
    pub fn links(mut self, paths: &[PathBuf]) -> Profile {
        if let Some(confined) = self.confined.as_mut() {
            confined
                .links
                .extend(paths.iter().map(|path| match (path.parent(), path.file_name()) {
                    (Some(parent), Some(name)) => resolved(parent).join(name),
                    _ => path.clone(),
                }));
        }
        self
    }

    /// The same with the files or folders at `paths` unreadable even where they lie in a readable folder, such as the
    /// configuration of the repository's shared git folder, which can hold a remote's address with its token.
    #[must_use]
    pub fn unreadable(mut self, paths: &[PathBuf]) -> Profile {
        if let Some(confined) = self.confined.as_mut() {
            confined.unreadable.extend(paths.iter().map(|path| resolved(path)));
        }
        self
    }

    /// The same with the Unix sockets at `paths` reachable, besides those in the gate's temporary folder, or for the
    /// own agent's profile, in its writable folders.
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
        if let Some(confined) = &self.confined {
            text.push_str(&confined.read_rules(&self.writable));
        }
        let folders: Vec<String> = self.writable.iter().map(|folder| subpath(folder)).collect();
        text.push_str(&format!(
            "(deny file-write*)\n\
             (allow file-write* {} (literal \"/dev/null\") (regex #\"^/dev/tty\") (regex #\"^/dev/fd/\"))\n",
            folders.join(" ")
        ));
        if let Some(confined) = &self.confined {
            text.push_str(&confined.closed_rules(self.network));
        }
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
            if self.network == Network::On {
                text.push_str(&format!(
                    "(allow network-outbound (remote unix-socket (path-literal \"{NAME_RESOLVER}\")))\n"
                ));
            }
            for socket in sockets {
                text.push_str(&format!(
                    "(allow network-outbound (remote unix-socket (path-literal \"{}\")))\n",
                    quoted(socket)
                ));
            }
            let own: Vec<&PathBuf> = match &self.confined {
                Some(confined) => vec![&confined.temp],
                None => self.writable.iter().collect(),
            };
            for folder in own {
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

impl Confined {
    /// Refuses reading below the hidden folders except the readable and `writable` paths, whose folders on the way
    /// down stay visible to `stat`, as resolving a path looks at each; then refuses the unreadable ones.
    fn read_rules(&self, writable: &[PathBuf]) -> String {
        let below_hidden = |path: &Path| self.hidden.iter().any(|folder| path.starts_with(folder));
        let open: Vec<&PathBuf> = self
            .readable
            .iter()
            .chain(writable)
            .filter(|path| below_hidden(path))
            .collect();
        let mut ancestors: Vec<PathBuf> = self.hidden.clone();
        ancestors.extend(self.links.iter().filter(|link| below_hidden(link)).cloned());
        for path in open.iter().copied().chain(&self.links) {
            let mut at = path.parent();
            while let Some(folder) = at.filter(|folder| below_hidden(folder)) {
                ancestors.push(folder.to_path_buf());
                at = folder.parent();
            }
        }
        ancestors.sort();
        ancestors.dedup();
        let mut text: String = self
            .hidden
            .iter()
            .map(|folder| format!("(deny file-read* {})\n", subpath(folder)))
            .collect();
        let literals: Vec<String> = ancestors
            .iter()
            .map(|folder| format!("(literal \"{}\")", quoted(folder)))
            .collect();
        text.push_str(&format!("(allow file-read-metadata {})\n", literals.join(" ")));
        if !open.is_empty() {
            let subpaths: Vec<String> = open.iter().map(|path| subpath(path)).collect();
            text.push_str(&format!("(allow file-read* {})\n", subpaths.join(" ")));
        }
        if !self.unreadable.is_empty() {
            let subpaths: Vec<String> = self.unreadable.iter().map(|path| subpath(path)).collect();
            text.push_str(&format!("(deny file-read* {})\n", subpaths.join(" ")));
        }
        text
    }

    /// Keeps the `.git` of the writable folders unwritable, so no gate points the orchestrator's git at a
    /// configuration of its own (E192), and closes every way to have a process outside the sandbox act for the gate.
    fn closed_rules(&self, network: Network) -> String {
        let mut text = String::new();
        if !self.pointers.is_empty() {
            let pointers: Vec<String> = self.pointers.iter().map(|path| subpath(path)).collect();
            text.push_str(&format!("(deny file-write* {})\n", pointers.join(" ")));
        }
        text.push_str("(deny appleevent-send)\n(deny lsopen)\n(deny mach-lookup)\n");
        let mut services: Vec<&str> = MACH_SERVICES.to_vec();
        if network == Network::On {
            services.extend_from_slice(NETWORK_SERVICES);
        }
        let names: Vec<String> = services
            .iter()
            .map(|name| format!("(global-name \"{name}\")"))
            .collect();
        text.push_str(&format!("(allow mach-lookup {})\n", names.join(" ")));
        let programs: Vec<String> = REFUSED_PROGRAMS
            .iter()
            .map(|program| format!("(literal \"{program}\")"))
            .collect();
        text.push_str(&format!("(deny process-exec {})\n", programs.join(" ")));
        text
    }
}

/// `(subpath "<path>")`.
fn subpath(path: &Path) -> String {
    format!("(subpath \"{}\")", quoted(path))
}

/// The user's own folder below `/var/folders`, which holds their temporary files and caches, resolved.
fn user_folder() -> Option<PathBuf> {
    let mut buffer = [0u8; 1024];
    // SAFETY: confstr writes at most `buffer.len()` bytes, the terminating NUL included, into the buffer it is handed,
    // which lives until the call returned.
    let length = unsafe { libc::confstr(libc::_CS_DARWIN_USER_TEMP_DIR, buffer.as_mut_ptr().cast(), buffer.len()) };
    if length == 0 || length > buffer.len() {
        return None;
    }
    let temp = Path::new(OsStr::from_bytes(&buffer[..length - 1]));
    let folder = resolved(temp.parent()?);
    folder.starts_with("/private/var/folders/").then_some(folder)
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
        .unreadable(&[PathBuf::from("/Users/a/Repos/shop/.git/config")])
        .render();
        for hidden in ["/Users/a", "/Users", "/Volumes"] {
            assert!(
                profile.contains(&format!("(deny file-read* (subpath \"{hidden}\"))")),
                "{profile}"
            );
        }
        assert!(profile.contains("(subpath \"/Users/a/.cargo\")"), "{profile}");
        assert!(profile.contains("(subpath \"/Users/a/Repos/splat\")"), "{profile}");
        assert!(
            profile.contains(
                "(allow file-read-metadata (literal \"/Users\") (literal \"/Users/a\") (literal \"/Users/a/.local\")"
            ),
            "{profile}"
        );
        assert!(profile.contains("(literal \"/Users/a/Repos\")"), "{profile}");
        assert!(
            profile.contains(
                "(deny file-read* (subpath \"/Users/a/.cargo/credentials\") (subpath \"/Users/a/.cargo/credentials.toml\")"
            ),
            "{profile}"
        );
        assert!(
            profile.contains("(subpath \"/Users/a/Repos/shop/.git/config\"))\n"),
            "{profile}"
        );
        assert!(profile.contains("(deny network*)\n"), "{profile}");
        assert!(
            !profile.contains("(allow network* (remote unix-socket))"),
            "no socket is open: {profile}"
        );
        assert!(profile.contains("(subpath \"/nonexistent-temp/run-1\")"), "{profile}");
        assert!(
            profile.contains("(deny file-write* (subpath \"/Users/a/Repos/shop-order/.git\"))"),
            "{profile}"
        );
    }

    #[test]
    fn a_gate_reaches_no_service_that_works_for_it_outside_the_sandbox() {
        let gate = Profile::gate(&[PathBuf::from("/w")], Path::new("/t"), Path::new("/h"));
        let off = gate.render();
        for rule in [
            "(deny appleevent-send)\n",
            "(deny lsopen)\n",
            "(deny mach-lookup)\n",
            "(global-name \"com.apple.system.opendirectoryd.libinfo\")",
            "(literal \"/usr/bin/open\")",
            "(literal \"/usr/bin/security\")",
            "(literal \"/bin/launchctl\")",
            "(literal \"/usr/bin/osascript\")",
        ] {
            assert!(off.contains(rule), "{rule}: {off}");
        }
        for closed in [
            "com.apple.SecurityServer",
            "launchservicesd",
            "pasteboard",
            "com.apple.dnssd.service",
        ] {
            assert!(!off.contains(closed), "{closed}: {off}");
        }
        assert!(!off.contains("mDNSResponder"), "{off}");
        let on = gate.network(Network::On).render();
        assert!(on.contains("(global-name \"com.apple.dnssd.service\")"), "{on}");
        assert!(on.contains("(path-literal \"/private/var/run/mDNSResponder\")"), "{on}");
        assert!(!on.contains("com.apple.SecurityServer"), "{on}");
    }

    #[test]
    fn with_the_network_on_unix_sockets_stay_closed_except_the_listed_and_the_temporary_folders() {
        let profile = Profile::gate(&[PathBuf::from("/w")], Path::new("/t"), Path::new("/h"))
            .network(Network::On)
            .sockets(&[PathBuf::from("/var/run/x.sock")])
            .render();
        assert!(
            profile.contains("(deny network* (remote unix-socket) (local unix-socket))"),
            "{profile}"
        );
        assert!(
            profile.contains("(path-literal \"/private/var/run/x.sock\")"),
            "{profile}"
        );
        assert!(profile.contains("(path-regex #\"^/t/\")"), "{profile}");
        assert!(!profile.contains("(path-regex #\"^/w/\")"), "{profile}");
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

    /// A shell command that connects to the Unix socket at `socket`.
    fn connect(socket: &Path) -> String {
        format!(
            "/usr/bin/perl -MIO::Socket::UNIX -e 'IO::Socket::UNIX->new(Peer => $ARGV[0]) or exit 3' '{}'",
            socket.display()
        )
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
        std::fs::write(home.join(".cargo/credentials.toml"), "token = \"x\"").unwrap();
        let worktree = home.join("Repos/shop");
        std::fs::write(worktree.join(".git"), "gitdir: /elsewhere\n").unwrap();
        let profile = Profile::gate(std::slice::from_ref(&worktree), &scratch.join("temp"), &home);
        let path = |p: &Path| p.display().to_string();
        let read = |p: &Path| format!("cat '{}'", path(p));
        assert!(!sandboxed(&profile, &read(&home.join(".ssh/id_ed25519"))));
        assert!(!sandboxed(&profile, &read(&home.join("Repos/.env"))));
        assert!(sandboxed(&profile, &read(&home.join(".cargo/config.toml"))));
        assert!(!sandboxed(&profile, &read(&home.join(".cargo/credentials.toml"))));
        assert!(!sandboxed(&profile, "ls /Users/Shared"));
        assert!(sandboxed(
            &profile,
            &format!("cd '{}' && echo x > made", path(&worktree))
        ));
        for change in ["echo 'gitdir: .x' > .git", "rm -f .git", "mv .git moved"] {
            assert!(
                !sandboxed(&profile, &format!("cd '{}' && {change}", path(&worktree))),
                "{change}"
            );
        }
        assert_eq!(
            std::fs::read_to_string(worktree.join(".git")).unwrap(),
            "gitdir: /elsewhere\n"
        );
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
        assert!(
            !sandboxed(&profile, &connect(&socket)),
            "a socket outside the list stays closed"
        );
        assert!(sandboxed(
            &profile.clone().sockets(std::slice::from_ref(&socket)),
            &connect(&socket)
        ));
    }

    #[test]
    fn a_gate_reaches_sockets_of_its_own_temporary_folder_but_not_those_in_its_worktree() {
        let scratch = ide_core::testing::scratch("sandbox-sockets");
        let (work, temp, home) = (scratch.join("w"), scratch.join("t"), scratch.join("h"));
        for folder in [&work, &temp, &home] {
            std::fs::create_dir_all(folder).unwrap();
        }
        let profile = Profile::gate(std::slice::from_ref(&work), &temp, &home);
        let server = work.join("s.sock");
        let _listener = UnixListener::bind(&server).unwrap();
        assert!(
            !sandboxed(&profile, &connect(&server)),
            "a server's socket in the worktree stays closed"
        );
        let own = format!(
            "/usr/bin/perl -MIO::Socket::UNIX -e '$l = IO::Socket::UNIX->new(Local => $ARGV[0], Listen => 1) or exit 3; \
             IO::Socket::UNIX->new(Peer => $ARGV[0]) or exit 4' '{}'",
            temp.join("o.sock").display()
        );
        assert!(
            sandboxed(&profile, &own),
            "a socket the gate made in its temporary folder"
        );
    }

    #[test]
    fn a_gate_starts_no_program_that_hands_work_outside() {
        let scratch = ide_core::testing::scratch("sandbox-programs");
        let (work, temp, home) = (scratch.join("w"), scratch.join("t"), scratch.join("h"));
        for folder in [&work, &temp, &home] {
            std::fs::create_dir_all(folder).unwrap();
        }
        let profile = Profile::gate(std::slice::from_ref(&work), &temp, &home);
        assert!(sandboxed(&profile, "/usr/bin/true"));
        for refused in [
            "/usr/bin/open https://example.com",
            "/usr/bin/security find-generic-password -s x",
            "/bin/launchctl list",
            "/usr/bin/osascript -e 'return 1'",
        ] {
            assert!(!sandboxed(&profile, refused), "{refused}");
        }
    }
}

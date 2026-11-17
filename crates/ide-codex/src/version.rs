//! Which Codex is installed: `codex --version`, and whether this build was tested against it (E147). An older one
//! gets the ACP adapter instead.

use std::io::Read;
use std::path::Path;
use std::process::{Command, Stdio};
use std::time::Duration;

use ide_process::{Kind, Supervisor};

use crate::session::SessionError;

/// The oldest release the protocol here was taken from and tested against (`docs/codex.md`).
pub const TESTED: Version = Version {
    major: 0,
    minor: 167,
    patch: 0,
};

/// How long `codex --version` may take.
const PROBE_TIMEOUT: Duration = Duration::from_secs(10);

/// A release of Codex.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Version {
    /// The major number.
    pub major: u32,
    /// The minor number.
    pub minor: u32,
    /// The patch number.
    pub patch: u32,
}

impl Version {
    /// The first `x.y.z` in `output`, such as the `codex-cli 0.167.0` that `codex --version` prints; a pre-release
    /// suffix is dropped.
    pub fn parse(output: &str) -> Option<Version> {
        output.split_whitespace().find_map(|word| {
            let core = word.trim_start_matches('v').split(['-', '+']).next()?;
            let mut parts = core.split('.').map(|p| p.parse::<u32>().ok());
            let (Some(Some(major)), Some(Some(minor)), Some(Some(patch)), None) =
                (parts.next(), parts.next(), parts.next(), parts.next())
            else {
                return None;
            };
            Some(Version { major, minor, patch })
        })
    }

    /// Whether the app-server protocol of this release is one this build speaks.
    pub fn is_supported(self) -> bool {
        self >= TESTED
    }
}

impl std::fmt::Display for Version {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}.{}.{}", self.major, self.minor, self.patch)
    }
}

/// `found` when this build speaks its protocol, else why not.
pub fn check(found: Option<Version>) -> Result<Version, SessionError> {
    match found {
        Some(version) if version.is_supported() => Ok(version),
        found => Err(SessionError::Unsupported { found, tested: TESTED }),
    }
}

/// Runs `program --version` with the environment `env` through `supervisor` and checks the release it names; this
/// waits for the process, so it runs off the main thread.
pub fn probe(supervisor: &Supervisor, program: &Path, env: &[(String, String)]) -> Result<Version, SessionError> {
    let output = run(supervisor, program, env, &["--version"], "codex --version")?;
    check(Version::parse(&output))
}

/// The names of every MCP server Codex's configuration starts, from `codex mcp list --json`, so an order can switch
/// each off by name (E191); this waits for the process, so it runs off the main thread.
pub fn mcp_servers(
    supervisor: &Supervisor,
    program: &Path,
    env: &[(String, String)],
) -> Result<Vec<String>, SessionError> {
    let output = run(supervisor, program, env, &["mcp", "list", "--json"], "codex mcp list")?;
    let listed: serde_json::Value =
        serde_json::from_str(&output).map_err(|e| SessionError::Start(format!("codex mcp list: {e}")))?;
    let servers = listed
        .as_array()
        .ok_or_else(|| SessionError::Start("codex mcp list: no list".into()))?;
    // Only servers of the configuration file take `enabled = false`; for one a plugin brings, Codex then refuses its
    // whole configuration ("invalid transport"). Plugins are off in an order anyway.
    let configured = configured_servers(env);
    Ok(servers
        .iter()
        .filter_map(|server| server["name"].as_str())
        .filter(|name| configured.iter().any(|c| c == name))
        .map(str::to_string)
        .collect())
}

/// The names of the MCP servers Codex's configuration file under `CODEX_HOME` or `~/.codex` defines.
fn configured_servers(env: &[(String, String)]) -> Vec<String> {
    let var = |name: &str| env.iter().find(|(k, _)| k == name).map(|(_, v)| v.clone());
    let home = var("CODEX_HOME")
        .map(std::path::PathBuf::from)
        .or_else(|| var("HOME").map(|home| Path::new(&home).join(".codex")));
    let Some(text) = home.and_then(|home| std::fs::read_to_string(home.join("config.toml")).ok()) else {
        return Vec::new();
    };
    server_names(&text)
}

/// The servers a `config.toml` defines in its `mcp_servers` table, however TOML spells that: `[mcp_servers.<name>]`
/// headers with or without quotes, spaces and comments, keys of an `[mcp_servers]` table, or dotted keys.
fn server_names(text: &str) -> Vec<String> {
    let mut names: Vec<String> = Vec::new();
    let mut table: Vec<String> = Vec::new();
    for line in text.lines() {
        let line = without_comment(line).trim();
        let path = if let Some(header) = line.strip_prefix('[') {
            table = key_path(header.trim_start_matches('[').split(']').next().unwrap_or_default());
            table.clone()
        } else if let Some((key, _)) = line.split_once('=') {
            table.iter().cloned().chain(key_path(key)).collect()
        } else {
            continue;
        };
        // A name with other characters would not survive the dotted `-c` path that switches it off.
        if let [first, name, ..] = path.as_slice()
            && first == "mcp_servers"
            && !name.is_empty()
            && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
            && !names.contains(name)
        {
            names.push(name.clone());
        }
    }
    names
}

/// `line` up to a `#` outside quotes.
fn without_comment(line: &str) -> &str {
    let mut quote = None;
    for (at, c) in line.char_indices() {
        match (quote, c) {
            (None, '#') => return &line[..at],
            (None, '"' | '\'') => quote = Some(c),
            (Some(open), c) if c == open => quote = None,
            _ => {}
        }
    }
    line
}

/// The parts of a dotted TOML key, without their quotes.
fn key_path(key: &str) -> Vec<String> {
    let mut parts = Vec::new();
    let mut part = String::new();
    let mut quote = None;
    for c in key.chars() {
        match (quote, c) {
            (Some(open), c) if c == open => quote = None,
            (Some(_), c) => part.push(c),
            (None, '"' | '\'') => quote = Some(c),
            (None, '.') => parts.push(std::mem::take(&mut part).trim().to_string()),
            (None, c) => part.push(c),
        }
    }
    parts.push(part.trim().to_string());
    parts
}

/// The standard output of `program args` with the environment `env`, ended after the probe's time limit.
fn run(
    supervisor: &Supervisor,
    program: &Path,
    env: &[(String, String)],
    args: &[&str],
    label: &str,
) -> Result<String, SessionError> {
    if !program.is_file() {
        return Err(SessionError::NotFound(program.to_path_buf()));
    }
    let mut command = Command::new(program);
    command
        .args(args)
        .env_clear()
        .envs(env.iter().map(|(k, v)| (k, v)))
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    let spawned = supervisor
        .spawn(command, Kind::Agent, label)
        .map_err(|e| SessionError::Start(e.to_string()))?;
    let Some(mut stdout) = spawned.stdout else {
        supervisor.stop(spawned.id, Duration::ZERO);
        return Err(SessionError::Start("the pipes are missing".into()));
    };
    let output = std::thread::scope(|scope| {
        let reader = scope.spawn(move || {
            let mut text = String::new();
            let _ = stdout.read_to_string(&mut text);
            text
        });
        if supervisor.wait(spawned.id, PROBE_TIMEOUT).is_none() {
            supervisor.stop(spawned.id, Duration::ZERO);
        }
        reader.join().unwrap_or_default()
    });
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_version_line_of_codex_reads_and_older_releases_are_refused() {
        assert_eq!(
            Version::parse("codex-cli 0.167.0\n"),
            Some(Version {
                major: 0,
                minor: 167,
                patch: 0
            })
        );
        assert_eq!(
            Version::parse("codex-cli 0.170.1-alpha.3").map(|v| v.to_string()),
            Some("0.170.1".into())
        );
        assert_eq!(Version::parse("codex-cli"), None);
        assert_eq!(Version::parse("codex 1.2"), None);
        assert!(check(Version::parse("codex-cli 0.167.0")).is_ok());
        assert!(check(Version::parse("codex-cli 1.0.0")).is_ok());
        assert_eq!(
            check(Version::parse("codex-cli 0.166.9")),
            Err(SessionError::Unsupported {
                found: Version::parse("0.166.9"),
                tested: TESTED
            })
        );
        assert_eq!(
            check(None),
            Err(SessionError::Unsupported {
                found: None,
                tested: TESTED
            })
        );
    }

    #[test]
    fn every_spelling_of_a_server_in_config_toml_is_found() {
        let text = r#"model = "gpt-5.5"
mcp_servers.inline.command = "a"

[mcp_servers.plain]
command = "npx"
args = [
  "--port=1",
]

[mcp_servers."quoted"] # the docs server
url = "https://example.test/#top"

[ mcp_servers.spaced ]
command = "b"

[mcp_servers.plain.env]
TOKEN = "x"

[mcp_servers]
keyed = { command = "c" }
dotted.command = "d"

[mcp_servers."dot.ted"]
command = "e"

[profiles.work]
model = "o3"
"#;
        assert_eq!(
            server_names(text),
            ["inline", "plain", "quoted", "spaced", "keyed", "dotted"]
        );
    }
}

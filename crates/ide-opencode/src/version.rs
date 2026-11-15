//! Which OpenCode is installed: `opencode --version`, and whether this build was tested against it (E147). An older
//! one gets the ACP route (`opencode acp`) instead.

use std::io::Read;
use std::path::Path;
use std::process::{Command, Stdio};
use std::time::Duration;

use ide_process::{Kind, Supervisor};

use crate::session::SessionError;

/// The oldest release whose API this crate was taken from and tested against: the schema in `tests/fixtures`.
pub const TESTED: Version = Version {
    major: 2,
    minor: 2,
    patch: 6,
};

/// How long `opencode --version` may take.
const PROBE_TIMEOUT: Duration = Duration::from_secs(10);

/// A release of OpenCode.
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
    /// The first `x.y.z` in `output`, such as the `opencode v2.2.6` that `opencode --version` prints; a leading `v`
    /// and a pre-release suffix are dropped.
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

    /// Whether the API of this release is one this build speaks.
    pub fn is_supported(self) -> bool {
        self >= TESTED
    }
}

impl std::fmt::Display for Version {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}.{}.{}", self.major, self.minor, self.patch)
    }
}

/// `found` when this build speaks its API, else why not.
pub fn check(found: Option<Version>) -> Result<Version, SessionError> {
    match found {
        Some(version) if version.is_supported() => Ok(version),
        found => Err(SessionError::Unsupported { found, tested: TESTED }),
    }
}

/// Runs `program --version` with the environment `env` through `supervisor` and checks the release it names; this
/// waits for the process, so it runs off the main thread.
pub fn probe(supervisor: &Supervisor, program: &Path, env: &[(String, String)]) -> Result<Version, SessionError> {
    if !program.is_file() {
        return Err(SessionError::NotFound(program.to_path_buf()));
    }
    let mut command = Command::new(program);
    command
        .arg("--version")
        .env_clear()
        .envs(env.iter().map(|(k, v)| (k, v)))
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    let spawned = supervisor
        .spawn(command, Kind::Agent, "opencode --version")
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
    check(Version::parse(&output))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_version_line_of_opencode_reads_and_older_releases_are_refused() {
        assert_eq!(Version::parse("opencode v2.2.6\n"), Some(TESTED));
        assert_eq!(
            Version::parse("opencode v2.3.0-beta.1").map(|v| v.to_string()),
            Some("2.3.0".into())
        );
        assert_eq!(Version::parse("2.2.6"), Some(TESTED));
        assert_eq!(Version::parse("opencode"), None);
        assert_eq!(Version::parse("opencode v2.2"), None);
        assert!(check(Version::parse("opencode v2.2.6")).is_ok());
        assert!(check(Version::parse("opencode v3.0.0")).is_ok());
        assert_eq!(
            check(Version::parse("opencode v2.2.5")),
            Err(SessionError::Unsupported {
                found: Version::parse("2.2.5"),
                tested: TESTED
            })
        );
        assert_eq!(
            check(Version::parse("opencode v1.14.0")),
            Err(SessionError::Unsupported {
                found: Version::parse("1.14.0"),
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
}

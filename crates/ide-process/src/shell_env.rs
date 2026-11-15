//! The environment of the user's login shell. An app started from the Finder inherits launchd's, whose PATH lacks
//! Homebrew, rustup, go and npm's folders, so language servers and agents would not be found.

use std::ffi::CStr;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use crate::pipes::Pipes;
use crate::supervisor::{Kind, Supervisor};

/// Marks the start and end of the environment in the shell's output, which may also hold what its startup files print.
const MARK: &str = "__IDE_ENV_7f3a__";

/// Variables that describe the capturing shell itself, not the user's environment.
const SESSION_ONLY: [&str; 4] = ["SHLVL", "PWD", "OLDPWD", "_"];

/// Why the environment could not be read.
#[derive(Debug)]
pub enum CaptureError {
    /// The shell did not start.
    Start(std::io::Error),
    /// The shell did not finish in time, for example because a startup file waits for input.
    Timeout,
    /// The output held no environment.
    NoEnvironment,
}

/// The user's login shell from the user database, else `/bin/zsh`.
pub fn user_shell() -> PathBuf {
    login_shell()
        .filter(|s| !s.is_empty())
        .map_or_else(|| PathBuf::from("/bin/zsh"), PathBuf::from)
}

/// The shell the user database names for the user, read with `getpwuid_r`, which any thread may call.
fn login_shell() -> Option<String> {
    // SAFETY: getuid only reads the calling process's user id.
    let uid = unsafe { libc::getuid() };
    let mut space = 1024;
    loop {
        let mut buffer = vec![0 as libc::c_char; space];
        // SAFETY: passwd is plain data for which all zero bytes are a valid value.
        let mut entry: libc::passwd = unsafe { std::mem::zeroed() };
        let mut found: *mut libc::passwd = std::ptr::null_mut();
        // SAFETY: entry and found are live locals and the buffer holds `space` bytes; getpwuid_r writes the entry's
        // strings into that buffer and nowhere else.
        let status = unsafe { libc::getpwuid_r(uid, &raw mut entry, buffer.as_mut_ptr(), space, &raw mut found) };
        if status == libc::ERANGE && space < 1 << 20 {
            space *= 4;
            continue;
        }
        if status != 0 || found.is_null() || entry.pw_shell.is_null() {
            return None;
        }
        // SAFETY: On success pw_shell points to a string ending in a zero byte within `buffer`, which is still alive.
        return Some(unsafe { CStr::from_ptr(entry.pw_shell) }.to_string_lossy().into_owned());
    }
}

/// The environment `shell` has as an interactive login shell in `dir`.
pub fn capture(
    supervisor: &Supervisor,
    shell: &Path,
    dir: &Path,
    timeout: Duration,
) -> Result<Vec<(String, String)>, CaptureError> {
    let script = format!(
        "cd {} 2>/dev/null; printf '%s' '{MARK}'; /usr/bin/env -0; printf '%s' '{MARK}'",
        quoted(dir)
    );
    let mut command = Command::new(shell);
    command
        .args(["-l", "-i", "-c", &script])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    let spawned = supervisor
        .spawn(command, Kind::Tool, "login shell")
        .map_err(CaptureError::Start)?;
    let stop = || supervisor.stop(spawned.id, Duration::from_millis(500));
    let Some(stdout) = spawned.stdout else {
        stop();
        return Err(CaptureError::NoEnvironment);
    };
    let mut pipes = Pipes::new(None, None, Some(stdout), None).map_err(|error| {
        stop();
        CaptureError::Start(error)
    })?;
    let deadline = Instant::now() + timeout;
    let mut output = Vec::new();
    while pipes.open() {
        let Some(left) = deadline
            .checked_duration_since(Instant::now())
            .filter(|left| !left.is_zero())
        else {
            stop();
            return Err(CaptureError::Timeout);
        };
        if pipes
            .step(Some(left), &mut |_, bytes| output.extend_from_slice(bytes))
            .is_err()
        {
            break;
        }
    }
    parse(&output).ok_or(CaptureError::NoEnvironment)
}

/// `path` as one word for a POSIX shell.
fn quoted(path: &Path) -> String {
    format!("'{}'", path.to_string_lossy().replace('\'', r"'\''"))
}

/// The variables between the marks, without those of the shell session itself.
fn parse(output: &[u8]) -> Option<Vec<(String, String)>> {
    let text = String::from_utf8_lossy(output);
    let start = text.find(MARK)? + MARK.len();
    let end = text.rfind(MARK)?;
    if end < start {
        return None;
    }
    let variables: Vec<(String, String)> = text[start..end]
        .split('\0')
        .filter_map(|entry| entry.split_once('='))
        .filter(|(name, _)| !name.is_empty() && !SESSION_ONLY.contains(name))
        .map(|(name, value)| (name.to_string(), value.to_string()))
        .collect();
    (!variables.is_empty()).then_some(variables)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_environment_is_read_between_the_marks_and_survives_noise() {
        let output = format!("motd\n{MARK}PATH=/a:/b\0MULTI=x\ny\0SHLVL=2\0EMPTY=\0junk\0{MARK}bye");
        let parsed = parse(output.as_bytes()).unwrap();
        assert_eq!(
            parsed,
            vec![
                ("PATH".into(), "/a:/b".into()),
                ("MULTI".into(), "x\ny".into()),
                ("EMPTY".into(), String::new()),
            ]
        );
        assert!(parse(b"no marks here").is_none());
    }

    #[test]
    fn a_path_is_quoted_for_the_shell() {
        assert_eq!(quoted(Path::new("/a b/it's")), r"'/a b/it'\''s'");
    }

    #[test]
    fn the_login_shell_gives_a_path() {
        let supervisor = Supervisor::new();
        let env = capture(
            &supervisor,
            Path::new("/bin/sh"),
            &std::env::temp_dir(),
            Duration::from_secs(10),
        )
        .unwrap();
        assert!(env.iter().any(|(name, value)| name == "PATH" && !value.is_empty()));
        assert!(supervisor.running().is_empty() || supervisor.running().len() == 1);
    }

    #[test]
    fn a_shell_that_hangs_is_given_up_on() {
        let supervisor = Supervisor::new();
        let script_dir = ide_core::testing::scratch("hang");
        let shell = script_dir.join("hang.sh");
        std::fs::write(&shell, "#!/bin/sh\nsleep 30\n").unwrap();
        std::fs::set_permissions(&shell, std::os::unix::fs::PermissionsExt::from_mode(0o755)).unwrap();
        let result = capture(&supervisor, &shell, &script_dir, Duration::from_millis(300));
        assert!(matches!(result, Err(CaptureError::Timeout)));
    }

    #[test]
    fn an_environment_printed_in_pieces_is_read_whole() {
        let supervisor = Supervisor::new();
        let script_dir = ide_core::testing::scratch("pieces");
        let shell = script_dir.join("slow.sh");
        let script = format!(
            "#!/bin/sh\nprintf '%s' '{MARK}'\nsleep 0.2\nprintf 'A=1\\000'\nsleep 0.2\nprintf 'B=2\\000{MARK}'\n"
        );
        std::fs::write(&shell, script).unwrap();
        std::fs::set_permissions(&shell, std::os::unix::fs::PermissionsExt::from_mode(0o755)).unwrap();
        let env = capture(&supervisor, &shell, &script_dir, Duration::from_secs(10)).unwrap();
        assert_eq!(
            env,
            [("A".to_string(), "1".to_string()), ("B".to_string(), "2".to_string())]
        );
    }

    #[test]
    fn the_users_shell_is_a_path() {
        assert!(user_shell().is_absolute());
    }

    #[test]
    fn threads_reading_the_users_shell_at_once_all_read_the_same() {
        let first = login_shell().expect("the user database knows the user running the tests");
        let threads: Vec<_> = (0..16)
            .map(|_| std::thread::spawn(|| (0..200).map(|_| login_shell()).collect::<Vec<_>>()))
            .collect();
        for thread in threads {
            for shell in thread.join().unwrap() {
                assert_eq!(shell.as_ref(), Some(&first));
            }
        }
    }
}

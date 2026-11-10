use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::mpsc::{Receiver, channel};
use std::time::{Duration, Instant};

use ide_process::Supervisor;
use ide_terminal::{Event, Options, Program, Session};

pub const PATIENCE: Duration = Duration::from_secs(10);

/// A session with its events collected.
pub struct Harness {
    pub session: Session,
    pub supervisor: Supervisor,
    pub events: Vec<Event>,
    receiver: Receiver<Event>,
}

impl Harness {
    pub fn start(options: Options, home: &Path) -> Harness {
        let supervisor = Supervisor::new();
        let (sender, receiver) = channel();
        let session = Session::spawn(&supervisor, &environment(home), options, move |event| {
            let _ = sender.send(event);
        })
        .expect("the session starts");
        Harness {
            session,
            supervisor,
            events: Vec::new(),
            receiver,
        }
    }

    /// Waits until `done` holds, collecting events; whether it did within `PATIENCE`.
    pub fn wait_until(&mut self, mut done: impl FnMut(&Harness) -> bool) -> bool {
        let deadline = Instant::now() + PATIENCE;
        loop {
            while let Ok(event) = self.receiver.try_recv() {
                self.events.push(event);
            }
            if done(self) {
                return true;
            }
            let Some(left) = deadline.checked_duration_since(Instant::now()) else {
                return false;
            };
            if let Ok(event) = self.receiver.recv_timeout(left.min(Duration::from_millis(50))) {
                self.events.push(event);
            }
        }
    }

    pub fn screen_has(&self, text: &str) -> bool {
        self.session.visible_text().iter().any(|line| line.contains(text))
    }

    pub fn screen_line(&self, text: &str) -> bool {
        self.session.visible_text().iter().any(|line| line.trim_end() == text)
    }

    pub fn saw(&self, wanted: &Event) -> bool {
        self.events.contains(wanted)
    }

    /// The number after `prefix` on the screen, as printed by `echo prefix$$`.
    pub fn number_after(&self, prefix: &str) -> Option<i32> {
        self.session.visible_text().iter().find_map(|line| {
            let at = line.find(prefix)? + prefix.len();
            let digits: String = line[at..].chars().take_while(char::is_ascii_digit).collect();
            digits.parse().ok()
        })
    }
}

impl Drop for Harness {
    /// Waits for the program to end, so that it writes nothing more into a test's folder once that is removed.
    fn drop(&mut self) {
        self.session.close();
        let deadline = Instant::now() + PATIENCE;
        while self.session.is_running() && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(10));
        }
    }
}

pub fn environment(home: &Path) -> Vec<(String, String)> {
    [
        ("PATH", "/usr/bin:/bin:/usr/sbin:/sbin"),
        ("HOME", &home.to_string_lossy()),
        ("USER", &std::env::var("USER").unwrap_or_default()),
        ("LANG", "en_US.UTF-8"),
        ("BASH_SILENCE_DEPRECATION_WARNING", "1"),
    ]
    .into_iter()
    .map(|(name, value)| (name.to_string(), value.to_string()))
    .collect()
}

/// An empty folder of its own for one test, removed with it.
pub struct TempDir(PathBuf);

impl std::ops::Deref for TempDir {
    type Target = Path;

    fn deref(&self) -> &Path {
        &self.0
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

pub fn temporary(name: &str) -> TempDir {
    static NEXT: AtomicU32 = AtomicU32::new(0);
    let n = NEXT.fetch_add(1, Ordering::Relaxed);
    let dir = std::env::temp_dir()
        .canonicalize()
        .unwrap()
        .join(format!("ide-terminal-{name}-{}-{n}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    TempDir(dir)
}

pub fn command(program: &str, args: &[&str], cwd: &Path) -> Options {
    Options {
        program: Program::Command {
            program: program.into(),
            args: args.iter().map(ToString::to_string).collect(),
        },
        shell_integration: None,
        ..Options::new(cwd)
    }
}

/// Whether a process numbered `pid` exists, asked of `kill -0`.
pub fn alive(pid: i32) -> bool {
    std::process::Command::new("/bin/kill")
        .args(["-0", &pid.to_string()])
        .stderr(std::process::Stdio::null())
        .status()
        .is_ok_and(|status| status.success())
}

pub fn gone_within(pid: i32, limit: Duration) -> bool {
    let deadline = Instant::now() + limit;
    while alive(pid) {
        if Instant::now() > deadline {
            return false;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    true
}

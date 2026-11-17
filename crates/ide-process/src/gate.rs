//! Running a work order's gate commands, such as `make check` (E187, C67): through the supervisor as [`Kind::Gate`],
//! with a deadline and a cap on its output; past either, or when cancelled, its whole process group ends, and the run
//! returns only once the supervisor saw the leader end and nothing is left of the group. What a command left behind
//! in its group when it ended is ended the same way, so no grandchild holds a pipe or the Mac awake.
//!
//! A process that starts a process group or session of its own leaves the group and is out of reach here.

use std::ffi::OsStr;
use std::io;
use std::path::Path;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use ide_core::Cancel;

use crate::pipes::{Pipes, Stream};
use crate::sandbox::Profile;
use crate::supervisor::{Ending, Kind, ProcessId, Supervisor};
use crate::tail::Tail;

/// How often a running gate is looked at for being cancelled, past its time or over its output.
const POLL: Duration = Duration::from_millis(50);

/// How long the pipes of a gate whose leader ended may stay open before what is left of its group is ended.
const AFTERMATH: Duration = Duration::from_secs(2);

/// How long a stopped gate gets for each step from SIGTERM to SIGKILL.
const GRACE: Duration = Duration::from_millis(500);

/// How long a run waits for the end of a gate it stopped before it reports the end unconfirmed.
const CONFIRM: Duration = Duration::from_secs(10);

/// How far a gate may go.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Limits {
    /// How long it may run (E168: 60 min unless the workflow file says otherwise).
    pub deadline: Duration,
    /// How many bytes it may print on both streams together before it is stopped.
    pub output: u64,
    /// How much of the end of each stream is kept for the log.
    pub kept: usize,
}

impl Default for Limits {
    fn default() -> Limits {
        Limits {
            deadline: Duration::from_secs(60 * 60),
            output: 256 * 1024 * 1024,
            kept: 1024 * 1024,
        }
    }
}

/// Why a gate was stopped before it ended on its own.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stopped {
    /// It ran past its deadline.
    Deadline,
    /// It printed more than its output limit.
    Output,
    /// Its run was cancelled.
    Cancelled,
}

/// How a gate ran.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Outcome {
    /// How its leader ended.
    pub ending: Ending,
    /// Why it was stopped, if it was.
    pub stopped: Option<Stopped>,
    /// Whether the supervisor reported the leader's end and nothing of its group was left afterwards.
    pub confirmed: bool,
    /// The end of its standard output, at most [`Limits::kept`] bytes.
    pub stdout: String,
    /// The end of its standard error, at most [`Limits::kept`] bytes.
    pub stderr: String,
    /// How many bytes it printed on both streams.
    pub printed: u64,
    /// How long it ran.
    pub took: Duration,
}

impl Outcome {
    /// Whether it ended on its own with exit code 0.
    pub fn passed(&self) -> bool {
        self.stopped.is_none() && self.ending == Ending::Code(0)
    }
}

/// How gates are run: through `supervisor`, with `env` only, inside `sandbox` when there is one, within `limits`.
#[derive(Clone, Copy)]
pub struct Gate<'a> {
    /// The supervisor every gate runs under.
    pub supervisor: &'a Supervisor,
    /// The whole environment of a gate, such as `order_env::for_orders` gives.
    pub env: &'a [(String, String)],
    /// The profile the gate runs in (E187); `None` only for tests of the runner itself.
    pub sandbox: Option<&'a Profile>,
    /// How far it may go.
    pub limits: Limits,
    /// What stops it early.
    pub cancel: Option<&'a Cancel>,
}

impl Gate<'_> {
    /// Runs `program` with `args` in `dir`, listed in the activity view as `label`, until it ended and nothing of its
    /// process group is left.
    pub fn run<S: AsRef<OsStr>>(&self, program: &Path, args: &[S], dir: &Path, label: &str) -> io::Result<Outcome> {
        let mut command = match self.sandbox {
            Some(profile) => profile.command(program),
            None => Command::new(program),
        };
        command
            .args(args)
            .current_dir(dir)
            .env_clear()
            .envs(self.env.iter().map(|(k, v)| (k, v)))
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        let started = Instant::now();
        let spawned = self.supervisor.spawn_awaited(command, Kind::Gate, label)?;
        let group = libc::pid_t::try_from(spawned.pid).unwrap_or(0);
        let pipes = Pipes::new(None, None, spawned.stdout, spawned.stderr).ok();
        let mut run = Run {
            gate: self,
            id: spawned.id,
            deadline: started + self.limits.deadline,
            stdout: Tail::new(self.limits.kept),
            stderr: Tail::new(self.limits.kept),
            printed: 0,
            stopped: None,
        };
        let ending = run.serve(pipes);
        let group_gone = end_group(group);
        if ending.is_none() {
            self.supervisor.release(spawned.id);
        }
        Ok(Outcome {
            confirmed: ending.is_some() && group_gone,
            ending: ending.unwrap_or(Ending::Unknown),
            stopped: run.stopped,
            stdout: run.stdout.text(),
            stderr: run.stderr.text(),
            printed: run.printed,
            took: started.elapsed(),
        })
    }
}

/// One gate while it runs.
struct Run<'g, 'a> {
    gate: &'g Gate<'a>,
    id: ProcessId,
    deadline: Instant,
    stdout: Tail,
    stderr: Tail,
    printed: u64,
    stopped: Option<Stopped>,
}

impl Run<'_, '_> {
    /// Serves the pipes until the leader ended and they closed or [`AFTERMATH`] passed; stops the gate when it must.
    /// `None` when a stopped gate's end did not come within [`CONFIRM`].
    fn serve(&mut self, mut pipes: Option<Pipes>) -> Option<Ending> {
        let supervisor = self.gate.supervisor;
        let mut ended: Option<(Ending, Instant)> = None;
        let mut stopping_since: Option<Instant> = None;
        loop {
            let open = pipes.as_ref().is_some_and(Pipes::open);
            let now = Instant::now();
            if let Some((ending, until)) = ended {
                if !open || now >= until {
                    return Some(ending);
                }
            } else {
                if stopping_since.is_none()
                    && let Some(why) = self.why_stop(now)
                {
                    self.stopped = Some(why);
                    supervisor.stop(self.id, GRACE);
                    stopping_since = Some(now);
                }
                if stopping_since.is_some_and(|since| now >= since + CONFIRM) {
                    return None;
                }
                if let Some(ending) = supervisor.wait(self.id, if open { Duration::ZERO } else { POLL }) {
                    ended = Some((ending, Instant::now() + AFTERMATH));
                    continue;
                }
            }
            let look = ended.map_or(POLL, |(_, until)| until.saturating_duration_since(now).min(POLL));
            if open && let Some(served) = pipes.as_mut() {
                let (stdout, stderr, printed) = (&mut self.stdout, &mut self.stderr, &mut self.printed);
                let step = served.step(Some(look), &mut |stream, bytes| {
                    *printed += bytes.len() as u64;
                    match stream {
                        Stream::Stdout => stdout.push(bytes),
                        Stream::Stderr => stderr.push(bytes),
                    }
                });
                if step.is_err() {
                    pipes = None;
                }
            }
        }
    }

    fn why_stop(&self, now: Instant) -> Option<Stopped> {
        if self.gate.cancel.is_some_and(Cancel::is_cancelled) {
            Some(Stopped::Cancelled)
        } else if now >= self.deadline {
            Some(Stopped::Deadline)
        } else if self.printed > self.gate.limits.output {
            Some(Stopped::Output)
        } else {
            None
        }
    }
}

/// Ends what is left of process group `group`, SIGTERM first, then SIGKILL; whether nothing of it is left.
fn end_group(group: libc::pid_t) -> bool {
    if group <= 1 {
        return false;
    }
    for signal in [libc::SIGTERM, libc::SIGKILL] {
        if !crate::system::group_exists(group) {
            return true;
        }
        // SAFETY: kill only sends a signal. The group number is the gate's, taken when it started; the system reuses
        // no number while a group of that number has members, which `group_exists` saw a moment ago.
        unsafe { libc::kill(-group, signal) };
        if crate::reaper::group_ends_within(group, GRACE) {
            return true;
        }
    }
    crate::reaper::group_ends_within(group, CONFIRM)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn env() -> Vec<(String, String)> {
        vec![("PATH".into(), "/usr/bin:/bin".into())]
    }

    fn run(supervisor: &Supervisor, limits: Limits, script: &str) -> Outcome {
        let env = env();
        let gate = Gate {
            supervisor,
            env: &env,
            sandbox: None,
            limits,
            cancel: None,
        };
        gate.run(Path::new("/bin/sh"), &["-c", script], Path::new("/"), "gate")
            .unwrap()
    }

    fn quick(deadline: Duration) -> Limits {
        Limits {
            deadline,
            ..Limits::default()
        }
    }

    /// Whether process `pid` is gone within a few seconds.
    fn gone(pid: &str) -> bool {
        let Ok(pid) = pid.trim().parse::<libc::pid_t>() else {
            return false;
        };
        let until = Instant::now() + Duration::from_secs(5);
        // SAFETY: Signal 0 only asks whether the process exists.
        while unsafe { libc::kill(pid, 0) } == 0 {
            if Instant::now() >= until {
                return false;
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        true
    }

    #[test]
    fn a_gate_that_passes_reports_its_output_and_is_listed_as_a_gate() {
        let supervisor = Supervisor::new();
        let seen = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let heard = seen.clone();
        let _subscription = supervisor.subscribe(move |event| {
            if let crate::Event::Started(info) = event {
                heard.lock().unwrap().push(info.kind);
            }
        });
        let outcome = run(&supervisor, Limits::default(), "echo out; echo err >&2");
        assert!(outcome.passed() && outcome.confirmed, "{outcome:?}");
        assert_eq!((outcome.stdout.as_str(), outcome.stderr.as_str()), ("out\n", "err\n"));
        assert_eq!(outcome.printed, 8);
        assert_eq!(*seen.lock().unwrap(), [Kind::Gate]);
        let failed = run(&supervisor, Limits::default(), "exit 2");
        assert_eq!((failed.ending, failed.passed()), (Ending::Code(2), false));
    }

    #[test]
    fn a_silent_hang_ends_at_the_deadline_with_its_whole_group() {
        let supervisor = Supervisor::new();
        let started = Instant::now();
        let outcome = run(
            &supervisor,
            quick(Duration::from_millis(300)),
            "sleep 30 & echo $! > /dev/stderr; wait",
        );
        assert_eq!(outcome.stopped, Some(Stopped::Deadline));
        assert!(outcome.confirmed, "{outcome:?}");
        assert!(!outcome.passed());
        assert!(started.elapsed() < Duration::from_secs(8), "{:?}", started.elapsed());
        assert!(gone(&outcome.stderr), "the sleep it started is ended too");
        assert!(supervisor.running().is_empty());
    }

    #[test]
    fn endless_output_is_stopped_at_its_limit_and_only_its_end_is_kept() {
        let supervisor = Supervisor::new();
        let limits = Limits {
            output: 4 * 1024 * 1024,
            kept: 64 * 1024,
            ..quick(Duration::from_secs(60))
        };
        let started = Instant::now();
        let outcome = run(&supervisor, limits, "exec yes gate");
        assert_eq!(outcome.stopped, Some(Stopped::Output));
        assert!(outcome.confirmed, "{:?}", outcome.ending);
        assert!(outcome.printed > limits.output);
        assert!(outcome.stdout.len() <= limits.kept);
        assert!(started.elapsed() < Duration::from_secs(30), "{:?}", started.elapsed());
    }

    #[test]
    fn children_that_hold_the_pipes_after_the_gate_ended_are_ended() {
        let supervisor = Supervisor::new();
        let started = Instant::now();
        let outcome = run(&supervisor, Limits::default(), "sleep 30 & echo $!");
        assert_eq!((outcome.ending, outcome.stopped), (Ending::Code(0), None));
        assert!(outcome.confirmed);
        assert!(gone(&outcome.stdout), "the child holding the pipes is gone");
        assert!(
            started.elapsed() < AFTERMATH + Duration::from_secs(5),
            "{:?}",
            started.elapsed()
        );
    }

    #[test]
    fn a_cancelled_gate_is_stopped() {
        let supervisor = Supervisor::new();
        let env = env();
        let cancel = Cancel::new();
        cancel.cancel();
        let gate = Gate {
            supervisor: &supervisor,
            env: &env,
            sandbox: None,
            limits: Limits::default(),
            cancel: Some(&cancel),
        };
        let outcome = gate
            .run(Path::new("/bin/sleep"), &["30"], Path::new("/"), "sleep")
            .unwrap();
        assert_eq!(outcome.stopped, Some(Stopped::Cancelled));
        assert!(outcome.confirmed);
    }

    #[test]
    fn a_gate_runs_inside_its_sandbox() {
        let scratch = ide_core::testing::scratch("gate-sandbox");
        let (work, temp, home) = (scratch.join("work"), scratch.join("temp"), scratch.join("home"));
        for folder in [&work, &temp, &home] {
            std::fs::create_dir_all(folder).unwrap();
        }
        let profile = Profile::gate(std::slice::from_ref(&work), &temp, &home);
        let supervisor = Supervisor::new();
        let env = env();
        let gate = Gate {
            supervisor: &supervisor,
            env: &env,
            sandbox: Some(&profile),
            limits: Limits::default(),
            cancel: None,
        };
        let script = format!("echo x > made && echo x > '{}/escaped'", scratch.display());
        let outcome = gate
            .run(Path::new("/bin/sh"), &["-c", script.as_str()], &work, "gate")
            .unwrap();
        assert!(!outcome.passed(), "{outcome:?}");
        assert!(work.join("made").exists() && !scratch.join("escaped").exists());
    }

    #[test]
    fn a_gate_reads_a_readable_folder_through_a_link_only_when_the_link_is_followable() {
        let scratch = ide_core::testing::scratch("gate-sandbox-link");
        let (work, temp, home) = (scratch.join("work"), scratch.join("temp"), scratch.join("home"));
        let (real, holder) = (home.join("repos/dep"), home.join("data"));
        for folder in [&work, &temp, &real, &holder] {
            std::fs::create_dir_all(folder).unwrap();
        }
        std::fs::write(real.join("file"), "content").unwrap();
        let link = holder.join("dep");
        std::os::unix::fs::symlink(&real, &link).unwrap();
        let supervisor = Supervisor::new();
        let env = env();
        let read = |profile: &Profile| {
            let gate = Gate {
                supervisor: &supervisor,
                env: &env,
                sandbox: Some(profile),
                limits: Limits::default(),
                cancel: None,
            };
            let script = format!("cat '{}/file'", link.display());
            gate.run(Path::new("/bin/sh"), &["-c", script.as_str()], &work, "gate")
                .unwrap()
        };
        let profile = Profile::gate(std::slice::from_ref(&work), &temp, &home).readable(std::slice::from_ref(&real));
        let refused = read(&profile);
        assert!(!refused.passed(), "{refused:?}");
        let followed = read(&profile.links(std::slice::from_ref(&link)));
        assert!(followed.passed() && followed.stdout == "content", "{followed:?}");
    }
}

//! The guard that keeps a selftest app from taking the keyboard: it follows Launch Services' notifications while the
//! app runs, ends the app the moment it becomes the frontmost app and fails the run.

use std::io::{BufRead, BufReader};
use std::process::{Child, ChildStdout, Command, Stdio};
use std::sync::{Arc, Mutex, PoisonError};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

const LSAPPINFO: &str = "/usr/bin/lsappinfo";

/// How long the guard waits, after the app's process ended, for Launch Services to report its death, so that it also
/// sees an activation just before it.
const DEATH_NOTICE: Duration = Duration::from_secs(2);

/// A Launch Services notification the guard acts on.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Notice {
    /// An app checked in under the application serial number `asn`.
    Born {
        pid: u32,
        asn: String,
    },
    /// The app `asn` became the frontmost app, which `from` was before.
    Front {
        asn: String,
        from: Option<String>,
    },
    Died {
        pid: u32,
    },
}

/// The notification on one line of `lsappinfo listen`, if it is one the guard acts on.
pub(crate) fn parse(line: &str) -> Option<Notice> {
    let kind = line.strip_prefix("Notification: ")?.split_whitespace().next()?;
    let pid = || field(line, "\"pid\"=")?.parse().ok();
    match kind {
        "kLSNotifyApplicationBirth" => Some(Notice::Born {
            pid: pid()?,
            asn: field(line, "\"LSASN\"=")?.to_string(),
        }),
        "kLSNotifyBecameFrontmost" => Some(Notice::Front {
            asn: field(line, "\"LSASN\"=")?.to_string(),
            from: field(line, "\"LSOtherASN\"=").map(str::to_string),
        }),
        "kLSNotifyApplicationDeath" => Some(Notice::Died { pid: pid()? }),
        _ => None,
    }
}

/// The value after `key` in a notification's payload, up to the separator after it.
fn field<'a>(line: &'a str, key: &str) -> Option<&'a str> {
    let rest = &line[line.find(key)? + key.len()..];
    let end = rest.find([',', ' ', '}']).unwrap_or(rest.len());
    Some(&rest[..end]).filter(|value| !value.is_empty())
}

/// What the guard learned so far about the apps it guards.
#[derive(Debug, Default)]
pub(crate) struct Seen {
    guarded: Vec<u32>,
    born: Vec<(u32, String)>,
    died: Vec<u32>,
    /// The apps that were frontmost when a guarded app took over, one entry per time.
    taken_from: Vec<Option<String>>,
}

impl Seen {
    /// Notes `notice`; returns the process to end now if a guarded app just became frontmost.
    pub(crate) fn note(&mut self, notice: Notice) -> Option<u32> {
        match notice {
            Notice::Born { pid, asn } => self.born.push((pid, asn)),
            Notice::Died { pid } => self.died.push(pid),
            Notice::Front { asn, from } => {
                let (pid, _) = self
                    .born
                    .iter()
                    .find(|(pid, born)| *born == asn && self.guarded.contains(pid))?;
                self.taken_from.push(from);
                return Some(*pid);
            }
        }
        None
    }

    fn registered_and_not_dead(&self) -> bool {
        self.born
            .iter()
            .any(|(pid, _)| self.guarded.contains(pid) && !self.died.contains(pid))
    }
}

/// What a guarded run did to the keyboard: nothing, or the problem that fails it, naming the app it first took the
/// keyboard from by `name`.
pub(crate) fn verdict(seen: &Seen, name: impl Fn(&str) -> String) -> Option<String> {
    let first = seen.taken_from.first()?;
    let from = first.as_deref().map_or_else(|| "the app in front".to_string(), &name);
    Some(format!(
        "the app became the frontmost app and took the keyboard from {from}; the runner ended it at once (see \
         \"Selftest\" in docs/development.md)"
    ))
}

/// A running guard: `lsappinfo listen` and the thread that reads it.
pub(crate) struct Guard {
    listener: Child,
    reader: Option<JoinHandle<()>>,
    seen: Arc<Mutex<Seen>>,
}

impl Guard {
    /// Starts listening; returns once Launch Services delivers notifications, before any app is launched.
    pub(crate) fn start() -> Result<Guard, String> {
        let mut listener = Command::new(LSAPPINFO)
            .args(["listen", "+birth", "+becameFrontmost", "+death"])
            // Run after `listen` has subscribed, so its answer on the first line says the guard is ready.
            .arg("front")
            .arg("forever")
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| format!("{LSAPPINFO}: {e}"))?;
        let mut lines = BufReader::new(listener.stdout.take().ok_or("lsappinfo has no output")?);
        let mut first = String::new();
        if lines.read_line(&mut first).map_err(|e| format!("{LSAPPINFO}: {e}"))? == 0 {
            let _ = listener.kill();
            let _ = listener.wait();
            return Err(format!("{LSAPPINFO} ended before it listened"));
        }
        let seen = Arc::new(Mutex::new(Seen::default()));
        let reader = Some(std::thread::spawn({
            let seen = Arc::clone(&seen);
            move || follow(lines, &seen)
        }));
        Ok(Guard { listener, reader, seen })
    }

    /// Guards the app in process `pid` from now on.
    pub(crate) fn guard(&self, pid: u32) {
        self.lock().guarded.push(pid);
    }

    /// Stops listening once Launch Services reported the end of the guarded app, and says whether it took the
    /// keyboard.
    pub(crate) fn finish(mut self) -> Option<String> {
        let started = Instant::now();
        while self.lock().registered_and_not_dead() && started.elapsed() < DEATH_NOTICE {
            std::thread::sleep(Duration::from_millis(20));
        }
        let _ = self.listener.kill();
        let _ = self.listener.wait();
        if let Some(reader) = self.reader.take() {
            let _ = reader.join();
        }
        verdict(&self.lock(), display_name)
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, Seen> {
        self.seen.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

impl Drop for Guard {
    fn drop(&mut self) {
        let _ = self.listener.kill();
        let _ = self.listener.wait();
    }
}

fn follow(lines: BufReader<ChildStdout>, seen: &Mutex<Seen>) {
    for line in lines.lines().map_while(Result::ok) {
        let Some(notice) = parse(&line) else { continue };
        let end = seen.lock().unwrap_or_else(PoisonError::into_inner).note(notice);
        if let Some(pid) = end {
            let _ = Command::new("/bin/kill")
                .args(["-KILL", &pid.to_string()])
                .stderr(Stdio::null())
                .status();
        }
    }
}

/// The display name of the running app `asn`, which `lsappinfo info -only name` puts in quotes first.
fn display_name(asn: &str) -> String {
    let listed = Command::new(LSAPPINFO)
        .args(["info", "-only", "name", asn])
        .stderr(Stdio::null())
        .output()
        .map(|out| String::from_utf8_lossy(&out.stdout).into_owned())
        .unwrap_or_default();
    quoted_name(&listed).map_or_else(|| asn.to_string(), |name| format!("{name} ({asn})"))
}

fn quoted_name(listed: &str) -> Option<&str> {
    listed
        .trim_start()
        .strip_prefix('"')?
        .split('"')
        .next()
        .filter(|n| !n.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;

    const BIRTH: &str = "Notification: kLSNotifyApplicationBirth time=+0.000926018s  dataRef={ \
        \"ApplicationType\"=\"Foreground\", \"CFBundleIdentifier\"=\"local.agentide.selftest\", \
        \"LSASN\"=ASN:0x0-0x2406404:, \"LSParentASN\"=ASN:0x0-0x14b44b3:, \"pid\"=76094 } \
        affectedASN=\"Vibeedit\" ASN:0x0-0x2406404:  context=0 sessionID=186b4 notificationID=0x104d04af0";

    fn front(asn: &str, from: &str) -> String {
        format!(
            "Notification: kLSNotifyBecameFrontmost time=11/09/2026 21:34:47.326 dataRef={{ \
             \"ApplicationType\"=\"Foreground\", \"CFBundleIdentifier\"=\"local.agentide.selftest\", \
             \"LSASN\"={asn}, \"LSFrontApplicationSeed\"=3164, \"LSOtherASN\"={from} }} affectedASN=\"Vibeedit\" \
             {asn}  context=0 sessionID=186b4 notificationID=0x103140a90"
        )
    }

    const WARP: &str = "ASN:0x0-0x14b44b3:";
    const OURS: &str = "ASN:0x0-0x2406404:";

    #[test]
    fn the_notifications_of_launch_services_are_read_from_their_payload() {
        assert_eq!(
            parse(BIRTH),
            Some(Notice::Born {
                pid: 76094,
                asn: OURS.to_string()
            })
        );
        assert_eq!(
            parse(&front(OURS, WARP)),
            Some(Notice::Front {
                asn: OURS.to_string(),
                from: Some(WARP.to_string())
            })
        );
        let death = "Notification: kLSNotifyApplicationDeath time=+0.00682s  dataRef={ \"LSASN\"=ASN:0x0-0x2406404:, \
                     \"LSExitStatus\"=0, \"pid\"=76094 } affectedASN=ASN:0x0-0x2406404:  context=0";
        assert_eq!(parse(death), Some(Notice::Died { pid: 76094 }));
        assert_eq!(parse("ASN:0x0-0x2373371:"), None);
        assert_eq!(
            parse("Notification: kLSNotifyApplicationReady time=+0.1s dataRef={ \"pid\"=1 }"),
            None
        );
        assert_eq!(
            parse("Notification: kLSNotifyApplicationBirth time=+0.1s dataRef={ }"),
            None
        );
    }

    #[test]
    fn a_guarded_app_that_becomes_frontmost_is_ended_and_fails_the_run() {
        let mut seen = Seen::default();
        seen.guarded.push(76094);
        assert_eq!(seen.note(parse(BIRTH).unwrap()), None);
        assert_eq!(seen.note(parse(&front(OURS, WARP)).unwrap()), Some(76094));
        let problem = verdict(&seen, |asn| format!("Warp ({asn})")).unwrap();
        assert!(
            problem.starts_with(&format!(
                "the app became the frontmost app and took the keyboard from Warp ({WARP})"
            )),
            "{problem}"
        );
    }

    #[test]
    fn other_apps_becoming_frontmost_leave_the_run_alone() {
        let mut seen = Seen::default();
        seen.guarded.push(76094);
        seen.note(parse(BIRTH).unwrap());
        let parallel = "ASN:0x0-0x2407405:";
        seen.note(Notice::Born {
            pid: 80000,
            asn: parallel.to_string(),
        });
        assert_eq!(seen.note(parse(&front(parallel, WARP)).unwrap()), None);
        assert_eq!(seen.note(parse(&front(WARP, parallel)).unwrap()), None);
        assert_eq!(verdict(&seen, str::to_string), None);
    }

    #[test]
    fn the_guard_waits_for_the_death_of_a_registered_app_only() {
        let mut seen = Seen::default();
        seen.guarded.push(76094);
        assert!(!seen.registered_and_not_dead());
        seen.note(parse(BIRTH).unwrap());
        assert!(seen.registered_and_not_dead());
        seen.note(Notice::Died { pid: 76094 });
        assert!(!seen.registered_and_not_dead());
    }

    #[test]
    fn an_app_is_named_by_the_name_lsappinfo_quotes_first() {
        let listed = "\"Warp\" ASN:0x0-0x14b44b3: \n    bundleID=[ NULL ] \n";
        assert_eq!(quoted_name(listed), Some("Warp"));
        assert_eq!(quoted_name("[ NULL ]  ASN:0x0-0x14b44b3: \n"), None);
        assert_eq!(quoted_name(""), None);
    }
}

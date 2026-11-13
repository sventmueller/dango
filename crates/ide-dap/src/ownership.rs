//! Ending a launched program that outlived its adapter, with its process group (Review C7). The app's own group and
//! programs the session does not own are never signalled; callers decide ownership before they call this.

use std::time::Duration;

/// Whether a process numbered `pid` exists.
pub(crate) fn alive(pid: u32) -> bool {
    // SAFETY: Signal 0 only asks whether the process exists; nothing is delivered.
    libc::pid_t::try_from(pid).is_ok_and(|pid| pid > 0 && unsafe { libc::kill(pid, 0) } == 0)
}

/// Sends SIGTERM to `pid`'s process group, or to `pid` alone when it shares the app's group, and SIGKILL after
/// `grace` if it is still there.
pub(crate) fn end_program(pid: u32, grace: Duration) {
    if !alive(pid) {
        return;
    }
    signal(pid, libc::SIGTERM);
    ide_core::thread::detach_or_run("dap-end-program", move || {
        let step = Duration::from_millis(50);
        let mut waited = Duration::ZERO;
        while waited < grace {
            if !alive(pid) {
                return;
            }
            std::thread::sleep(step);
            waited += step;
        }
        if alive(pid) {
            signal(pid, libc::SIGKILL);
        }
    });
}

fn signal(pid: u32, signal: i32) {
    let Ok(pid) = libc::pid_t::try_from(pid) else {
        return;
    };
    if pid <= 1 {
        return;
    }
    // SAFETY: getpgid only reads the group of a process; 0 names the calling process.
    let (group, own) = unsafe { (libc::getpgid(pid), libc::getpgid(0)) };
    let target = if group > 1 && group != own { -group } else { pid };
    // SAFETY: kill only sends a signal; the target is the program's group, or the program alone when its group is the
    // app's, so the app never signals itself.
    unsafe { libc::kill(target, signal) };
}

#[cfg(test)]
mod tests {
    use std::os::unix::process::CommandExt;
    use std::process::Command;
    use std::time::Instant;

    use super::*;

    #[test]
    fn a_program_in_a_group_of_its_own_goes_with_its_group() {
        let mut leader = Command::new("/bin/sh")
            .args(["-c", "sleep 30 & wait"])
            .process_group(0)
            .spawn()
            .unwrap();
        let pid = leader.id();
        std::thread::sleep(Duration::from_millis(100));
        end_program(pid, Duration::from_millis(300));
        let deadline = Instant::now() + Duration::from_secs(5);
        while leader.try_wait().unwrap().is_none() && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(20));
        }
        assert!(leader.try_wait().unwrap().is_some());
    }
}

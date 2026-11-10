//! The calls into the C library: process groups, process arguments, signals, the PTY's window size and the host name.

use std::ffi::CStr;
use std::fs::File;
use std::io;
use std::os::fd::AsRawFd;
use std::sync::{Arc, OnceLock};

use alacritty_terminal::tty::{EventedReadWrite, Pty};
use polling::{Event, PollMode, Poller};

/// `PROC_PGRP_ONLY` from `<sys/proc_info.h>`, which the libc crate does not define.
const PROC_PGRP_ONLY: u32 = 2;

/// The foreground process group of the terminal whose master side is `master`.
pub(crate) fn foreground_group(master: &File) -> Option<i32> {
    // SAFETY: tcgetpgrp only reads the state of a descriptor that `master` keeps open.
    let group = unsafe { libc::tcgetpgrp(master.as_raw_fd()) };
    (group > 0).then_some(group)
}

/// The processes in process group `group`.
pub(crate) fn group_members(group: i32) -> Vec<i32> {
    let Ok(typeinfo) = u32::try_from(group) else {
        return Vec::new();
    };
    let mut pids: Vec<libc::pid_t> = vec![0; 64];
    loop {
        let Ok(capacity) = libc::c_int::try_from(pids.len() * size_of::<libc::pid_t>()) else {
            return Vec::new();
        };
        // SAFETY: the buffer holds `capacity` bytes and proc_listpids writes no more than it is given.
        let filled = unsafe { libc::proc_listpids(PROC_PGRP_ONLY, typeinfo, pids.as_mut_ptr().cast(), capacity) };
        let Ok(filled) = usize::try_from(filled) else {
            return Vec::new();
        };
        let count = filled / size_of::<libc::pid_t>();
        // A full buffer may have cut the list short.
        if count < pids.len() || pids.len() >= 4096 {
            pids.truncate(count);
            pids.retain(|&pid| pid > 0);
            return pids;
        }
        pids.resize(pids.len() * 4, 0);
    }
}

/// The arguments `pid` was started with, `None` when the system does not tell (another user's process, a zombie).
pub(crate) fn arguments(pid: i32) -> Option<Vec<String>> {
    let mut buffer = vec![0u8; argument_space()?];
    let mut mib = [libc::CTL_KERN, libc::KERN_PROCARGS2, pid];
    let mut size = buffer.len();
    // SAFETY: mib names three integers, the buffer holds `size` bytes and sysctl writes no more than that into it.
    let status = unsafe {
        libc::sysctl(
            mib.as_mut_ptr(),
            3,
            buffer.as_mut_ptr().cast(),
            &raw mut size,
            std::ptr::null_mut(),
            0,
        )
    };
    if status != 0 {
        return None;
    }
    buffer.truncate(size);
    crate::foreground::parse_procargs(&buffer)
}

/// The most bytes a process's arguments and environment take, which is what `KERN_PROCARGS2` needs room for.
fn argument_space() -> Option<usize> {
    static SPACE: OnceLock<Option<usize>> = OnceLock::new();
    *SPACE.get_or_init(|| {
        let mut mib = [libc::CTL_KERN, libc::KERN_ARGMAX];
        let mut value: libc::c_int = 0;
        let mut size = size_of::<libc::c_int>();
        // SAFETY: mib names two integers and the output is one c_int whose size is passed along.
        let status = unsafe {
            libc::sysctl(
                mib.as_mut_ptr(),
                2,
                (&raw mut value).cast(),
                &raw mut size,
                std::ptr::null_mut(),
                0,
            )
        };
        if status == 0 { usize::try_from(value).ok() } else { None }
    })
}

/// The short name the system keeps for `pid`, for when its arguments cannot be read.
pub(crate) fn process_name(pid: i32) -> Option<String> {
    let mut buffer = [0u8; 256];
    let capacity = u32::try_from(buffer.len()).ok()?;
    // SAFETY: the buffer holds `capacity` bytes and proc_name writes no more than that, ending with a zero byte.
    let length = unsafe { libc::proc_name(pid, buffer.as_mut_ptr().cast(), capacity) };
    let length = usize::try_from(length).ok().filter(|&n| n > 0)?;
    Some(String::from_utf8_lossy(&buffer[..length.min(buffer.len())]).into_owned())
}

/// Sends `signal` to every process in group `group`; whether the group existed.
pub(crate) fn signal_group(group: i32, signal: libc::c_int) -> bool {
    if group <= 1 {
        return false;
    }
    // SAFETY: kill only sends a signal; a group number above 1 can never address every process.
    unsafe { libc::kill(-group, signal) == 0 }
}

/// Whether any process is left in group `group`.
pub(crate) fn group_exists(group: i32) -> bool {
    if group <= 1 {
        return false;
    }
    // SAFETY: Signal 0 only asks whether the group exists; nothing is delivered.
    let status = unsafe { libc::kill(-group, 0) };
    status == 0 || io::Error::last_os_error().raw_os_error() == Some(libc::EPERM)
}

/// Tells the PTY its size, which sends the foreground job SIGWINCH.
pub(crate) fn set_window_size(master: &File, size: &libc::winsize) -> io::Result<()> {
    // SAFETY: TIOCSWINSZ reads one winsize from the pointer, which refers to a live value.
    let status = unsafe { libc::ioctl(master.as_raw_fd(), libc::TIOCSWINSZ, std::ptr::from_ref(size)) };
    if status == 0 {
        Ok(())
    } else {
        Err(io::Error::last_os_error())
    }
}

/// This Mac's host name, as shells report it in OSC 7.
pub(crate) fn host_name() -> &'static str {
    static NAME: OnceLock<String> = OnceLock::new();
    NAME.get_or_init(|| {
        let mut buffer = [0 as libc::c_char; 256];
        // SAFETY: the buffer holds its length in bytes; gethostname writes no more and the last byte stays zero.
        let status = unsafe { libc::gethostname(buffer.as_mut_ptr(), buffer.len() - 1) };
        if status != 0 {
            return String::new();
        }
        // SAFETY: the buffer was zeroed and its last byte never written, so it ends with a zero byte.
        unsafe { CStr::from_ptr(buffer.as_ptr()) }
            .to_string_lossy()
            .into_owned()
    })
}

/// Adds the PTY's descriptor and its child-exit signal to `poller`.
pub(crate) fn register(pty: &mut Pty, poller: &Arc<Poller>, interest: Event) -> io::Result<()> {
    // SAFETY: the reader thread owns both the PTY and the poller and calls `deregister` before either is dropped.
    unsafe { pty.register(poller, interest, PollMode::Level) }
}

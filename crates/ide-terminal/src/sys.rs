//! The calls into the C library: the foreground group, process arguments, signals, the PTY's window size, waiting for
//! output and the host name; what concerns any process is in `ide_process::system`.

use std::ffi::CStr;
use std::fs::File;
use std::io;
use std::os::fd::AsRawFd;
use std::sync::OnceLock;

/// The foreground process group of the terminal whose master side is `master`.
pub(crate) fn foreground_group(master: &File) -> Option<i32> {
    // SAFETY: tcgetpgrp only reads the state of a descriptor that `master` keeps open.
    let group = unsafe { libc::tcgetpgrp(master.as_raw_fd()) };
    (group > 0).then_some(group)
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

/// Sends `signal` to every process in group `group`; whether the group existed.
pub(crate) fn signal_group(group: i32, signal: libc::c_int) -> bool {
    if group <= 1 {
        return false;
    }
    // SAFETY: kill only sends a signal; a group number above 1 can never address every process.
    unsafe { libc::kill(-group, signal) == 0 }
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

/// Whether `file` can be read, or is hung up, within `milliseconds`; false when the time ran out.
pub(crate) fn wait_readable(file: &File, milliseconds: i32) -> bool {
    let mut wanted = libc::pollfd {
        fd: file.as_raw_fd(),
        events: libc::POLLIN,
        revents: 0,
    };
    // SAFETY: poll reads and writes the one pollfd it is given, which lives for the length of the call.
    let ready = unsafe { libc::poll(&raw mut wanted, 1, milliseconds) };
    ready > 0 || (ready < 0 && io::Error::last_os_error().kind() != io::ErrorKind::Interrupted)
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

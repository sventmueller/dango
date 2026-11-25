//! A sampler of the main thread that shows what a long step does, on with `IDE_SELFTEST_STACKS`: every millisecond it
//! reads the main thread's frames, and after each step from the stall threshold on, or from the variable's number of
//! milliseconds, it prints the functions its samples were in, most often first (see "Selftest" in docs/development.md).

use std::cell::Cell;
use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::ffi::{CStr, c_void};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use ide_core::LockIgnoringPoison;
use objc2_core_foundation::{CFRetained, CFRunLoop, CFRunLoopActivity, CFRunLoopObserver, kCFRunLoopCommonModes};

use crate::util::guarded;

/// The variable that turns the sampler on.
const VARIABLE: &str = "IDE_SELFTEST_STACKS";
const EVERY: Duration = Duration::from_millis(1);
/// How long samples are kept, longer than any step worth a look.
const KEPT: Duration = Duration::from_secs(2);
const DEPTH: usize = 96;
/// How many of the functions a long step's samples were in it prints.
const SHOWN: usize = 40;

type Samples = Arc<Mutex<VecDeque<(Instant, Vec<usize>)>>>;

/// `thread_get_state`'s flavor for the registers of an arm64 thread.
const ARM_THREAD_STATE64: i32 = 6;

/// `arm_thread_state64_t`.
#[repr(C)]
#[derive(Default)]
struct ThreadState {
    x: [u64; 29],
    fp: u64,
    lr: u64,
    sp: u64,
    pc: u64,
    cpsr: u32,
    flags: u32,
}

unsafe extern "C" {
    fn thread_suspend(thread: libc::mach_port_t) -> libc::kern_return_t;
    fn thread_resume(thread: libc::mach_port_t) -> libc::kern_return_t;
    fn thread_get_state(
        thread: libc::mach_port_t,
        flavor: i32,
        state: *mut u32,
        count: *mut u32,
    ) -> libc::kern_return_t;
}

/// Starts sampling the main thread and reporting its steps from `threshold` on, if `IDE_SELFTEST_STACKS` is set;
/// called on the main thread.
pub(super) fn install(threshold: Duration) {
    let Some(value) = std::env::var_os(VARIABLE) else {
        return;
    };
    // A number of milliseconds reports from there on, for steps shorter than a stall such as a scroll's frames.
    let threshold = value
        .to_str()
        .and_then(|ms| ms.parse().ok())
        .map_or(threshold, Duration::from_millis);
    // SAFETY: The calling thread is the main thread; its port and stack stay as long as the process.
    let (port, low, high) = unsafe {
        let me = libc::pthread_self();
        let top = libc::pthread_get_stackaddr_np(me) as usize;
        (
            libc::pthread_mach_thread_np(me),
            top - libc::pthread_get_stacksize_np(me),
            top,
        )
    };
    let samples: Samples = Arc::default();
    let theirs = Arc::clone(&samples);
    if let Err(why) = crate::util::spawn("selftest stacks", move || sample(port, low, high, &theirs)) {
        super::check("the main thread is sampled", false, why.0);
        return;
    }
    observe(samples, threshold);
}

fn sample(port: libc::mach_port_t, low: usize, high: usize, samples: &Samples) {
    loop {
        std::thread::sleep(EVERY);
        let frames = frames(port, low, high);
        let now = Instant::now();
        let mut kept = samples.lock_ignoring_poison();
        while kept.front().is_some_and(|(at, _)| now.duration_since(*at) > KEPT) {
            kept.pop_front();
        }
        kept.push_back((now, frames));
    }
}

/// The return addresses on the main thread's stack now, innermost first. Nothing allocates while the thread is
/// suspended, as it may hold the allocator's lock.
fn frames(port: libc::mach_port_t, low: usize, high: usize) -> Vec<usize> {
    let mut frames = Vec::with_capacity(DEPTH);
    let mut state = ThreadState::default();
    let mut count = u32::try_from(size_of::<ThreadState>() / 4).unwrap_or(0);
    // SAFETY: The main thread's port is valid for the life of the process; it is resumed below on every path.
    if unsafe { thread_suspend(port) } != 0 {
        return frames;
    }
    // SAFETY: `state` holds `count` 32-bit words of the flavor's layout.
    let got = unsafe { thread_get_state(port, ARM_THREAD_STATE64, (&raw mut state).cast(), &raw mut count) };
    if got == 0 {
        frames.push(usize::try_from(state.pc).unwrap_or(0));
        let mut fp = usize::try_from(state.fp).unwrap_or(0);
        while frames.len() < DEPTH && fp >= low && fp + 16 <= high && fp.is_multiple_of(8) {
            // SAFETY: `fp` lies within the suspended main thread's stack, where a frame record holds the previous
            // frame pointer and the return address.
            let (next, back) = unsafe { (*(fp as *const usize), *((fp + 8) as *const usize)) };
            if back == 0 {
                break;
            }
            frames.push(back);
            if next <= fp {
                break;
            }
            fp = next;
        }
    }
    // SAFETY: Resumes the thread suspended above.
    unsafe { thread_resume(port) };
    frames
}

fn observe(samples: Samples, threshold: Duration) {
    let Some(main) = CFRunLoop::main() else { return };
    let last = Cell::new(Instant::now());
    let block = block2::RcBlock::new(move |_: *mut CFRunLoopObserver, activity: CFRunLoopActivity| {
        guarded((), || {
            let now = Instant::now();
            let start = last.replace(now);
            if activity.contains(CFRunLoopActivity::AfterWaiting) || activity.contains(CFRunLoopActivity::Entry) {
                return;
            }
            if now.duration_since(start) >= threshold {
                report(&samples, start, now);
            }
        });
    });
    // SAFETY: The default allocator and a block the run loop calls on the main thread.
    let observer: Option<CFRetained<CFRunLoopObserver>> =
        unsafe { CFRunLoopObserver::with_handler(None, CFRunLoopActivity::AllActivities.0, true, 0, Some(&block)) };
    let Some(observer) = observer else { return };
    // SAFETY: Reading a constant CoreFoundation exports.
    let common = unsafe { kCFRunLoopCommonModes };
    main.add_observer(Some(&observer), common);
    // The run loop keeps the observer as long as the app runs.
    std::mem::forget(observer);
}

/// Prints the functions the samples between `start` and `end` were in, with how many samples each, most first.
fn report(samples: &Samples, start: Instant, end: Instant) {
    let taken: Vec<Vec<usize>> = samples
        .lock_ignoring_poison()
        .iter()
        .filter(|(at, _)| *at >= start && *at <= end)
        .map(|(_, frames)| frames.clone())
        .collect();
    let mut names: BTreeMap<usize, String> = BTreeMap::new();
    let mut inclusive: BTreeMap<String, usize> = BTreeMap::new();
    let mut leaves: BTreeMap<String, usize> = BTreeMap::new();
    for frames in &taken {
        let mut seen = BTreeSet::new();
        for (depth, address) in frames.iter().enumerate() {
            let name = names.entry(*address).or_insert_with(|| symbol(*address)).clone();
            if depth == 0 {
                *leaves.entry(name.clone()).or_default() += 1;
            }
            if seen.insert(name.clone()) {
                *inclusive.entry(name).or_default() += 1;
            }
        }
    }
    let ranked = |counts: BTreeMap<String, usize>| {
        let mut counts: Vec<(String, usize)> = counts.into_iter().collect();
        counts.sort_by_key(|(_, count)| std::cmp::Reverse(*count));
        counts.truncate(SHOWN);
        counts
    };
    let took = end.duration_since(start).as_millis();
    let mut text = format!("stacks of a {took} ms step, {} samples:\n", taken.len());
    for (name, count) in ranked(inclusive) {
        text.push_str(&format!("  {count:4} {name}\n"));
    }
    text.push_str("  innermost:\n");
    for (name, count) in ranked(leaves).into_iter().take(10) {
        text.push_str(&format!("  {count:4} {name}\n"));
    }
    eprint!("{text}");
}

fn symbol(address: usize) -> String {
    // SAFETY: `Dl_info` is plain data that `dladdr` fills.
    let mut info: libc::Dl_info = unsafe { std::mem::zeroed() };
    // SAFETY: `dladdr` only reads the loader's tables for the address.
    if unsafe { libc::dladdr(address as *const c_void, &raw mut info) } == 0 || info.dli_sname.is_null() {
        return format!("{address:#x}");
    }
    // SAFETY: `dladdr` set the name to a string of the image's symbol table.
    let name = unsafe { CStr::from_ptr(info.dli_sname) }.to_string_lossy();
    demangled(&name)
}

/// A Rust symbol of the legacy mangling as a path without its hash; any other name as it is.
fn demangled(name: &str) -> String {
    let Some(mut rest) = name.trim_start_matches('_').strip_prefix("ZN") else {
        return name.to_string();
    };
    let mut parts = Vec::new();
    while let Some(digits) = rest.find(|c: char| !c.is_ascii_digit()).filter(|n| *n > 0) {
        let Ok(length) = rest[..digits].parse::<usize>() else {
            break;
        };
        let Some(part) = rest.get(digits..digits + length) else {
            break;
        };
        parts.push(part);
        rest = &rest[digits + length..];
    }
    if parts
        .last()
        .is_some_and(|last| last.len() == 17 && last.starts_with('h'))
    {
        parts.pop();
    }
    let path = parts.join("::");
    [
        ("$LT$", "<"),
        ("$GT$", ">"),
        ("$RF$", "&"),
        ("$BP$", "*"),
        ("$C$", ","),
        ("$u20$", " "),
        ("$u7b$", "{"),
        ("$u7d$", "}"),
        ("$u27$", "'"),
        ("..", "::"),
    ]
    .iter()
    .fold(path, |path, (from, to)| path.replace(from, to))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_legacy_rust_symbol_reads_as_its_path() {
        assert_eq!(
            demangled("__ZN8ide_core4diag6report17h0123456789abcdefE"),
            "ide_core::diag::report"
        );
        assert_eq!(
            demangled("__ZN62_$LT$ide$u20$as$u20$core..fmt..Debug$GT$3fmt17h0123456789abcdefE"),
            "<ide as core::fmt::Debug>::fmt"
        );
        assert_eq!(demangled("_CFRunLoopRun"), "_CFRunLoopRun");
    }
}

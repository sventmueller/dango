//! Saving for the self-test, which checks the file right after: the save runs as in the product, and the run loop
//! turns until it is over.

use std::cell::Cell;
use std::rc::Rc;
use std::time::{Duration, Instant};

use objc2_foundation::{NSDate, NSDefaultRunLoopMode, NSRunLoop};

use super::EditorTextView;

/// How long a self-test waits for a save, longer than any write of its small files takes.
const SAVE_WAIT: Duration = Duration::from_secs(10);

/// Turns the main run loop until `done` holds or `limit` passed; whether it holds.
pub(crate) fn run_until(done: impl Fn() -> bool, limit: Duration) -> bool {
    let end = Instant::now() + limit;
    while !done() && Instant::now() < end {
        let step = NSDate::dateWithTimeIntervalSinceNow(0.01);
        // SAFETY: Reading a constant Foundation exports.
        let mode = unsafe { NSDefaultRunLoopMode };
        NSRunLoop::currentRunLoop().runMode_beforeDate(mode, &step);
    }
    done()
}

/// Saves `view` and waits until the save is over; `save_as` always asks for a file. Whether the file was written.
pub(crate) fn save(view: &EditorTextView, save_as: bool) -> bool {
    let written = Rc::new(Cell::new(None));
    let heard = Rc::clone(&written);
    super::save_then(view, save_as, Box::new(move |_, done| heard.set(Some(done))));
    run_until(|| written.get().is_some(), SAVE_WAIT);
    written.get() == Some(true)
}

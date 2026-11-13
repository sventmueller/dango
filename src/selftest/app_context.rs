//! The way back from worker threads (E97, Review C11): an answer carries the id of its window, and one for a window
//! that closed before it arrived is dropped.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use objc2::MainThreadMarker;

use super::{check, finish};
use crate::window::{self, WindowController};

/// How long the worker takes for its answer; the window closes right away. Once the open window's answer came, the
/// closed window's answer gets this long again, since a dropped answer leaves nothing to wait for.
const WORK: Duration = Duration::from_millis(300);

pub fn run() {
    let Some(mtm) = MainThreadMarker::new() else { return };
    let Some(open) = window::open_windows().first().cloned() else {
        check("a window opens", false, "none");
        finish("app-context");
        return;
    };
    let closing = WindowController::open_new(mtm, None);
    let closed_id = closing.id();
    let reached_open = answer_later(open.id());
    let reached_closed = answer_later(closed_id);
    if let Some(window) = closing.window() {
        window.close();
    }
    drop(closing);
    let open_flag = Arc::clone(&reached_open);
    super::helpers::wait_until(
        move || open_flag.load(Ordering::SeqCst),
        move |_| {
            super::helpers::after(WORK, move || {
                let gone = crate::context::app().windows.get(closed_id).is_none();
                check(
                    "a window that closed leaves the open ones",
                    gone,
                    format!("window {closed_id}"),
                );
                check(
                    "a worker's answer for a window that closed before it came is dropped",
                    !reached_closed.load(Ordering::SeqCst),
                    "",
                );
                check(
                    "a worker's answer for an open window reaches it",
                    reached_open.load(Ordering::SeqCst),
                    "",
                );
                finish("app-context");
            })
        },
    );
}

/// Starts a worker whose answer goes to the window `id` after `WORK`; the flag says whether it got there.
fn answer_later(id: u64) -> Arc<AtomicBool> {
    let reached = Arc::new(AtomicBool::new(false));
    let flag = Arc::clone(&reached);
    std::thread::spawn(move || {
        std::thread::sleep(WORK);
        window::deliver(id, move |controller| {
            flag.store(controller.id() == id, Ordering::SeqCst);
        });
    });
    reached
}

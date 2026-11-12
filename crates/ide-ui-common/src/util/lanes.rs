use ide_core::lane::contained;

/// What a callback from the system does, with `fallback` if a bug in it panics; a panic would otherwise unwind through
/// the system's frames.
pub fn guarded<R>(fallback: R, f: impl FnOnce() -> R) -> R {
    contained(f).unwrap_or(fallback)
}

/// Runs `f` on the main thread when its turn comes. dispatch2 calls it through `extern "C"`, so a panic that left it
/// would end the app.
pub fn main_soon(f: impl FnOnce() + Send + 'static) {
    dispatch2::DispatchQueue::main().exec_async(move || {
        contained(f);
    });
}

/// `main_soon` after `delay`.
pub fn main_after(delay: std::time::Duration, f: impl FnOnce() + Send + 'static) {
    if let Ok(when) = dispatch2::DispatchTime::try_from(delay) {
        let _ = dispatch2::DispatchQueue::main().after(when, move || {
            contained(f);
        });
    }
}

/// Runs `f` on the main thread from the run loop instead of the main dispatch queue, for work that may show a modal
/// alert: the queue runs one callback at a time, so a modal session inside one holds up every other until it ends.
pub fn main_on_run_loop(f: impl FnOnce() + Send + 'static) {
    let f = std::cell::Cell::new(Some(f));
    let block = block2::RcBlock::new(move || {
        if let Some(f) = f.take() {
            contained(f);
        }
    });
    // SAFETY: The block holds only what may go to another thread, and the main run loop calls it on the main thread.
    unsafe { objc2_foundation::NSRunLoop::mainRunLoop().performBlock(&block) };
}

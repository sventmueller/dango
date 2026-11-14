//! The one way the app's AppKit code starts a thread for one piece of work, on `ide_core::thread::detach`: named, and
//! with a failure to start that comes back to the caller in the user's words instead of a panic.

use std::sync::{Arc, Mutex, mpsc};

use ide_core::LockIgnoringPoison;
use ide_core::explain::{Explain, Explained};

/// Runs `f` on a new thread called `name`; the error says, in the user's words, why the thread did not start.
pub fn spawn(name: &str, f: impl FnOnce() + Send + 'static) -> Result<(), Explained> {
    spawn_with(name, (), move |()| f()).map_err(|((), why)| why)
}

/// `spawn` that hands `payload` to `f`, or back with the reason when the thread did not start, so that a callback in
/// it can still be told.
pub fn spawn_with<T: Send + 'static>(
    name: &str,
    payload: T,
    f: impl FnOnce(T) + Send + 'static,
) -> Result<(), (T, Explained)> {
    let slot = Arc::new(Mutex::new(Some(payload)));
    let theirs = Arc::clone(&slot);
    let started = ide_core::thread::detach(name, move || {
        let payload = theirs.lock_ignoring_poison().take();
        if let Some(payload) = payload {
            f(payload);
        }
    });
    started.map_err(|unstarted| {
        drop(unstarted.work);
        let why = Explained(splat_i18n::tf(
            "Konnte keinen Thread starten: {}",
            &[&unstarted.error.explain()],
        ));
        let payload = slot.lock_ignoring_poison().take();
        (payload.expect("a thread that never started took nothing"), why)
    })
}

/// The result of work on its own thread, to wait for.
pub struct Joined<T>(mpsc::Receiver<T>);

impl<T> Joined<T> {
    /// Waits for the work's result; `None` if it panicked.
    pub fn join(self) -> Option<T> {
        self.0.recv().ok()
    }
}

/// Runs `f` on a new thread called `name` and hands back what to wait on for its result.
pub fn spawn_joined<T: Send + 'static>(
    name: &str,
    f: impl FnOnce() -> T + Send + 'static,
) -> Result<Joined<T>, Explained> {
    let (sent, received) = mpsc::sync_channel(1);
    spawn(name, move || {
        let _ = sent.send(f());
    })?;
    Ok(Joined(received))
}

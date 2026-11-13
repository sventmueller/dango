use ide_core::LockIgnoringPoison;

/// Runs `f` on a new thread called `name`; the error says, in the user's words, why the thread did not start.
pub fn spawn(name: &str, f: impl FnOnce() + Send + 'static) -> Result<(), String> {
    spawn_with(name, (), move |()| f()).map_err(|((), why)| why)
}

/// `spawn` that hands `payload` to `f`, or back with the reason when the thread did not start, so that a callback in
/// it can still be told.
pub fn spawn_with<T: Send + 'static>(
    name: &str,
    payload: T,
    f: impl FnOnce(T) + Send + 'static,
) -> Result<(), (T, String)> {
    let slot = std::sync::Arc::new(std::sync::Mutex::new(Some(payload)));
    let theirs = slot.clone();
    let started = std::thread::Builder::new().name(name.into()).spawn(move || {
        let payload = theirs.lock_ignoring_poison().take();
        if let Some(payload) = payload {
            f(payload);
        }
    });
    let Err(error) = started else { return Ok(()) };
    let payload = slot
        .lock_ignoring_poison()
        .take()
        .expect("a thread that never started took nothing");
    Err((
        payload,
        splat_i18n::tf("Konnte keinen Thread starten: {}", &[&error.to_string()]),
    ))
}

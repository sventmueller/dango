//! The cold start: from `open` to the window, to the first frame with text and to the first key the app takes.

use std::path::Path;
use std::time::{Duration, Instant};

use crate::compare::apps;
use crate::compare::capture::Capture;
use crate::compare::clock;
use crate::compare::detect;
use crate::compare::fingerprint::EDITOR;
use crate::compare::input::{self, Input};
use crate::compare::interrupt;
use crate::compare::keys::Layout;
use crate::compare::options::App;
use crate::compare::stats::ms;

/// The times of one cold start in milliseconds from `open`; none for a step that did not happen in time.
#[derive(Debug, Clone, Default)]
pub(crate) struct Cold {
    pub(crate) window_ms: Option<f64>,
    pub(crate) content_ms: Option<f64>,
    pub(crate) key_ms: Option<f64>,
}

/// Keys go out this often until one shows.
const KEY_EVERY: Duration = Duration::from_millis(100);
const KEY_LIMIT: usize = 100;

/// Starts the capture as soon as ScreenCaptureKit lists the new process.
fn capture(pid: libc::pid_t, window: apps::Window) -> Result<Capture, String> {
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        interrupt::check()?;
        match Capture::start(pid, window.frame, EDITOR, Duration::from_secs(5)) {
            Ok(capture) => return Ok(capture),
            Err(error) if Instant::now() > deadline => return Err(error),
            Err(_) => std::thread::sleep(Duration::from_millis(20)),
        }
    }
}

/// The middle of the editor region of `window`.
fn editor_center(window: apps::Window) -> objc2_core_foundation::CGPoint {
    let frame = window.frame;
    objc2_core_foundation::CGPoint::new(
        frame.origin.x + frame.size.width * (EDITOR.left + EDITOR.right) / 2.0,
        frame.origin.y + frame.size.height * (EDITOR.top + EDITOR.bottom) / 2.0,
    )
}

/// Waits up to `limit` for `find` to see something in the frames.
fn wait_for(
    capture: &Capture,
    limit: Duration,
    find: impl Fn(&[detect::Frame]) -> Option<u64>,
) -> Result<Option<u64>, String> {
    let deadline = Instant::now() + limit;
    while Instant::now() < deadline {
        interrupt::sleep(Duration::from_millis(5))?;
        if let Some(at) = capture.with_frames(&find) {
            return Ok(Some(at));
        }
    }
    Ok(None)
}

/// Types `x` every `KEY_EVERY` until a frame shows a typed line; returns when it showed and how many keys went out.
fn first_key(capture: &Capture, input: &Input) -> Result<(Option<u64>, usize), String> {
    let first = clock::now_ns();
    for sent in 1..=KEY_LIMIT {
        input.char('x')?;
        interrupt::sleep(KEY_EVERY)?;
        if let Some(at) = capture.with_frames(|frames| detect::first_typed_line(frames, first)) {
            return Ok((Some(at), sent));
        }
    }
    Ok((None, KEY_LIMIT))
}

/// One cold start of `app` in `home` opening `file`, which an earlier launch left with the project open; quits the
/// instance again.
pub(crate) fn cold_start(app: App, home: &Path, file: &Path, layout: &Layout) -> Result<Cold, String> {
    // Both apps get the file on launch, as from Finder, so the first key needs no way through a file tree.
    let mut instance = apps::launch(app, home, &[file])?;
    let opened = instance.opened_ns;
    let (window, seen) = instance.window(Duration::from_secs(30))?;
    let mut cold = Cold {
        window_ms: Some(ms(seen - opened)),
        ..Cold::default()
    };
    let capture = capture(instance.pid, window)?;
    let content = wait_for(&capture, Duration::from_secs(20), |frames| {
        detect::first_content(frames, 0)
    })?;
    cold.content_ms = content.map(|at| ms(at.saturating_sub(opened)));
    if content.is_none() {
        let counts = capture.counts();
        eprintln!(
            "    cold: no frame with text within 20 s; {} complete frames, highest spread {}, needed {}",
            counts.complete,
            capture.with_frames(|frames| frames.iter().map(|f| f.spread).max().unwrap_or(0)),
            detect::CONTENT_SPREAD
        );
    } else {
        let input = Input::new(instance.pid, layout.clone())?;
        // A person clicks into the text before typing; the restored focus may be elsewhere.
        apps::activate(instance.pid);
        let role = apps::focused_role(instance.pid);
        // This app's welcome page has a button in the middle that opens a folder panel; click only into text.
        if app == App::Zed || role.as_deref() == Some("AXTextArea") {
            input.click(editor_center(window), window.id)?;
        } else {
            eprintln!("    cold: no click, the focused element is {role:?}, not a text area");
        }
        interrupt::sleep(Duration::from_millis(200))?;
        if let Some(dialog) = crate::compare::tree::dialog(instance.pid) {
            eprintln!("    cold: the click opened a {dialog}; the editor was not restored there; pressing Escape");
            input.key(crate::compare::keys::ESCAPE, input::Mods::default())?;
        }
        let (key, sent) = first_key(&capture, &input)?;
        cold.key_ms = key.map(|at| ms(at.saturating_sub(opened)));
        if key.is_none() {
            let counts = capture.counts();
            eprintln!(
                "    cold: {sent} keys without a frame that changed like a typed line; {} complete frames ({} \
                 changed); front process {:?} (measured {}); focused element {}",
                counts.complete,
                counts.changed,
                apps::frontmost(),
                instance.pid,
                apps::focused_role(instance.pid).unwrap_or_else(|| "unknown".into())
            );
        }
        interrupt::sleep(Duration::from_millis(300))?;
        for _ in 0..sent {
            input.undo()?;
            std::thread::sleep(Duration::from_millis(30));
        }
        interrupt::sleep(Duration::from_millis(1500))?;
    }
    drop(capture);
    instance.quit(Duration::from_secs(15));
    Ok(cold)
}

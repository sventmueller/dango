//! The measurements against a running instance: a key to the screen, a smooth scroll and a tab switch.

use std::path::PathBuf;
use std::time::Duration;

use objc2_core_foundation::{CGPoint, CGRect, CGSize};
use objc2_core_graphics::CGWarpMouseCursorPosition;

use crate::compare::apps::{self, Window};
use crate::compare::capture::Capture;
use crate::compare::clock;
use crate::compare::debug;
use crate::compare::detect;
use crate::compare::fingerprint::{Changes, Fingerprint, THRESHOLD};
use crate::compare::input::{self, Input, Phase};
use crate::compare::interrupt;
use crate::compare::keys;
use crate::compare::options::App;
use crate::compare::stats::{self, Smoothness};
use crate::compare::tree;

/// How long nothing may change for the picture to count as idle.
const QUIET: Duration = Duration::from_millis(300);
/// The longest wait for idleness; an app that never rests is measured anyway.
const SETTLE_LIMIT: Duration = Duration::from_secs(4);
/// How long the picture is watched for what changes on its own, longer than a cursor's blink.
const WATCH: Duration = Duration::from_millis(1100);
/// The longest wait for an input's change.
const ANSWER_LIMIT: Duration = Duration::from_secs(1);
/// The scroll: an event every 8.33 ms, as a trackpad sends at 120 Hz, for three seconds.
const SCROLL_STEP_NS: u64 = 8_333_333;
const SCROLL_EVENTS: u64 = 360;
const SCROLL_DELTA: i32 = 6;

/// A measured instance with its input and its capture.
pub(crate) struct Session<'a> {
    pub(crate) input: &'a Input,
    pub(crate) capture: &'a Capture,
    pub(crate) window: Window,
    pub(crate) refresh_hz: f64,
    pub(crate) app: App,
    /// Where `--debug-frames` writes, with the app's name for the files.
    pub(crate) debug: Option<(PathBuf, &'static str)>,
}

/// Latencies in milliseconds and how many inputs saw no change in time.
#[derive(Debug, Clone, Default)]
pub(crate) struct Samples {
    pub(crate) ms: Vec<f64>,
    pub(crate) missed: usize,
    /// Of the missed: inputs after which no frame changed in the measured region at all.
    pub(crate) unchanged: usize,
    /// Of the missed: inputs after which frames changed only where the picture also changes on its own.
    pub(crate) masked: usize,
    /// The largest mask of a sample, in cells.
    pub(crate) mask_cells: usize,
}

/// What came of one input.
enum Answer {
    Seen(f64),
    Unchanged,
    Masked,
}

impl Session<'_> {
    fn fail_if_stopped(&self) -> Result<(), String> {
        match self.capture.stopped() {
            Some(why) => Err(format!("the capture stopped: {why}")),
            None => Ok(()),
        }
    }

    /// Waits until nothing changed for `QUIET`, at most `SETTLE_LIMIT`.
    pub(crate) fn settle(&self) -> Result<(), String> {
        let start = clock::now_ns();
        loop {
            interrupt::sleep(Duration::from_millis(20))?;
            self.fail_if_stopped()?;
            let now = clock::now_ns();
            let last = self
                .capture
                .with_frames(detect::last_change)
                .unwrap_or(start)
                .max(start);
            if now.saturating_sub(last) >= nanos(QUIET) || now.saturating_sub(start) >= nanos(SETTLE_LIMIT) {
                self.capture.clear();
                return Ok(());
            }
        }
    }

    /// Settles, then watches what changes without input.
    fn idle_mask(&self) -> Result<Option<Changes>, String> {
        self.settle()?;
        let from = clock::now_ns();
        interrupt::sleep(WATCH)?;
        let to = clock::now_ns();
        Ok(self.capture.with_frames(|frames| detect::mask(frames, from, to)))
    }

    /// Posts an input with `post` and waits for the first change outside `mask`.
    fn answer(&self, mask: Option<&Changes>, post: impl FnOnce() -> Result<u64, String>) -> Result<Answer, String> {
        let at = post()?;
        let deadline = at + nanos(ANSWER_LIMIT);
        loop {
            interrupt::sleep(Duration::from_millis(5))?;
            if let Some(seen) = self
                .capture
                .with_frames(|frames| detect::first_change(frames, at, mask))
            {
                return Ok(Answer::Seen(stats::ms(seen - at)));
            }
            if clock::now_ns() > deadline {
                let any = self
                    .capture
                    .with_frames(|frames| detect::first_change(frames, at, None));
                return Ok(if any.is_some() {
                    Answer::Masked
                } else {
                    Answer::Unchanged
                });
            }
        }
    }

    /// Saves the latest whole frame for `--debug-frames`.
    fn debug_frame(&self, phase: &str, when: &str) {
        let Some((dir, app)) = &self.debug else {
            return;
        };
        let Some(snapshot) = self.capture.snapshot() else {
            eprintln!("    {phase}: no frame to save {when} the input");
            return;
        };
        let name = format!("{app}-{phase}");
        match debug::write(dir, &name, when, &snapshot, self.capture.roi()) {
            Ok(()) => eprintln!(
                "    {phase}: saved {}",
                dir.join(format!("{name}-{when}.png")).display()
            ),
            Err(e) => eprintln!("    {phase}: {e}"),
        }
    }

    /// Measures `runs` inputs of `post`, each after idleness, running `after` unmeasured once it was seen.
    fn samples(
        &self,
        phase: &str,
        runs: usize,
        mut post: impl FnMut(usize) -> Result<u64, String>,
        mut after: impl FnMut(usize) -> Result<(), String>,
    ) -> Result<Samples, String> {
        let mut samples = Samples::default();
        for run in 0..runs {
            let mask = self.idle_mask()?;
            samples.mask_cells = samples.mask_cells.max(mask.as_ref().map_or(0, Changes::count));
            if run == 0 {
                self.debug_frame(phase, "before");
            }
            match self.answer(mask.as_ref(), || post(run))? {
                Answer::Seen(ms) => samples.ms.push(ms),
                Answer::Unchanged => (samples.missed, samples.unchanged) = (samples.missed + 1, samples.unchanged + 1),
                Answer::Masked => (samples.missed, samples.masked) = (samples.missed + 1, samples.masked + 1),
            }
            if run == 0 {
                interrupt::sleep(Duration::from_millis(200))?;
                self.debug_frame(phase, "after");
            }
            after(run)?;
        }
        self.explain(phase, &samples);
        Ok(samples)
    }

    /// Says on stderr how a phase went, and for inputs without an answer what the capture saw.
    pub(crate) fn explain(&self, phase: &str, samples: &Samples) {
        let counts = self.capture.counts();
        let pid = self.input.pid();
        eprintln!(
            "    {phase}: {} seen, {} without any changed frame in the measured region within {} s, {} changed only \
             where the picture also changes on its own (mask up to {} cells)",
            samples.ms.len(),
            samples.unchanged,
            ANSWER_LIMIT.as_secs(),
            samples.masked,
            samples.mask_cells
        );
        if samples.missed > 0 || samples.ms.is_empty() {
            eprintln!(
                "    {phase}: capture so far {} complete frames ({} changed), {} idle or blank, {} unreadable, last \
                 frame {:.1} ms after its display time; front process {:?} (measured {pid}); focused element {}",
                counts.complete,
                counts.changed,
                counts.other,
                counts.unreadable,
                counts.lag_ns as f64 / 1e6,
                apps::frontmost(),
                apps::focused_role(pid).unwrap_or_else(|| "unknown".into())
            );
        }
    }

    /// The middle of the measured region, in points from the top left of the main display.
    fn editor_center(&self) -> CGPoint {
        let (frame, roi) = (self.window.frame, self.capture.roi());
        CGPoint::new(
            frame.origin.x + frame.size.width * (roi.left + roi.right) / 2.0,
            frame.origin.y + frame.size.height * (roi.top + roi.bottom) / 2.0,
        )
    }

    /// Types a character unmeasured and undoes it, to see that keys reach the editor before the samples rely on it.
    pub(crate) fn check_typing(&self, file: &str) -> Result<bool, String> {
        let mask = self.idle_mask()?;
        let before = self.capture.print();
        let after = self.input.char('x')?;
        let seen = self.change_after(after, ANSWER_LIMIT)?;
        let reached = seen.is_some();
        match seen {
            Some(at) => {
                eprintln!(
                    "    {file}: a typed key showed after {:.1} ms; keys reach the editor",
                    stats::ms(at - after)
                );
                interrupt::sleep(Duration::from_millis(150))?;
                self.check_undo(file, before.as_ref(), mask.as_ref())?;
            }
            None => eprintln!(
                "    {file}: a typed key changed nothing in the measured region within {} s; the editor does not \
                 have the keyboard (focused element {}), so the samples will miss",
                ANSWER_LIMIT.as_secs(),
                apps::focused_role(self.input.pid()).unwrap_or_else(|| "unknown".into())
            ),
        }
        interrupt::sleep(Duration::from_millis(300))?;
        Ok(reached)
    }

    /// Undoes the warm-up key and checks that the picture is back as before it, apart from `mask`; deletes the
    /// character with Backspace and takes that for the samples if ⌘Z did not.
    fn check_undo(&self, file: &str, before: Option<&Fingerprint>, mask: Option<&Changes>) -> Result<(), String> {
        let Some(before) = before else {
            self.input.undo()?;
            return Ok(());
        };
        let left = || -> Result<usize, String> {
            interrupt::sleep(Duration::from_millis(500))?;
            self.settle()?;
            let changed = self.capture.print().map(|now| now.changes_from(before, THRESHOLD));
            Ok(changed.map_or(usize::MAX, |c| mask.map_or_else(|| c.count(), |m| c.without(m).count())))
        };
        self.input.undo()?;
        let cells = left()?;
        if cells == 0 {
            eprintln!(
                "    {file}: ⌘Z restored the picture (focused element {})",
                self.focused()
            );
            return Ok(());
        }
        eprintln!(
            "    {file}: ⌘Z left {cells} cells changed (focused element {}); deleting with Backspace instead",
            self.focused()
        );
        self.input.undo_with_backspace();
        self.input.undo()?;
        match left()? {
            0 => eprintln!("    {file}: Backspace restored the picture; the samples delete their character with it"),
            n => eprintln!("    {file}: Backspace still left {n} cells changed; the samples' characters may pile up"),
        }
        Ok(())
    }

    fn focused(&self) -> String {
        apps::focused_role(self.input.pid()).unwrap_or_else(|| "unknown".into())
    }

    /// When the first frame after `after` changed anywhere in the measured region, waiting at most `limit`.
    fn change_after(&self, after: u64, limit: Duration) -> Result<Option<u64>, String> {
        let deadline = after + nanos(limit);
        loop {
            if let Some(at) = self
                .capture
                .with_frames(|frames| detect::first_change(frames, after, None))
            {
                return Ok(Some(at));
            }
            if clock::now_ns() > deadline {
                return Ok(None);
            }
            interrupt::sleep(Duration::from_millis(10))?;
        }
    }

    /// Fails when the measured app shows a dialog, sheet or panel that Escape does not close.
    pub(crate) fn no_dialog(&self) -> Result<(), String> {
        let pid = self.input.pid();
        let Some(dialog) = tree::dialog(pid) else {
            return Ok(());
        };
        eprintln!("    a {dialog} of the measured app is open; pressing Escape");
        self.input.key(keys::ESCAPE, input::Mods::default())?;
        interrupt::sleep(Duration::from_millis(500))?;
        match tree::dialog(pid) {
            Some(still) => Err(format!(
                "a {still} of the measured app stays open; the phase stops here"
            )),
            None => Ok(()),
        }
    }

    /// Brings the instance to the front and clicks into its editor, which then has the keyboard.
    pub(crate) fn focus_editor(&self) -> Result<(), String> {
        self.front()?;
        self.no_dialog()?;
        // Without an open file this app's centre is the welcome page, whose button there opens a folder panel.
        if self.app == App::Ours && !self.text_focused() {
            eprintln!(
                "    no click into the window: the focused element is {}, not a text area",
                self.focused()
            );
            return Ok(());
        }
        self.input.click(self.editor_center(), self.window.id)?;
        interrupt::sleep(Duration::from_millis(300))?;
        let pid = self.input.pid();
        if apps::frontmost() != Some(pid) {
            eprintln!(
                "    the measured process {pid} is not in front: {:?} is",
                apps::frontmost()
            );
        }
        Ok(())
    }

    /// Opens `file` of the project through the app's quick open.
    fn text_focused(&self) -> bool {
        apps::focused_role(self.input.pid()).as_deref() == Some("AXTextArea")
    }

    /// Waits up to `limit` for a text area to have the keyboard.
    fn wait_for_text(&self, limit: Duration) -> Result<bool, String> {
        let deadline = std::time::Instant::now() + limit;
        while std::time::Instant::now() < deadline {
            if self.text_focused() {
                return Ok(true);
            }
            interrupt::sleep(Duration::from_millis(50))?;
        }
        Ok(self.text_focused())
    }

    /// Starts opening `file`: a click on its row in the file tree where Accessibility shows one, else, as for Zed,
    /// whose project panel it does not show, its quick open; the time right before.
    fn start_opening(&self, file: &str) -> Result<u64, String> {
        if let Some((row, what)) = tree::find_named(self.input.pid(), file, self.window.frame) {
            eprintln!("    {file}: clicking {what}");
            let at = clock::now_ns();
            self.input.click(row, self.window.id)?;
            // A single click shows the file as a preview and keeps the keyboard in the tree; a changed editor is
            // the sign it opened, a double click the way when it did not.
            if self.app == App::Ours && self.change_after(at, Duration::from_millis(1500))?.is_none() {
                eprintln!("    {file}: the click changed nothing in the editor; double-clicking the row");
                let again = clock::now_ns();
                self.input.double_click(row, self.window.id)?;
                if self.change_after(again, Duration::from_secs(3))?.is_none() {
                    return Err(format!(
                        "{file} did not open: the editor stayed the same after a double click"
                    ));
                }
            }
            return Ok(at);
        }
        if self.app == App::Ours {
            return Err(format!("no row {file} in the file tree of the measured window"));
        }
        eprintln!("    {file}: no row in the accessibility tree; opening it through quick open");
        let at = self.input.shortcut(input::CMD, 'p')?;
        interrupt::sleep(Duration::from_millis(800))?;
        self.input.text(file)?;
        // The finder lists the recent file first until its search for the typed name is done.
        interrupt::sleep(Duration::from_millis(1500))?;
        self.input.key(keys::RETURN, input::Mods::default())?;
        Ok(at)
    }

    /// Opens `file` of the project: this app by a click on its row in the file tree, which Accessibility shows, Zed
    /// through its quick open, as its tree is not in the accessibility tree. Fails if the editor region stays the same.
    pub(crate) fn open_file(&self, file: &str) -> Result<(), String> {
        self.front()?;
        self.no_dialog()?;
        self.settle()?;
        let step = format!("open-{}", file.split('.').next().unwrap_or(file));
        self.debug_frame(&step, "before");
        let opened = match self.start_opening(file) {
            Err(error) if tree::dialog(self.input.pid()).is_some() => {
                eprintln!("    {file}: {error}; a dialog came up, closing it and trying once more");
                self.no_dialog()?;
                self.start_opening(file)
            }
            other => other,
        };
        let shown = match opened {
            Ok(started) => self.change_after(started, Duration::from_secs(3)),
            Err(error) => Err(error),
        };
        interrupt::sleep(Duration::from_millis(800))?;
        self.debug_frame(&step, "after");
        if shown?.is_none() {
            return Err(format!(
                "opening {file} changed nothing in the editor region within 3 s"
            ));
        }
        self.no_dialog()?;
        if self.app == App::Ours {
            self.click_into_text(file)?;
        }
        Ok(())
    }

    /// Clicks into the text of this app's editor, which a preview leaves without the keyboard: into its text area
    /// where Accessibility shows one, else right of the file tree, a third down.
    fn click_into_text(&self, file: &str) -> Result<(), String> {
        let pid = self.input.pid();
        let frame = self.window.frame;
        let area = tree::frame_of_role(pid, "AXTextArea", frame).or_else(|| {
            let tree = tree::frame_of_role(pid, "AXOutline", frame)?;
            let left = tree.origin.x + tree.size.width;
            Some(CGRect::new(
                CGPoint::new(left, frame.origin.y),
                CGSize::new(frame.origin.x + frame.size.width - left, frame.size.height),
            ))
        });
        let Some(area) = area else {
            return Err(format!(
                "{file}: neither a text area nor the file tree shows in the accessibility tree"
            ));
        };
        let at = CGPoint::new(
            area.origin.x + area.size.width / 2.0,
            area.origin.y + area.size.height * 0.3,
        );
        self.input.click(at, self.window.id)?;
        if self.wait_for_text(Duration::from_secs(2))? {
            eprintln!("    {file}: clicked into the text at {at:?}; a text area has the keyboard");
            Ok(())
        } else {
            Err(format!(
                "{file}: after a click into the text at {at:?} the focused element is {}",
                self.focused()
            ))
        }
    }

    /// Puts the cursor at the start of `line` through the app's go-to-line.
    pub(crate) fn go_to_line(&self, line: usize) -> Result<(), String> {
        self.input.shortcut(input::CTRL, 'g')?;
        interrupt::sleep(Duration::from_millis(500))?;
        self.input.text(&line.to_string())?;
        interrupt::sleep(Duration::from_millis(300))?;
        self.input.key(keys::RETURN, input::Mods::default())?;
        interrupt::sleep(Duration::from_millis(800))
    }

    /// A character typed at the cursor, undone with ⌘Z unmeasured after each sample.
    pub(crate) fn typing(&self, phase: &str, runs: usize) -> Result<Samples, String> {
        self.samples(
            phase,
            runs,
            |_| self.input.char('x'),
            |_| {
                interrupt::sleep(Duration::from_millis(100))?;
                self.input.undo().map(drop)
            },
        )
    }

    /// The next tab and the previous one by turns.
    pub(crate) fn tab_switch(&self, runs: usize) -> Result<Samples, String> {
        self.samples(
            "tabs",
            runs,
            |run| {
                let key = if run.is_multiple_of(2) { keys::RIGHT } else { keys::LEFT };
                self.input.key(key, input::CTRL_CMD)
            },
            |_| Ok(()),
        )
    }

    /// One scroll of three seconds, down for an even `run` and up for an odd one; the times of its changed frames.
    fn scroll_once(&self, run: usize) -> Result<(Vec<u64>, u64), String> {
        let mask = self.idle_mask()?;
        let at = self.editor_center();
        self.input.hover(at, self.window.id)?;
        interrupt::sleep(Duration::from_millis(100))?;
        if run == 0 {
            self.debug_frame("scroll", "before");
        }
        let delta = if run.is_multiple_of(2) {
            -SCROLL_DELTA
        } else {
            SCROLL_DELTA
        };
        let start = clock::now_ns();
        for i in 0..=SCROLL_EVENTS {
            interrupt::check()?;
            let due = start + i * SCROLL_STEP_NS;
            while clock::now_ns() < due {
                std::thread::sleep(Duration::from_micros(200));
            }
            let (phase, amount) = match i {
                0 => (Phase::Began, delta),
                i if i == SCROLL_EVENTS => (Phase::Ended, 0),
                _ => (Phase::Changed, delta),
            };
            self.input.scroll(amount, phase, at, self.window.id)?;
        }
        let end = clock::now_ns();
        if run == 0 {
            self.debug_frame("scroll", "after");
        }
        interrupt::sleep(Duration::from_millis(300))?;
        let times = self.capture.with_frames(|frames| {
            detect::changes_between(frames, start, end + nanos(Duration::from_millis(100)), mask.as_ref())
        });
        Ok((times, end - start))
    }

    /// `runs` scrolls, with their intervals together.
    pub(crate) fn scrolling(&self, runs: usize) -> Result<(Option<Smoothness>, Vec<f64>), String> {
        let mut parts = Vec::new();
        let mut intervals = Vec::new();
        let mut idle = 0;
        for run in 0..runs {
            let (times, duration) = self.scroll_once(run)?;
            idle += usize::from(times.len() < 2);
            intervals.extend(times.windows(2).map(|w| stats::ms(w[1] - w[0])));
            parts.extend(stats::smoothness(&times, self.refresh_hz, duration));
        }
        self.park_pointer();
        self.explain(
            "scroll",
            &Samples {
                ms: intervals.clone(),
                missed: idle,
                unchanged: idle,
                ..Samples::default()
            },
        );
        Ok((stats::combine(&parts, &intervals), intervals))
    }

    /// Moves the pointer to the window's title bar, where it hovers over nothing that reacts.
    pub(crate) fn park_pointer(&self) {
        let frame = self.window.frame;
        let _ = CGWarpMouseCursorPosition(CGPoint::new(
            frame.origin.x + frame.size.width * 0.5,
            frame.origin.y + 8.0,
        ));
    }

    /// Brings the instance to the front again, as a dialog of another app may have taken it.
    pub(crate) fn front(&self) -> Result<(), String> {
        apps::activate(self.input.pid());
        interrupt::sleep(Duration::from_millis(300))
    }
}

/// `duration` in nanoseconds.
pub(crate) fn nanos(duration: Duration) -> u64 {
    u64::try_from(duration.as_nanos()).unwrap_or(u64::MAX)
}

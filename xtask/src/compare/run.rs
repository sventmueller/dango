//! A whole comparison: per app a scratch home, a first launch, the measured session and the cold starts.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

use objc2::MainThreadMarker;
use objc2_app_kit::NSScreen;
use objc2_core_foundation::{CGPoint, CGRect, CGSize};
use objc2_core_graphics::{CGDisplayBounds, CGDisplayCopyDisplayMode, CGDisplayMode, CGMainDisplayID};

use crate::compare::apps::{self, Instance};
use crate::compare::capture::{self, Capture};
use crate::compare::cold::{self, Cold};
use crate::compare::defaults::Guard;
use crate::compare::fingerprint::EDITOR;
use crate::compare::fixtures::{self, MARKDOWN_FILE, MARKDOWN_LINE, RUST_FILE, RUST_LINE};
use crate::compare::input::{self, Input};
use crate::compare::interrupt;
use crate::compare::keys::{self, Layout};
use crate::compare::measure::{Samples, Session};
use crate::compare::options::{App, Options, Phase};
use crate::compare::permissions;
use crate::compare::report::{self, AppResult, Machine, Report};
use crate::compare::stats::summarize;
use crate::files;

/// The display's refresh rate and where it came from.
fn refresh_rate() -> (f64, &'static str) {
    if let Some(mtm) = MainThreadMarker::new()
        && let Some(screen) = NSScreen::mainScreen(mtm)
    {
        let fps = screen.maximumFramesPerSecond();
        if fps > 0 {
            return (fps as f64, "NSScreen.maximumFramesPerSecond");
        }
    }
    let mode = CGDisplayCopyDisplayMode(CGMainDisplayID());
    let hz = CGDisplayMode::refresh_rate(mode.as_deref());
    if hz > 0.0 {
        (hz, "CGDisplayMode")
    } else {
        (60.0, "angenommen")
    }
}

fn output(program: &str, args: &[&str]) -> String {
    crate::stdout(Command::new(program).args(args)).map_or_else(|_| "?".into(), |out| out.trim().to_string())
}

/// The version an app's bundle declares.
fn version(app: App) -> String {
    let plist = app.bundle().join("Contents/Info.plist");
    output(
        "plutil",
        &[
            "-extract",
            "CFBundleShortVersionString",
            "raw",
            "-o",
            "-",
            &plist.to_string_lossy(),
        ],
    )
}

/// The frame both apps' windows get: the same place and size on the main display.
fn window_frame() -> CGRect {
    let display = CGDisplayBounds(CGMainDisplayID());
    let width = (display.size.width - 160.0).clamp(800.0, 1280.0);
    let height = (display.size.height - 160.0).clamp(600.0, 860.0);
    CGRect::new(
        CGPoint::new(display.origin.x + 80.0, display.origin.y + 80.0),
        CGSize::new(width, height),
    )
}

/// Writes the settings that make the apps comparable into a fresh `home`.
fn prepare_home(app: App, home: &Path, project: &Path) -> Result<(), String> {
    if home.exists() {
        files::remove_dir_all(home)?;
    }
    files::create_dir_all(home.parent().unwrap_or(home))?;
    files::create_private_dir(home)?;
    match app {
        App::Ours => {
            let data = home.join("Library/Application Support").join(app.bundle_id());
            fixtures::write_json(&data.join("settings.json"), &fixtures::our_settings())?;
            fixtures::write_json(&data.join("keybindings.json"), &fixtures::our_keybindings())?;
            fixtures::write_json(&data.join("trust.json"), &fixtures::our_trust(project))
        }
        App::Zed => {
            let config = home.join(".config/zed");
            fixtures::write_json(&config.join("settings.json"), &fixtures::zed_settings())?;
            fixtures::write_json(&config.join("keymap.json"), &fixtures::zed_keymap())
        }
    }
}

/// What an app writes under `home` on its first launch with a folder, which shows that the home reached it: this
/// app's state with the folder among the recent ones, Zed's data folder with its database.
fn first_written(app: App, home: &Path) -> PathBuf {
    let support = home.join("Library/Application Support");
    match app {
        App::Ours => support.join(app.bundle_id()).join("state.json"),
        App::Zed => support.join("Zed"),
    }
}

/// A first launch, so first-run screens are behind the measurement; fails if the app did not take the scratch home.
fn first_launch(app: App, home: &Path, project: &Path) -> Result<(), String> {
    let mut first = apps::launch(app, home, &[project])?;
    first.window(Duration::from_secs(30))?;
    interrupt::sleep(Duration::from_secs(4))?;
    let written = first_written(app, home);
    let isolated = written.exists();
    first.quit(Duration::from_secs(15));
    if !isolated {
        return Err(format!(
            "{} did not write {}; HOME did not reach it, so it may have used the real data folder",
            app.title(),
            written.display()
        ));
    }
    Ok(())
}

fn record(samples: &Samples, raw: &mut Vec<f64>) -> Option<crate::compare::stats::Summary> {
    raw.clone_from(&samples.ms);
    summarize(&samples.ms, samples.missed)
}

/// Starts the measured instance with its window in place and the capture running, logging each step.
fn start(app: App, home: &Path, project: &Path, debug: bool) -> Result<(Instance, apps::Window, Capture), String> {
    let (rust, markdown) = (project.join(RUST_FILE), project.join(MARKDOWN_FILE));
    let instance = apps::launch(app, home, &[project, &rust, &markdown])?;
    eprintln!("  {}: started as pid {}", app.title(), instance.pid);
    let (first, _) = instance.window(Duration::from_secs(30))?;
    eprintln!("  {}: window {} at {:?}", app.title(), first.id, first.frame);
    apps::activate(instance.pid);
    interrupt::sleep(Duration::from_secs(1))?;
    let window = apps::place(instance.pid, window_frame())?;
    eprintln!("  {}: window {} placed at {:?}", app.title(), window.id, window.frame);
    let capture = Capture::start_keeping(instance.pid, window.frame, EDITOR, debug, Duration::from_secs(10))?;
    interrupt::sleep(Duration::from_millis(500))?;
    let counts = capture.counts();
    eprintln!(
        "  {}: capture started, {} complete frames in the first 0.5 s, editor region {:?}",
        app.title(),
        counts.complete,
        EDITOR
    );
    if counts.complete == 0 {
        eprintln!(
            "  {}: no complete frame yet; the stream may see nothing of the window",
            app.title()
        );
    }
    Ok((instance, window, capture))
}

/// Opens `file` at `line` with the editor focused, and checks with a typed key that it has the keyboard; once more
/// after Escape and a click into the text if not.
fn open_at(session: &Session<'_>, file: &str, line: usize) -> Result<(), String> {
    session.open_file(file)?;
    for attempt in 0..2 {
        if attempt > 0 {
            session.input.key(keys::ESCAPE, input::Mods::default())?;
            interrupt::sleep(Duration::from_millis(300))?;
        }
        session.focus_editor()?;
        if line == 1 && attempt > 0 {
            session.input.key(keys::UP, input::CMD)?;
            interrupt::sleep(Duration::from_millis(500))?;
        } else {
            session.go_to_line(line)?;
        }
        let role = apps::focused_role(session.input.pid());
        eprintln!(
            "    {file}:{line} opened; focused element {}",
            role.as_deref().unwrap_or("unknown")
        );
        if session.check_typing(file)? {
            return Ok(());
        }
    }
    eprintln!("    {file}: keys still do not reach the editor; its samples will miss");
    Ok(())
}

/// The measured session: the phases `options` picks, with both files open afterwards for the cold start.
fn session(app: App, home: &Path, project: &Path, options: &Options, result: &mut AppResult) -> Result<(), String> {
    let (mut instance, window, capture) = start(app, home, project, options.debug_frames)?;
    let input = Input::new(instance.pid, Layout::current()?)?;
    let session = Session {
        input: &input,
        capture: &capture,
        window,
        refresh_hz: refresh_rate().0,
        app,
        debug: options.debug_frames.then(|| (options.out.join("debug"), app.name())),
    };
    session.park_pointer();
    session.settle()?;
    open_at(&session, RUST_FILE, RUST_LINE)?;
    if options.runs_phase(Phase::TypingRust) {
        println!("  {}: typing in {RUST_FILE}", app.title());
        let samples = session.typing(Phase::TypingRust.name(), options.runs)?;
        result.typing_rust = record(&samples, &mut result.raw.typing_rust_ms);
    }
    if options.runs_phase(Phase::Scroll) {
        println!("  {}: scrolling", app.title());
        session.focus_editor()?;
        let (scroll, intervals) = session.scrolling(options.runs.div_ceil(6))?;
        result.scroll = scroll;
        result.raw.scroll_intervals_ms = intervals;
    }
    open_at(&session, MARKDOWN_FILE, MARKDOWN_LINE)?;
    if options.runs_phase(Phase::TypingMarkdown) {
        println!("  {}: typing in {MARKDOWN_FILE}", app.title());
        let samples = session.typing(Phase::TypingMarkdown.name(), options.runs)?;
        result.typing_markdown = record(&samples, &mut result.raw.typing_markdown_ms);
    }
    session.focus_editor()?;
    input.key(keys::LEFT, input::CTRL_CMD)?;
    if options.runs_phase(Phase::Tabs) {
        println!("  {}: switching tabs", app.title());
        result.tab_switch = record(&session.tab_switch(options.runs)?, &mut result.raw.tab_switch_ms);
        if !options.runs.is_multiple_of(2) {
            input.key(keys::LEFT, input::CTRL_CMD)?;
        }
    }
    session.settle()?;
    drop(capture);
    instance.quit(Duration::from_secs(15));
    Ok(())
}

/// The cold starts, after the session left the project open, each opening `file` as Finder would.
fn cold_starts(app: App, home: &Path, file: &Path, options: &Options, result: &mut AppResult) -> Result<(), String> {
    let layout = Layout::current()?;
    let mut all: Vec<Cold> = Vec::new();
    for run in 0..options.cold_runs {
        println!("  {}: cold start {}/{}", app.title(), run + 1, options.cold_runs);
        all.push(cold::cold_start(app, home, file, &layout)?);
        interrupt::sleep(Duration::from_secs(2))?;
    }
    let pick = |f: fn(&Cold) -> Option<f64>| {
        let values: Vec<f64> = all.iter().filter_map(f).collect();
        let missed = all.len() - values.len();
        (values, missed)
    };
    for (get, summary, raw) in [
        (
            (|c: &Cold| c.window_ms) as fn(&Cold) -> Option<f64>,
            &mut result.cold_window,
            &mut result.raw.cold_window_ms,
        ),
        (
            |c: &Cold| c.content_ms,
            &mut result.cold_content,
            &mut result.raw.cold_content_ms,
        ),
        (|c: &Cold| c.key_ms, &mut result.cold_key, &mut result.raw.cold_key_ms),
    ] {
        let (values, missed) = pick(get);
        *summary = summarize(&values, missed);
        *raw = values;
    }
    Ok(())
}

/// Everything measured of `app`; a failure ends its part and becomes a note.
fn measure(app: App, out: &Path, options: &Options) -> AppResult {
    let mut result = AppResult {
        app: app.title().to_string(),
        version: version(app),
        bundle_id: app.bundle_id().to_string(),
        ..AppResult::default()
    };
    let home = out.join("homes").join(app.name());
    let project = out.join("fixtures/project");
    let mut steps = || -> Result<(), String> {
        fixtures::write_project(&project)?;
        let project = project
            .canonicalize()
            .map_err(|e| format!("{}: {e}", project.display()))?;
        prepare_home(app, &home, &project)?;
        let scratch = project.to_string_lossy().into_owned();
        let guard = if app == App::Ours {
            Some(Guard::new(app.bundle_id(), &scratch)?)
        } else {
            None
        };
        let outcome = first_launch(app, &home, &project)
            .and_then(|()| session(app, &home, &project, options, &mut result))
            .and_then(|()| {
                if options.runs_phase(Phase::Cold) {
                    cold_starts(app, &home, &project.join(RUST_FILE), options, &mut result)
                } else {
                    Ok(())
                }
            });
        if let Some(guard) = &guard {
            guard.restore(app.bundle_id());
        }
        outcome
    };
    if let Err(error) = steps() {
        eprintln!("{}: {error}", app.title());
        result.notes.push(format!("abgebrochen: {error}"));
    }
    result
}

fn machine(options: &Options, layout: &Layout) -> Machine {
    let (refresh_hz, source) = refresh_rate();
    let frame = window_frame();
    Machine {
        macos: output("sw_vers", &["-productVersion"]),
        model: output("sysctl", &["-n", "hw.model"]),
        keyboard_layout: layout.name.clone(),
        refresh_hz,
        refresh_source: source.into(),
        window: [frame.origin.x, frame.origin.y, frame.size.width, frame.size.height],
        runs: options.runs,
        cold_runs: options.cold_runs,
        started_unix: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_secs()),
    }
}

/// The apps of `options` that are not installed.
fn absent(options: &Options) -> Vec<String> {
    options
        .apps
        .iter()
        .filter(|app| !app.executable().exists())
        .map(|app| format!("{} is not installed at {}", app.title(), app.bundle().display()))
        .collect()
}

/// Checks permissions, apps, keyboard and capture without starting or touching any app.
pub(crate) fn check(options: &Options) -> Result<(), String> {
    let missing = permissions::missing();
    for permission in &missing {
        println!("missing: {permission}");
    }
    let mut problems = absent(options);
    for app in &options.apps {
        let running = apps::pids_of(&app.executable());
        println!(
            "{} {} at {}; running now, left alone: {running:?}",
            app.title(),
            version(*app),
            app.bundle().display()
        );
    }
    match Layout::current() {
        Ok(layout) => {
            println!("keyboard layout {}", layout.name);
            for c in "xzpg.0123456789codersnt".chars() {
                if let Err(e) = layout.stroke(c) {
                    problems.push(e);
                }
            }
        }
        Err(e) => problems.push(e),
    }
    let (hz, source) = refresh_rate();
    println!(
        "display refreshes {hz} times a second ({source}); windows go to {:?}",
        window_frame()
    );
    if missing.is_empty()
        && let Err(e) = capture::shareable(Duration::from_secs(5))
    {
        problems.push(e);
    }
    problems.extend(missing.iter().map(|m| format!("missing permission: {m}")));
    if problems.is_empty() {
        println!("ready: cargo xtask compare takes over keyboard, pointer and screen for about 10 to 15 minutes");
        Ok(())
    } else {
        Err(problems.join("\n"))
    }
}

/// Measures every app of `options` and writes the report.
pub(crate) fn run(options: &Options) -> Result<(), String> {
    interrupt::install();
    permissions::require()?;
    let absent = absent(options);
    if !absent.is_empty() {
        return Err(absent.join("\n"));
    }
    let layout = Layout::current()?;
    let before = apps::frontmost();
    let mut report = Report {
        machine: machine(options, &layout),
        apps: Vec::new(),
    };
    for app in &options.apps {
        println!("{}: measuring", app.title());
        report.apps.push(measure(*app, &options.out, options));
        if interrupt::check().is_err() {
            break;
        }
    }
    if let Some(pid) = before {
        apps::give_back(pid);
    }
    let path = report::write(&options.out, &report)?;
    println!("{}", path.display());
    interrupt::check()
}

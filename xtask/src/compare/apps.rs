//! Instances of the measured apps that this run started itself, with a home of their own; every other process of
//! the same app, above all the one this session may run in, stays untouched.

use std::collections::BTreeSet;
use std::ffi::c_void;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::ptr::NonNull;
use std::time::{Duration, Instant};

use objc2::rc::Retained;
use objc2::runtime::AnyObject;
use objc2_app_kit::{NSApplicationActivationOptions, NSRunningApplication};
use objc2_application_services::{AXError, AXUIElement, AXValue, AXValueType};
use objc2_core_foundation::{CFBoolean, CFRetained, CFString, CFType, CGPoint, CGRect, CGSize};
use objc2_core_graphics::{CGWindowListCopyWindowInfo, CGWindowListOption};
use objc2_foundation::{NSArray, NSDictionary, NSNumber, NSString};

use crate::compare::clock;
use crate::compare::input::{self, Input};
use crate::compare::interrupt;
use crate::compare::keys::Layout;
use crate::compare::options::App;

/// The processes running `executable`.
pub(crate) fn pids_of(executable: &Path) -> BTreeSet<libc::pid_t> {
    let mut pids = vec![0 as libc::pid_t; 8192];
    let size = i32::try_from(pids.len() * size_of::<libc::pid_t>()).unwrap_or(i32::MAX);
    // SAFETY: the buffer holds `size` bytes of pids.
    let count = unsafe { libc::proc_listallpids(pids.as_mut_ptr().cast::<c_void>(), size) };
    pids.truncate(usize::try_from(count).unwrap_or(0));
    pids.into_iter()
        .filter(|pid| *pid > 0 && path_of(*pid).as_deref() == Some(executable))
        .collect()
}

fn path_of(pid: libc::pid_t) -> Option<PathBuf> {
    let mut buffer = vec![0u8; 4096];
    let size = u32::try_from(buffer.len()).unwrap_or(0);
    // SAFETY: the buffer holds `size` bytes for the path.
    let length = unsafe { libc::proc_pidpath(pid, buffer.as_mut_ptr().cast::<c_void>(), size) };
    buffer.truncate(usize::try_from(length).ok().filter(|n| *n > 0)?);
    String::from_utf8(buffer).ok().map(PathBuf::from)
}

/// Whether the process `pid` still exists.
fn alive(pid: libc::pid_t) -> bool {
    // SAFETY: signal 0 only asks whether the process exists.
    unsafe { libc::kill(pid, 0) == 0 }
}

/// An on-screen window of a process: its number and frame in points from the top left of the main display.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Window {
    pub(crate) id: u32,
    pub(crate) frame: CGRect,
}

/// The on-screen windows of `pid` in the normal layer, largest first.
pub(crate) fn windows(pid: libc::pid_t) -> Vec<Window> {
    let options = CGWindowListOption::OptionOnScreenOnly | CGWindowListOption::ExcludeDesktopElements;
    let Some(list) = CGWindowListCopyWindowInfo(options, 0) else {
        return Vec::new();
    };
    // SAFETY: the list is a CFArray of CFDictionaries with CFString keys, toll-free bridged; it outlives the borrow.
    let list: &NSArray<NSDictionary<NSString, AnyObject>> = unsafe {
        CFRetained::as_ptr(&list)
            .cast::<NSArray<NSDictionary<NSString, AnyObject>>>()
            .as_ref()
    };
    let number = |info: &NSDictionary<NSString, AnyObject>, key: &str| {
        info.objectForKey(&NSString::from_str(key))
            .and_then(|value| value.downcast::<NSNumber>().ok())
    };
    let mut found: Vec<Window> = list
        .iter()
        .filter(|info| number(info, "kCGWindowOwnerPID").is_some_and(|n| n.intValue() == pid))
        .filter(|info| number(info, "kCGWindowLayer").is_some_and(|n| n.intValue() == 0))
        .filter_map(|info| {
            let id = number(&info, "kCGWindowNumber")?.unsignedIntValue();
            let bounds = info
                .objectForKey(&NSString::from_str("kCGWindowBounds"))?
                .downcast::<NSDictionary>()
                .ok()?;
            // SAFETY: the window server's bounds are a dictionary of CFString keys and CFNumber values.
            let bounds = unsafe { Retained::cast_unchecked::<NSDictionary<NSString, AnyObject>>(bounds) };
            let value = |key| number(&bounds, key).map(|n| n.doubleValue());
            let frame = CGRect::new(
                CGPoint::new(value("X")?, value("Y")?),
                CGSize::new(value("Width")?, value("Height")?),
            );
            Some(Window { id, frame })
        })
        .filter(|w| w.frame.size.width >= 300.0 && w.frame.size.height >= 200.0)
        .collect();
    found.sort_by(|a, b| area(b).total_cmp(&area(a)));
    found
}

fn area(window: &Window) -> f64 {
    window.frame.size.width * window.frame.size.height
}

pub(crate) fn ax_attribute(element: &AXUIElement, name: &str) -> Option<CFRetained<CFType>> {
    let mut value: *const CFType = std::ptr::null();
    // SAFETY: `value` is a valid out-parameter; a returned value is owned by the caller (the Copy rule).
    let error = unsafe { element.copy_attribute_value(&CFString::from_str(name), NonNull::from(&mut value)) };
    let value = NonNull::new(value.cast_mut())?;
    // SAFETY: AXUIElementCopyAttributeValue returned a +1 reference.
    (error == AXError::Success).then(|| unsafe { CFRetained::from_raw(value) })
}

fn ax_set(element: &AXUIElement, name: &str, value: &CFType) -> Result<(), String> {
    // SAFETY: setting an attribute of a live element to a CF value.
    let error = unsafe { element.set_attribute_value(&CFString::from_str(name), value) };
    (error == AXError::Success)
        .then_some(())
        .ok_or_else(|| format!("{name}: AXError {}", error.0))
}

fn ax_value<T>(kind: AXValueType, mut value: T) -> Result<CFRetained<AXValue>, String> {
    // SAFETY: `value` is a CGPoint or CGSize matching `kind`, copied by AXValueCreate.
    unsafe { AXValue::new(kind, NonNull::from(&mut value).cast()) }.ok_or_else(|| "no AXValue".to_string())
}

/// The process whose app has the keyboard now.
pub(crate) fn frontmost() -> Option<libc::pid_t> {
    // SAFETY: the system-wide element needs nothing; the timeout keeps a busy app from stalling the question.
    let system = unsafe { AXUIElement::new_system_wide() };
    // SAFETY: a messaging timeout on a live element.
    let _ = unsafe { system.set_messaging_timeout(0.25) };
    let app = ax_attribute(&system, "AXFocusedApplication")?
        .downcast::<AXUIElement>()
        .ok()?;
    let mut pid: libc::pid_t = 0;
    // SAFETY: `pid` is a valid out-parameter.
    let error = unsafe { app.pid(NonNull::from(&mut pid)) };
    (error == AXError::Success && pid > 0).then_some(pid)
}

/// Brings the app of `pid` to the front with its main window.
pub(crate) fn activate(pid: libc::pid_t) {
    // SAFETY: an element for a process this run started.
    let app = unsafe { AXUIElement::new_application(pid) };
    let _ = ax_set(&app, "AXFrontmost", CFBoolean::new(true));
    if let Some(window) = main_window(&app) {
        // SAFETY: an action of a live window element.
        let _ = unsafe { window.perform_action(&CFString::from_str("AXRaise")) };
    }
    if let Some(running) = NSRunningApplication::runningApplicationWithProcessIdentifier(pid) {
        running.activateWithOptions(NSApplicationActivationOptions::ActivateAllWindows);
    }
}

/// The accessibility role of the element of `pid` that has the keyboard, such as `AXTextArea`.
pub(crate) fn focused_role(pid: libc::pid_t) -> Option<String> {
    // SAFETY: an element for a process this run started.
    let app = unsafe { AXUIElement::new_application(pid) };
    let focused = ax_attribute(&app, "AXFocusedUIElement")?
        .downcast::<AXUIElement>()
        .ok()?;
    let role = ax_attribute(&focused, "AXRole")?.downcast::<CFString>().ok()?;
    Some(role.to_string())
}

/// Asks the app of `pid`, which had the keyboard before the run, to take it back, without sending it anything else.
pub(crate) fn give_back(pid: libc::pid_t) {
    if let Some(running) = NSRunningApplication::runningApplicationWithProcessIdentifier(pid) {
        running.activateWithOptions(NSApplicationActivationOptions::empty());
    }
}

pub(crate) fn main_window(app: &AXUIElement) -> Option<CFRetained<AXUIElement>> {
    ["AXMainWindow", "AXFocusedWindow"]
        .into_iter()
        .find_map(|name| ax_attribute(app, name)?.downcast::<AXUIElement>().ok())
}

/// Moves and sizes the main window of `pid` to `frame` and waits until the window server shows it there.
pub(crate) fn place(pid: libc::pid_t, frame: CGRect) -> Result<Window, String> {
    // SAFETY: an element for a process this run started.
    let app = unsafe { AXUIElement::new_application(pid) };
    for _ in 0..20 {
        interrupt::check()?;
        if let Some(window) = main_window(&app) {
            let origin = ax_value(AXValueType::CGPoint, frame.origin)?;
            let size = ax_value(AXValueType::CGSize, frame.size)?;
            ax_set(&window, "AXPosition", &origin)?;
            ax_set(&window, "AXSize", &size)?;
            ax_set(&window, "AXPosition", &origin)?;
        }
        std::thread::sleep(Duration::from_millis(150));
        if let Some(window) = windows(pid).first().filter(|w| close(w.frame, frame)) {
            return Ok(*window);
        }
    }
    Err(format!("the window of process {pid} did not take the frame {frame:?}"))
}

fn close(a: CGRect, b: CGRect) -> bool {
    [
        (a.origin.x, b.origin.x),
        (a.origin.y, b.origin.y),
        (a.size.width, b.size.width),
        (a.size.height, b.size.height),
    ]
    .iter()
    .all(|(x, y)| (x - y).abs() <= 2.0)
}

/// An instance of an app this run started, ended when dropped.
pub(crate) struct Instance {
    pub(crate) app: App,
    pub(crate) pid: libc::pid_t,
    /// When `open` was started, on the host clock in nanoseconds.
    pub(crate) opened_ns: u64,
    ended: bool,
}

/// Starts a new instance of `app` with `home` as its home folder, opening `paths` through the open-documents event,
/// and finds its process among those that ran before; this app opens only the folders of them, Zed the files too.
pub(crate) fn launch(app: App, home: &Path, paths: &[&Path]) -> Result<Instance, String> {
    let before = pids_of(&app.executable());
    let mut open = Command::new("open");
    open.arg("-n").arg("-a").arg(app.bundle());
    open.arg("--env").arg(format!("HOME={}", home.display()));
    if app == App::Ours {
        open.arg("--env").arg(format!("CFFIXED_USER_HOME={}", home.display()));
    }
    open.args(paths);
    if app == App::Ours {
        // AppKit reads `-Key value` arguments as defaults of this launch alone; Zed would take them for paths.
        open.arg("--args");
        for (key, value) in [
            ("ApplePersistenceIgnoreState", "YES"),
            ("NSQuitAlwaysKeepsWindows", "NO"),
            ("SUEnableAutomaticChecks", "NO"),
        ] {
            open.arg(format!("-{key}")).arg(value);
        }
    }
    let opened_ns = clock::now_ns();
    let status = open.status().map_err(|e| format!("open: {e}"))?;
    if !status.success() {
        return Err(format!("open -n -a {} failed: {status}", app.bundle().display()));
    }
    let deadline = Instant::now() + Duration::from_secs(20);
    while Instant::now() < deadline {
        interrupt::check()?;
        if let Some(pid) = pids_of(&app.executable()).difference(&before).next().copied() {
            return Ok(Instance {
                app,
                pid,
                opened_ns,
                ended: false,
            });
        }
        std::thread::sleep(Duration::from_millis(2));
    }
    Err(format!("{} started no new process within 20 s", app.title()))
}

impl Instance {
    /// Waits for the instance's first on-screen window; returns it and when it was first seen.
    pub(crate) fn window(&self, timeout: Duration) -> Result<(Window, u64), String> {
        let deadline = Instant::now() + timeout;
        while Instant::now() < deadline {
            interrupt::check()?;
            if let Some(window) = windows(self.pid).first() {
                return Ok((*window, clock::now_ns()));
            }
            if !alive(self.pid) {
                return Err(format!("{} ended before it showed a window", self.app.title()));
            }
            std::thread::sleep(Duration::from_millis(2));
        }
        Err(format!(
            "{} showed no window within {} s",
            self.app.title(),
            timeout.as_secs()
        ))
    }

    /// Quits the instance as a person would, then kills it if it is still there after `timeout`.
    pub(crate) fn quit(&mut self, timeout: Duration) {
        if self.ended {
            return;
        }
        self.ended = true;
        let running: Option<Retained<NSRunningApplication>> =
            NSRunningApplication::runningApplicationWithProcessIdentifier(self.pid);
        let accepted = running.as_ref().is_some_and(|running| running.terminate());
        if self.wait_for_end(timeout / 2) {
            return;
        }
        // An app that ignores the quit event, or asks first, may still take ⌘Q from its own menu.
        activate(self.pid);
        let keyed = Layout::current()
            .and_then(|layout| Input::new(self.pid, layout))
            .and_then(|input| input.shortcut(input::CMD, 'q'));
        if self.wait_for_end(timeout / 2) {
            return;
        }
        eprintln!(
            "{} (pid {}) did not quit (quit event {}, ⌘Q {}); killing it",
            self.app.title(),
            self.pid,
            if accepted { "accepted" } else { "refused" },
            keyed.map_or_else(|e| format!("not sent: {e}"), |_| "sent".into())
        );
        if let Some(running) = &running {
            running.forceTerminate();
        }
        // SAFETY: a signal to the process this run started.
        unsafe { libc::kill(self.pid, libc::SIGKILL) };
    }

    /// Whether the process ended within `timeout`.
    fn wait_for_end(&self, timeout: Duration) -> bool {
        let deadline = Instant::now() + timeout;
        while Instant::now() < deadline && alive(self.pid) {
            std::thread::sleep(Duration::from_millis(20));
        }
        !alive(self.pid)
    }
}

impl Drop for Instance {
    fn drop(&mut self) {
        self.quit(Duration::from_secs(10));
    }
}

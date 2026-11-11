//! The plan's usage, which the status line of Claude's tabs shows: from `get_usage`, which the CLI marks experimental,
//! else from the limits the session reported. The report is the account's, so one answer serves every tab.

use std::cell::RefCell;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, SystemTime};

use ide_claude::Session;
use ide_claude::control::UsageReport;
use ide_thread::Thread;

use crate::util::main_soon;

/// How old a usage report may get before it is asked for again.
const FRESH: Duration = Duration::from_secs(120);

static ASKING: AtomicBool = AtomicBool::new(false);

thread_local! {
    static REPORT: RefCell<Option<(SystemTime, Option<UsageReport>)>> = const { RefCell::new(None) };
}

/// A window of the plan: the CLI's name for it, the share used from 0 to 1, and when it starts over.
pub struct Window {
    pub name: String,
    pub used: f64,
    pub resets: Option<String>,
}

/// The plan's weekly and five-hour window, in that order and where known: from the latest report, else from what
/// `thread`'s session reported.
pub fn windows(thread: &Thread) -> Vec<Window> {
    let report = REPORT.with(|r| r.borrow().as_ref().and_then(|(_, r)| r.clone()));
    let mut windows: Vec<Window> = match report.filter(|r| !r.windows.is_empty()) {
        Some(report) => report
            .windows
            .iter()
            .map(|w| Window {
                name: w.name.clone(),
                used: w.utilization.unwrap_or(0.0) / 100.0,
                resets: w.resets_at.as_deref().map(super::iso_time),
            })
            .collect(),
        None => thread
            .rate_limits()
            .iter()
            .map(|l| Window {
                name: l.window.clone(),
                used: l.utilization.unwrap_or(0.0),
                resets: l.resets_at.map(super::time_of_day),
            })
            .collect(),
    };
    windows.retain(|w| matches!(w.name.as_str(), "seven_day" | "five_hour"));
    windows.sort_by_key(|w| w.name != "seven_day");
    windows
}

/// Asks `session` for the plan's usage when the last answer is old; the answer redraws the status line of every tab.
pub fn fetch(session: &Session) {
    let stale = REPORT.with(|r| {
        r.borrow()
            .as_ref()
            .is_none_or(|(at, _)| at.elapsed().unwrap_or(FRESH) >= FRESH)
    });
    if !stale || ASKING.swap(true, Ordering::SeqCst) {
        return;
    }
    session.usage().then(move |result| {
        let report = result.ok();
        main_soon(move || {
            REPORT.with(|r| *r.borrow_mut() = Some((SystemTime::now(), report)));
            ASKING.store(false, Ordering::SeqCst);
            for tab in super::all() {
                tab.refresh();
            }
        });
    });
}

/// Takes `report` as the latest answer to `get_usage`, as the self-test hands one over without a session.
#[cfg(feature = "selftest")]
pub fn inject(report: UsageReport) {
    REPORT.with(|r| *r.borrow_mut() = Some((SystemTime::now(), Some(report))));
    for tab in super::all() {
        tab.refresh();
    }
}

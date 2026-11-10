//! The account and the plan's usage windows. The bars come from `get_usage`, which the CLI marks experimental, and
//! fall back to the limits the sessions reported; with neither there are no bars.

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, SystemTime};

use ide_claude::control::UsageReport;
use objc2::MainThreadMarker;
use objc2::rc::Retained;
use objc2_app_kit::{
    NSColor, NSControlSize, NSLayoutAttribute, NSProgressIndicator, NSProgressIndicatorStyle, NSStackView, NSTextField,
};
use splat_i18n::{t, tf};

use super::rows;
use crate::agent::AgentTab;
use crate::util::{main_soon, ns};

/// How old a usage report may get before it is asked for again.
const FRESH: Duration = Duration::from_secs(120);

/// One window of the plan: its name, the share used from 0 to 1, and when it starts over.
struct Window {
    name: String,
    used: f64,
    resets: Option<String>,
}

static ASKING: AtomicBool = AtomicBool::new(false);

thread_local! {
    static REPORT: RefCell<Option<(SystemTime, Option<UsageReport>)>> = const { RefCell::new(None) };
}

pub struct Usage {
    pub view: Retained<NSStackView>,
    account: Retained<NSTextField>,
    bars: Retained<NSStackView>,
}

impl Usage {
    pub fn new(mtm: MainThreadMarker) -> Usage {
        let account = rows::small_label(mtm, t("Noch keine Sitzung"));
        let bars = NSStackView::new(mtm);
        bars.setOrientation(objc2_app_kit::NSUserInterfaceLayoutOrientation::Vertical);
        bars.setAlignment(NSLayoutAttribute::Leading);
        bars.setSpacing(6.0);
        let view = NSStackView::new(mtm);
        view.setOrientation(objc2_app_kit::NSUserInterfaceLayoutOrientation::Vertical);
        view.setAlignment(NSLayoutAttribute::Leading);
        view.setSpacing(6.0);
        view.addArrangedSubview(&account);
        view.addArrangedSubview(&bars);
        bars.setTranslatesAutoresizingMaskIntoConstraints(false);
        bars.widthAnchor()
            .constraintEqualToAnchor(&view.widthAnchor())
            .setActive(true);
        Usage { view, account, bars }
    }

    /// Shows the account and the usage the sessions of `tabs` know about, and asks for fresher numbers.
    pub fn show(&self, tabs: &[Rc<AgentTab>]) {
        let account = tabs
            .iter()
            .find_map(|t| t.capabilities().account.filter(|a| a.email.is_some()));
        let text = match &account {
            Some(account) => {
                let mut text = account.email.clone().unwrap_or_default();
                if let Some(plan) = &account.subscription_type {
                    text.push_str(&format!(" · {}", plan_name(plan)));
                }
                text
            }
            None if tabs.is_empty() => t("Noch keine Sitzung").to_string(),
            None => t("Kein Konto gemeldet").to_string(),
        };
        self.account.setStringValue(&ns(&text));
        self.fetch(tabs);
        let report = REPORT.with(|r| r.borrow().as_ref().and_then(|(_, r)| r.clone()));
        let windows = match report.filter(|r| !r.windows.is_empty()) {
            Some(report) => report
                .windows
                .iter()
                .map(|w| Window {
                    name: w.name.clone(),
                    used: w.utilization.unwrap_or(0.0) / 100.0,
                    resets: w.resets_at.as_deref().map(crate::agent::iso_time),
                })
                .collect(),
            None => reported_limits(tabs),
        };
        self.draw(&windows);
    }

    fn draw(&self, windows: &[Window]) {
        for view in self.bars.arrangedSubviews().iter() {
            self.bars.removeArrangedSubview(&view);
            view.removeFromSuperview();
        }
        let Some(mtm) = MainThreadMarker::new() else { return };
        for window in windows {
            let name = rows::small_label(mtm, &crate::agent::window_name(&window.name));
            name.setTranslatesAutoresizingMaskIntoConstraints(false);
            name.widthAnchor().constraintEqualToConstant(64.0).setActive(true);
            let bar = NSProgressIndicator::new(mtm);
            bar.setStyle(NSProgressIndicatorStyle::Bar);
            bar.setIndeterminate(false);
            bar.setControlSize(NSControlSize::Small);
            bar.setMinValue(0.0);
            bar.setMaxValue(100.0);
            bar.setDoubleValue((window.used * 100.0).clamp(0.0, 100.0));
            let percent = rows::small_label(mtm, &crate::agent::percent(window.used));
            if window.used >= 0.8 {
                percent.setTextColor(Some(&NSColor::systemOrangeColor()));
            }
            let line = NSStackView::new(mtm);
            line.setOrientation(objc2_app_kit::NSUserInterfaceLayoutOrientation::Horizontal);
            line.setSpacing(6.0);
            for view in [&*name as &objc2_app_kit::NSView, &bar, &percent] {
                line.addArrangedSubview(view);
            }
            self.bars.addArrangedSubview(&line);
            line.setTranslatesAutoresizingMaskIntoConstraints(false);
            line.widthAnchor()
                .constraintEqualToAnchor(&self.bars.widthAnchor())
                .setActive(true);
            if let Some(resets) = &window.resets {
                let label = rows::small_label(mtm, &tf("Zurückgesetzt {}", &[resets]));
                label.setTextColor(Some(&NSColor::tertiaryLabelColor()));
                self.bars.addArrangedSubview(&label);
            }
        }
        self.bars.setHidden(windows.is_empty());
    }

    /// Asks a live session for its usage when the last answer is old; a failure hides the bars until the next try.
    fn fetch(&self, tabs: &[Rc<AgentTab>]) {
        let stale = REPORT.with(|r| {
            r.borrow()
                .as_ref()
                .is_none_or(|(at, _)| at.elapsed().unwrap_or(FRESH) >= FRESH)
        });
        if !stale || ASKING.load(Ordering::SeqCst) {
            return;
        }
        let Some(session) = tabs.iter().find_map(|t| t.session()) else {
            return;
        };
        ASKING.store(true, Ordering::SeqCst);
        session.usage().then(move |result| {
            let report = result.ok();
            main_soon(move || {
                REPORT.with(|r| *r.borrow_mut() = Some((SystemTime::now(), report)));
                ASKING.store(false, Ordering::SeqCst);
                super::refresh_all();
            });
        });
    }

    /// The texts of the bars, for the self-test.
    #[cfg(feature = "selftest")]
    pub fn texts(&self) -> Vec<String> {
        let mut out = vec![self.account.stringValue().to_string()];
        for view in self.bars.arrangedSubviews().iter() {
            if let Some(stack) = view.downcast_ref::<NSStackView>() {
                for part in stack.arrangedSubviews().iter() {
                    if let Some(label) = part.downcast_ref::<NSTextField>() {
                        out.push(label.stringValue().to_string());
                    }
                }
            }
        }
        out
    }
}

/// Takes `report` as the latest answer to `get_usage`, as the self-test hands one over without a session.
#[cfg(feature = "selftest")]
pub fn inject(report: UsageReport) {
    REPORT.with(|r| *r.borrow_mut() = Some((SystemTime::now(), Some(report))));
}

/// The limits the sessions reported with their answers, one per window.
fn reported_limits(tabs: &[Rc<AgentTab>]) -> Vec<Window> {
    let mut windows: BTreeMap<String, Window> = BTreeMap::new();
    for tab in tabs {
        for limit in tab.snapshot().rate_limits() {
            windows.insert(
                limit.window.clone(),
                Window {
                    name: limit.window.clone(),
                    used: limit.utilization.unwrap_or(0.0),
                    resets: limit.resets_at.map(crate::agent::time_of_day),
                },
            );
        }
    }
    windows.into_values().collect()
}

fn plan_name(plan: &str) -> String {
    match plan {
        "max" => "Max".into(),
        "pro" => "Pro".into(),
        "team" => "Team".into(),
        "enterprise" => "Enterprise".into(),
        other => other.to_string(),
    }
}

//! The `settings-toolbar` scenario: on every tab the settings window is wide enough for all tabs' items in its toolbar,
//! none of them in the overflow menu. `settings-toolbar-long` runs it in Greek, the language with the longest labels.

use std::time::Duration;

use objc2::MainThreadMarker;
use objc2::rc::Retained;
use objc2_app_kit::NSTabViewController;

use super::helpers::after;
use super::{check, finish, snapshot};

/// How long the window takes to resize to a tab it switched to.
const SETTLE: Duration = Duration::from_millis(700);

fn name() -> &'static str {
    if std::env::var("IDE_SELFTEST").is_ok_and(|scenario| scenario == "settings-toolbar-long") {
        "settings-toolbar-long"
    } else {
        "settings-toolbar"
    }
}

pub fn run() {
    let Some(mtm) = MainThreadMarker::new() else { return };
    crate::settings::show(mtm);
    after(SETTLE, || step(0, Vec::new()));
}

/// Shows the tab at `index` and counts the toolbar items it shows; `crowded` names the tabs that hid some.
fn step(index: usize, crowded: Vec<String>) {
    let Some(tabs) = tabs() else {
        check("the settings window opens", false, "");
        return finish(name());
    };
    let items = tabs.tabViewItems();
    let Some(label) = items.iter().nth(index).map(|item| item.label().to_string()) else {
        check(
            "every settings tab shows all tabs' toolbar items, none in the overflow menu",
            crowded.is_empty() && items.count() >= 6,
            format!("{} tabs, crowded: {crowded:?}", items.count()),
        );
        return finish(name());
    };
    tabs.setSelectedTabViewItemIndex(isize::try_from(index).unwrap_or(0));
    after(SETTLE, move || counted(index, label, crowded));
}

fn counted(index: usize, label: String, mut crowded: Vec<String>) {
    let Some(window) = crate::settings::window() else {
        check("the settings window opens", false, "");
        return finish(name());
    };
    let (shown, all) = window.toolbar().map_or((0, 0), |toolbar| {
        (
            toolbar.visibleItems().map_or(0, |visible| visible.count()),
            toolbar.items().count(),
        )
    });
    if shown != all || all == 0 {
        let width = window.contentView().map_or(0.0, |view| view.frame().size.width);
        crowded.push(format!("{label}: {shown} of {all} at {width:.0} pt"));
    }
    snapshot(&window, &format!("{}-{index}", name()));
    step(index + 1, crowded);
}

fn tabs() -> Option<Retained<NSTabViewController>> {
    crate::settings::window()?
        .contentViewController()?
        .downcast::<NSTabViewController>()
        .ok()
}

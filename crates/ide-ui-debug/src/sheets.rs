//! The debugger's sheets: a question with two answers, and the list of processes to attach to.

use std::cell::Cell;

use block2::RcBlock;
use objc2::{DefinedClass, MainThreadOnly};
use objc2_app_kit::{NSAccessibility, NSAlert, NSAlertFirstButtonReturn, NSModalResponse, NSPopUpButton, NSWindow};
use objc2_foundation::{NSPoint, NSRect, NSSize};
use splat_i18n::t;

use super::panel::{DebugPanel, with_panel};
use ide_ui_common::util::{guarded, ns};

/// Shows `alert` as a sheet on `window`; `then` learns whether the first button ended it.
pub fn answer(alert: &NSAlert, window: &NSWindow, then: impl FnOnce(bool) + 'static) {
    let then = Cell::new(Some(then));
    let handler = RcBlock::new(move |response: NSModalResponse| {
        guarded((), || {
            if let Some(then) = then.take() {
                then(response == NSAlertFirstButtonReturn);
            }
        });
    });
    alert.beginSheetModalForWindow_completionHandler(window, Some(&handler));
}

/// Asks which of `found` to attach to; the panel's waiting pick gets the chosen one.
pub fn show_processes(panel: &DebugPanel, found: Vec<ide_dap::Process>) {
    let Some(window) = panel.controller().and_then(|c| c.window()) else {
        return;
    };
    let mtm = panel.mtm();
    let alert = NSAlert::new(mtm);
    alert.setMessageText(&ns(t("An Prozess anhängen")));
    alert.setInformativeText(&ns(t(
        "Der Debugger hängt sich an das laufende Programm. Es gehört nicht der App: Trennen lässt es weiterlaufen.",
    )));
    alert.addButtonWithTitle(&ns(t("Anhängen")));
    alert.addButtonWithTitle(&ns(t("Abbrechen")));
    let list = NSPopUpButton::initWithFrame_pullsDown(
        NSPopUpButton::alloc(mtm),
        NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(360.0, 26.0)),
        false,
    );
    list.setAccessibilityLabel(Some(&ns(t("Prozess"))));
    for process in &found {
        list.addItemWithTitle(&ns(&format!("{} ({})", process.name, process.pid)));
    }
    let empty = found.is_empty();
    if empty {
        list.addItemWithTitle(&ns(t("Keine Prozesse gefunden")));
        list.setEnabled(false);
    }
    alert.setAccessoryView(Some(&list));
    let pids: Vec<u32> = found.iter().map(|p| p.pid).collect();
    let id = panel.id();
    answer(&alert, &window, move |agreed| {
        let chosen = usize::try_from(list.indexOfSelectedItem())
            .ok()
            .and_then(|i| pids.get(i).copied());
        with_panel(id, |panel| {
            let then = panel.ivars().links.borrow_mut().picking.take();
            if let (true, Some(pid), Some(then)) = (agreed && !empty, chosen, then) {
                then(panel, pid);
            }
        });
    });
}

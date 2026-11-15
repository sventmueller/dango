//! The debugger's sheet with the list of processes to attach to.

use ide_ui_common::alert::{Alert, Role, first};
use objc2::{DefinedClass, MainThreadOnly};
use objc2_app_kit::{NSAccessibility, NSPopUpButton};
use objc2_foundation::{NSPoint, NSRect, NSSize};
use splat_i18n::t;

use super::panel::{DebugPanel, with_panel};
use ide_ui_common::util::ns;

/// Asks which of `found` to attach to; the panel's waiting pick gets the chosen one.
pub(crate) fn show_processes(panel: &DebugPanel, found: Vec<ide_dap::Process>) {
    let Some(window) = panel.controller().and_then(|c| c.window()) else {
        return;
    };
    let mtm = panel.mtm();
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
    let pids: Vec<u32> = found.iter().map(|p| p.pid).collect();
    let id = panel.id();
    Alert::new(mtm, t("An Prozess anhängen"))
        .informative(t(
            "Der Debugger hängt sich an das laufende Programm. Es gehört nicht der App: Trennen lässt es weiterlaufen.",
        ))
        .button(t("Anhängen"), Role::Plain)
        .button(t("Abbrechen"), Role::Plain)
        .accessory(&list)
        .sheet(&window, move |response| {
            let agreed = first(response);
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

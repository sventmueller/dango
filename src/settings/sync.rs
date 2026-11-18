//! The switch of the settings sync in the General tab (E112): greyed with the reason while iCloud Drive is off, and
//! a way around a refused access, picking the folder in an open panel.

use std::cell::RefCell;

use ide_core::settings::Settings;
use objc2::rc::Retained;
use objc2::runtime::{AnyObject, NSObject};
use objc2::{DefinedClass, MainThreadMarker, MainThreadOnly, define_class, msg_send, sel};
use objc2_app_kit::{NSButton, NSColor, NSControlStateValueOff, NSControlStateValueOn, NSOpenPanel, NSTextField};
use objc2_foundation::NSObjectProtocol;
use splat_i18n::{t, tf};

use super::form::{self, Row};
use crate::settings_sync::{self, Status};
use crate::util::{guarded, ns, set_action};

pub(crate) struct SyncIvars {
    switch: RefCell<Option<Retained<NSButton>>>,
    note: RefCell<Option<Retained<NSTextField>>>,
}

define_class!(
    // SAFETY: NSObject has no subclassing requirements and `SyncSettings` does not implement Drop.
    #[unsafe(super(NSObject))]
    #[thread_kind = MainThreadOnly]
    #[name = "IDESyncSettings"]
    #[ivars = SyncIvars]
    pub(crate) struct SyncSettings;

    unsafe impl NSObjectProtocol for SyncSettings {}

    impl SyncSettings {
        #[unsafe(method(ideToggleSettingsSync:))]
        fn toggle(&self, sender: Option<&AnyObject>) {
            guarded((), || {
                let on = sender
                    .and_then(|s| s.downcast_ref::<NSButton>())
                    .is_some_and(|b| b.state() == NSControlStateValueOn);
                settings_sync::set_enabled(self.mtm(), on);
                self.reload();
            });
        }

        #[unsafe(method(ideChooseSyncFolder:))]
        fn choose(&self, _sender: Option<&AnyObject>) {
            guarded((), || {
                let mtm = self.mtm();
                let panel = NSOpenPanel::openPanel(mtm);
                panel.setCanChooseDirectories(true);
                panel.setCanChooseFiles(false);
                panel.setCanCreateDirectories(true);
                panel.setMessage(Some(&ns(t(
                    "Wähle einen Ordner in iCloud Drive, den deine Macs für den Abgleich teilen.",
                ))));
                if panel.runModal() != objc2_app_kit::NSModalResponseOK {
                    return;
                }
                let picked = panel.URL().and_then(|url| url.path()).map(|p| p.to_string());
                if let Some(picked) = picked {
                    settings_sync::use_folder(mtm, std::path::Path::new(&picked));
                }
                self.reload();
            });
        }
    }
);

impl SyncSettings {
    fn new(mtm: MainThreadMarker) -> Retained<SyncSettings> {
        let this = Self::alloc(mtm).set_ivars(SyncIvars {
            switch: RefCell::new(None),
            note: RefCell::new(None),
        });
        // SAFETY: NSObject's initializer, called once on the newly allocated object.
        unsafe { msg_send![super(this), init] }
    }

    /// Shows the switch and the note as the sync is now.
    fn reload(&self) {
        let mtm = self.mtm();
        let (Some(switch), Some(note)) = (self.ivars().switch.borrow().clone(), self.ivars().note.borrow().clone())
        else {
            return;
        };
        let on = Settings::load_user().get(&settings_sync::ENABLED);
        let available = settings_sync::available();
        switch.setState(if on {
            NSControlStateValueOn
        } else {
            NSControlStateValueOff
        });
        switch.setEnabled(available || on);
        let (text, warn) = match settings_sync::status(mtm) {
            _ if !available => (t("iCloud Drive ist auf diesem Mac aus. Schalte es in den Systemeinstellungen ein, oder wähle einen Ordner.").to_string(), false),
            Status::Off => (String::new(), false),
            Status::NoICloud => (t("iCloud Drive ist auf diesem Mac aus.").to_string(), true),
            Status::Running => (t("Gleicht ab. Pfade, lokale Adressen und Zustimmungen bleiben auf diesem Mac.").to_string(), false),
            Status::Paused => (tf("Abgleich pausiert: Ein anderer Mac nutzt eine neuere Version. Aktualisiere {} auf diesem Mac.", &[ide_product::NAME]), true),
            Status::Failed(why) => (tf("Abgleich fehlgeschlagen: {}. Erlaube den Zugriff auf iCloud Drive oder wähle den Ordner selbst.", &[&why]), true),
        };
        note.setStringValue(&ns(&text));
        let color = if warn {
            NSColor::systemRedColor()
        } else {
            NSColor::secondaryLabelColor()
        };
        note.setTextColor(Some(&color));
    }
}

/// The rows of the General tab for the settings sync; `keep` holds the object their controls send their actions to.
pub(crate) fn rows(mtm: MainThreadMarker, keep: &RefCell<Option<Retained<NSObject>>>) -> Vec<Row> {
    let target = SyncSettings::new(mtm);
    // SAFETY: The checkbox sends its action to the target, which answers it, set right below.
    let switch = unsafe {
        NSButton::checkboxWithTitle_target_action(
            &ns(t("Über iCloud Drive mit meinen anderen Macs abgleichen")),
            None,
            None,
            mtm,
        )
    };
    set_action(&switch, &target, sel!(ideToggleSettingsSync:));
    // SAFETY: The button sends its action to the target, which answers it, set right below.
    let choose = unsafe { NSButton::buttonWithTitle_target_action(&ns(t("Ordner wählen …")), None, None, mtm) };
    set_action(&choose, &target, sel!(ideChooseSyncFolder:));
    let note = form::value(mtm, "");
    *target.ivars().switch.borrow_mut() = Some(switch.clone());
    *target.ivars().note.borrow_mut() = Some(note.clone());
    target.reload();
    let line = form::line(mtm, &[&switch, &choose]);
    let rows = vec![Row::group(mtm, t("Abgleich:"), &line, &switch), Row::under(&note)];
    *keep.borrow_mut() = Some(Retained::into_super(target));
    rows
}

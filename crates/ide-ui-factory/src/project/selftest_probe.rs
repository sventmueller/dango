//! The self-test's hooks of `project.rs`.

use super::*;

use crate::selftest::ProjectRowSeen;

impl ProjectSheet {
    /// The window the dialog is on.
    pub(crate) fn window(&self) -> ide_ui_common::window::Window {
        self.ivars().target.window()
    }

    /// Whether the dialog is still on its window.
    pub(crate) fn open_now(&self) -> bool {
        self.ui().panel.sheetParent().is_some()
    }

    /// What every row shows.
    pub(crate) fn seen(&self) -> Vec<ProjectRowSeen> {
        self.ui()
            .rows
            .iter()
            .map(|row| ProjectRowSeen {
                title: row.value.title(),
                value: match &row.control {
                    Control::Field(field) | Control::Shown(field) => field.stringValue().to_string(),
                    Control::Choice(choice) => choice
                        .titleOfSelectedItem()
                        .map(|title| title.to_string())
                        .unwrap_or_default(),
                },
                origin: row.origin.stringValue().to_string(),
                resettable: row.reset.isEnabled(),
            })
            .collect()
    }

    /// Presses "Auf Vorgabe zurücksetzen" of `value`'s row; whether it could be pressed.
    pub(crate) fn press_reset(&self, value: ProjectValue) -> bool {
        let enabled = self
            .ui()
            .rows
            .iter()
            .any(|row| row.value == value && row.reset.isEnabled());
        if enabled {
            self.reset(value);
        }
        enabled
    }

    /// Presses "Sichern".
    pub(crate) fn press_save(&self) {
        self.save();
    }
}

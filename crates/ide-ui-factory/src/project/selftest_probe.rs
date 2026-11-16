//! The self-test's hooks of `project.rs`.

use super::*;

use crate::model::settings::role_title;
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

    /// What every row shows: the roles as the editor shows each, then the values; a value the project sets has a
    /// bold title and goes back to the default when its field is emptied.
    pub(crate) fn seen(&self) -> Vec<ProjectRowSeen> {
        let file = self.file();
        let mut seen: Vec<ProjectRowSeen> = Role::ALL
            .into_iter()
            .map(|role| {
                let (words, origin) = file.role(role);
                ProjectRowSeen {
                    title: role_title(role).to_string(),
                    value: words.join(", "),
                    origin: origin.words().to_string(),
                    resettable: origin == Origin::File,
                }
            })
            .collect();
        seen.extend(self.ui().rows.iter().map(|row| {
            let bold = row.caption.font().is_some_and(|font| {
                font.fontDescriptor()
                    .symbolicTraits()
                    .contains(objc2_app_kit::NSFontDescriptorSymbolicTraits::TraitBold)
            });
            let origin = if bold { Origin::File } else { Origin::Default };
            ProjectRowSeen {
                title: row.value.title(),
                value: match &row.control {
                    Control::Field(field) => field.stringValue().to_string(),
                    Control::Choice(choice) => choice
                        .titleOfSelectedItem()
                        .map(|title| title.to_string())
                        .unwrap_or_default(),
                },
                origin: origin.words().to_string(),
                resettable: bold,
            }
        }));
        seen
    }

    /// Takes `value` back to the default as the user does: empties its field, picks the menu's default, or presses
    /// the role's "Auf Vorgabe zurücksetzen"; whether the value was set by the project.
    pub(crate) fn press_reset(&self, value: ProjectValue) -> bool {
        if let ProjectValue::Role(_) = value {
            let set = self.file().origin(value) == Origin::File;
            if set {
                self.reset(value);
            }
            return set;
        }
        let Some(row) = self.ui().rows.iter().find(|row| row.value == value) else {
            return false;
        };
        if self.file().origin(value) != Origin::File {
            return false;
        }
        match &row.control {
            Control::Field(field) => field.setStringValue(&ns("")),
            Control::Choice(choice) => choice.selectItemAtIndex(0),
        }
        self.field_done();
        true
    }

    /// Presses "Sichern".
    pub(crate) fn press_save(&self) {
        self.save();
    }
}

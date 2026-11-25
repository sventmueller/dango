//! Opening a file from the tree in a pane (E145): with Option beside the document in front, with Option and Shift in
//! the pane the user picks from a menu, beside it, below it, or in a window tab of its own.

use objc2::rc::Retained;
use objc2::{DefinedClass, MainThreadOnly, sel};
use objc2_app_kit::{NSMenu, NSOutlineView};
use splat_i18n::{t, tf};

use super::model::Kind;
use super::view::{FileTreeSource, path_of};
use crate::util::{item, separator, set_target};
use crate::window::panes::Side;

impl FileTreeSource {
    /// A click with Option on the file in `row` opens it beside the document in front, or with `choose` offers where;
    /// whether the row is a file, which a folder's Option click leaves to opening its folders inside.
    pub(super) fn row_alt_clicked(&self, view: &NSOutlineView, row: isize, choose: bool) -> bool {
        let Some(path) = (row >= 0)
            .then(|| view.itemAtRow(row))
            .flatten()
            .and_then(|item| path_of(&item))
        else {
            return false;
        };
        if path.is_dir() {
            return false;
        }
        let Some(controller) = self.ivars().controller.load() else {
            return false;
        };
        if !choose {
            controller.open_file_beside(&path, Side::Right);
            return true;
        }
        if let Ok(row) = usize::try_from(row) {
            // Shift extends the selection before the click arrives; the pane menu is for this one file.
            view.selectRowIndexes_byExtendingSelection(&objc2_foundation::NSIndexSet::indexSetWithIndex(row), false);
        }
        self.ivars()
            .model
            .borrow_mut()
            .open_menu(Some(&path), Some(Kind::File), &[]);
        let menu = self.pane_menu(&controller.pane_choices());
        let rect = view.rectOfRow(row);
        let point = objc2_foundation::NSPoint::new(rect.origin.x + 24.0, rect.origin.y + rect.size.height);
        menu.popUpMenuPositioningItem_atLocation_inView(None, point, Some(view));
        true
    }

    /// The menu of where a file opens: each pane of a split tab in front by its number and the document it shows,
    /// beside and below the document in front, and a window tab of its own while the tab is not split.
    pub(super) fn pane_menu(&self, panes: &[(usize, Option<String>)]) -> Retained<NSMenu> {
        let mtm = self.mtm();
        let menu = NSMenu::new(mtm);
        menu.setAutoenablesItems(false);
        let add = |title: &str, action, tag: isize| {
            let entry = item(mtm, title, Some(action), "");
            entry.setTag(tag);
            set_target(&entry, self);
            menu.addItem(&entry);
        };
        if panes.len() > 1 {
            for (number, front) in panes {
                let name = front.clone().unwrap_or_else(|| t("Leerer Bereich").to_string());
                let title = tf("Im Bereich {}: {}", &[&number.to_string(), &name]);
                add(
                    &title,
                    sel!(ideOpenInPaneChosen:),
                    isize::try_from(*number).unwrap_or(0),
                );
            }
            menu.addItem(&separator(mtm));
        }
        add(t("Daneben öffnen"), sel!(ideOpenBesideClicked:), 0);
        add(t("Darunter öffnen"), sel!(ideOpenBelowClicked:), 0);
        if panes.len() < 2 {
            add(t("In neuem Tab öffnen"), sel!(ideOpenInPaneChosen:), 0);
        }
        menu
    }

    /// Opens the file the menu opened on in the pane numbered `number`, or with 0 in a window tab of its own.
    pub(super) fn open_in_chosen_pane(&self, number: isize) {
        let Some(path) = self.ivars().model.borrow().clicked().map(std::path::Path::to_path_buf) else {
            return;
        };
        if let Some(controller) = self.ivars().controller.load() {
            let number = usize::try_from(number).ok().filter(|number| *number > 0);
            controller.open_file_in_pane(&path, number);
        }
    }
}

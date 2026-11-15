//! What the panel's controls, checkboxes, menus and fields do, and how the panel shows the model again afterwards.

use ide_ui_common::window::{Notices, Trust};
use std::path::Path;

use ide_ui_common::util::FlagGuard;
use objc2::DefinedClass;
use objc2::runtime::AnyObject;
use objc2_app_kit::{NSButton, NSControlStateValueOn, NSOutlineView, NSTextField};

use super::model::Command;
use super::model::breakpoints::Edit;
use super::panel::DebugPanel;
use super::rows::tag_of;

impl DebugPanel {
    /// Runs a control's or the Debug menu's command on the current session.
    pub(crate) fn run_command(&self, command: Command) {
        let effects = self.model().run(command);
        self.apply(effects);
    }

    /// Whether `command` can run now, for the controls and the Debug menu.
    pub(crate) fn can(&self, command: Command) -> bool {
        self.read().can(command)
    }

    /// Sets or clears the breakpoint at `line` of `path`, as a click in the gutter and ⌘\ do.
    pub(crate) fn toggle_breakpoint(&self, path: &Path, line: usize) {
        let effects = {
            let mut model = self.model();
            model.breakpoints.toggle(path, line);
            model.breakpoints_changed(Some(path.to_path_buf()))
        };
        self.apply(effects);
    }

    /// Changes the breakpoint at `line` of `path`.
    pub(crate) fn change_breakpoint(&self, path: &Path, line: usize, edit: Edit) {
        let effects = {
            let mut model = self.model();
            if !model.breakpoints.change(path, line, edit) {
                return;
            }
            model.breakpoints_changed(Some(path.to_path_buf()))
        };
        self.apply(effects);
    }

    /// The edits of `path`'s text moved its breakpoints; sessions learn the new lines.
    pub fn text_changed(&self, path: &Path, change: &ide_document::Change) {
        let effects = {
            let mut model = self.model();
            if !model.breakpoints.shift(path, change) {
                return;
            }
            model.breakpoints_changed(Some(path.to_path_buf()))
        };
        self.apply(effects);
    }

    pub(super) fn breakpoint_switched(&self, sender: Option<&AnyObject>) {
        let Some(index) = tag_of(sender) else { return };
        let on = sender
            .and_then(|s| s.downcast_ref::<NSButton>())
            .is_some_and(|b| b.state() == NSControlStateValueOn);
        let found = self
            .read()
            .breakpoints
            .all()
            .get(index)
            .map(|b| (b.path.clone(), b.line));
        if let Some((path, line)) = found {
            self.change_breakpoint(&path, line, Edit::Enable(on));
        }
    }

    pub(super) fn filter_switched(&self, sender: Option<&AnyObject>) {
        let Some(index) = tag_of(sender) else { return };
        let on = sender
            .and_then(|s| s.downcast_ref::<NSButton>())
            .is_some_and(|b| b.state() == NSControlStateValueOn);
        let id = self.read().filters.get(index).map(|f| f.id.clone());
        if let Some(id) = id {
            let effects = self.model().set_filter(&id, on);
            self.apply(effects);
        }
    }

    pub(super) fn watch_typed(&self, sender: Option<&AnyObject>) {
        let Some(field) = sender.and_then(|s| s.downcast_ref::<NSTextField>()) else {
            return;
        };
        let text = field.stringValue().to_string();
        self.add_watch(&text);
        field.setStringValue(&ide_ui_common::util::ns(""));
    }

    /// Adds a watch expression, which the inspector shows and evaluates while the program is paused.
    pub(crate) fn add_watch(&self, expression: &str) {
        let effects = self.model().add_watch(expression);
        self.apply(effects);
    }

    pub(super) fn remove_watch_of(&self, sender: Option<&AnyObject>) {
        if let Some(index) = tag_of(sender) {
            let effects = self.model().remove_watch(index);
            self.apply(effects);
        }
    }

    pub(super) fn edit_breakpoint_of(&self, sender: Option<&AnyObject>) {
        if let Some((what, path, line)) = super::gutter::menu_target(sender) {
            super::gutter::edit(self, what, &path, line);
        }
    }

    pub(super) fn remove_breakpoint_of(&self, sender: Option<&AnyObject>) {
        if let Some((_, path, line)) = super::gutter::menu_target(sender)
            && self.read().breakpoints.at(&path, line).is_some()
        {
            self.toggle_breakpoint(&path, line);
        }
    }

    /// Shows the model again: the list with its rows open as the model and the user want them, the controls, the
    /// inspector, the editors' breakpoints and paused line, and the activity view.
    pub(crate) fn refresh(&self) {
        let ui = self.ui();
        let model = self.read();
        let detaches = super::model::stops_by_detaching(&model);
        ui.enable_controls(&|command| model.can(command), detaches);
        let trusted = self.controller().is_some_and(|c| c.is_trusted());
        ui.untrusted.setHidden(trusted);
        ui.start.setEnabled(
            trusted
                && !model
                    .sessions
                    .iter()
                    .any(|s| s.phase == super::model::session::Phase::Starting),
        );
        let open = model.open_rows();
        drop(model);
        self.reload(&ui.outline, |panel| {
            open.iter().map(|key| panel.panel_item(key)).collect()
        });
        let mut parts = vec![
            super::model::variables::Row::Part(super::model::variables::Part::Variables),
            super::model::variables::Row::Part(super::model::variables::Part::Watches),
        ];
        // The innermost scope stands open, as its variables are what the user wants to see first.
        if let Some(first) = self.read().variables.children(None).into_iter().next() {
            parts.push(super::model::variables::Row::Node(first));
        }
        self.reload(&ui.inspector.outline, |panel| {
            parts.iter().map(|row| panel.inspector_item(row)).collect()
        });
        self.forget_gone_items();
        if let Some(window) = self.controller() {
            crate::host::host().refresh_editors(window.id());
        }
        if let Some(controller) = self.controller() {
            controller.refresh_activity();
        }
    }

    /// Reloads `view`, keeps the rows the user opened, opens those `open` names and keeps the selection.
    fn reload(
        &self,
        view: &NSOutlineView,
        open: impl Fn(&DebugPanel) -> Vec<objc2::rc::Retained<ide_ui_common::panel::RowItem>>,
    ) {
        let _unfolding = FlagGuard::raise(&self.ivars().unfolding);
        let selected = view.selectedRow();
        let selected_item = (selected >= 0).then(|| view.itemAtRow(selected)).flatten();
        view.reloadData();
        for item in open(self) {
            // SAFETY: The item is one of this panel's row objects, which the view knows after the reload.
            unsafe { view.expandItem(Some(&item)) };
        }
        if let Some(item) = selected_item {
            // SAFETY: The item is one of this panel's row objects.
            let row = unsafe { view.rowForItem(Some(&item)) };
            if let Ok(row) = usize::try_from(row) {
                view.selectRowIndexes_byExtendingSelection(
                    &objc2_foundation::NSIndexSet::indexSetWithIndex(row),
                    false,
                );
            }
        }
    }

    /// Selects the row of the paused frame, as a stop does.
    pub(crate) fn select_paused_frame(&self) {
        let selection = self.read().selected;
        let Some(selection) = selection else { return };
        let Some(thread) = selection.thread else { return };
        let item = self.panel_item(&super::model::rows::Key::Frame(
            selection.session,
            thread,
            selection.frame,
        ));
        let view = &self.ui().outline;
        // SAFETY: The item is one of this panel's row objects.
        let row = unsafe { view.rowForItem(Some(&item)) };
        if let Ok(index) = usize::try_from(row) {
            let _unfolding = FlagGuard::raise(&self.ivars().unfolding);
            view.selectRowIndexes_byExtendingSelection(&objc2_foundation::NSIndexSet::indexSetWithIndex(index), false);
            view.scrollRowToVisible(row);
        }
    }
}

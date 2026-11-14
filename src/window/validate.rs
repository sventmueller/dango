//! Which of the window's menu commands work right now; unavailable ones show greyed out (E101).

use objc2::{DefinedClass, sel};
use objc2_app_kit::{NSControlStateValueOff, NSControlStateValueOn, NSMenuItem};

use super::{Panel, WindowController, activity_bar, browser_commands};

impl WindowController {
    pub(super) fn validate(&self, item: &NSMenuItem) -> bool {
        let action = item.action();
        let ivars = self.ivars();
        if action == Some(sel!(idePlanComment:)) {
            return ide_ui_agent::plan::keyboard_target(self).is_some();
        }
        if action == Some(sel!(ideContainerCommand:)) {
            return crate::containers_panel::validate(self, item);
        }
        if action == Some(sel!(ideShowSessionSubagents:)) {
            return ide_ui_agent::can_show_work(self, crate::activity::Part::Subagents);
        }
        if action == Some(sel!(ideShowSessionBackground:)) {
            return ide_ui_agent::can_show_work(self, crate::activity::Part::Background);
        }
        if action == Some(sel!(ideShowPanel:)) {
            let current = ivars.panel_open.get().then(|| ivars.panel.get()).flatten();
            let on = Panel::from_tag(item.tag()) == current && current.is_some();
            item.setState(if on {
                NSControlStateValueOn
            } else {
                NSControlStateValueOff
            });
            return true;
        }
        if action == Some(sel!(ideToggleActivitySymbol:)) {
            let hidden = activity_bar::hidden();
            let shown = Panel::from_tag(item.tag()).is_some_and(|panel| !hidden.contains(&panel));
            item.setState(if shown {
                NSControlStateValueOn
            } else {
                NSControlStateValueOff
            });
            return !shown || hidden.len() + 1 < Panel::ALL.len();
        }
        if action == Some(sel!(ideQuickOpen:)) || action == Some(sel!(ideSearchProject:)) {
            return ivars.workspace.borrow().is_some();
        }
        if action == Some(sel!(ideProjectSymbols:)) {
            return self
                .project_root()
                .is_some_and(|root| crate::project::is_trusted(&root));
        }
        if let Some(action) = action.filter(|action| browser_commands().contains(action)) {
            return crate::browser::validate(self, action);
        }
        if action == Some(sel!(ideNewWorktreeThread:)) {
            let index = usize::try_from(item.tag()).unwrap_or(usize::MAX);
            return crate::worktrees::can_start(self) && crate::worktrees::agent_usable(index);
        }
        if action == Some(sel!(ideNewAcpThread:)) {
            let index = usize::try_from(item.tag()).unwrap_or(usize::MAX);
            return ide_ui_agent::preset_menu()
                .get(index)
                .is_some_and(|(_, usable)| *usable);
        }
        if action == Some(sel!(ideNextTab:)) || action == Some(sel!(idePreviousTab:)) {
            return self
                .window()
                .and_then(|window| window.tabbedWindows())
                .is_some_and(|tabbed| tabbed.count() > 1);
        }
        if action == Some(sel!(ideFocusNextPanel:)) {
            return self.current_tab().is_some_and(|tab| tab.documents().len() > 1);
        }
        true
    }
}

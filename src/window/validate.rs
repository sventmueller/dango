//! Which of the window's menu commands work right now; unavailable ones show greyed out (E101).

use objc2::runtime::Sel;
use objc2::sel;
use objc2_app_kit::{NSControlStateValueOff, NSControlStateValueOn, NSMenuItem};

use super::{Panel, WindowController, activity_bar, browser_commands};

impl WindowController {
    pub(super) fn validate(&self, item: &NSMenuItem) -> bool {
        let action = item.action();
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
            let current = self.shown_panel();
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
            return self.has_workspace();
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
            let usable = super::agent_choices::now(self.context())
                .worktree
                .get(index)
                .is_some_and(|(_, usable, _)| *usable);
            return usable && crate::worktrees::can_start(self);
        }
        if action == Some(sel!(ideNewAcpThread:)) {
            let index = usize::try_from(item.tag()).unwrap_or(usize::MAX);
            return super::agent_choices::now(self.context())
                .presets
                .get(index)
                .is_some_and(|(_, usable)| *usable);
        }
        if action == Some(sel!(ideNextTab:)) || action == Some(sel!(idePreviousTab:)) {
            return self
                .window()
                .and_then(|window| window.tabbedWindows())
                .is_some_and(|tabbed| tabbed.count() > 1);
        }
        if let Some(answer) = self.validate_pane_command(action) {
            return answer;
        }
        true
    }
}

impl WindowController {
    /// Whether a command on the panes of the tab in front can run: moving through panes needs two, closing one needs
    /// two or a document in it, filling the tab with one needs two, moving through a pane's tabs needs two in it.
    fn validate_pane_command(&self, action: Option<Sel>) -> Option<bool> {
        let action = action?;
        let (panes, documents, zoomed) = self.pane_commands().unwrap_or((0, 0, false));
        let answer = match action {
            a if a == sel!(ideFocusNextPanel:) => panes > 1,
            a if a == sel!(ideClosePane:) => panes > 1 || documents > 0,
            a if a == sel!(ideZoomPane:) => panes > 1 || zoomed,
            a if a == sel!(ideNextPaneTab:) || a == sel!(idePreviousPaneTab:) => documents > 1,
            a if a == sel!(ideSplitRight:) || a == sel!(ideSplitBelow:) => panes > 0,
            _ => return None,
        };
        Some(answer)
    }
}

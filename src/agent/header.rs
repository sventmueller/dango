//! The head of the AI tab: Claude's mark and the session's title, then bookmark, history and a new session.

use objc2::rc::Retained;
use objc2::{MainThreadMarker, MainThreadOnly, sel};
use objc2_app_kit::{NSButton, NSClickGestureRecognizer, NSColor, NSStackView, NSTextField};
use objc2_foundation::NSEdgeInsets;
use splat_i18n::t;

use super::style;
use super::target::AgentTarget;
use super::ui;
use crate::util::{ns, symbol};

pub struct Header {
    pub view: Retained<NSStackView>,
    /// The agent's mark, in its colour.
    mark: Retained<objc2_app_kit::NSImageView>,
    title: Retained<NSTextField>,
    bookmark: Retained<NSButton>,
    focus: Retained<NSButton>,
    /// Opens the overview of the session's subagents and background tasks.
    pub tasks: Retained<NSButton>,
}

impl Header {
    pub fn new(mtm: MainThreadMarker, target: &AgentTarget) -> Header {
        let mark = ui::icon(mtm, "sparkle", "", 14.0, &style::terracotta());
        let title = ui::label(
            mtm,
            t("Neue Sitzung"),
            &style::bold(style::BODY),
            &NSColor::labelColor(),
        );
        title.setToolTip(Some(&ns(t("Doppelklick zum Umbenennen"))));
        // SAFETY: The recognizer sends its action to the target, which answers it with a method that takes the sender;
        // the view keeps the recognizer, AppKit holds the target weakly.
        let rename = unsafe {
            NSClickGestureRecognizer::initWithTarget_action(
                NSClickGestureRecognizer::alloc(mtm),
                Some(target),
                Some(sel!(ideAgentRename:)),
            )
        };
        rename.setNumberOfClicksRequired(2);
        title.addGestureRecognizer(&rename);
        let bookmark = ui::symbol_button(
            mtm,
            "bookmark",
            t("Lesezeichen"),
            13.0,
            target,
            sel!(ideAgentBookmark:),
            0,
        );
        let history = ui::symbol_button(
            mtm,
            "clock.arrow.circlepath",
            t("Frühere Sitzungen"),
            13.0,
            target,
            sel!(ideAgentHistory:),
            0,
        );
        let new = ui::symbol_button(
            mtm,
            "plus.bubble",
            t("Neue Sitzung"),
            13.0,
            target,
            sel!(ideAgentNewSession:),
            0,
        );
        let focus = ui::symbol_button(
            mtm,
            "text.bubble",
            t("Nur die Unterhaltung zeigen"),
            13.0,
            target,
            sel!(ideAgentFocusView:),
            0,
        );
        let tasks = ui::symbol_button(
            mtm,
            "list.bullet.indent",
            t("Aufgaben dieser Sitzung"),
            13.0,
            target,
            sel!(ideAgentTasks:),
            0,
        );
        let spacer = ui::spacer(mtm);
        let view = ui::row(
            mtm,
            8.0,
            &[&mark, &title, &spacer, &focus, &tasks, &bookmark, &history, &new],
        );
        view.setEdgeInsets(NSEdgeInsets {
            top: 0.0,
            left: 14.0,
            bottom: 0.0,
            right: 12.0,
        });
        ui::fixed_height(&view, 36.0);
        Header {
            view,
            mark,
            title,
            bookmark,
            focus,
            tasks,
        }
    }

    /// Shows the agent's mark in `colour`, described as `agent`.
    pub fn set_agent(&self, agent: &str, colour: &objc2_app_kit::NSColor) {
        self.mark.setContentTintColor(Some(colour));
        crate::util::set_label(&self.mark, Some(agent));
        self.mark.setToolTip(Some(&ns(agent)));
    }

    pub fn set_title(&self, title: &str) {
        self.title.setStringValue(&ns(title));
    }

    /// Marks the focus view as on or off.
    pub fn set_focus_view(&self, on: bool) {
        let (name, color) = if on {
            ("text.bubble.fill", style::terracotta())
        } else {
            ("text.bubble", NSColor::secondaryLabelColor())
        };
        self.focus
            .setImage(symbol(name, t("Nur die Unterhaltung zeigen")).as_deref());
        self.focus.setContentTintColor(Some(&color));
        self.focus.setState(if on {
            objc2_app_kit::NSControlStateValueOn
        } else {
            objc2_app_kit::NSControlStateValueOff
        });
    }

    pub fn set_bookmarked(&self, on: bool) {
        let (name, color) = if on {
            ("bookmark.fill", style::terracotta())
        } else {
            ("bookmark", NSColor::secondaryLabelColor())
        };
        self.bookmark.setImage(symbol(name, t("Lesezeichen")).as_deref());
        self.bookmark.setContentTintColor(Some(&color));
    }
}

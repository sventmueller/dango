//! The head of the AI tab: Claude's mark and the session's title, then Remote Control, bookmark, history and a new
//! session.

use ide_claude::RemoteControl;
use objc2::rc::Retained;
use objc2::{MainThreadMarker, MainThreadOnly, sel};
use objc2_app_kit::{NSButton, NSClickGestureRecognizer, NSColor, NSStackView, NSTextField};
use objc2_foundation::NSEdgeInsets;
use splat_i18n::{t, tf};

use super::style;
use super::target::AgentTarget;
use super::ui;
use crate::util::{ns, symbol};

pub struct Header {
    pub view: Retained<NSStackView>,
    /// The agent's mark; its colour is the tab bar's to show.
    mark: Retained<objc2_app_kit::NSImageView>,
    title: Retained<NSTextField>,
    bookmark: Retained<NSButton>,
    focus: Retained<NSButton>,
    /// Opens the overview of the session's subagents and background tasks.
    pub tasks: Retained<NSButton>,
    remote: Retained<NSButton>,
}

impl Header {
    pub fn new(mtm: MainThreadMarker, target: &AgentTarget) -> Header {
        let mark = ui::icon(mtm, "sparkle", "", 14.0, &NSColor::secondaryLabelColor());
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
        let remote = ui::symbol_button(
            mtm,
            "iphone",
            t("Remote Control: auf dem Handy oder auf claude.ai weiterarbeiten"),
            13.0,
            target,
            sel!(ideAgentRemoteControl:),
            0,
        );
        remote.setHidden(true);
        let spacer = ui::spacer(mtm);
        let view = ui::row(
            mtm,
            8.0,
            &[
                &mark, &title, &spacer, &remote, &focus, &tasks, &bookmark, &history, &new,
            ],
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
            remote,
        }
    }

    /// The Remote Control button's tooltip, while it shows.
    #[cfg(feature = "selftest")]
    pub fn remote_tip(&self) -> Option<String> {
        let tip = self.remote.toolTip().map(|t| t.to_string());
        tip.filter(|_| !self.remote.isHidden())
    }

    /// Shows where the session stands with Remote Control; hidden where the CLI does not offer it.
    pub fn set_remote(&self, remote: &RemoteControl) {
        let interrupted;
        let (name, color, tip) = match remote {
            RemoteControl::Unavailable => {
                self.remote.setHidden(true);
                return;
            }
            RemoteControl::Off => (
                "iphone",
                NSColor::secondaryLabelColor(),
                t("Remote Control: auf dem Handy oder auf claude.ai weiterarbeiten"),
            ),
            RemoteControl::Connecting => (
                "iphone.radiowaves.left.and.right",
                NSColor::secondaryLabelColor(),
                t("Remote Control verbindet …"),
            ),
            RemoteControl::Connected(_) => (
                "iphone.radiowaves.left.and.right",
                NSColor::controlAccentColor(),
                t("Remote Control ist an: diese Sitzung läuft auch auf dem Handy und auf claude.ai"),
            ),
            RemoteControl::Failed(why) => {
                interrupted = tf("Remote Control ist unterbrochen: {}", &[why]);
                ("iphone.slash", NSColor::systemOrangeColor(), interrupted.as_str())
            }
        };
        self.remote.setImage(symbol(name, tip).as_deref());
        self.remote.setContentTintColor(Some(&color));
        self.remote.setToolTip(Some(&ns(tip)));
        self.remote.setHidden(false);
    }

    /// Describes the agent's mark as `agent`.
    pub fn set_agent(&self, agent: &str) {
        crate::util::set_label(&self.mark, Some(agent));
        self.mark.setToolTip(Some(&ns(agent)));
    }

    pub fn set_title(&self, title: &str) {
        self.title.setStringValue(&ns(title));
    }

    /// The width the title takes, the width its whole text would need, and the head's width, for the self-test.
    #[cfg(feature = "selftest")]
    pub fn title_widths(&self) -> (f64, f64, f64) {
        self.view.layoutSubtreeIfNeeded();
        (
            self.title.frame().size.width,
            self.title.intrinsicContentSize().width,
            self.view.frame().size.width,
        )
    }

    /// Marks the focus view as on or off.
    pub fn set_focus_view(&self, on: bool) {
        let (name, color) = if on {
            ("text.bubble.fill", NSColor::controlAccentColor())
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
            ("bookmark.fill", NSColor::controlAccentColor())
        } else {
            ("bookmark", NSColor::secondaryLabelColor())
        };
        self.bookmark.setImage(symbol(name, t("Lesezeichen")).as_deref());
        self.bookmark.setContentTintColor(Some(&color));
    }
}

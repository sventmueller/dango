//! The head of the AI tab: the session's title, then Remote Control, the focus view, the session's subagents and
//! background processes, bookmark, history and a new session.

use ide_claude::RemoteControl;
use objc2::rc::Retained;
use objc2::{MainThreadMarker, MainThreadOnly, sel};
use objc2_app_kit::{NSButton, NSClickGestureRecognizer, NSColor, NSStackView, NSTextField};
use objc2_foundation::NSEdgeInsets;
use splat_i18n::{t, tf};

use crate::work::{Part, Tally};

use super::style;
use super::target::AgentTarget;
use super::ui;
use ide_ui_common::util::{ns, symbol};
#[cfg(feature = "selftest")]
mod selftest_probe;

/// The symbol of the button for the session's subagents, the one the activity view shows before each of them.
const SUBAGENTS: &str = "person.2";

/// The symbol of the button for the session's background processes.
const BACKGROUND: &str = "terminal";

/// The button that opens the list of `part`, unavailable until the session has some.
fn work_button(mtm: MainThreadMarker, target: &AgentTarget, part: Part) -> Retained<NSButton> {
    let (name, action) = match part {
        Part::Subagents | Part::All => (SUBAGENTS, sel!(ideAgentSubagentList:)),
        Part::Background => (BACKGROUND, sel!(ideAgentBackgroundList:)),
    };
    let button = ui::symbol_button(
        mtm,
        name,
        part.heading(),
        ide_ui_common::style::symbol::SMALL,
        target,
        action,
        0,
    );
    button.setEnabled(false);
    button
}

/// Puts the title's text on the left edge of the window's tab bar, which AppKit insets 8 points from the content.
const TITLE_INSET: f64 = 8.0;

pub struct Header {
    pub view: Retained<NSStackView>,
    title: Retained<NSTextField>,
    bookmark: Retained<NSButton>,
    focus: Retained<NSButton>,
    /// Opens the list of the session's subagents.
    pub subagents: Retained<NSButton>,
    /// Opens the list of the session's background processes.
    pub background: Retained<NSButton>,
    remote: Retained<NSButton>,
}

impl Header {
    pub fn new(mtm: MainThreadMarker, target: &AgentTarget) -> Header {
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
            ide_ui_common::style::symbol::SMALL,
            target,
            sel!(ideAgentBookmark:),
            0,
        );
        let history = ui::symbol_button(
            mtm,
            "clock.arrow.circlepath",
            t("Frühere Sitzungen"),
            ide_ui_common::style::symbol::SMALL,
            target,
            sel!(ideAgentHistory:),
            0,
        );
        let new = ui::symbol_button(
            mtm,
            "plus.bubble",
            t("Neue Sitzung"),
            ide_ui_common::style::symbol::SMALL,
            target,
            sel!(ideAgentNewSession:),
            0,
        );
        let focus = ui::symbol_button(
            mtm,
            "text.bubble",
            t("Nur die Unterhaltung zeigen"),
            ide_ui_common::style::symbol::SMALL,
            target,
            sel!(ideAgentFocusView:),
            0,
        );
        let subagents = work_button(mtm, target, Part::Subagents);
        let background = work_button(mtm, target, Part::Background);
        let remote = ui::symbol_button(
            mtm,
            "iphone",
            t("Remote Control: auf dem Handy oder auf claude.ai weiterarbeiten"),
            ide_ui_common::style::symbol::SMALL,
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
                &title,
                &spacer,
                &remote,
                &focus,
                &subagents,
                &background,
                &bookmark,
                &history,
                &new,
            ],
        );
        view.setEdgeInsets(NSEdgeInsets {
            top: 0.0,
            left: TITLE_INSET,
            bottom: 0.0,
            right: 12.0,
        });
        ui::fixed_height(&view, 36.0);
        Header {
            view,
            title,
            bookmark,
            focus,
            subagents,
            background,
            remote,
        }
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

    /// Shows what the list of `part` holds: in the accent colour while something runs, the numbers in the tooltip and
    /// for VoiceOver, unavailable while it is empty; `silent` names an agent that reports no such work.
    pub fn set_work(&self, part: Part, tally: Tally, silent: Option<&str>) {
        let button = self.work_button(part);
        let (running, ended) = (tally.running.to_string(), tally.ended.to_string());
        let subagents = part == Part::Subagents;
        let tip = match silent {
            Some(agent) if subagents => tf("{} meldet keine Subagenten", &[agent]),
            Some(agent) => tf("{} meldet keine Hintergrundprozesse", &[agent]),
            None if tally.listed == 0 && subagents => {
                t("Diese Sitzung hat noch keine Subagenten gestartet").to_string()
            }
            None if tally.listed == 0 => t("Diese Sitzung hat noch keine Hintergrundprozesse gestartet").to_string(),
            None if subagents => tf("Subagenten: {} laufend, {} beendet", &[&running, &ended]),
            None => tf("Hintergrundprozesse: {} laufend, {} beendet", &[&running, &ended]),
        };
        button.setToolTip(Some(&ns(&tip)));
        ide_ui_common::util::set_label(button, Some(&tip));
        button.setEnabled(silent.is_none() && tally.listed > 0);
        let color = if tally.running > 0 && silent.is_none() {
            NSColor::controlAccentColor()
        } else {
            NSColor::secondaryLabelColor()
        };
        button.setContentTintColor(Some(&color));
    }

    /// The button that opens the list of `part`; the subagents' for the whole overview.
    pub fn work_button(&self, part: Part) -> &NSButton {
        match part {
            Part::Subagents | Part::All => &self.subagents,
            Part::Background => &self.background,
        }
    }

    pub fn set_title(&self, title: &str) {
        self.title.setStringValue(&ns(title));
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

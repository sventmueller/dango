//! What the timeline's place shows when there is no conversation: the empty state, or what keeps the session from
//! starting.

use objc2::rc::Retained;
use objc2::{MainThreadMarker, sel};
use objc2_app_kit::{NSColor, NSLayoutAttribute, NSStackView, NSTextAlignment, NSView};
use splat_i18n::{t, tf};

use super::source::Problem;
use super::style;
use super::target::AgentTarget;
use super::ui;
use crate::util::ns;

fn centered(mtm: MainThreadMarker, views: &[&NSView]) -> Retained<NSStackView> {
    let stack = ui::row(mtm, 10.0, views);
    stack.setOrientation(objc2_app_kit::NSUserInterfaceLayoutOrientation::Vertical);
    stack.setAlignment(NSLayoutAttribute::CenterX);
    stack
}

/// The empty state: Claude's /model tip, nothing for the other agents.
pub fn empty(mtm: MainThreadMarker, agent: &super::Agent) -> Retained<NSStackView> {
    let mut views: Vec<Retained<NSView>> = Vec::new();
    // `/model` is a command of Claude Code; the other agents choose their model elsewhere.
    if *agent == super::Agent::Claude {
        let tip = ui::wrapping(
            mtm,
            t("Tippe /model, um das passende Modell für die Aufgabe zu wählen."),
            &style::body(),
            &NSColor::secondaryLabelColor(),
            320.0,
        );
        tip.setAlignment(NSTextAlignment::Center);
        tip.setSelectable(false);
        views.push(ui::view(&tip));
    }
    let views: Vec<&NSView> = views.iter().map(|view| &**view).collect();
    centered(mtm, &views)
}

/// What keeps the session from starting, and what the user can do about it.
/// The states only an ACP agent has: not there, being installed, waiting for a sign-in.
fn acp_problem(
    mtm: MainThreadMarker,
    target: &AgentTarget,
    problem: &Problem,
    agent: &str,
    methods: &[ide_acp::AuthMethod],
) -> Option<Retained<NSStackView>> {
    let mut buttons: Vec<Retained<NSView>> = Vec::new();
    let (symbol, title, detail) = match problem {
        Problem::Missing(plan) => {
            let detail = match plan {
                Some(plan) => {
                    buttons.push(ui::view(&ui::button(
                        mtm,
                        t("Installieren …"),
                        target,
                        sel!(ideAgentInstall:),
                        0,
                    )));
                    tf(
                        "Weder im PATH der Login-Shell noch von dieser App installiert. Installierbar ist Version {}.",
                        &[&plan.version],
                    )
                }
                None => t("Weder im PATH der Login-Shell noch von dieser App installiert.").to_string(),
            };
            ("shippingbox", tf("{} ist nicht installiert", &[agent]), detail)
        }
        Problem::Installing(progress) => (
            "arrow.down.circle",
            tf("{} wird installiert", &[agent]),
            progress.clone(),
        ),
        Problem::AcpSignIn => {
            for (n, method) in methods.iter().enumerate() {
                let tag = isize::try_from(n).unwrap_or(0);
                let button = ui::button(mtm, &method.name, target, sel!(ideAgentSignIn:), tag);
                if let Some(description) = &method.description {
                    button.setToolTip(Some(&ns(description)));
                }
                buttons.push(ui::view(&button));
            }
            let detail = if methods.iter().any(|m| m.terminal.is_some()) {
                tf(
                    "{} möchte eine Anmeldung, bevor es eine Sitzung öffnet. Bei der Anmeldung im Terminal liegt der Befehl in der Zwischenablage.",
                    &[agent],
                )
            } else {
                tf("{} möchte eine Anmeldung, bevor es eine Sitzung öffnet.", &[agent])
            };
            (
                "person.crop.circle.badge.exclamationmark",
                t("Anmeldung nötig").to_string(),
                detail,
            )
        }
        Problem::Exited(why) => (
            "stop.circle",
            tf("{} wurde beendet", &[agent]),
            why.as_ref().map_or_else(
                || t("Das Programm hat sich ohne Meldung beendet.").to_string(),
                |why| tf("Letzte Meldung: {}", &[why]),
            ),
        ),
        _ => return None,
    };
    if !matches!(problem, Problem::Installing(_)) {
        buttons.push(ui::view(&ui::button(
            mtm,
            t("Erneut versuchen"),
            target,
            sel!(ideAgentRetry:),
            0,
        )));
    }
    let icon = ui::icon(mtm, symbol, &title, 30.0, &NSColor::tertiaryLabelColor());
    let heading = ui::label(mtm, &title, &style::bold(15.0), &NSColor::labelColor());
    let text = ui::wrapping(mtm, &detail, &style::body(), &NSColor::secondaryLabelColor(), 360.0);
    text.setAlignment(NSTextAlignment::Center);
    let refs: Vec<&NSView> = buttons.iter().map(|v| &**v).collect();
    let row = ui::row(mtm, 8.0, &refs);
    Some(centered(mtm, &[&icon, &heading, &text, &row]))
}

pub fn problem(
    mtm: MainThreadMarker,
    target: &AgentTarget,
    problem: &Problem,
    agent: &str,
    methods: &[ide_acp::AuthMethod],
) -> Retained<NSStackView> {
    if let Some(view) = acp_problem(mtm, target, problem, agent, methods) {
        return view;
    }
    let (symbol, title, detail) = match problem {
        Problem::Untrusted => (
            "lock.shield",
            t("Ordner nicht vertraut").to_string(),
            tf(
                "{} startet nur in Ordnern, denen du vertraust: Agenten führen Befehle aus und lesen die Regeln des \
                 Projekts.",
                &[agent],
            ),
        ),
        Problem::NotFound => (
            "exclamationmark.triangle",
            t("Claude Code nicht gefunden").to_string(),
            t(
                "Installiere die Claude-Code-CLI, etwa mit „curl -fsSL https://claude.ai/install.sh | bash“, und \
               versuche es dann erneut.",
            )
            .to_string(),
        ),
        Problem::SignIn(_) => (
            "person.crop.circle.badge.exclamationmark",
            t("Anmeldung nötig").to_string(),
            tf(
                "Melde dich im Terminal mit „claude“ an; {} liest und speichert keine Zugangsdaten.",
                &[ide_product::NAME],
            ),
        ),
        Problem::Start(why) => (
            "exclamationmark.octagon",
            tf("{} ist nicht gestartet", &[agent]),
            tf("Fehler: {}", &[why]),
        ),
        Problem::NoModel => (
            "key",
            t("Noch kein Modell").to_string(),
            t("Trag in den Einstellungen unter „Agent“ einen Anbieter mit Schlüssel ein und wähl ein Modell, oder richte Ollama oder LM Studio ein.").to_string(),
        ),
        Problem::Missing(_) | Problem::Installing(_) | Problem::AcpSignIn | Problem::Exited(_) => {
            return centered(mtm, &[]);
        }
    };
    let icon = ui::icon(mtm, symbol, &title, 30.0, &NSColor::tertiaryLabelColor());
    let heading = ui::label(mtm, &title, &style::bold(15.0), &NSColor::labelColor());
    let text = ui::wrapping(mtm, &detail, &style::body(), &NSColor::secondaryLabelColor(), 360.0);
    text.setAlignment(NSTextAlignment::Center);
    let buttons = buttons(mtm, target, problem);
    let refs: Vec<&NSView> = buttons.iter().map(|v| &**v).collect();
    let row = ui::row(mtm, 8.0, &refs);
    centered(mtm, &[&icon, &heading, &text, &row])
}

/// What the user can do about `problem`: trust the folder, sign in, open the settings, or try again.
fn buttons(mtm: MainThreadMarker, target: &AgentTarget, problem: &Problem) -> Vec<Retained<NSView>> {
    let mut buttons: Vec<Retained<NSView>> = Vec::new();
    match problem {
        Problem::Untrusted => {
            buttons.push(ui::view(&ui::button(
                mtm,
                t("Ordner vertrauen"),
                target,
                sel!(ideAgentTrust:),
                0,
            )));
        }
        Problem::SignIn(_) => {
            buttons.push(ui::view(&ui::button(
                mtm,
                t("claude im Terminal anmelden"),
                target,
                sel!(ideAgentSignIn:),
                0,
            )));
            buttons.push(ui::view(&ui::button(
                mtm,
                t("Erneut versuchen"),
                target,
                sel!(ideAgentRetry:),
                0,
            )));
        }
        Problem::NoModel => {
            for (title, action) in [
                (t("Einstellungen öffnen"), sel!(ideAgentSettings:)),
                (t("Erneut versuchen"), sel!(ideAgentRetry:)),
            ] {
                buttons.push(ui::view(&ui::button(mtm, title, target, action, 0)));
            }
        }
        Problem::NotFound
        | Problem::Start(_)
        | Problem::Missing(_)
        | Problem::Installing(_)
        | Problem::AcpSignIn
        | Problem::Exited(_) => {
            buttons.push(ui::view(&ui::button(
                mtm,
                t("Erneut versuchen"),
                target,
                sel!(ideAgentRetry:),
                0,
            )));
        }
    }
    buttons
}

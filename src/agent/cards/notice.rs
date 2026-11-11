//! Status lines in the timeline: compaction, retries, errors, hooks, limits, interruptions.

use std::time::SystemTime;

use ide_thread::{Entry, EntryId, EntryKind, HookOutcome, Lane, LimitStatus, Notice, RateLimit, Thread};
use objc2::rc::Retained;
use objc2::sel;
use objc2_app_kit::{NSColor, NSView};
use splat_i18n::{t, tf};

use super::super::format;
use super::super::style;
use super::super::ui;
use super::{Ctx, Part, disclosure, is_auth, mono_text, notice_color, pack};

/// The window of a plan limit as the user reads it.
pub fn window_name(window: &str) -> String {
    match window {
        "five_hour" => t("5-Stundenlimit").to_string(),
        "seven_day" => t("Wochenlimit").to_string(),
        "seven_day_opus" => t("7 Tage Opus").to_string(),
        "seven_day_sonnet" => t("7 Tage Sonnet").to_string(),
        other => other.replace('_', " "),
    }
}

fn reset(at: Option<SystemTime>) -> String {
    at.map(|at| tf(" · zurückgesetzt {}", &[&format::time_of_day(at)]))
        .unwrap_or_default()
}

/// What a notice says, on one line.
pub fn text(notice: &Notice) -> String {
    match notice {
        Notice::Compaction {
            manual, tokens_before, ..
        } => {
            let freed = notice.freed_tokens().map_or_else(
                || tf("{} Tokens vorher", &[&format::tokens(*tokens_before)]),
                |freed| tf("{} Tokens frei", &[&format::tokens(freed)]),
            );
            let how = if *manual { t("von Hand") } else { t("automatisch") };
            tf("Verdichtet · {} ({})", &[&freed, how])
        }
        Notice::Retrying {
            attempt,
            max_attempts,
            error,
            http_status,
            ..
        } => {
            let mut text = tf(
                "Wiederhole … (Versuch {} von {})",
                &[&(attempt + 1).to_string(), &max_attempts.to_string()],
            );
            text.push_str(&format!(" · {error}"));
            if let Some(status) = http_status {
                text.push_str(&format!(" ({status})"));
            }
            text
        }
        Notice::Error { message, .. } if message.is_empty() => t("Fehler").to_string(),
        Notice::Error { message, .. } => message.clone(),
        Notice::Hook {
            name,
            event,
            exit_code,
            outcome,
            ..
        } => {
            let state = match outcome {
                HookOutcome::Running => t("läuft"),
                HookOutcome::Success => t("fertig"),
                HookOutcome::Error => t("fehlgeschlagen"),
                HookOutcome::Cancelled => t("abgebrochen"),
            };
            let mut text = tf("Hook {} ({}) · {}", &[name, event, state]);
            if let Some(code) = exit_code.filter(|c| *c != 0) {
                text.push_str(&tf(" · Code {}", &[&code.to_string()]));
            }
            text
        }
        Notice::RateLimit(limit) => limit_text(limit),
        Notice::Interrupted => t("Unterbrochen").to_string(),
        Notice::RewindRejected { .. } => {
            t("Zurückspulen abgelehnt, die Sitzung geht ohne Rücksprung weiter").to_string()
        }
        Notice::Info { text, .. } => text.clone(),
    }
}

pub fn limit_text(limit: &RateLimit) -> String {
    let used = limit
        .utilization
        .map(|u| format!(" {} %", (u * 100.0).round()))
        .unwrap_or_default();
    match limit.status {
        LimitStatus::Rejected => tf(
            "Nutzungslimit {} erreicht{}",
            &[&window_name(&limit.window), &reset(limit.resets_at)],
        ),
        _ => tf(
            "Nutzung {}:{}{}",
            &[&window_name(&limit.window), &used, &reset(limit.resets_at)],
        ),
    }
}

fn symbol(notice: &Notice) -> &'static str {
    match notice {
        Notice::Compaction { .. } => "arrow.down.right.and.arrow.up.left",
        Notice::Retrying { .. } => "arrow.clockwise",
        Notice::Error { .. } => "exclamationmark.triangle",
        Notice::Hook { .. } => "link",
        Notice::RateLimit(_) => "gauge.with.dots.needle.67percent",
        Notice::Interrupted => "stop.circle",
        Notice::RewindRejected { .. } => "arrow.uturn.backward.circle",
        Notice::Info { .. } => "info.circle",
    }
}

/// The hooks that ran fine in the turn of `entry`.
pub fn turn_hooks<'a>(thread: &'a Thread, entry: &Entry) -> Vec<&'a Notice> {
    let main = thread.entries(&Lane::Main);
    let at = main.iter().position(|e| e.id == entry.id).unwrap_or(0);
    let start = main[..at]
        .iter()
        .rposition(|e| matches!(e.kind, EntryKind::UserMessage(_)))
        .map_or(0, |n| n + 1);
    let end = main[at..]
        .iter()
        .position(|e| matches!(e.kind, EntryKind::UserMessage(_)))
        .map_or(main.len(), |n| at + n);
    let runs: Vec<&'a Notice> = main[start..end]
        .iter()
        .filter_map(|e| match &e.kind {
            EntryKind::Notice(
                notice @ Notice::Hook {
                    outcome: HookOutcome::Success,
                    ..
                },
            ) => Some(notice),
            _ => None,
        })
        .collect();
    runs
}

/// The line that counts the hooks of a turn that ran fine.
pub fn hook_group_text(thread: &Thread, entry: &Entry) -> String {
    tf("Hooks: {} erfolgreich", &[&turn_hooks(thread, entry).len().to_string()])
}

/// The hooks of `entry`'s turn that ran fine, as one quiet line that opens to each run.
pub fn hook_group(ctx: &Ctx<'_>, entry: &Entry) -> Retained<NSView> {
    let mtm = ctx.mtm;
    let runs = turn_hooks(ctx.thread, entry);
    let title = hook_group_text(ctx.thread, entry);
    let icon = ui::icon(mtm, "link", &title, 10.0, &NSColor::tertiaryLabelColor());
    let label = ui::label(mtm, &title, &style::small(), &NSColor::tertiaryLabelColor());
    let chevron = disclosure(ctx, entry.id, Part::Detail);
    let spacer = ui::spacer(mtm);
    let line = ui::row(mtm, 5.0, &[&icon, &label, &chevron, &spacer]);
    if !ctx.is_open(entry.id, Part::Detail) {
        return ui::view(&line);
    }
    let mut parts: Vec<Retained<NSView>> = vec![ui::view(&line)];
    for run in runs {
        let Notice::Hook {
            name, event, output, ..
        } = run
        else {
            continue;
        };
        parts.push(ui::view(&ui::label(
            mtm,
            &format!("{name} ({event})"),
            &style::small(),
            &NSColor::secondaryLabelColor(),
        )));
        if !output.trim().is_empty() {
            parts.push(ui::view(&mono_text(
                ctx,
                output.trim_end(),
                ctx.width - 16.0,
                &NSColor::tertiaryLabelColor(),
            )));
        }
    }
    let refs: Vec<&NSView> = parts.iter().map(|v| &**v).collect();
    ui::view(&ui::column(mtm, 3.0, &refs))
}

pub fn card(ctx: &Ctx<'_>, id: EntryId, notice: &Notice) -> Retained<NSView> {
    let mtm = ctx.mtm;
    let color = notice_color(notice);
    let line_text = text(notice);
    let icon = ui::icon(mtm, symbol(notice), &line_text, 11.0, &color);
    let label = ui::wrapping(mtm, &line_text, &style::small(), &color, ctx.width - 60.0);
    let mut views: Vec<Retained<NSView>> = vec![ui::view(&icon), ui::view(&label)];
    let output = match notice {
        Notice::Hook { output, .. } if !output.trim().is_empty() => Some(output.clone()),
        _ => None,
    };
    if output.is_some() {
        views.push(ui::view(&disclosure(ctx, id, Part::Detail)));
    }
    if is_auth(notice) {
        views.push(ui::view(&ui::link(
            mtm,
            t("claude im Terminal anmelden"),
            ctx.target,
            sel!(ideAgentSignIn:),
            pack(id, 0),
        )));
    }
    views.push(ui::spacer(mtm));
    let refs: Vec<&NSView> = views.iter().map(|v| &**v).collect();
    let line = ui::row(mtm, 5.0, &refs);
    line.setAlignment(objc2_app_kit::NSLayoutAttribute::FirstBaseline);
    match output.filter(|_| ctx.is_open(id, Part::Detail)) {
        Some(output) => {
            let text = mono_text(ctx, &output, ctx.width - 18.0, &NSColor::secondaryLabelColor());
            let frame = ui::framed(
                mtm,
                &text,
                &NSColor::separatorColor(),
                Some(&style::code_background()),
                6.0,
                ui::insets(6.0, 8.0, 6.0, 8.0),
            );
            ui::view(&ui::column(mtm, 4.0, &[&line, &frame]))
        }
        None => ui::view(&line),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_compaction_says_how_much_it_freed() {
        let notice = Notice::Compaction {
            manual: false,
            tokens_before: 180_000,
            tokens_after: Some(42_000),
        };
        assert_eq!(text(&notice), "Verdichtet · 138 k Tokens frei (automatisch)");
    }

    #[test]
    fn a_retry_counts_the_attempt_that_comes() {
        let notice = Notice::Retrying {
            attempt: 2,
            max_attempts: 10,
            delay: std::time::Duration::from_secs(2),
            error: "server_error".into(),
            http_status: Some(529),
        };
        assert_eq!(text(&notice), "Wiederhole … (Versuch 3 von 10) · server_error (529)");
    }
}

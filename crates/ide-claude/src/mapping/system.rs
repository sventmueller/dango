//! The system frames: session start, status, retries, compaction, hooks, rate limits and notices.

use std::time::{Duration, SystemTime, UNIX_EPOCH};

use ide_thread::{
    AgentMessage, EntryKind, ErrorKind, HookOutcome, InfoLevel, Lane, LimitStatus, Notice, RateLimit, Status,
    ToolPatch, ToolStatus, Update,
};

use super::context::context_used;
use super::{Mapper, notice, put, seconds};
use crate::protocol::{Hook, RateLimitInfo, System};

impl Mapper {
    pub(super) fn system(&mut self, s: &System, now: SystemTime) -> Vec<Update> {
        match s {
            System::Init(init) => {
                self.model = Some(init.model.clone()).filter(|m| !m.is_empty());
                let mut out = vec![Update::Session(init.session_id.clone())];
                if let Some(model) = &self.model {
                    out.push(Update::Model(model.clone()));
                }
                if !init.permission_mode.is_empty() {
                    out.push(Update::Mode(init.permission_mode.clone()));
                }
                if init.effort.is_some() {
                    out.push(Update::Effort(init.effort.clone()));
                }
                out
            }
            System::Status {
                status,
                permission_mode,
                compact_failed,
                compact_error,
            } => status_frame(
                status.as_deref(),
                permission_mode.as_ref(),
                *compact_failed,
                compact_error,
                now,
            ),
            System::ApiRetry(r) => vec![put(
                &Lane::Main,
                Some(format!("retry:{}", self.turn)),
                now,
                EntryKind::Notice(Notice::Retrying {
                    attempt: r.attempt,
                    max_attempts: r.max_retries,
                    delay: Duration::from_millis(r.retry_delay_ms),
                    error: r.error.clone(),
                    http_status: r.error_status,
                }),
            )],
            System::CompactBoundary(c) => {
                let mut out = vec![notice(
                    now,
                    Notice::Compaction {
                        manual: c.trigger == "manual",
                        tokens_before: c.pre_tokens,
                        tokens_after: c.post_tokens,
                    },
                )];
                if let Some(after) = c.post_tokens {
                    out.push(context_used(after));
                }
                out
            }
            System::TaskStarted(t) => self.task_started(t, now),
            System::TaskProgress(p) => self.task_progress(p, now),
            System::TaskUpdated(u) => self.task_updated(u, now),
            System::TaskNotification(n) => self.task_notification(n, now),
            System::BackgroundTasksChanged(tasks) => self.background_changed(tasks, now),
            System::Hook(hook) => vec![hook_notice(hook, now)],
            System::SessionStateChanged(state) => match state.as_str() {
                "idle" => vec![self.status(Status::Idle)],
                "running" => vec![self.status(Status::Running)],
                "requires_action" => vec![self.status(Status::RequiresAction)],
                _ => Vec::new(),
            },
            _ => self.other_system(s, now),
        }
    }

    fn other_system(&mut self, s: &System, now: SystemTime) -> Vec<Update> {
        match s {
            System::Informational { content, level } => {
                let level = match level.as_str() {
                    "warning" => InfoLevel::Warning,
                    "suggestion" => InfoLevel::Suggestion,
                    "notice" => InfoLevel::Notice,
                    _ => InfoLevel::Info,
                };
                vec![notice(
                    now,
                    Notice::Info {
                        level,
                        text: content.clone(),
                    },
                )]
            }
            System::LocalCommandOutput(text) => vec![put(
                &Lane::Main,
                None,
                now,
                EntryKind::AgentMessage(AgentMessage {
                    markdown: text.clone(),
                    streaming: false,
                }),
            )],
            System::Notification(text) => vec![notice(
                now,
                Notice::Info {
                    level: InfoLevel::Notice,
                    text: text.clone(),
                },
            )],
            System::PermissionDenied { tool_use_id, .. } => vec![Update::Tool {
                key: tool_use_id.clone(),
                patch: ToolPatch {
                    status: Some(ToolStatus::Failed),
                    ..ToolPatch::default()
                },
            }],
            System::Refusal { content, retracted } => {
                let mut out: Vec<Update> = retracted.iter().flat_map(|uuid| self.retract(uuid)).collect();
                out.push(notice(
                    now,
                    Notice::Info {
                        level: InfoLevel::Warning,
                        text: content.clone(),
                    },
                ));
                out
            }
            System::WritesLost(code) => vec![notice(
                now,
                Notice::Error {
                    kind: ErrorKind::Disk,
                    message: code.clone(),
                },
            )],
            _ => Vec::new(),
        }
    }
}

fn status_frame(
    status: Option<&str>,
    mode: Option<&String>,
    compact_failed: bool,
    compact_error: &Option<String>,
    now: SystemTime,
) -> Vec<Update> {
    let mut out = vec![Update::Activity(match status {
        Some("compacting") => Some(ide_thread::Activity::Compacting),
        Some("requesting") => Some(ide_thread::Activity::Requesting),
        _ => None,
    })];
    if let Some(mode) = mode {
        out.push(Update::Mode(mode.clone()));
    }
    if compact_failed {
        out.push(notice(
            now,
            Notice::Error {
                kind: ErrorKind::Compaction,
                message: compact_error.clone().unwrap_or_default(),
            },
        ));
    }
    out
}

pub(super) fn rate_limit(info: &RateLimitInfo, now: SystemTime) -> Vec<Update> {
    let status = match info.status.as_str() {
        "allowed" => LimitStatus::Allowed,
        "rejected" => LimitStatus::Rejected,
        _ => LimitStatus::Warning,
    };
    let limit = RateLimit {
        window: info.rate_limit_type.clone().unwrap_or_else(|| "unknown".into()),
        status,
        utilization: info.utilization,
        resets_at: info.resets_at.and_then(|s| seconds(s).map(|d| UNIX_EPOCH + d)),
    };
    let mut out = vec![Update::RateLimit(limit.clone())];
    if status != LimitStatus::Allowed {
        out.push(put(
            &Lane::Main,
            Some(format!("limit:{}:{}", limit.window, info.status)),
            now,
            EntryKind::Notice(Notice::RateLimit(limit)),
        ));
    }
    out
}

fn hook_notice(hook: &Hook, now: SystemTime) -> Update {
    let outcome = match hook.outcome.as_deref() {
        None => HookOutcome::Running,
        Some("success") => HookOutcome::Success,
        Some("cancelled") => HookOutcome::Cancelled,
        Some(_) => HookOutcome::Error,
    };
    let output = if hook.output.is_empty() {
        hook.stderr.clone()
    } else {
        hook.output.clone()
    };
    put(
        &Lane::Main,
        Some(format!("hook:{}", hook.hook_id)),
        now,
        EntryKind::Notice(Notice::Hook {
            id: hook.hook_id.clone(),
            name: hook.hook_name.clone(),
            event: hook.hook_event.clone(),
            output,
            exit_code: hook.exit_code,
            outcome,
        }),
    )
}

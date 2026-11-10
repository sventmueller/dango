//! Compacting the conversation (E42): when the context window is nearly full, or the provider says the conversation
//! no longer fits, the same model summarizes the older turns into one summary, to which the open to-dos and the
//! files touched are added as they are; the last turns stay as they were. `/compact` does it on the user's word.

use std::collections::BTreeSet;
use std::time::SystemTime;

use ide_thread::{Activity, EntryKind, ErrorKind as ThreadError, Lane, Notice, Todo, TodoStatus, Update};
use serde_json::Value;

use crate::chat::{Cancel, Request, Stop};
use crate::conversation::{Block, Message, Role};
use crate::notes::Note;
use crate::session::{Compacted, Shared};

/// The share of the context window, in percent, from which the conversation is compacted.
const THRESHOLD: u64 = 85;
/// How many of the last turns stay as they were, where they fit in half the window.
const KEEP_TURNS: usize = 2;
/// The longest summary, in tokens.
const SUMMARY_TOKENS: u64 = 8_192;
/// Characters of one part of the conversation the summarizing model reads at most.
const PART_CHARS: usize = 4_000;
/// The window assumed for a model whose window is not known.
const DEFAULT_WINDOW: u64 = 128_000;
/// How a summary begins, so that a resumed thread shows it as a compaction rather than as a message.
pub(crate) const MARK: &str = "<conversation-summary";

const SYSTEM: &str = "You compact the conversation of a coding agent with its user, so that the agent can go on \
    without the full history. Write a concise summary for the agent, in the language of the conversation, that \
    keeps: what the user asked for, with their preferences and constraints; what was done and decided, and why; the \
    files and code that matter, with their paths; errors met and how they were resolved; and what remains to be \
    done, with the next step. Leave out what no longer matters, such as file contents that can be read again. Write \
    only the summary, without a preamble.";

/// Whether the conversation fills so much of the context window that it is compacted before the next request.
pub(crate) fn due(shared: &Shared) -> bool {
    let state = shared.lock();
    let Some(size) = state.context_size.filter(|size| *size > 0) else {
        return false;
    };
    let used = state.context_used.unwrap_or_else(|| estimate(&state.conversation));
    used.saturating_mul(100) >= size.saturating_mul(THRESHOLD)
}

/// Compacts the conversation, telling the user; whether it was compacted.
pub(crate) fn run(shared: &Shared, cancel: &Cancel, manual: bool, focus: Option<&str>) -> bool {
    let (messages, provider, model, window, before, todos) = {
        let state = shared.lock();
        (
            state.conversation.clone(),
            state.provider.clone(),
            state.model.clone(),
            state.context_size.unwrap_or(DEFAULT_WINDOW),
            state.context_used.unwrap_or_else(|| estimate(&state.conversation)),
            state.thread.todos().to_vec(),
        )
    };
    let cut = cut(&messages, window);
    let older = &messages[..cut];
    if !older.iter().any(|m| m.role == Role::Assistant) {
        if manual {
            crate::turn::notice(
                shared,
                Notice::Info {
                    level: ide_thread::InfoLevel::Notice,
                    text: shared.wording.note(&Note::NothingToCompact),
                },
            );
        }
        return false;
    }
    let mut prompt = format!(
        "The conversation to summarize:\n\n<transcript>\n{}\n</transcript>",
        transcript(older, window)
    );
    if let Some(focus) = focus {
        prompt.push_str(&format!("\n\nThe user asks you to keep this in mind: {focus}"));
    }
    let asked = [Message::user(&prompt)];
    let request = Request {
        model: &model,
        system: SYSTEM,
        messages: &asked,
        tools: &[],
        max_tokens: Some(SUMMARY_TOKENS),
        effort: None,
    };
    shared.apply(vec![Update::Activity(Some(Activity::Compacting))]);
    let result = provider.stream(&request, cancel, &mut |_| {});
    shared.apply(vec![Update::Activity(None)]);
    let reply = match result {
        Ok(reply) if reply.stop != Stop::Cancelled && !reply.message.text().trim().is_empty() => reply,
        Ok(_) => return false,
        Err(error) => {
            crate::turn::notice(
                shared,
                Notice::Error {
                    kind: ThreadError::Compaction,
                    message: shared.wording.note(&Note::CompactionFailed(error)),
                },
            );
            return false;
        }
    };
    crate::turn::count(shared, &reply.usage);
    let continues = cut == messages.len() && !manual;
    let summary = summary(&reply.message.text(), older, &todos, (manual, before), continues);
    let mut kept = messages[cut..].to_vec();
    match kept.first_mut() {
        Some(first) => first.content.insert(0, Block::text(&summary)),
        None => kept.push(Message::user(&summary)),
    }
    replace(shared, kept, manual, before);
    true
}

/// Puts the compacted conversation in place, with its notice.
fn replace(shared: &Shared, kept: Vec<Message>, manual: bool, before: u64) {
    let key = shared.key("n");
    let after = estimate(&kept);
    {
        let mut state = shared.lock();
        state.conversation = kept;
        state.context_used = None;
        state.compacted = Some(Compacted {
            key: key.clone(),
            manual,
            tokens_before: before,
        });
    }
    let mut usage = crate::turn::usage(shared);
    usage.context_used = Some(after);
    shared.apply(vec![
        Update::Put {
            lane: Lane::Main,
            key: Some(key),
            at: SystemTime::now(),
            entry: EntryKind::Notice(Notice::Compaction {
                manual,
                tokens_before: before,
                tokens_after: None,
            }),
        },
        Update::Usage(usage),
    ]);
    shared.save();
}

/// Where the kept part begins: the last turns that fit in half the window, else nothing is kept.
fn cut(messages: &[Message], window: u64) -> usize {
    let starts: Vec<usize> = messages
        .iter()
        .enumerate()
        .filter(|(_, m)| m.role == Role::User && m.content.iter().any(|b| matches!(b, Block::Text { .. })))
        .map(|(n, _)| n)
        .collect();
    (1..=KEEP_TURNS)
        .rev()
        .filter(|keep| starts.len() > *keep)
        .map(|keep| starts[starts.len() - keep])
        .find(|at| estimate(&messages[*at..]) <= window / 2)
        .unwrap_or(messages.len())
}

/// Roughly how many tokens `messages` take: four characters a token, an image some hundreds.
pub(crate) fn estimate(messages: &[Message]) -> u64 {
    let chars: usize = messages
        .iter()
        .flat_map(|m| &m.content)
        .map(|block| match block {
            Block::Text { text, .. } | Block::Thinking { text, .. } => text.len(),
            Block::Image { .. } => 6_400,
            Block::RedactedThinking { data } => data.len(),
            Block::ToolUse { input, .. } => input.to_string().len(),
            Block::ToolResult { content, .. } => content.len(),
        })
        .sum();
    u64::try_from(chars / 4).unwrap_or(u64::MAX)
}

fn clip(text: &str, limit: usize) -> String {
    let mut clipped: String = text.chars().take(limit).collect();
    if clipped.len() < text.len() {
        clipped.push_str(" […]");
    }
    clipped
}

/// The older turns as text for the summarizing model, its middle left out where it would not fit.
fn transcript(messages: &[Message], window: u64) -> String {
    let mut parts = Vec::new();
    for message in messages {
        for block in &message.content {
            let part = match (message.role, block) {
                (Role::User, Block::Text { text, .. }) if text.starts_with(MARK) => {
                    format!("[summary of an earlier part]\n{text}")
                }
                (Role::User, Block::Text { text, .. }) => format!("[user]\n{}", clip(text, PART_CHARS * 4)),
                (Role::User, Block::Image { .. }) => "[the user attached an image]".into(),
                (Role::Assistant, Block::Text { text, .. }) => format!("[agent]\n{}", clip(text, PART_CHARS * 2)),
                (_, Block::ToolUse { name, input, .. }) => {
                    format!("[agent calls {name}] {}", clip(&input.to_string(), PART_CHARS))
                }
                (
                    _,
                    Block::ToolResult {
                        name,
                        content,
                        is_error,
                        ..
                    },
                ) => {
                    let failed = if *is_error { ", failed" } else { "" };
                    format!("[result of {name}{failed}]\n{}", clip(content, PART_CHARS))
                }
                _ => continue,
            };
            parts.push(part);
        }
    }
    let text = parts.join("\n\n");
    let limit = usize::try_from(window.saturating_mul(2)).unwrap_or(usize::MAX);
    let total = text.chars().count();
    if total <= limit {
        return text;
    }
    let head: String = text.chars().take(limit / 5).collect();
    let tail: String = text.chars().skip(total - (limit - limit / 5)).collect();
    format!("{head}\n\n[… a part of the conversation is left out …]\n\n{tail}")
}

/// The summary as the conversation keeps it: the model's words, then the files touched and the open to-dos, which
/// are listed as they are rather than left to the model.
fn summary(text: &str, older: &[Message], todos: &[Todo], (manual, before): (bool, u64), continues: bool) -> String {
    let (mut changed, mut read) = (BTreeSet::new(), BTreeSet::new());
    for message in older {
        for block in &message.content {
            match block {
                Block::ToolUse { name, input, .. } => {
                    let path = input.get("path").and_then(Value::as_str).map(str::to_string);
                    match name.as_str() {
                        "edit_file" | "write_file" => changed.extend(path),
                        "read_file" => read.extend(path),
                        _ => {}
                    }
                }
                Block::Text { text, .. } if text.starts_with(MARK) => {
                    for line in text.lines() {
                        let listed = |prefix: &str| line.strip_prefix(prefix).map(|rest| rest.split(", "));
                        if let Some(paths) = listed("Files changed: ") {
                            changed.extend(paths.map(str::to_string));
                        } else if let Some(paths) = listed("Files read: ") {
                            read.extend(paths.map(str::to_string));
                        }
                    }
                }
                _ => {}
            }
        }
    }
    let mut out = format!(
        "{MARK} manual=\"{manual}\" tokens=\"{before}\">\nThe earlier part of this conversation was compacted. Its \
         summary:\n\n{}\n",
        text.trim()
    );
    let read: Vec<String> = read.difference(&changed).cloned().collect();
    for (title, paths) in [
        ("Files changed", changed.into_iter().collect::<Vec<_>>()),
        ("Files read", read),
    ] {
        if !paths.is_empty() {
            out.push_str(&format!("\n{title}: {}", paths.join(", ")));
        }
    }
    let open: Vec<String> = todos
        .iter()
        .filter(|todo| todo.status != TodoStatus::Completed)
        .map(|todo| {
            let status = if todo.status == TodoStatus::InProgress {
                "in progress"
            } else {
                "pending"
            };
            format!("- [{status}] {}", todo.content)
        })
        .collect();
    if !open.is_empty() {
        out.push_str(&format!("\nOpen to-dos:\n{}", open.join("\n")));
    }
    out.push_str("\n</conversation-summary>\n\n");
    if continues {
        out.push_str("Continue with the task where the summary ends.");
    }
    out
}

/// Whether the user started the compaction and how many tokens the window held before, when `text` is a summary.
pub(crate) fn summary_mark(text: &str) -> Option<(bool, u64)> {
    let head = text.strip_prefix(MARK)?.lines().next()?;
    let value = |name: &str| {
        let start = head.find(&format!("{name}=\""))? + name.len() + 2;
        head[start..].split('"').next()
    };
    Some((value("manual")? == "true", value("tokens")?.parse().ok()?))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn call(id: &str, name: &str, path: &str) -> Message {
        Message {
            role: Role::Assistant,
            content: vec![Block::ToolUse {
                id: id.into(),
                name: name.into(),
                input: json!({"path": path}),
                signature: None,
                invalid: None,
            }],
        }
    }

    fn result(id: &str, name: &str, content: &str) -> Message {
        Message {
            role: Role::User,
            content: vec![Block::ToolResult {
                tool_use_id: id.into(),
                name: name.into(),
                content: content.into(),
                is_error: false,
                note: None,
            }],
        }
    }

    fn turn(n: usize, size: usize) -> Vec<Message> {
        vec![
            Message::user(&format!("task {n}")),
            call(&format!("r{n}"), "read_file", &format!("f{n}.rs")),
            result(&format!("r{n}"), "read_file", &"x".repeat(size)),
            Message {
                role: Role::Assistant,
                content: vec![Block::text("done")],
            },
        ]
    }

    #[test]
    fn the_last_turns_are_kept_while_they_fit_in_half_the_window() {
        let messages: Vec<Message> = (0..4).flat_map(|n| turn(n, 400)).collect();
        assert_eq!(cut(&messages, 1_000), 8, "the last two turns stay");
        assert_eq!(cut(&messages, 300), 12, "only the last turn fits");
        assert_eq!(cut(&messages, 50), 16, "nothing fits, so all is summarized");
        assert_eq!(cut(&turn(0, 10), 1_000), 4, "a single turn is summarized whole");
    }

    #[test]
    fn the_summary_lists_the_files_and_open_todos_and_reads_back_its_mark() {
        let older: Vec<Message> = [turn(0, 10), vec![call("e1", "edit_file", "f0.rs")]].concat();
        let todos = vec![
            Todo {
                content: "Fix it".into(),
                active_form: None,
                status: TodoStatus::InProgress,
            },
            Todo {
                content: "Read it".into(),
                active_form: None,
                status: TodoStatus::Completed,
            },
        ];
        let text = summary("They fixed f0.", &older, &todos, (true, 91_000), false);
        assert!(text.contains("They fixed f0.\n\nFiles changed: f0.rs\nOpen to-dos:\n- [in progress] Fix it"));
        assert!(!text.contains("Files read") && !text.contains("Read it"), "{text}");
        assert_eq!(summary_mark(&text), Some((true, 91_000)));
        let again = summary("More.", &[Message::user(&text)], &[], (false, 5), true);
        assert!(again.contains("Files changed: f0.rs") && again.ends_with("where the summary ends."));
        assert_eq!(summary_mark("plain"), None);
    }

    #[test]
    fn a_transcript_too_long_for_the_window_leaves_out_its_middle() {
        let messages: Vec<Message> = (0..40).flat_map(|n| turn(n, 3_000)).collect();
        let text = transcript(&messages, 10_000);
        assert!(text.starts_with("[user]\ntask 0") && text.contains("left out") && text.ends_with("done"));
        assert!(text.chars().count() < 21_000);
    }
}

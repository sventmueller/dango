//! The timeline of a resumed thread, rebuilt from its stored conversation: the user's messages without the files
//! attached to them, the answers, the reasoning, each tool call with what its result holds of data, and where the
//! conversation was compacted.

use std::collections::HashMap;
use std::time::SystemTime;

use ide_thread::{
    AgentMessage, EntryKind, Lane, Notice, Thought, Todo, TodoStatus, ToolCall, ToolOutput, ToolStatus, Update,
    UserMessage,
};
use serde_json::Value;

use crate::conversation::{Block, Message, Role};
use crate::notes::{ToolNote, Wording};
use crate::workspace::Workspace;

/// A call's stored result.
#[derive(Clone, Copy)]
struct Kept<'a> {
    content: &'a str,
    is_error: bool,
    note: Option<&'a ToolNote>,
}

/// The updates that show `messages` as a finished timeline, the cards' notes in `wording`.
pub(crate) fn updates(messages: &[Message], workspace: &Workspace, wording: &dyn Wording) -> Vec<Update> {
    let results: HashMap<&str, Kept<'_>> = messages
        .iter()
        .flat_map(|m| &m.content)
        .filter_map(|block| match block {
            Block::ToolResult {
                tool_use_id,
                content,
                is_error,
                note,
                ..
            } => Some((
                tool_use_id.as_str(),
                Kept {
                    content,
                    is_error: *is_error,
                    note: note.as_ref(),
                },
            )),
            _ => None,
        })
        .collect();
    let at = SystemTime::now();
    let mut updates = Vec::new();
    let mut todos: Option<Vec<Todo>> = None;
    for (n, message) in messages.iter().enumerate() {
        for (m, block) in message.content.iter().enumerate() {
            let key = format!("r{n}.{m}");
            let entry = match (message.role, block) {
                (Role::User, Block::Text { text, .. }) if text.starts_with(crate::compact::MARK) => {
                    let (manual, tokens_before) = crate::compact::summary_mark(text).unwrap_or_default();
                    EntryKind::Notice(Notice::Compaction {
                        manual,
                        tokens_before,
                        tokens_after: None,
                    })
                }
                (Role::User, Block::Text { text, .. }) => EntryKind::UserMessage(UserMessage {
                    text: typed(text).to_string(),
                    attachments: images(&message.content),
                    message_id: None,
                }),
                (Role::Assistant, Block::Text { text, .. }) => EntryKind::AgentMessage(AgentMessage {
                    markdown: text.clone(),
                    streaming: false,
                }),
                (Role::Assistant, Block::Thinking { text, .. }) if !text.is_empty() => EntryKind::Thought(Thought {
                    text: text.clone(),
                    duration: None,
                    streaming: false,
                }),
                (Role::Assistant, Block::ToolUse { id, name, input, .. }) => {
                    if name == "todo_write" {
                        todos = todo_list(input).or(todos);
                    }
                    let result = results.get(id.as_str()).copied();
                    EntryKind::ToolCall(card(id, name, input, result, workspace, wording))
                }
                _ => continue,
            };
            updates.push(Update::Put {
                lane: Lane::Main,
                key: Some(key),
                at,
                entry,
            });
        }
    }
    if let Some(todos) = todos {
        updates.push(Update::Todos(todos));
    }
    updates
}

/// The images of a user message, to show with it again.
fn images(content: &[Block]) -> Vec<ide_thread::Attachment> {
    use base64::Engine;
    content
        .iter()
        .filter_map(|block| match block {
            Block::Image { media_type, data } => Some(ide_thread::Attachment::Image {
                name: None,
                media_type: media_type.clone(),
                data: base64::engine::general_purpose::STANDARD.decode(data).ok()?.into(),
            }),
            _ => None,
        })
        .collect()
}

/// What the user typed, without the files and selections the app attached after it.
fn typed(text: &str) -> &str {
    ["\n\n<file path=", "\n\n<selection path="]
        .iter()
        .filter_map(|mark| text.find(mark))
        .min()
        .map_or(text, |end| &text[..end])
}

fn card(
    id: &str,
    name: &str,
    input: &Value,
    result: Option<Kept<'_>>,
    workspace: &Workspace,
    wording: &dyn Wording,
) -> ToolCall {
    let (kind, title, view) = crate::tools::card(name, input, workspace);
    let mut call = ToolCall::new(id, name, kind);
    call.title = title;
    call.view = view;
    call.input = input.clone();
    let worded = |kept: Kept<'_>| kept.note.map(|note| ToolOutput::Text(wording.tool(note)));
    (call.status, call.output) = match result {
        Some(kept) if !kept.is_error => (
            ToolStatus::Completed,
            worded(kept).or_else(|| output(name, kept.content)),
        ),
        Some(kept) => {
            let shown = (name == "run_command").then(|| output(name, kept.content)).flatten();
            (ToolStatus::Failed, worded(kept).or(shown))
        }
        None => (ToolStatus::Failed, None),
    };
    call
}

/// What a card shows of a stored result: its data, without what the tools told the model in their own words.
fn output(name: &str, content: &str) -> Option<ToolOutput> {
    let without_hint = || {
        let (rest, last) = content.rsplit_once('\n').unwrap_or(("", content));
        if last.starts_with('(') && last.ends_with(')') {
            rest
        } else {
            content
        }
    };
    let numbered = |line: &str| {
        line.split_once('\t')
            .is_some_and(|(n, _)| n.trim().parse::<u64>().is_ok())
    };
    let said = [
        crate::tools::NO_MATCHES,
        crate::tools::NO_FILES,
        &ToolNote::NoProblems.for_model(),
        &ToolNote::NoLanguageServer.for_model(),
    ]
    .contains(&content);
    let data = match name {
        _ if said => return None,
        "read_file" if !content.lines().next().is_some_and(numbered) => return None,
        "read_file" | "grep" | "glob" => without_hint(),
        "diagnostics" => content,
        "web_fetch" => content.split_once("\n\n").map_or(content, |(_, page)| page),
        "run_command" => return Some(terminal(content)),
        _ => return None,
    };
    Some(ToolOutput::Text(data.to_string()))
}

/// A command's stored result, its streams taken from the tags they were sent to the model in.
fn terminal(content: &str) -> ToolOutput {
    let stream = |name: &str| {
        let start = content.find(&format!("<{name}>\n"))? + name.len() + 3;
        let end = content[start..].find(&format!("\n</{name}>"))? + start;
        Some(content[start..end].to_string())
    };
    ToolOutput::Terminal {
        stdout: stream("stdout").unwrap_or_default(),
        stderr: stream("stderr").unwrap_or_default(),
        interrupted: content
            .lines()
            .next()
            .is_some_and(|status| status.contains(", stopped")),
    }
}

fn todo_list(input: &Value) -> Option<Vec<Todo>> {
    input
        .get("todos")?
        .as_array()?
        .iter()
        .map(|item| {
            let status = match item.get("status")?.as_str()? {
                "pending" => TodoStatus::Pending,
                "in_progress" => TodoStatus::InProgress,
                "completed" => TodoStatus::Completed,
                _ => return None,
            };
            Some(Todo {
                content: item.get("content")?.as_str()?.to_string(),
                active_form: item.get("active_form").and_then(Value::as_str).map(str::to_string),
                status,
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stored_results_show_their_data_without_the_words_for_the_model() {
        assert_eq!(
            output(
                "read_file",
                "     1\tfn main() {}\n(3 more lines; read on with offset 2)"
            ),
            Some(ToolOutput::Text("     1\tfn main() {}".into()))
        );
        assert_eq!(output("read_file", "src/a.rs is empty."), None);
        assert_eq!(output("grep", "No matches."), None);
        assert_eq!(output("diagnostics", "No problems reported."), None);
        assert_eq!(
            output("web_fetch", "The following is the content of x.\n\nDocs"),
            Some(ToolOutput::Text("Docs".into()))
        );
        assert_eq!(
            output(
                "run_command",
                "exit code 1, stopped by the user\n<stdout>\nhi\n</stdout>"
            ),
            Some(ToolOutput::Terminal {
                stdout: "hi".into(),
                stderr: String::new(),
                interrupted: true
            })
        );
        assert_eq!(output("edit_file", "Edited a.rs: 1 replacement(s)."), None);
    }

    #[test]
    fn attachments_are_left_out_of_the_shown_message() {
        assert_eq!(typed("Fix it\n\n<file path=\"a.rs\">\nx\n</file>"), "Fix it");
        assert_eq!(typed("plain"), "plain");
    }
}

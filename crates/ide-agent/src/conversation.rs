//! The conversation as the agent keeps and stores it, the same for every provider; each wire module turns it into
//! its own request shape.

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::provider::Wire;

/// Who said something.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Role {
    /// The user, or the tool results the agent sends back.
    User,
    /// The model.
    Assistant,
}

/// One turn of the conversation.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Message {
    /// Who said it.
    pub role: Role,
    /// What it holds, in order.
    pub content: Vec<Block>,
}

impl Message {
    /// A user message of plain text.
    pub fn user(text: &str) -> Message {
        Message {
            role: Role::User,
            content: vec![Block::text(text)],
        }
    }

    /// Its text blocks, joined.
    pub fn text(&self) -> String {
        self.content
            .iter()
            .filter_map(|block| match block {
                Block::Text { text, .. } => Some(text.as_str()),
                _ => None,
            })
            .collect()
    }

    /// The tool calls it makes.
    pub fn tool_uses(&self) -> impl Iterator<Item = (&str, &str, &Value)> {
        self.content.iter().filter_map(|block| match block {
            Block::ToolUse { id, name, input, .. } => Some((id.as_str(), name.as_str(), input)),
            _ => None,
        })
    }
}

/// One part of a message.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Block {
    /// Text.
    Text {
        /// The text.
        text: String,
        /// Gemini's signature of its reasoning, returned with the part it came on.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        signature: Option<String>,
    },
    /// An image the user attached.
    Image {
        /// Its type, such as `image/png`.
        media_type: String,
        /// Its bytes in base64.
        data: String,
    },
    /// The model's reasoning, as far as the provider shows it.
    Thinking {
        /// What the provider shows of it, which may be empty.
        text: String,
        /// Anthropic's signature, without which the block cannot be sent back.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        signature: Option<String>,
        /// The protocol it came over; only that one takes it back.
        wire: Wire,
    },
    /// Reasoning the provider returns only as opaque data, to be sent back unchanged.
    RedactedThinking {
        /// The data.
        data: String,
    },
    /// A call of one of the agent's tools.
    ToolUse {
        /// The provider's id for the call, which the result names.
        id: String,
        /// The tool.
        name: String,
        /// Its input, always an object.
        input: Value,
        /// Gemini's signature of the reasoning behind the call.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        signature: Option<String>,
        /// The input as it arrived when it was not a JSON object, which the call then answers with an error.
        #[serde(skip)]
        invalid: Option<String>,
    },
    /// What a tool call returned.
    ToolResult {
        /// The call.
        tool_use_id: String,
        /// The tool, which Gemini's answer names.
        name: String,
        /// What the model reads.
        content: String,
        /// Whether the call failed.
        #[serde(default)]
        is_error: bool,
        /// What the call's card says, which the app words again when the thread is resumed; no provider sees it.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        note: Option<crate::notes::ToolNote>,
    },
}

impl Block {
    /// A text block.
    pub fn text(text: &str) -> Block {
        Block::Text {
            text: text.to_string(),
            signature: None,
        }
    }
}

/// Consecutive messages of the same role as one, which every protocol takes.
pub(crate) fn merged(messages: &[Message]) -> Vec<Message> {
    let mut out: Vec<Message> = Vec::with_capacity(messages.len());
    for message in messages {
        match out.last_mut() {
            Some(last) if last.role == message.role => last.content.extend(message.content.iter().cloned()),
            _ => out.push(message.clone()),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_conversation_survives_storage_but_an_invalid_input_does_not() {
        let messages = vec![
            Message::user("hi"),
            Message {
                role: Role::Assistant,
                content: vec![
                    Block::Thinking {
                        text: "hm".into(),
                        signature: Some("sig".into()),
                        wire: Wire::Anthropic,
                    },
                    Block::ToolUse {
                        id: "t1".into(),
                        name: "read_file".into(),
                        input: serde_json::json!({"path": "a"}),
                        signature: None,
                        invalid: Some("{bad".into()),
                    },
                ],
            },
        ];
        let stored = serde_json::to_string(&messages).unwrap();
        let read: Vec<Message> = serde_json::from_str(&stored).unwrap();
        let Block::ToolUse { invalid, .. } = &read[1].content[1] else {
            panic!()
        };
        assert_eq!(invalid, &None);
        assert_eq!(read[0], messages[0]);
        assert!(stored.contains("\"wire\":\"anthropic\""));
    }

    #[test]
    fn neighbours_of_one_role_merge() {
        let merged = merged(&[Message::user("a"), Message::user("b")]);
        assert_eq!(merged.len(), 1);
        assert_eq!(merged[0].text(), "ab");
    }
}

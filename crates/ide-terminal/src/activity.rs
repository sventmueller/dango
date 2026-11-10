//! What a program's output does besides drawing: whether it changes what the screen shows, as opposed to only setting
//! the title, colours or the cursor, and the titles and bells in it. Telling these apart lets an observer see work in
//! a program that redraws its text, but not in one that only animates its window title while it waits.

/// Where in an escape sequence the watcher is, across chunks of output.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
enum State {
    #[default]
    Text,
    Escape,
    /// An escape sequence with intermediate bytes, such as a character set designation.
    Intermediate,
    Csi,
    /// An OSC string, collected when it may be a title.
    Osc,
    OscEscape,
    /// A DCS, APC, PM or SOS string, ignored up to its terminator.
    Ignored,
    IgnoredEscape,
}

/// What one chunk of output held.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Seen {
    /// Characters were written, or text erased, inserted or scrolled.
    pub content: bool,
    /// The bell rang.
    pub bell: bool,
    /// Window titles set (OSC 0 and 2), in order.
    pub titles: Vec<String>,
}

/// Follows a program's output chunk by chunk.
#[derive(Debug, Default)]
pub struct OutputWatch {
    state: State,
    osc: Vec<u8>,
}

/// The longest OSC string kept; titles are short and anything longer is not one.
const MAX_OSC: usize = 4096;

impl OutputWatch {
    /// What `bytes`, the next chunk of output, holds.
    pub fn feed(&mut self, bytes: &[u8]) -> Seen {
        let mut seen = Seen::default();
        for &byte in bytes {
            self.state = match self.state {
                State::Text => match byte {
                    0x1b => State::Escape,
                    0x07 => {
                        seen.bell = true;
                        State::Text
                    }
                    b'\n' => {
                        // A line feed at the bottom scrolls the text.
                        seen.content = true;
                        State::Text
                    }
                    0x20..=0x7e | 0x80..=0xff => {
                        seen.content = true;
                        State::Text
                    }
                    _ => State::Text,
                },
                State::Escape => match byte {
                    b'[' => State::Csi,
                    b']' => {
                        self.osc.clear();
                        State::Osc
                    }
                    b'P' | b'X' | b'^' | b'_' => State::Ignored,
                    0x20..=0x2f => State::Intermediate,
                    // Index, next line and reverse index move text when they scroll.
                    b'D' | b'E' | b'M' => {
                        seen.content = true;
                        State::Text
                    }
                    _ => State::Text,
                },
                State::Intermediate => match byte {
                    0x30..=0x7e => State::Text,
                    _ => State::Intermediate,
                },
                State::Csi => match byte {
                    // Erase, insert, delete and scroll change the text; other finals move, colour or set modes.
                    b'J' | b'K' | b'L' | b'M' | b'P' | b'@' | b'X' | b'S' | b'T' => {
                        seen.content = true;
                        State::Text
                    }
                    0x40..=0x7e => State::Text,
                    _ => State::Csi,
                },
                State::Osc => match byte {
                    0x07 => {
                        self.end_osc(&mut seen);
                        State::Text
                    }
                    0x1b => State::OscEscape,
                    _ => {
                        if self.osc.len() < MAX_OSC {
                            self.osc.push(byte);
                        }
                        State::Osc
                    }
                },
                State::OscEscape => {
                    if byte == b'\\' {
                        self.end_osc(&mut seen);
                        State::Text
                    } else {
                        State::Osc
                    }
                }
                State::Ignored => match byte {
                    0x07 => State::Text,
                    0x1b => State::IgnoredEscape,
                    _ => State::Ignored,
                },
                State::IgnoredEscape => {
                    if byte == b'\\' {
                        State::Text
                    } else {
                        State::Ignored
                    }
                }
            };
        }
        seen
    }

    fn end_osc(&mut self, seen: &mut Seen) {
        let text = String::from_utf8_lossy(&self.osc);
        if let Some((code, title)) = text.split_once(';')
            && (code == "0" || code == "2")
        {
            seen.titles.push(title.to_string());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn text_counts_as_content_titles_and_cursor_moves_do_not() {
        let mut watch = OutputWatch::default();
        let seen = watch.feed(b"\x1b]0;\xe2\xa0\x8b project\x07\x1b[?25l\x1b[12;3H\x1b[1m\x1b[?25h");
        assert!(!seen.content);
        assert_eq!(seen.titles, vec!["⠋ project".to_string()]);
        assert!(watch.feed(b"\x1b[2;1H\xe2\x9c\xbb Thinking").content);
        assert!(watch.feed(b"\x1b[2K").content, "an erased line");
        assert!(watch.feed(b"\x07").bell);
        assert!(
            !watch.feed(b"\x1b(B\x1bP+q544e\x1b\\").content,
            "a charset and a DCS query"
        );
    }

    #[test]
    fn sequences_split_across_chunks_are_followed() {
        let mut watch = OutputWatch::default();
        assert!(!watch.feed(b"\x1b]2;half a ti").content);
        let seen = watch.feed(b"tle\x1b\\x");
        assert_eq!(seen.titles, vec!["half a title".to_string()]);
        assert!(seen.content, "the x after the title");
        assert!(!watch.feed(b"\x1b[").content);
        assert!(!watch.feed(b"38;5;1m").content);
    }
}

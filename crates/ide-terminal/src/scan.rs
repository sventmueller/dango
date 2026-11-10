//! A second look at the raw output for what the terminal emulator drops: shell integration marks (OSC 133), the
//! working directory (OSC 7) and announcements of local web servers in plain text.

use std::collections::HashSet;
use std::ffi::OsString;
use std::os::unix::ffi::OsStringExt;
use std::path::PathBuf;

use crate::devserver;
use crate::event::{Event, WorkingDirectory};

/// Longer OSC strings are cut; the ones read here are short.
const OSC_LIMIT: usize = 4096;

/// Longer lines are not searched for server addresses.
const LINE_LIMIT: usize = 2048;

/// How many server addresses a session remembers to report each only once.
const SERVER_LIMIT: usize = 256;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum State {
    Ground,
    Escape,
    EscapeIntermediate,
    Csi,
    Osc,
    /// DCS, SOS, PM and APC strings, which end at ST.
    Ignore,
    IgnoreEscape,
}

/// Reads output alongside the emulator and collects the events it finds.
pub(crate) struct Scanner {
    state: State,
    osc: Vec<u8>,
    line: Vec<u8>,
    line_cut: bool,
    marks_seen: bool,
    command_running: bool,
    servers: HashSet<String>,
    events: Vec<Event>,
}

impl Scanner {
    pub(crate) fn new() -> Scanner {
        Scanner {
            state: State::Ground,
            osc: Vec::new(),
            line: Vec::new(),
            line_cut: false,
            marks_seen: false,
            command_running: false,
            servers: HashSet::new(),
            events: Vec::new(),
        }
    }

    /// Reads `bytes`, which may end in the middle of a sequence.
    pub(crate) fn feed(&mut self, bytes: &[u8]) {
        let mut at = 0;
        while at < bytes.len() {
            if self.state == State::Ground {
                let rest = &bytes[at..];
                match memchr::memchr3(0x1b, b'\n', b'\r', rest) {
                    None => {
                        self.push_text(rest);
                        at = bytes.len();
                    }
                    Some(special) => {
                        self.push_text(&rest[..special]);
                        if rest[special] == 0x1b {
                            self.state = State::Escape;
                        } else {
                            self.end_line();
                        }
                        at += special + 1;
                    }
                }
            } else {
                self.step(bytes[at]);
                at += 1;
            }
        }
    }

    /// The events found since the last call.
    pub(crate) fn take_events(&mut self) -> Vec<Event> {
        std::mem::take(&mut self.events)
    }

    fn step(&mut self, byte: u8) {
        // CAN and SUB abort any sequence.
        if matches!(byte, 0x18 | 0x1a) {
            self.state = State::Ground;
            return;
        }
        self.state = match (self.state, byte) {
            (State::Escape, b'[') => State::Csi,
            (State::Escape, b']') => {
                self.osc.clear();
                State::Osc
            }
            (State::Escape, b'P' | b'X' | b'^' | b'_') => State::Ignore,
            (State::Escape | State::EscapeIntermediate, 0x20..=0x2f) => State::EscapeIntermediate,
            (State::Escape | State::EscapeIntermediate | State::Csi, 0x1b) => State::Escape,
            (State::Csi, 0x40..=0x7e) | (State::Escape | State::EscapeIntermediate, _) => State::Ground,
            (State::Csi, _) => State::Csi,
            // Like vte, an ESC ends the OSC string whatever follows; BEL ends it in xterm's older form.
            (State::Osc, 0x07 | 0x1b) => {
                self.dispatch_osc();
                if byte == 0x1b { State::Escape } else { State::Ground }
            }
            (State::Osc, _) => {
                if self.osc.len() < OSC_LIMIT {
                    self.osc.push(byte);
                }
                State::Osc
            }
            (State::Ignore, 0x1b) => State::IgnoreEscape,
            (State::Ignore, _) | (State::IgnoreEscape, 0x1b) => State::Ignore,
            (State::IgnoreEscape, _) | (State::Ground, _) => State::Ground,
        };
    }

    fn push_text(&mut self, text: &[u8]) {
        if self.line_cut || text.is_empty() {
            return;
        }
        if self.line.len() + text.len() > LINE_LIMIT {
            self.line_cut = true;
            self.line.clear();
            return;
        }
        self.line.extend_from_slice(text);
    }

    fn end_line(&mut self) {
        let searched = !self.line.is_empty() && !self.line_cut && (self.command_running || !self.marks_seen);
        if searched {
            for url in devserver::detect(&self.line) {
                let known = self.servers.contains(devserver::identity(&url));
                if !known && self.servers.len() < SERVER_LIMIT {
                    self.servers.insert(devserver::identity(&url).to_string());
                    self.events.push(Event::DevServer(url));
                }
            }
        }
        self.line.clear();
        self.line_cut = false;
    }

    fn dispatch_osc(&mut self) {
        let osc = std::mem::take(&mut self.osc);
        let (code, rest) = match osc.iter().position(|&b| b == b';') {
            Some(split) => (&osc[..split], &osc[split + 1..]),
            None => (&osc[..], &[][..]),
        };
        match code {
            b"7" => {
                if let Some(directory) = parse_osc7(rest) {
                    self.events.push(Event::WorkingDirectory(directory));
                }
            }
            b"133" => self.mark(rest),
            _ => {}
        }
        self.osc = osc;
        self.osc.clear();
    }

    /// FinalTerm's marks: A prompt, B input, C output, D end of the command with its exit code.
    fn mark(&mut self, mark: &[u8]) {
        let Some(&kind) = mark.first() else { return };
        match kind {
            b'A' => {
                self.marks_seen = true;
                self.command_running = false;
                self.events.push(Event::Prompt);
            }
            b'B' => self.marks_seen = true,
            b'C' => {
                self.marks_seen = true;
                self.command_running = true;
                self.events.push(Event::CommandStarted);
            }
            b'D' => {
                self.marks_seen = true;
                self.command_running = false;
                let exit_code = mark
                    .get(1..)
                    .and_then(|rest| rest.strip_prefix(b";"))
                    .and_then(|rest| rest.split(|&b| b == b';').next())
                    .and_then(|code| std::str::from_utf8(code).ok())
                    .and_then(|code| code.parse::<i32>().ok());
                self.events.push(Event::CommandFinished { exit_code });
            }
            _ => {}
        }
    }
}

/// The directory in an OSC 7 URL: `file://host/path` with the path percent-encoded, or kitty's
/// `kitty-shell-cwd://host/path` without encoding.
fn parse_osc7(url: &[u8]) -> Option<WorkingDirectory> {
    let (rest, encoded) = if let Some(rest) = url.strip_prefix(b"file://") {
        (rest, true)
    } else {
        (url.strip_prefix(b"kitty-shell-cwd://")?, false)
    };
    let slash = rest.iter().position(|&b| b == b'/')?;
    let path = &rest[slash..];
    let path = if encoded { percent_decode(path) } else { path.to_vec() };
    Some(WorkingDirectory {
        host: String::from_utf8_lossy(&rest[..slash]).into_owned(),
        path: PathBuf::from(OsString::from_vec(path)),
    })
}

fn percent_decode(text: &[u8]) -> Vec<u8> {
    let mut decoded = Vec::with_capacity(text.len());
    let mut at = 0;
    while at < text.len() {
        let hex = text.get(at + 1..at + 3).and_then(|pair| {
            let pair = std::str::from_utf8(pair).ok()?;
            u8::from_str_radix(pair, 16).ok()
        });
        match (text[at], hex) {
            (b'%', Some(byte)) => {
                decoded.push(byte);
                at += 3;
            }
            (byte, _) => {
                decoded.push(byte);
                at += 1;
            }
        }
    }
    decoded
}

#[cfg(test)]
mod tests {
    use super::*;

    fn events(chunks: &[&[u8]]) -> Vec<Event> {
        let mut scanner = Scanner::new();
        for chunk in chunks {
            scanner.feed(chunk);
        }
        scanner.take_events()
    }

    fn directory(host: &str, path: &str) -> Event {
        Event::WorkingDirectory(WorkingDirectory {
            host: host.into(),
            path: path.into(),
        })
    }

    #[test]
    fn osc_7_reports_the_decoded_directory_with_either_terminator() {
        assert_eq!(
            events(&[b"\x1b]7;file://mac.local/Users/me/My%20Project\x07"]),
            [directory("mac.local", "/Users/me/My Project")]
        );
        assert_eq!(events(&[b"\x1b]7;file:///tmp/%C3%A4\x1b\\"]), [directory("", "/tmp/ä")]);
        assert_eq!(
            events(&[b"\x1b]7;kitty-shell-cwd://h/a%20b\x07"]),
            [directory("h", "/a%20b")]
        );
        assert!(events(&[b"\x1b]7;not a url\x07"]).is_empty());
    }

    #[test]
    fn osc_133_marks_report_prompts_commands_and_exit_codes() {
        let found = events(&[
            b"\x1b]133;A\x07$ \x1b]133;B\x07",
            b"\x1b]133;C\x07out\r\n\x1b]133;D;127\x07",
        ]);
        assert_eq!(
            found,
            [
                Event::Prompt,
                Event::CommandStarted,
                Event::CommandFinished { exit_code: Some(127) }
            ]
        );
        assert_eq!(
            events(&[b"\x1b]133;D\x07"]),
            [Event::CommandFinished { exit_code: None }]
        );
        assert_eq!(
            events(&[b"\x1b]133;D;0;aid=1\x1b\\"]),
            [Event::CommandFinished { exit_code: Some(0) }]
        );
    }

    #[test]
    fn a_sequence_split_across_reads_is_still_recognised() {
        let whole: &[u8] = b"x\x1b]133;D;2\x07y\x1b]7;file:///a\x07";
        for split in 0..whole.len() {
            let (first, second) = whole.split_at(split);
            assert_eq!(
                events(&[first, second]),
                [Event::CommandFinished { exit_code: Some(2) }, directory("", "/a")],
                "split at {split}"
            );
        }
    }

    #[test]
    fn colours_inside_an_address_do_not_hide_it() {
        let vite = b"  \x1b[32m\xe2\x9e\x9c\x1b[39m  \x1b[1mLocal\x1b[22m:   \x1b[36mhttp://localhost:\x1b[1m5173\x1b[22m/\x1b[39m\r\n";
        assert_eq!(events(&[vite]), [Event::DevServer("http://localhost:5173/".into())]);
    }

    #[test]
    fn a_server_is_reported_once_per_session() {
        let line: &[u8] = b"Local: http://localhost:3000/\nLocal: http://localhost:3000\n";
        assert_eq!(
            events(&[line, line]),
            [Event::DevServer("http://localhost:3000/".into())]
        );
    }

    #[test]
    fn with_shell_marks_only_command_output_is_searched_for_servers() {
        let typed = b"\x1b]133;A\x07$ \x1b]133;B\x07curl http://localhost:8080\r\n";
        let output = b"\x1b]133;C\x07listening on port 9000\r\n\x1b]133;D;0\x07";
        let found = events(&[typed, output]);
        assert_eq!(
            found,
            [
                Event::Prompt,
                Event::CommandStarted,
                Event::DevServer("http://localhost:9000".into()),
                Event::CommandFinished { exit_code: Some(0) }
            ]
        );
    }

    #[test]
    fn other_strings_and_escapes_are_skipped_and_an_aborted_osc_reports_nothing() {
        let noise = b"\x1bP1$r0m\x1b\\\x1b(B\x1b]0;title\x07\x1b]133;C\x18\x1b[?2004h";
        assert!(events(&[noise]).is_empty());
    }
}

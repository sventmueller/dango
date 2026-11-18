//! Job logs made readable: terminal escape sequences, GitLab's section markers and GitHub's timestamps go, error and
//! warning lines and section titles are marked, and the part around the failure can be cut out for an agent.

/// What a line of a log is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LineKind {
    /// Ordinary output.
    Plain,
    /// The title of a group or section, such as a step's command.
    Section,
    /// An error the runner reported.
    Error,
    /// A warning the runner reported.
    Warning,
}

/// One line of a readable log.
#[derive(Debug, Clone, PartialEq, Eq)]
#[expect(missing_docs, reason = "the names say it")]
pub struct LogLine {
    /// The text, without escape sequences.
    pub text: String,
    pub kind: LineKind,
}

/// The lines of `raw`, as GitHub or GitLab send it, made readable.
pub fn lines(raw: &str) -> Vec<LogLine> {
    let mut out = Vec::new();
    let mut title_next = false;
    for line in raw.split('\n') {
        let (marker, rest) = section_marker(line);
        let text = without_escapes(last_overwrite(rest));
        let text = without_timestamp(&text).to_string();
        if marker == Some(Marker::Start) {
            title_next = true;
        }
        if marker.is_some() && text.trim().is_empty() {
            continue;
        }
        let Some((text, mut kind)) = command(&text) else {
            continue;
        };
        if title_next && !text.trim().is_empty() {
            kind = LineKind::Section;
            title_next = false;
        }
        out.push(LogLine { text, kind });
    }
    while out.last().is_some_and(|l| l.text.trim().is_empty()) {
        out.pop();
    }
    out
}

/// `lines` as one text.
pub fn text(lines: &[LogLine]) -> String {
    lines.iter().map(|l| l.text.as_str()).collect::<Vec<_>>().join("\n")
}

/// The part of a failed job's log that shows the failure: up to `max` lines ending a little after the last error, or
/// the last `max` lines when nothing is marked as an error.
pub fn excerpt(lines: &[LogLine], max: usize) -> String {
    let failing = lines.iter().rposition(|l| {
        let lower = l.text.to_lowercase();
        l.kind == LineKind::Error || lower.contains("error") || lower.contains("failed") || lower.contains("panicked")
    });
    let end = failing.map_or(lines.len(), |i| (i + 6).min(lines.len()));
    let start = end.saturating_sub(max);
    text(&lines[start..end])
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Marker {
    Start,
    End,
}

/// GitLab's `section_start:<time>:<name>[options]\r` and `section_end:<time>:<name>\r` at the start of a line, which
/// may follow an erase-line sequence.
fn section_marker(line: &str) -> (Option<Marker>, &str) {
    let bare = line.trim_start_matches("\u{1b}[0K");
    for (prefix, marker) in [("section_start:", Marker::Start), ("section_end:", Marker::End)] {
        if let Some(rest) = bare.strip_prefix(prefix) {
            let after = rest.split_once('\r').map_or("", |(_, after)| after);
            return (Some(marker), after);
        }
    }
    (None, line)
}

/// What a terminal shows of a line that carriage returns overwrite: the last part with text.
fn last_overwrite(line: &str) -> &str {
    line.split('\r')
        .rev()
        .find(|part| !without_escapes(part).trim().is_empty())
        .unwrap_or("")
}

/// `text` without ANSI CSI and OSC sequences and other escapes.
fn without_escapes(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        if c != '\u{1b}' {
            if !c.is_control() || c == '\t' {
                out.push(c);
            }
            continue;
        }
        match chars.next() {
            Some('[') => {
                for c in chars.by_ref() {
                    if ('\u{40}'..='\u{7e}').contains(&c) {
                        break;
                    }
                }
            }
            Some(']') => {
                while let Some(c) = chars.next() {
                    if c == '\u{7}' || (c == '\u{1b}' && chars.peek() == Some(&'\\')) {
                        if c == '\u{1b}' {
                            chars.next();
                        }
                        break;
                    }
                }
            }
            _ => {}
        }
    }
    out
}

/// A line without the timestamp GitHub puts before each line, such as `2026-11-09T12:00:00.1234567Z `.
fn without_timestamp(line: &str) -> &str {
    let bytes = line.as_bytes();
    let stamped =
        bytes.len() > 20 && bytes[..4].iter().all(u8::is_ascii_digit) && bytes[4] == b'-' && bytes[10] == b'T';
    if !stamped {
        return line;
    }
    line.split_once("Z ").map_or(line, |(_, rest)| rest)
}

/// GitHub's workflow commands at the start of a line, turned into their meaning; `None` for one that only ends a group.
fn command(text: &str) -> Option<(String, LineKind)> {
    if text.starts_with("##[endgroup]") {
        return None;
    }
    for (prefix, kind, replacement) in [
        ("##[group]", LineKind::Section, ""),
        ("##[error]", LineKind::Error, "Error: "),
        ("##[warning]", LineKind::Warning, "Warning: "),
    ] {
        if let Some(rest) = text.strip_prefix(prefix) {
            return Some((format!("{replacement}{rest}"), kind));
        }
    }
    Some((text.to_string(), LineKind::Plain))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_github_log_loses_its_timestamps_and_marks_groups_and_errors() {
        let raw = "2026-11-09T12:00:00.1234567Z ##[group]Run cargo test\n\
                   2026-11-09T12:00:01.0000000Z \u{1b}[36;1mcargo test\u{1b}[0m\n\
                   2026-11-09T12:00:02.0000000Z ##[endgroup]\n\
                   2026-11-09T12:00:03.0000000Z test parser ... FAILED\n\
                   2026-11-09T12:00:04.0000000Z ##[error]Process completed with exit code 101.\n";
        let lines = lines(raw);
        let kinds: Vec<_> = lines.iter().map(|l| (l.text.as_str(), l.kind)).collect();
        assert_eq!(
            kinds,
            [
                ("Run cargo test", LineKind::Section),
                ("cargo test", LineKind::Plain),
                ("test parser ... FAILED", LineKind::Plain),
                ("Error: Process completed with exit code 101.", LineKind::Error),
            ]
        );
    }

    #[test]
    fn a_gitlab_trace_loses_its_section_markers_and_overwritten_progress() {
        let raw = "\u{1b}[0KRunning with gitlab-runner 17.0.0\n\
                   section_start:1731150000:step_script\r\u{1b}[0K\u{1b}[0K\u{1b}[36;1mExecuting \"step_script\" stage\u{1b}[0;m\n\
                   Downloading 10%\rDownloading 100%\n\
                   \u{1b}]8;;https://example.com\u{7}link\u{1b}]8;;\u{7}\n\
                   section_end:1731150010:step_script\r\u{1b}[0K\n\
                   \u{1b}[31;1mERROR: Job failed: exit code 1\u{1b}[0;m\n";
        let lines = lines(raw);
        let texts: Vec<_> = lines.iter().map(|l| l.text.as_str()).collect();
        assert_eq!(
            texts,
            [
                "Running with gitlab-runner 17.0.0",
                "Executing \"step_script\" stage",
                "Downloading 100%",
                "link",
                "ERROR: Job failed: exit code 1",
            ]
        );
        assert_eq!(lines[1].kind, LineKind::Section);
        assert!(!text(&lines).contains('\u{1b}'));
    }

    #[test]
    fn the_excerpt_ends_shortly_after_the_last_error() {
        let raw: String = (1..=100)
            .map(|n| {
                if n == 70 {
                    "##[error]boom\n".to_string()
                } else {
                    format!("line {n}\n")
                }
            })
            .collect();
        let cut = excerpt(&lines(&raw), 10);
        let shown: Vec<_> = cut.lines().collect();
        assert_eq!(shown.len(), 10);
        assert_eq!(shown[4], "Error: boom");
        assert_eq!(shown.last(), Some(&"line 75"));
        let quiet: String = (1..=5).map(|n| format!("ok {n}\n")).collect();
        assert_eq!(excerpt(&lines(&quiet), 3), "ok 3\nok 4\nok 5");
    }
}

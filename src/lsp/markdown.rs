//! The Markdown of hover texts as styled text: code blocks and code spans in the code font, headings and `**bold**`
//! in bold, links as their text. Hover texts use little else, so this is no full Markdown reader.

use objc2::rc::Retained;
use objc2::runtime::AnyObject;
use objc2::{AllocAnyThread, Message};
use objc2_app_kit::{NSColor, NSFont, NSFontAttributeName, NSForegroundColorAttributeName};
use objc2_foundation::{NSAttributedString, NSDictionary, NSMutableAttributedString, NSString};

use ide_ui_common::constant;
use ide_ui_common::util::ns;

/// How a piece of text looks.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Style {
    Text,
    Bold,
    Code,
}

/// The pieces of `markdown` with their styles; lines keep their breaks, blank runs shrink to one.
fn pieces(markdown: &str) -> Vec<(String, Style)> {
    let mut out: Vec<(String, Style)> = vec![];
    let mut in_code = false;
    let mut blank = true;
    for line in markdown.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("```") {
            in_code = !in_code;
            continue;
        }
        if in_code {
            out.push((format!("{line}\n"), Style::Code));
            blank = false;
            continue;
        }
        if trimmed.is_empty() || trimmed.chars().all(|c| c == '-' || c == '*' || c == '_') && trimmed.len() >= 3 {
            if !blank {
                out.push(("\n".into(), Style::Text));
                blank = true;
            }
            continue;
        }
        blank = false;
        if let Some(heading) = trimmed.strip_prefix('#') {
            out.push((format!("{}\n", heading.trim_start_matches('#').trim()), Style::Bold));
            continue;
        }
        out.extend(inline(line));
        out.push(("\n".into(), Style::Text));
    }
    while out.last().is_some_and(|(text, _)| text.trim().is_empty()) {
        out.pop();
    }
    if let Some((last, _)) = out.last_mut() {
        while last.ends_with('\n') {
            last.pop();
        }
    }
    out
}

/// The code spans, bold runs, links and escapes of one line.
fn inline(line: &str) -> Vec<(String, Style)> {
    let mut out = vec![];
    let mut text = String::new();
    let mut chars = line.chars().peekable();
    let mut bold = false;
    let flush = |text: &mut String, out: &mut Vec<(String, Style)>, bold: bool| {
        if !text.is_empty() {
            out.push((std::mem::take(text), if bold { Style::Bold } else { Style::Text }));
        }
    };
    while let Some(c) = chars.next() {
        match c {
            '\\' if chars.peek().is_some_and(char::is_ascii_punctuation) => {
                text.extend(chars.next());
            }
            '`' => {
                flush(&mut text, &mut out, bold);
                let code: String = chars.by_ref().take_while(|c| *c != '`').collect();
                out.push((code, Style::Code));
            }
            '*' if chars.peek() == Some(&'*') => {
                chars.next();
                flush(&mut text, &mut out, bold);
                bold = !bold;
            }
            '[' => {
                let rest: String = chars.clone().collect();
                match rest
                    .find("](")
                    .and_then(|close| rest[close..].find(')').map(|end| (close, close + end)))
                {
                    Some((close, end)) => {
                        text.push_str(&rest[..close]);
                        for _ in 0..rest[..=end].chars().count() {
                            chars.next();
                        }
                    }
                    None => text.push('['),
                }
            }
            c => text.push(c),
        }
    }
    flush(&mut text, &mut out, bold);
    out
}

fn attributes(font: &NSFont, color: &NSColor) -> Retained<NSDictionary<NSString, AnyObject>> {
    NSDictionary::<NSString, AnyObject>::from_retained_objects(
        &[
            constant!(NSFontAttributeName),
            constant!(NSForegroundColorAttributeName),
        ],
        &[
            Retained::<AnyObject>::from(font.retain()),
            Retained::<AnyObject>::from(color.retain()),
        ],
    )
}

/// `markdown` as styled text at `size` points.
pub fn attributed(markdown: &str, size: f64) -> Retained<NSAttributedString> {
    let result = NSMutableAttributedString::new();
    let text = attributes(&NSFont::systemFontOfSize(size), &NSColor::labelColor());
    let bold = attributes(&NSFont::boldSystemFontOfSize(size), &NSColor::labelColor());
    let code = attributes(&crate::lsp::host::code_font(size - 0.5), &NSColor::labelColor());
    for (piece, style) in pieces(markdown) {
        let attributes = match style {
            Style::Text => &text,
            Style::Bold => &bold,
            Style::Code => &code,
        };
        // SAFETY: The dictionary holds a font and a colour under their keys.
        let part = unsafe {
            NSAttributedString::initWithString_attributes(NSAttributedString::alloc(), &ns(&piece), Some(attributes))
        };
        result.appendAttributedString(&part);
    }
    result.into_super()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn plain(markdown: &str) -> Vec<(String, Style)> {
        pieces(markdown)
    }

    #[test]
    fn code_blocks_spans_bold_and_links_get_their_styles() {
        let parts = plain("```rust\nfn double(n: u32) -> u32\n```\n\n---\n\nDoubles **n**, see [docs](https://x).");
        assert_eq!(parts[0], ("fn double(n: u32) -> u32\n".into(), Style::Code));
        assert!(parts.contains(&("n".into(), Style::Bold)));
        let text: String = parts.iter().map(|(t, _)| t.as_str()).collect();
        assert_eq!(text, "fn double(n: u32) -> u32\n\nDoubles n, see docs.");
    }

    #[test]
    fn escapes_and_snake_case_stay_readable() {
        let text: String = plain(r"a\_b snake_case `x_y`").iter().map(|(t, _)| t.clone()).collect();
        assert_eq!(text, "a_b snake_case x_y");
    }
}

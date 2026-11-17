//! Reading a web page as text. Its content is marked as untrusted data for the model (E22): a page can carry
//! instructions meant to steer the agent.

use serde_json::Value;

use ide_thread::ToolOutput;

use super::{Context, Outcome, required};
use crate::notes::ToolNote;
use crate::policy::Refusal;

/// Characters of a page the model gets at most.
const MAX_TEXT: usize = 100_000;
/// Bytes of a page that are read at most.
const MAX_PAGE: u64 = 5 * 1024 * 1024;

/// The host of an http or https URL.
pub(super) fn host(url: &str) -> Result<String, ToolNote> {
    let rest = url
        .strip_prefix("https://")
        .or_else(|| url.strip_prefix("http://"))
        .ok_or_else(|| ToolNote::NotWeb(url.to_string()))?;
    let authority = rest.split(['/', '?', '#']).next().unwrap_or_default();
    let host = authority.rsplit('@').next().unwrap_or_default();
    let host = if host.starts_with('[') {
        host.split_inclusive(']').next().unwrap_or_default()
    } else {
        host.split(':').next().unwrap_or_default()
    };
    if host.is_empty() {
        return Err(ToolNote::NoHost(url.to_string()));
    }
    Ok(host.to_ascii_lowercase())
}

pub(super) fn fetch(input: &Value, context: &Context<'_>) -> Result<Outcome, ToolNote> {
    let url = required(input, "url")?;
    host(url)?;
    if context.sandbox {
        return Err(ToolNote::Refused(Refusal::NoNetwork));
    }
    // The workspace's client waits five minutes for a silent server, so the fetch gets connections of its own that
    // cancelling the turn closes.
    let http = context.workspace.http().separate();
    let closing = http.clone();
    context.cancel.on_cancel(move || closing.close());
    let fetched = http.get_text(url, MAX_PAGE);
    http.close();
    // A closed connection can also read as a page that simply ended, so what came is not trusted after a cancel.
    if context.cancel.is_cancelled() {
        return Err(ToolNote::Withdrawn);
    }
    let (kind, body) = fetched.map_err(ToolNote::Fetch)?;
    let text = if kind.contains("html") {
        html_text(&body)
    } else if kind.starts_with("text/") || kind.contains("json") || kind.contains("xml") || kind.is_empty() {
        body
    } else {
        return Err(ToolNote::NotText {
            url: url.to_string(),
            kind,
        });
    };
    let total = text.chars().count();
    let mut text: String = text.chars().take(MAX_TEXT).collect();
    if total > MAX_TEXT {
        text.push_str("\n[… cut]");
    }
    Ok(Outcome {
        content: ide_thread::external_text(&format!("the content of {url}"), &text),
        output: Some(ToolOutput::Text(text)),
        ..Outcome::default()
    })
}

/// The readable text of an HTML page: no scripts, styles or tags, entities decoded, blocks on lines of their own.
pub(crate) fn html_text(html: &str) -> String {
    let mut out = String::with_capacity(html.len() / 2);
    let lower = html.to_ascii_lowercase();
    let mut at = 0;
    while at < html.len() {
        let Some(open) = html[at..].find('<').map(|i| at + i) else {
            out.push_str(&decode(&html[at..]));
            break;
        };
        out.push_str(&decode(&html[at..open]));
        let tag_end = html[open..].find('>').map_or(html.len(), |i| open + i + 1);
        let tag = &lower[open..tag_end];
        let name: String = tag
            .trim_start_matches(['<', '/'])
            .chars()
            .take_while(char::is_ascii_alphanumeric)
            .collect();
        at = tag_end;
        if tag.starts_with("<!--") {
            at = lower[open..].find("-->").map_or(html.len(), |i| open + i + 3);
            continue;
        }
        if matches!(name.as_str(), "script" | "style" | "noscript" | "template") && !tag.starts_with("</") {
            let close = format!("</{name}");
            at = lower[at..].find(&close).map_or(html.len(), |i| at + i);
            continue;
        }
        if matches!(
            name.as_str(),
            "br" | "p"
                | "div"
                | "li"
                | "tr"
                | "h1"
                | "h2"
                | "h3"
                | "h4"
                | "h5"
                | "h6"
                | "pre"
                | "section"
                | "article"
                | "header"
                | "footer"
                | "table"
                | "ul"
                | "ol"
                | "blockquote"
        ) {
            out.push('\n');
        }
    }
    let mut lines: Vec<String> = Vec::new();
    for line in out.lines() {
        let line = line.split_whitespace().collect::<Vec<_>>().join(" ");
        if !line.is_empty() {
            lines.push(line);
        }
    }
    lines.join("\n").trim().to_string()
}

fn decode(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(amp) = rest.find('&') {
        out.push_str(&rest[..amp]);
        rest = &rest[amp..];
        // Bytes, not characters: the twelfth byte can fall inside a character of the page's text.
        let Some(semi) = rest.as_bytes()[..rest.len().min(12)].iter().position(|b| *b == b';') else {
            out.push('&');
            rest = &rest[1..];
            continue;
        };
        let entity = &rest[1..semi];
        let decoded = match entity {
            "amp" => Some('&'),
            "lt" => Some('<'),
            "gt" => Some('>'),
            "quot" => Some('"'),
            "apos" | "#39" => Some('\''),
            "nbsp" => Some(' '),
            e if e.starts_with("#x") || e.starts_with("#X") => {
                u32::from_str_radix(&e[2..], 16).ok().and_then(char::from_u32)
            }
            e if e.starts_with('#') => e[1..].parse::<u32>().ok().and_then(char::from_u32),
            _ => None,
        };
        match decoded {
            Some(c) => {
                out.push(c);
                rest = &rest[semi + 1..];
            }
            None => {
                out.push('&');
                rest = &rest[1..];
            }
        }
    }
    out.push_str(rest);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hosts_come_from_http_addresses_only() {
        assert_eq!(host("https://Docs.rs/serde?x=1").unwrap(), "docs.rs");
        assert_eq!(host("http://user@127.0.0.1:8080/a").unwrap(), "127.0.0.1");
        assert!(host("file:///etc/passwd").is_err());
        assert!(host("https:///x").is_err());
    }

    #[test]
    fn pages_lose_scripts_styles_and_tags_but_keep_their_blocks() {
        let html = "<html><head><style>p{}</style><script>alert('x<y')</script></head><body>\
                    <h1>Title</h1><p>One &amp; two&nbsp;&lt;three&gt; &#x41;&#66;</p><!-- hidden --><ul><li>a</li>\
                    <li>b</li></ul></body></html>";
        assert_eq!(html_text(html), "Title\nOne & two <three> AB\na\nb");
    }

    #[test]
    fn an_ampersand_before_text_of_several_bytes_a_character_stays_as_it_is() {
        assert_eq!(html_text("<p>R&日本語の文書</p>"), "R&日本語の文書");
        assert_eq!(html_text("Müller & Söhne &amp; Töchter"), "Müller & Söhne & Töchter");
        assert_eq!(html_text("&日本語;"), "&日本語;");
    }
}

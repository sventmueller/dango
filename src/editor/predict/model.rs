//! What edit prediction decides without AppKit (E92): the settings, which files and providers may take part, and what
//! a provider's proposal does to the text it was asked about, bound to that text's revision, selection and range so
//! that a proposal is dropped as soon as any of them changed (Review C6).

use std::ops::Range;
use std::path::Path;

use ide_core::settings::Key;
use ide_document::Revision;
use ide_predict::Prediction;
use serde::{Deserialize, Serialize};

/// Which provider proposes edits.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Choice {
    #[default]
    Off,
    Copilot,
    Ollama,
    LmStudio,
}

impl Choice {
    /// Every choice, in the order the settings list them.
    pub const ALL: [Choice; 4] = [Choice::Off, Choice::Copilot, Choice::Ollama, Choice::LmStudio];
}

/// A model of a local server: where the server listens and which model fills in the middle.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Local {
    pub url: String,
    pub model: String,
}

/// The provider the user chose.
pub const PROVIDER: Key<Choice> = Key::user("editPrediction", ide_core::settings::Sharing::Shared, Choice::default);
/// The languages, by language id, that get no proposals.
pub const OFF_FOR: Key<Vec<String>> = Key::user("editPredictionOffFor", ide_core::settings::Sharing::Shared, Vec::new);
/// Patterns of file names that never go to a provider.
pub const EXCLUDE: Key<Vec<String>> = Key::user(
    "editPredictionExclude",
    ide_core::settings::Sharing::Shared,
    default_exclusions,
);
/// Ollama's address and model.
pub const OLLAMA: Key<Local> = Key::user("editPredictionOllama", ide_core::settings::Sharing::PerMac, || Local {
    url: ide_predict::fim::OLLAMA_BASE.to_string(),
    model: String::new(),
});
/// LM Studio's address and model.
pub const LM_STUDIO: Key<Local> = Key::user("editPredictionLmStudio", ide_core::settings::Sharing::PerMac, || {
    Local {
        url: ide_predict::fim::LMSTUDIO_BASE.to_string(),
        model: String::new(),
    }
});
/// The providers outside the Mac the user allowed to get the text, by [`consent_id`].
pub const CONSENT: Key<Vec<String>> = Key::user("editPredictionConsent", ide_core::settings::Sharing::PerMac, Vec::new);

/// The patterns no file matching goes to a provider by, unless the user changes them: secrets and keys.
pub fn default_exclusions() -> Vec<String> {
    [".env*", "*.pem", "*.key", "id_*"].map(String::from).to_vec()
}

/// Whether `path` matches one of `patterns`: a pattern without `/` is matched against the file's name, one with `/`
/// against as many of the path's last components; `*` stands for any run of characters, `?` for one.
pub fn excluded(path: &Path, patterns: &[String]) -> bool {
    let components: Vec<String> = path
        .components()
        .map(|c| c.as_os_str().to_string_lossy().into_owned())
        .collect();
    patterns
        .iter()
        .map(|p| p.trim())
        .filter(|p| !p.is_empty())
        .any(|pattern| {
            let parts = pattern.trim_start_matches('/').split('/').count();
            let tail = components.len().saturating_sub(parts);
            components.len() >= parts && glob(pattern.trim_start_matches('/'), &components[tail..].join("/"))
        })
}

/// Whether `text` matches the wildcard `pattern`, where `*` does not cross a `/`.
fn glob(pattern: &str, text: &str) -> bool {
    let (pattern, text): (Vec<char>, Vec<char>) = (pattern.chars().collect(), text.chars().collect());
    let (mut p, mut t) = (0, 0);
    let mut star: Option<(usize, usize)> = None;
    while t < text.len() {
        match pattern.get(p) {
            Some('*') => {
                star = Some((p, t));
                p += 1;
            }
            Some(&c) if c == text[t] || (c == '?' && text[t] != '/') => {
                p += 1;
                t += 1;
            }
            _ => match star {
                Some((sp, st)) if text[st] != '/' => {
                    p = sp + 1;
                    t = st + 1;
                    star = Some((sp, st + 1));
                }
                _ => return false,
            },
        }
    }
    pattern[p..].iter().all(|c| *c == '*')
}

/// Whether `url` names this Mac: localhost or a loopback address.
pub fn loopback(url: &str) -> bool {
    let rest = url.split_once("://").map_or(url, |(_, rest)| rest);
    let authority = rest.split('/').next().unwrap_or("");
    let authority = authority.rsplit_once('@').map_or(authority, |(_, host)| host);
    let host = if let Some(bracketed) = authority.strip_prefix('[') {
        bracketed.split(']').next().unwrap_or("")
    } else {
        authority.split(':').next().unwrap_or("")
    };
    host.eq_ignore_ascii_case("localhost")
        || host == "::1"
        || host.parse::<std::net::Ipv4Addr>().is_ok_and(|ip| ip.is_loopback())
}

/// The name a provider's consent is kept under; `None` for a model on this Mac, which needs none (E101).
pub fn consent_id(choice: Choice, local: Option<&Local>) -> Option<String> {
    match (choice, local) {
        (Choice::Off, _) => None,
        (Choice::Copilot, _) => Some("copilot".to_string()),
        (_, Some(local)) if loopback(&local.url) => None,
        (Choice::Ollama, local) => Some(format!("ollama {}", local.map_or("", |l| l.url.as_str()))),
        (Choice::LmStudio, local) => Some(format!("lmstudio {}", local.map_or("", |l| l.url.as_str()))),
    }
}

/// How a proposal shows.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Kind {
    /// Text to put in at the caret: `first` goes on the caret's line, `rest` holds the lines below, if any.
    Insert { first: String, rest: Option<String> },
    /// Text near the caret to replace or delete, shown as before and after.
    Change,
    /// A change on another line, counted from 0, which the first Tab goes to.
    Elsewhere { line: usize },
}

/// A provider's proposal, bound to the text it was made for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Proposal {
    /// The revision of the document the text belongs to (E24).
    pub revision: Revision,
    /// The selection it was made for, in bytes.
    pub selection: Range<usize>,
    /// The bytes it replaces, without what the provider repeated of the text around them.
    pub range: Range<usize>,
    /// What `range` holds.
    pub old: String,
    /// What it puts there.
    pub text: String,
    pub kind: Kind,
    /// As the provider sent it, for its feedback once taken.
    pub prediction: Prediction,
}

/// The line of byte `at` in `text`, counted from 0.
fn line_of(text: &str, at: usize) -> usize {
    text.as_bytes()[..at.min(text.len())]
        .iter()
        .filter(|b| **b == b'\n')
        .count()
}

/// The range of `prediction` and its text in `text`, less the start and end they share with what the range holds;
/// `None` when it is out of bounds, not on character boundaries, or changes nothing.
pub fn normalize(text: &str, prediction: &Prediction) -> Option<(Range<usize>, String)> {
    let range = prediction.range.clone();
    let old = text.get(range.clone())?;
    let new = prediction.text.as_str();
    let prefix: usize = old
        .chars()
        .zip(new.chars())
        .take_while(|(a, b)| a == b)
        .map(|(a, _)| a.len_utf8())
        .sum();
    let (old_rest, new_rest) = (&old[prefix..], &new[prefix..]);
    let suffix: usize = old_rest
        .chars()
        .rev()
        .zip(new_rest.chars().rev())
        .take_while(|(a, b)| a == b)
        .map(|(a, _)| a.len_utf8())
        .sum();
    let range = range.start + prefix..range.end - suffix;
    let new = &new_rest[..new_rest.len() - suffix];
    (!range.is_empty() || !new.is_empty()).then(|| (range, new.to_string()))
}

/// How a change of `range` to `new` in `text` shows with the caret or selection at `selection`.
pub fn kind(text: &str, selection: &Range<usize>, range: &Range<usize>, new: &str) -> Kind {
    if range.is_empty() && selection.is_empty() && range.start == selection.start {
        return match new.split_once('\n') {
            Some((first, rest)) => Kind::Insert {
                first: first.to_string(),
                rest: Some(rest.to_string()),
            },
            None => Kind::Insert {
                first: new.to_string(),
                rest: None,
            },
        };
    }
    let line = line_of(text, range.start);
    let caret = line_of(text, selection.start);
    let last = line_of(text, range.end);
    if (line..=last).contains(&caret) {
        Kind::Change
    } else {
        Kind::Elsewhere { line }
    }
}

impl Proposal {
    /// The first of `predictions` that changes something in `text`, as asked at `revision` with `selection`.
    pub fn first(
        text: &str,
        revision: Revision,
        selection: Range<usize>,
        predictions: Vec<Prediction>,
    ) -> Option<Proposal> {
        predictions.into_iter().find_map(|prediction| {
            let (range, new) = normalize(text, &prediction)?;
            Some(Proposal {
                revision,
                kind: kind(text, &selection, &range, &new),
                selection: selection.clone(),
                old: text[range.clone()].to_string(),
                range,
                text: new,
                prediction,
            })
        })
    }

    /// Whether it still fits the document: the same revision and selection, and its range holding what it held.
    pub fn holds(&self, revision: Revision, selection: &Range<usize>, text: &str) -> bool {
        self.revision == revision && self.selection == *selection && text.get(self.range.clone()) == Some(&self.old)
    }

    /// The same proposal after the caret went to it on purpose, as the first Tab does: bound to `selection` now.
    pub fn moved(&self, text: &str, selection: Range<usize>) -> Proposal {
        Proposal {
            kind: kind(text, &selection, &self.range, &self.text),
            selection,
            ..self.clone()
        }
    }

    /// The first word of an insertion, which ⌥→ takes; `None` for anything but a pure insertion at the caret.
    pub fn word(&self) -> Option<&str> {
        matches!(self.kind, Kind::Insert { .. }).then(|| first_word(&self.text))
    }

    /// What is left of an insertion after its first `taken` bytes went into the text, which made `text` of revision
    /// `revision`; `None` when nothing is left.
    pub fn rest(&self, taken: usize, text: &str, revision: Revision) -> Option<Proposal> {
        let left = self.text.get(taken..).filter(|rest| !rest.is_empty())?;
        let at = self.range.start + taken;
        let selection = at..at;
        Some(Proposal {
            revision,
            kind: kind(text, &selection, &(at..at), left),
            selection,
            range: at..at,
            old: String::new(),
            text: left.to_string(),
            prediction: self.prediction.clone(),
        })
    }
}

/// The first word of `text` with the blanks before it; a single other character where no word follows them.
pub fn first_word(text: &str) -> &str {
    let blanks = text.len() - text.trim_start_matches([' ', '\t']).len();
    let rest = &text[blanks..];
    let word = rest
        .char_indices()
        .find(|(_, c)| !(c.is_alphanumeric() || *c == '_'))
        .map_or(rest.len(), |(i, _)| i);
    let end = if word == 0 {
        rest.chars().next().map_or(0, char::len_utf8)
    } else {
        word
    };
    &text[..blanks + end]
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;

    fn prediction(range: Range<usize>, text: &str) -> Prediction {
        Prediction {
            range,
            text: text.into(),
            feedback: None,
        }
    }

    #[test]
    fn secrets_and_keys_are_excluded_by_default_and_patterns_match_names_or_path_tails() {
        let defaults = default_exclusions();
        for name in [
            ".env",
            ".env.local",
            "server.pem",
            "tls.key",
            "id_ed25519",
            "id_rsa.pub",
        ] {
            assert!(excluded(&PathBuf::from("/p").join(name), &defaults), "{name}");
        }
        for name in ["main.rs", "environment.rs", "keys.rs", "my_id_rsa"] {
            assert!(!excluded(&PathBuf::from("/p").join(name), &defaults), "{name}");
        }
        let own = vec!["secrets/*".to_string(), "*.sql?".to_string()];
        assert!(excluded(Path::new("/p/secrets/a.txt"), &own));
        assert!(
            !excluded(Path::new("/p/secrets/deep/a.txt"), &own),
            "* does not cross a folder"
        );
        assert!(excluded(Path::new("/p/dump.sqlx"), &own));
        assert!(!excluded(Path::new("/p/dump.sql"), &own));
        assert!(
            !excluded(Path::new("/p/a.rs"), &[" ".to_string()]),
            "a blank pattern matches nothing"
        );
    }

    #[test]
    fn only_copilot_and_servers_off_this_mac_need_consent() {
        let local = |url: &str| Local {
            url: url.into(),
            model: "m".into(),
        };
        assert_eq!(consent_id(Choice::Copilot, None).as_deref(), Some("copilot"));
        assert_eq!(consent_id(Choice::Ollama, Some(&local("http://localhost:11434"))), None);
        assert_eq!(
            consent_id(Choice::LmStudio, Some(&local("http://127.0.0.1:1234/v1"))),
            None
        );
        assert_eq!(consent_id(Choice::Ollama, Some(&local("http://[::1]:11434"))), None);
        assert_eq!(
            consent_id(Choice::Ollama, Some(&local("http://studio.local:11434"))).as_deref(),
            Some("ollama http://studio.local:11434")
        );
        assert_eq!(consent_id(Choice::Off, None), None);
    }

    #[test]
    fn a_proposal_loses_what_it_repeats_of_the_text_around_it() {
        let text = "let total = sc;\n";
        let (range, new) = normalize(text, &prediction(4..15, "total = scale(21, 2);")).unwrap();
        assert_eq!((range, new.as_str()), (14..14, "ale(21, 2)"));
        assert_eq!(normalize(text, &prediction(0..3, "let")), None, "changes nothing");
        assert_eq!(normalize(text, &prediction(10..99, "x")), None, "out of bounds");
        assert_eq!(normalize("ä", &prediction(1..2, "x")), None, "inside a character");
    }

    #[test]
    fn an_insertion_at_the_caret_splits_into_its_line_and_the_lines_below() {
        let text = "fn main() {\n    \n}\n";
        let one = Proposal::first(text, Revision::new(3), 16..16, vec![prediction(16..16, "run();")]).unwrap();
        assert_eq!(
            one.kind,
            Kind::Insert {
                first: "run();".into(),
                rest: None
            }
        );
        let more = Proposal::first(
            text,
            Revision::new(3),
            16..16,
            vec![prediction(16..16, "let a = 1;\n    run(a);")],
        )
        .unwrap();
        assert_eq!(
            more.kind,
            Kind::Insert {
                first: "let a = 1;".into(),
                rest: Some("    run(a);".into())
            }
        );
    }

    #[test]
    fn replacing_and_deleting_near_the_caret_is_a_change_and_further_away_a_jump() {
        let text = "let a = 1;\nlet b = 2;\nlet c = a + b;\n";
        let replace = Proposal::first(text, Revision::new(1), 4..4, vec![prediction(4..5, "x")]).unwrap();
        assert_eq!(replace.kind, Kind::Change);
        assert_eq!(replace.old, "a");
        let delete = Proposal::first(text, Revision::new(1), 0..0, vec![prediction(8..10, "")]).unwrap();
        assert_eq!((delete.kind, delete.text.as_str()), (Kind::Change, ""));
        let away = Proposal::first(text, Revision::new(1), 0..0, vec![prediction(30..35, "a * b")]).unwrap();
        assert_eq!(away.kind, Kind::Elsewhere { line: 2 });
        let there = away.moved(text, 30..30);
        assert_eq!(there.kind, Kind::Change, "after the jump the caret is on its line");
        assert_eq!(there.selection, 30..30);
    }

    #[test]
    fn a_proposal_holds_only_for_its_revision_selection_and_text() {
        let text = "let a = 1;\n";
        let proposal = Proposal::first(text, Revision::new(7), 10..10, vec![prediction(10..10, " // one")]).unwrap();
        assert!(proposal.holds(Revision::new(7), &(10..10), text));
        assert!(
            !proposal.holds(Revision::new(8), &(10..10), text),
            "an answer that arrives after a further edit is dropped"
        );
        assert!(!proposal.holds(Revision::new(7), &(9..9), text), "the caret moved");
        let replace = Proposal::first(text, Revision::new(7), 4..4, vec![prediction(4..5, "b")]).unwrap();
        assert!(
            !replace.holds(Revision::new(7), &(4..4), "let x = 1;\n"),
            "its range no longer holds what it replaces"
        );
    }

    #[test]
    fn option_right_takes_a_word_at_a_time_of_an_insertion_only() {
        assert_eq!(first_word("  total = 1"), "  total");
        assert_eq!(first_word("(21, 2)"), "(");
        assert_eq!(first_word("\n    x"), "\n");
        assert_eq!(first_word("größe"), "größe");
        let text = "let t = sc\n";
        let proposal = Proposal::first(text, Revision::new(1), 10..10, vec![prediction(10..10, "ale(21, 2)")]).unwrap();
        assert_eq!(proposal.word(), Some("ale"));
        let after = "let t = scale\n";
        let rest = proposal.rest(3, after, Revision::new(2)).unwrap();
        assert_eq!(
            (rest.range.clone(), rest.text.as_str(), rest.revision),
            (13..13, "(21, 2)", Revision::new(2))
        );
        assert!(rest.holds(Revision::new(2), &(13..13), after));
        assert_eq!(proposal.rest(10, "", Revision::new(2)), None, "nothing left");
        let replace = Proposal::first("let a;\n", Revision::new(1), 4..4, vec![prediction(4..5, "b")]).unwrap();
        assert_eq!(replace.word(), None);
    }
}

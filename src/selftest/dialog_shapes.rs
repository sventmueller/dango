//! One reference per overlay function for the controls it shows, in order and without values (E146: the same
//! function has the same interface for every agent). The scenarios of Claude (`agent-dialogs`), an ACP agent
//! (`acp-dialogs`) and Codex (`codex`) each hold their overlays against it; only enabled or not and the values may
//! differ between agents.

use super::agent::tab_of;
use super::check;
use ide_ui_agent::AgentKey;

/// An overlay function.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Function {
    Status,
    Mcp,
    Config,
    Memory,
    Account,
}

/// Whether `controls` are the ones `function` shows.
fn fits(function: Function, controls: &[String]) -> bool {
    let tokens: Vec<&str> = controls.iter().map(String::as_str).collect();
    match function {
        Function::Status => tokens.is_empty(),
        Function::Account => tokens == ["Abmelden"],
        Function::Mcp => mcp(&tokens),
        Function::Config => {
            let pairs = tokens
                .chunks(2)
                .take_while(|pair| *pair == ["popup", "Übernehmen"])
                .count();
            tokens[pairs * 2..].iter().all(|t| *t == "Datei öffnen")
        }
        Function::Memory => {
            let files = tokens
                .iter()
                .take_while(|t| matches!(**t, "Öffnen" | "Anlegen"))
                .count();
            files > 0
                && tokens[files..]
                    .iter()
                    .all(|t| !matches!(*t, "popup" | "field" | "Öffnen" | "Anlegen"))
        }
    }
}

/// Each server: its switch, Reconnect, Sign in or out where it has a sign-in, Remove; then Restart and Add.
fn mcp(tokens: &[&str]) -> bool {
    let mut rest = tokens;
    while let ["switch", "Neu verbinden", more @ ..] = rest {
        rest = match more {
            ["Anmelden" | "Abmelden", "Entfernen", after @ ..] | ["Entfernen", after @ ..] => after,
            _ => return false,
        };
    }
    rest == ["Sitzung neu starten", "Hinzufügen …"]
}

/// Checks the open overlay of `key` against the reference of `function`.
pub(crate) fn check_shape(key: AgentKey, function: Function) {
    let controls = tab_of(key).and_then(|t| t.dialog_controls()).unwrap_or_default();
    let modal = tab_of(key).is_some_and(|t| t.dialog_is_modal());
    check(
        match function {
            Function::Status => "/status shows the controls every agent's /status shows, in a modal group",
            Function::Mcp => "/mcp shows the controls every agent's /mcp shows, in a modal group",
            Function::Config => "/config shows the controls every agent's /config shows, in a modal group",
            Function::Memory => "/memory shows the controls every agent's /memory shows, in a modal group",
            Function::Account => "/logout shows the controls every agent's /logout shows, in a modal group",
        },
        modal && fits(function, &controls),
        format!("{controls:?}"),
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tokens(list: &[&str]) -> Vec<String> {
        list.iter().map(|t| (*t).to_string()).collect()
    }

    #[test]
    fn the_references_take_what_every_agent_shows_and_nothing_else() {
        let mcp = tokens(&[
            "switch",
            "Neu verbinden",
            "Entfernen",
            "switch",
            "Neu verbinden",
            "Anmelden",
            "Entfernen",
            "Hinzufügen …",
        ]);
        assert!(fits(Function::Mcp, &mcp));
        assert!(!fits(Function::Mcp, &tokens(&["Neu verbinden", "Hinzufügen …"])));
        assert!(fits(
            Function::Config,
            &tokens(&["popup", "Übernehmen", "Datei öffnen"])
        ));
        assert!(!fits(Function::Config, &tokens(&["field"])));
        assert!(fits(
            Function::Memory,
            &tokens(&["Öffnen", "Anlegen", "switch", "Open folder"])
        ));
        assert!(!fits(Function::Memory, &tokens(&["switch"])));
        assert!(fits(Function::Status, &[]));
    }
}

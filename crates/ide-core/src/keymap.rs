//! The shortcut of every command in one table, which the user can override in `keybindings.json`.
//!
//! A shortcut reads like `alt+cmd+p`: modifiers `cmd`, `shift`, `alt`, `ctrl` joined by `+`, then the key, which is a
//! character or one of the names `escape`, `space`, `return`, `tab`, `left`, `right`, `up`, `down` and `f1` to `f19`.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use crate::paths;

/// A command and the shortcut it has unless the user changes it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Binding {
    /// The command's identifier, as menus and `keybindings.json` name it.
    pub command: &'static str,
    /// Its shortcut by default; empty for none.
    pub key: &'static str,
}

const fn bind(command: &'static str, key: &'static str) -> Binding {
    Binding { command, key }
}

/// Every command with a shortcut.
pub const DEFAULTS: &[Binding] = &[
    bind("app.settings", "cmd+,"),
    bind("window.new", "shift+cmd+n"),
    bind("window.close", "shift+cmd+w"),
    bind("project.open", "cmd+o"),
    bind("project.quickOpen", "cmd+p"),
    bind("project.search", "shift+cmd+f"),
    bind("file.new", "cmd+n"),
    bind("file.save", "cmd+s"),
    bind("file.saveAs", "shift+cmd+s"),
    bind("edit.toggleComment", "cmd+/"),
    bind("edit.goToLine", "ctrl+g"),
    bind("view.wrap", "alt+z"),
    bind("view.whitespace", ""),
    bind("view.inspector", "alt+cmd+i"),
    bind("view.preview", "shift+cmd+v"),
    bind("edit.pastePlain", "alt+shift+cmd+v"),
    bind("format.bold", "cmd+b"),
    bind("format.italic", "cmd+i"),
    bind("format.strikethrough", "shift+cmd+x"),
    bind("format.code", "alt+shift+cmd+c"),
    bind("format.link", "shift+cmd+k"),
    bind("format.heading1", "alt+cmd+1"),
    bind("format.heading2", "alt+cmd+2"),
    bind("format.heading3", "alt+cmd+3"),
    bind("tab.new", "cmd+t"),
    bind("tab.close", "cmd+w"),
    bind("tab.next", "shift+cmd+]"),
    bind("tab.previous", "shift+cmd+["),
    bind("sidebar.toggle", "ctrl+cmd+s"),
    bind("panel.files", "cmd+1"),
    bind("panel.search", "cmd+2"),
    bind("panel.agents", "cmd+3"),
    bind("panel.git", "cmd+4"),
    bind("panel.tests", "cmd+5"),
    bind("panel.debug", "cmd+6"),
    bind("panel.ci", "cmd+7"),
    bind("panel.containers", "cmd+8"),
    bind("panel.leitstand", "cmd+9"),
    bind("order.new", ""),
    bind("order.open", ""),
    bind("order.board", ""),
    bind("order.settings", ""),
    bind("test.runAtCursor", "ctrl+alt+cmd+u"),
    bind("test.rerun", "ctrl+alt+cmd+g"),
    bind("window.nextPanel", "alt+cmd+`"),
    bind("pane.splitRight", "ctrl+cmd+t"),
    bind("pane.splitBelow", "ctrl+alt+cmd+t"),
    bind("pane.close", "ctrl+shift+cmd+w"),
    bind("pane.zoom", "ctrl+shift+cmd+return"),
    bind("pane.nextTab", "ctrl+alt+cmd+]"),
    bind("pane.previousTab", "ctrl+alt+cmd+["),
    bind("terminal.new", "ctrl+`"),
    bind("terminal.clear", "cmd+k"),
    bind("agent.focus", "cmd+escape"),
    bind("plan.comment", "alt+cmd+k"),
    bind("browser.focusAddress", "cmd+l"),
    bind("browser.reload", "cmd+r"),
    bind("browser.back", "cmd+["),
    bind("browser.forward", "cmd+]"),
    bind("code.complete", "alt+escape"),
    bind("code.hover", "ctrl+cmd+i"),
    bind("code.definition", "ctrl+cmd+j"),
    bind("code.references", "shift+ctrl+cmd+f"),
    bind("code.symbols", "shift+cmd+o"),
    bind("project.symbols", "alt+cmd+o"),
    bind("code.callHierarchy", "shift+ctrl+cmd+h"),
    bind("code.rename", "ctrl+cmd+e"),
    bind("code.quickFix", "alt+cmd+a"),
    bind("code.format", "shift+alt+f"),
    bind("go.back", "ctrl+-"),
    bind("go.forward", "ctrl+shift+-"),
    bind("code.nextProblem", "cmd+'"),
    bind("code.previousProblem", "shift+cmd+'"),
    bind("edit.fold", "alt+cmd+left"),
    bind("edit.unfold", "alt+cmd+right"),
    bind("edit.foldAll", "shift+alt+cmd+left"),
    bind("edit.unfoldAll", "shift+alt+cmd+right"),
    bind("edit.addCaretAbove", "ctrl+shift+up"),
    bind("edit.addCaretBelow", "ctrl+shift+down"),
    bind("edit.addNextOccurrence", "cmd+d"),
    bind("dictation.toggle", "ctrl+alt+d"),
    bind("debug.continue", "ctrl+cmd+y"),
    bind("debug.stepOver", "f6"),
    bind("debug.stepInto", "f7"),
    bind("debug.stepOut", "f8"),
    bind("debug.stop", "cmd+."),
    bind("debug.toggleBreakpoint", "cmd+\\"),
    bind("debug.toggleBreakpoints", "cmd+y"),
];

/// The modifiers a shortcut may name, in the order the table writes them.
const MODIFIERS: [&str; 4] = ["ctrl", "alt", "shift", "cmd"];

/// The modifiers and the key of `shortcut`; `None` if a modifier is unknown or the key is missing. The key may be `+`.
pub fn split(shortcut: &str) -> Option<(Vec<&str>, &str)> {
    let (modifiers, key) = match shortcut.strip_suffix('+') {
        Some(rest) if rest.ends_with('+') || rest.is_empty() => (rest.trim_end_matches('+'), "+"),
        _ => shortcut.rsplit_once('+').unwrap_or(("", shortcut)),
    };
    if key.is_empty() {
        return None;
    }
    let modifiers: Vec<&str> = modifiers.split('+').filter(|m| !m.is_empty()).collect();
    modifiers
        .iter()
        .all(|m| MODIFIERS.contains(m))
        .then_some((modifiers, key))
}

/// `shortcut` with its modifiers in a fixed order, so two spellings of one shortcut compare equal.
#[cfg(test)]
fn normal(shortcut: &str) -> Option<String> {
    let (modifiers, key) = split(shortcut)?;
    let mut ordered: Vec<&str> = MODIFIERS.iter().copied().filter(|m| modifiers.contains(m)).collect();
    ordered.push(key);
    Some(ordered.join("+"))
}

/// The table with the user's changes.
#[derive(Debug, Clone, Default)]
pub struct Keymap {
    overrides: BTreeMap<String, String>,
}

impl Keymap {
    /// The table with the changes in `path`, and what in the file could not be used.
    pub fn load(path: &Path) -> (Keymap, Vec<String>) {
        let Ok(text) = std::fs::read_to_string(path) else {
            return (Keymap::default(), vec![]);
        };
        match serde_json::from_str::<BTreeMap<String, String>>(&text) {
            Ok(entries) => Keymap::with(entries),
            Err(e) => (Keymap::default(), vec![format!("{}: {e}", path.display())]),
        }
    }

    /// Where the user's changes are kept.
    pub fn user_path() -> PathBuf {
        paths::data_dir().join("keybindings.json")
    }

    /// The table with `entries`, a command each with its shortcut or `""` for none; unknown commands and shortcuts are
    /// reported and left out.
    pub(crate) fn with(entries: BTreeMap<String, String>) -> (Keymap, Vec<String>) {
        let mut problems = vec![];
        let mut overrides = BTreeMap::new();
        for (command, key) in entries {
            if !DEFAULTS.iter().any(|b| b.command == command) {
                problems.push(format!("unknown command {command}"));
            } else if !key.is_empty() && split(&key).is_none() {
                problems.push(format!("{command}: {key} is no shortcut"));
            } else {
                overrides.insert(command, key);
            }
        }
        (Keymap { overrides }, problems)
    }

    /// The shortcut of `command`; `None` if it has none.
    pub fn key(&self, command: &str) -> Option<&str> {
        let key = match self.overrides.get(command) {
            Some(key) => key.as_str(),
            None => DEFAULTS.iter().find(|b| b.command == command)?.key,
        };
        (!key.is_empty()).then_some(key)
    }

    /// Shortcuts that more than one command claims, each with the commands.
    #[cfg(test)]
    pub(crate) fn conflicts(&self) -> Vec<(String, Vec<&'static str>)> {
        let mut by_key: BTreeMap<String, Vec<&'static str>> = BTreeMap::new();
        for binding in DEFAULTS {
            if let Some(key) = self.key(binding.command).and_then(normal) {
                by_key.entry(key).or_default().push(binding.command);
            }
        }
        by_key.into_iter().filter(|(_, commands)| commands.len() > 1).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_defaults_are_valid_and_free_of_conflicts() {
        for binding in DEFAULTS {
            assert!(
                binding.key.is_empty() || split(binding.key).is_some(),
                "{} has {}",
                binding.command,
                binding.key
            );
        }
        assert_eq!(Keymap::default().conflicts(), vec![]);
        let mut commands: Vec<_> = DEFAULTS.iter().map(|b| b.command).collect();
        commands.sort_unstable();
        commands.dedup();
        assert_eq!(commands.len(), DEFAULTS.len());
    }

    #[test]
    fn shortcuts_split_into_modifiers_and_key() {
        assert_eq!(split("alt+cmd+p"), Some((vec!["alt", "cmd"], "p")));
        assert_eq!(split("cmd++"), Some((vec!["cmd"], "+")));
        assert_eq!(split("+"), Some((vec![], "+")));
        assert_eq!(split("hyper+k"), None);
        assert_eq!(split("cmd+"), None);
        assert_eq!(normal("cmd+shift+n"), normal("shift+cmd+n"));
    }

    #[test]
    fn the_user_can_change_remove_and_clash() {
        let entries = BTreeMap::from([
            ("tab.new".to_string(), "alt+cmd+t".to_string()),
            ("panel.ci".to_string(), String::new()),
            ("panel.git".to_string(), "cmd+1".to_string()),
            ("nonsense".to_string(), "cmd+k".to_string()),
            ("tab.close".to_string(), "meta+w".to_string()),
        ]);
        let (keymap, problems) = Keymap::with(entries);
        assert_eq!(keymap.key("tab.new"), Some("alt+cmd+t"));
        assert_eq!(keymap.key("panel.ci"), None);
        assert_eq!(keymap.key("tab.close"), Some("cmd+w"), "an invalid change is left out");
        assert_eq!(problems.len(), 2);
        assert_eq!(
            keymap.conflicts(),
            vec![("cmd+1".to_string(), vec!["panel.files", "panel.git"])]
        );
    }
}

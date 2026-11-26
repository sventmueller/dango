//! The terminal's settings, made from the app's own instead of Tapa's settings file (E199): the text size the editor
//! shares, the system's monospaced face, ⌥ as Meta, and Tapa's switches with Tapa's defaults.

use ide_core::settings::{Key, Settings, keys};
use term_config::Config;

use crate::context::AppContext;

/// Tapa's switches by their path in Tapa's settings file, each with the setting that holds it here.
const SWITCHES: [(&[&str], Key<bool>); 8] = [
    (&["input", "field"], keys::TERMINAL_INPUT_FIELD),
    (&["input", "vim"], keys::TERMINAL_INPUT_VIM),
    (&["input", "suggestions"], keys::TERMINAL_SUGGESTIONS),
    (&["input", "highlighting"], keys::TERMINAL_HIGHLIGHTING),
    (&["input", "underline_errors"], keys::TERMINAL_UNDERLINE_ERRORS),
    (&["terminal", "blocks"], keys::TERMINAL_BLOCKS),
    (&["terminal", "paste_protection"], keys::TERMINAL_PASTE_PROTECTION),
    (&["terminal", "copy_on_select"], keys::TERMINAL_COPY_ON_SELECT),
];

/// The field of `config` that the switch at `path` sets.
fn switch<'a>(config: &'a mut Config, path: &[&str]) -> Option<&'a mut bool> {
    Some(match path {
        ["input", "field"] => &mut config.input.field,
        ["input", "vim"] => &mut config.input.vim,
        ["input", "suggestions"] => &mut config.input.suggestions,
        ["input", "highlighting"] => &mut config.input.highlighting,
        ["input", "underline_errors"] => &mut config.input.underline_errors,
        ["terminal", "blocks"] => &mut config.terminal.blocks,
        ["terminal", "paste_protection"] => &mut config.terminal.paste_protection,
        ["terminal", "copy_on_select"] => &mut config.terminal.copy_on_select,
        _ => return None,
    })
}

/// The terminal's settings as the user's `settings` have them.
pub(super) fn of(settings: &Settings) -> Config {
    let mut config = Config::default();
    config.font.size = settings.get(&keys::FONT_SIZE);
    for (path, key) in &SWITCHES {
        if let Some(on) = switch(&mut config, path) {
            *on = settings.get(key);
        }
    }
    config
}

/// Whether the setting `name` changes what the terminals show or do.
fn concerns_terminals(name: &str) -> bool {
    [keys::FONT_SIZE.declared(), keys::OPTION_AS_META.declared()]
        .into_iter()
        .chain(SWITCHES.iter().map(|(_, key)| key.declared()))
        .any(|setting| setting.name == name)
}

/// The settings `names` changed, here or through the sync: the open terminals take up those of them they use.
pub(crate) fn settings_changed(cx: &AppContext, names: &[String]) {
    if !names.iter().any(|name| concerns_terminals(name)) {
        return;
    }
    cx.terminals().reload_config();
    for view in term_ui::all_views() {
        view.apply_config();
    }
}

/// Switches off the setting that holds Tapa's switch at `path`, as a warning's "do not ask again" asks.
pub(crate) fn switch_off(path: &[&str]) {
    let Some((_, key)) = SWITCHES.iter().find(|(at, _)| *at == path) else {
        return;
    };
    if let Err(error) = Settings::load_user().set(key, &false) {
        crate::diag::report("switching a terminal setting off", error);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_switch_sets_a_field_and_starts_as_in_tapa() {
        for (path, key) in &SWITCHES {
            let mut config = Config::default();
            let field = switch(&mut config, path).map(|on| *on);
            assert_eq!(field, Some(key.default_value()), "{path:?}");
        }
        assert!(switch(&mut Config::default(), &["terminal", "scrollback"]).is_none());
    }

    #[test]
    fn the_text_size_and_the_switches_concern_the_terminals() {
        assert!(concerns_terminals("fontSize"));
        assert!(concerns_terminals("optionAsMeta"));
        assert!(concerns_terminals("terminalPasteProtection"));
        assert!(!concerns_terminals("minimap"));
    }
}

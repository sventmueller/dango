//! Dictation into the AI composer and the editor through lowspeak (E13): one recording at a time, bound to the text
//! field that had the focus when it started, its text inserted at that field's cursor as one undo step. See
//! docs/dictation.md.
//!
//! The app registers each field as a `DictationField` and holds the dictations in its context through `Host`; the
//! crate knows no editor and no AI tab. Without the feature `engine` only this file remains, so builds without
//! dictation never compile lowspeak's whisper.cpp. How the menu writes the dictation's shortcut:
//!
//! ```
//! assert_eq!(ide_ui_dictation::written_shortcut("cmd+shift+d"), "⇧⌘D");
//! ```

#[cfg(feature = "engine")]
mod hub;

#[cfg(feature = "engine")]
pub use hub::*;

/// A text field that dictation goes into, as the app sees it.
pub trait DictationField {
    /// Shows `text` in the window the field was in, for a dictation whose field closed; whether there was one.
    fn notice(&self, text: &str) -> bool;
}

/// The keymap's `binding`, such as "cmd+shift+d", as macOS writes shortcuts; empty for one without a key.
pub fn written_shortcut(binding: &str) -> String {
    let Some((modifiers, key)) = ide_core::keymap::split(binding) else {
        return String::new();
    };
    let mut written: String = [("ctrl", "⌃"), ("alt", "⌥"), ("shift", "⇧"), ("cmd", "⌘")]
        .iter()
        .filter(|(name, _)| modifiers.contains(name))
        .map(|(_, glyph)| *glyph)
        .collect();
    written.push_str(&key.to_uppercase());
    written
}

#[cfg(test)]
mod tests {
    use super::written_shortcut;

    #[test]
    fn a_shortcut_is_written_in_the_order_of_macos_and_one_without_key_not_at_all() {
        assert_eq!(written_shortcut("shift+cmd+alt+ctrl+d"), "⌃⌥⇧⌘D");
        assert_eq!(written_shortcut("f5"), "F5");
        assert_eq!(written_shortcut(""), "");
        assert_eq!(written_shortcut("hyper+d"), "");
    }
}

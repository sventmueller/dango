//! VS Code's Seti icon theme, as the vendored `vs-seti-icon-theme.json` describes it: an icon per file name, per
//! extension and per language, each a glyph of the Seti font with a colour for dark and one for light themes.

use std::collections::HashMap;
use std::sync::OnceLock;

use serde_json::Value;

/// The theme as VS Code ships it, see `assets/seti/README.md`.
const JSON: &str = include_str!("../../assets/seti/vs-seti-icon-theme.json");

/// A colour as the theme writes it, `#rrggbb`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Rgb(pub u8, pub u8, pub u8);

/// The icon of a file.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Icon {
    /// The theme's name for it, such as `_rust`.
    pub id: &'static str,
    /// Its character in the Seti font.
    pub glyph: char,
    /// Its colour in a dark appearance; none for the colour of the text beside it.
    pub dark: Option<Rgb>,
    /// Its colour in a light appearance, from the theme's `_light` variant where it has one.
    pub light: Option<Rgb>,
}

#[derive(Debug, Clone, Copy)]
struct Definition {
    glyph: char,
    color: Option<Rgb>,
}

#[derive(Debug, Default)]
pub struct Theme {
    definitions: HashMap<String, Definition>,
    /// The icon of a file nothing else names.
    file: String,
    /// By lower-case file name.
    names: HashMap<String, String>,
    /// By lower-case extension, which may hold dots, such as `spec.ts`.
    extensions: HashMap<String, String>,
    /// By VS Code's language identifier.
    languages: HashMap<String, String>,
}

/// The vendored theme, read once.
pub fn theme() -> &'static Theme {
    static THEME: OnceLock<Theme> = OnceLock::new();
    THEME.get_or_init(|| {
        parse(JSON).unwrap_or_else(|| {
            crate::diag::report("file icons", "the vendored Seti theme does not parse");
            Theme::default()
        })
    })
}

fn parse(json: &str) -> Option<Theme> {
    let value: Value = serde_json::from_str(json).ok()?;
    let definitions = value
        .get("iconDefinitions")?
        .as_object()?
        .iter()
        .filter_map(|(id, definition)| {
            let glyph = glyph(definition.get("fontCharacter")?.as_str()?)?;
            let color = definition.get("fontColor").and_then(Value::as_str).and_then(color);
            Some((id.clone(), Definition { glyph, color }))
        })
        .collect();
    let map = |key: &str| -> HashMap<String, String> {
        value
            .get(key)
            .and_then(Value::as_object)
            .map(|entries| {
                entries
                    .iter()
                    .filter_map(|(k, v)| Some((k.to_lowercase(), v.as_str()?.to_string())))
                    .collect()
            })
            .unwrap_or_default()
    };
    Some(Theme {
        definitions,
        file: value.get("file")?.as_str()?.to_string(),
        names: map("fileNames"),
        extensions: map("fileExtensions"),
        languages: map("languageIds"),
    })
}

/// A glyph as the theme writes it, a backslash and the code point in hex: `\E023`.
fn glyph(text: &str) -> Option<char> {
    char::from_u32(u32::from_str_radix(text.strip_prefix('\\')?, 16).ok()?)
}

fn color(text: &str) -> Option<Rgb> {
    let hex = text.strip_prefix('#')?;
    if hex.len() != 6 {
        return None;
    }
    let byte = |at: usize| u8::from_str_radix(hex.get(at..at + 2)?, 16).ok();
    Some(Rgb(byte(0)?, byte(2)?, byte(4)?))
}

impl Theme {
    /// The icon VS Code shows for a file called `name`: by its whole name, then by its extensions from the longest,
    /// then by its language, then the theme's file icon.
    pub fn icon_of(&'static self, name: &str) -> Icon {
        let lower = name.to_lowercase();
        // An association whose icon the theme does not define draws nothing in VS Code, so the next one counts.
        let defined = |id: &&'static String| self.definitions.contains_key(id.as_str());
        let id = self
            .names
            .get(&lower)
            .filter(defined)
            .or_else(|| extensions(&lower).find_map(|extension| self.extensions.get(extension).filter(defined)))
            .or_else(|| {
                super::languages::language_of(&lower)
                    .and_then(|language| self.languages.get(language))
                    .filter(defined)
            })
            .unwrap_or(&self.file);
        self.icon(id)
    }

    fn icon(&'static self, id: &'static str) -> Icon {
        let fallback = Definition {
            glyph: ' ',
            color: None,
        };
        let dark = self.definitions.get(id).copied().unwrap_or(fallback);
        let light = self.definitions.get(&format!("{id}_light")).copied().unwrap_or(dark);
        Icon {
            id,
            glyph: dark.glyph,
            dark: dark.color,
            light: light.color,
        }
    }

    /// Every glyph the theme draws.
    #[cfg(test)]
    pub fn glyphs(&self) -> impl Iterator<Item = char> + '_ {
        self.definitions.values().map(|d| d.glyph)
    }

    /// Every icon name the associations refer to.
    #[cfg(test)]
    pub fn referenced(&self) -> impl Iterator<Item = &str> + '_ {
        self.names
            .values()
            .chain(self.extensions.values())
            .chain(self.languages.values())
            .chain([&self.file])
            .map(String::as_str)
    }

    #[cfg(test)]
    pub fn has(&self, id: &str) -> bool {
        self.definitions.contains_key(id)
    }
}

/// The extensions of a lower-case file name as VS Code tries them, the longest first: `a.spec.ts` has `spec.ts`
/// and `ts`, `.gitignore` has `gitignore`.
fn extensions(lower: &str) -> impl Iterator<Item = &str> {
    lower
        .match_indices('.')
        .map(|(at, _)| &lower[at + 1..])
        .filter(|extension| !extension.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn glyphs_colours_and_extensions_are_read_as_vs_code_writes_them() {
        assert_eq!(glyph("\\E023"), Some('\u{E023}'));
        assert_eq!(glyph("E023"), None);
        assert_eq!(color("#d4d7d6"), Some(Rgb(0xd4, 0xd7, 0xd6)));
        assert_eq!(color("#fff"), None);
        assert_eq!(extensions("a.spec.ts").collect::<Vec<_>>(), ["spec.ts", "ts"]);
        assert_eq!(extensions(".gitignore").collect::<Vec<_>>(), ["gitignore"]);
        assert_eq!(extensions("makefile").count(), 0);
        assert_eq!(extensions("trailing.").count(), 0);
    }

    #[test]
    fn the_vendored_theme_parses_whole() {
        let theme = parse(JSON).unwrap();
        assert!(theme.definitions.len() > 300, "{}", theme.definitions.len());
        assert!(theme.names.len() > 50 && theme.extensions.len() > 200 && theme.languages.len() > 50);
        assert_eq!(theme.file, "_default");
    }
}

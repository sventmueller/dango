//! The icon of a file in lists, as VS Code's default file icon theme shows it: Seti's glyph and colour for the
//! file's name, extension or language, from the theme VS Code ships, vendored in `assets/seti` (see the README there).
//! Folders have no icon, only the outline view's disclosure triangle, and keep the icon's room so their names line up
//! with the files'. The icons are decoration: VoiceOver reads the file's name beside them.

mod glyph;
mod languages;
mod theme;

use std::path::Path;

use objc2::MainThreadMarker;
use objc2::rc::Retained;
use objc2_app_kit::{NSImageScaling, NSImageView};

pub use theme::Icon;

/// The icon VS Code with Seti shows for a file called `name`.
pub fn icon_of(name: &str) -> Icon {
    theme::theme().icon_of(name)
}

/// An image view, 16 points square, with the icon of the file at `path`, or empty for a folder; hidden from
/// VoiceOver.
pub fn image_view(mtm: MainThreadMarker, path: &Path, folder: bool) -> Retained<NSImageView> {
    let view = NSImageView::new(mtm);
    view.setImageScaling(NSImageScaling::ScaleNone);
    if !folder {
        let name = path
            .file_name()
            .map_or_else(String::new, |n| n.to_string_lossy().into_owned());
        let icon = icon_of(&name);
        if let Some(image) = glyph::image(icon.glyph) {
            view.setImage(Some(&image));
            view.setContentTintColor(Some(&glyph::color(&icon)));
        }
    }
    crate::util::hide_from_voiceover(&view);
    view
}

#[cfg(test)]
mod tests {
    use super::theme::Rgb;
    use super::*;

    #[test]
    fn files_get_the_icons_vs_code_shows() {
        let cases = [
            ("main.rs", "_rust"),
            ("server.go", "_go2"),
            ("app.ts", "_typescript"),
            ("app.tsx", "_react"),
            ("index.js", "_javascript"),
            ("index.mjs", "_javascript"),
            ("view.jsx", "_react"),
            ("tool.py", "_python"),
            ("View.swift", "_swift"),
            ("lexer.c", "_c"),
            ("lexer.cpp", "_cpp"),
            ("lexer.h", "_c_1"),
            ("Main.java", "_java"),
            ("Main.kt", "_kotlin"),
            ("app.rb", "_ruby"),
            ("index.php", "_php"),
            ("build.sh", "_shell"),
            (".zshrc", "_shell"),
            ("package.json", "_json"),
            ("tsconfig.json", "_tsconfig"),
            ("rustfmt.toml", "_config"),
            ("ci.yml", "_yml"),
            ("compose.yaml", "_docker_3"),
            ("Info.plist", "_xml"),
            ("index.html", "_html_3"),
            ("site.css", "_css"),
            ("site.scss", "_sass"),
            ("README.md", "_info"),
            ("notes.md", "_markdown"),
            ("todo.txt", "_todo"),
            ("schema.sql", "_db"),
            ("api.proto", "_default"),
            ("Cargo.lock", "_default"),
            ("yarn.lock", "_yarn"),
            ("logo.png", "_image"),
            ("logo.svg", "_svg"),
            ("manual.pdf", "_pdf"),
            ("dist.zip", "_zip_1"),
            ("data.csv", "_csv"),
            ("Makefile", "_makefile"),
            ("Dockerfile", "_docker"),
            ("Dockerfile.dev", "_docker"),
            (".gitignore", "_git"),
            (".gitmodules", "_git"),
            ("LICENSE", "_license"),
            ("CHANGELOG.md", "_clock"),
            (".editorconfig", "_config"),
            (".env.local", "_config"),
            ("Cargo.toml", "_config"),
            ("parser_test.go", "_go2"),
            ("button.test.tsx", "_react_1"),
            ("login.spec.ts", "_typescript_1"),
            ("VERSION", "_clock"),
            ("random.xyz", "_default"),
        ];
        for (name, id) in cases {
            assert_eq!(icon_of(name).id, id, "{name}");
        }
    }

    #[test]
    fn a_light_appearance_takes_the_themes_light_colour() {
        let rust = icon_of("main.rs");
        assert_eq!(
            (rust.dark, rust.light),
            (Some(Rgb(0x6d, 0x80, 0x86)), Some(Rgb(0x62, 0x73, 0x79)))
        );
        let unknown = icon_of("random.xyz");
        assert_ne!(unknown.light, unknown.dark);
        let todo = icon_of("todo.txt");
        assert_eq!((todo.dark, todo.light), (None, None), "drawn in the colour of the text");
    }

    #[test]
    fn the_common_kinds_differ_by_glyph_or_colour() {
        let kinds = [
            "a.rs",
            "a.go",
            "a.ts",
            "a.tsx",
            "a.js",
            "a.py",
            "a.swift",
            "a.c",
            "a.cpp",
            "a.java",
            "a.kt",
            "a.rb",
            "a.php",
            "a.sh",
            "a.json",
            "a.yml",
            "a.html",
            "a.css",
            "a.scss",
            "a.md",
            "a.sql",
            "a.csv",
            "a.pdf",
            "a.png",
            "a.svg",
            "a.zip",
            "Makefile",
            "Dockerfile",
            ".gitignore",
            "README.md",
            "LICENSE",
            "a.toml",
            "a.unknown",
        ];
        let mut seen = std::collections::HashMap::new();
        for name in kinds {
            let icon = icon_of(name);
            if let Some(other) = seen.insert((icon.glyph, icon.dark), name) {
                panic!("{name} looks like {other}");
            }
        }
    }

    #[test]
    fn every_icon_the_theme_names_exists_and_every_glyph_is_in_the_font() {
        let theme = theme::theme();
        for id in theme.referenced() {
            assert!(theme.has(id), "no definition {id}");
        }
        let font = glyph::font().expect("Core Text reads the vendored font");
        for character in theme.glyphs() {
            assert!(
                glyph::glyph_of(&font, character).is_some(),
                "no glyph U+{:04X}",
                u32::from(character)
            );
        }
    }
}

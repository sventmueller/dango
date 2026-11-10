# Seti file icons

The file icons in lists are VS Code's default file icon theme, Seti, as VS Code ships it. The files here are copies,
read at build time; nothing is fetched while building or testing.

| File | Source | Licence |
|---|---|---|
| `seti.woff` | [microsoft/vscode](https://github.com/microsoft/vscode) `extensions/theme-seti/icons/seti.woff` at commit `ae15303824771284a26e84c78d5769a03051f50d`, built from [jesseweed/seti-ui](https://github.com/jesseweed/seti-ui) at commit `2d6c5e68b4ded73c92dac291845ee44e1182d511` (VS Code's `cgmanifest.json`) | `LICENSE-seti-ui.md` (MIT) |
| `vs-seti-icon-theme.json` | microsoft/vscode `extensions/theme-seti/icons/vs-seti-icon-theme.json` at the same commit: the glyph and colours of each icon, dark and `light`, and the icon of each file name, extension and language | `LICENSE-vscode.txt` (MIT) |

To update, take both files from a newer VS Code commit, note it here and run `cargo test -p ide-app file_icon`,
which checks that every icon the theme names exists and every glyph is in the font. The app carries both licences in
its acknowledgements (`xtask/src/licenses.rs`).

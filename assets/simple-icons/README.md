# Simple Icons language logos

The file icons of the most common languages are logos from [Simple Icons](https://simpleicons.org), npm package
`simple-icons` 16.37.0, copied unchanged from its `icons` folder; nothing is fetched while building or testing. Every
other file shows an SF Symbol.

Simple Icons is CC0, but some logos carry a license of their own and every logo is a trademark of its owner; both are
listed in `LICENSE.md`, which the app carries in its acknowledgements (`xtask/src/licenses.rs`). Logos under a
non-commercial license (Sass, Vue.js) are left out on purpose. Before adding a logo, look up its `license` and
`guidelines` in the package's `data/simple-icons.json`.

To update, copy the same files from a newer package version, note it here and run `cargo test -p ide-app file_icon`,
which checks that every logo parses.

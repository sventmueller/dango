# Development

How the app is built, run and tested. What it is made of, the decisions (E1–E27) and the phases are in
[plan.md](plan.md).

## Requirements
- macOS 14 or later with the Xcode Command Line Tools (`xcode-select --install`): the SDK, the linker, `codesign`
  and `plutil`. Xcode itself is needed only by `make icon` (below).
- Rust 1.99.1, as `rust-toolchain.toml` names it. Homebrew's `rust` builds everything here; rustup becomes necessary
  only for the universal release build, since Homebrew ships the standard library for this Mac's processor alone.
- The sibling checkouts `../splat` and `../lowspeak`, which the workspace takes crates from by path (E14).
- `cargo install --locked cargo-hakari`, which `make lint` uses to check the workspace-hack (below).

## App icon
The icon is the Icon Composer document `assets/AppIcon.icon` (the layer `Assets/dango.svg` on a white background, dark
in Dark Mode). Only Xcode's `actool` compiles it, so its output `assets/Assets.car` (macOS 26 and later) and
`assets/AppIcon.icns` (macOS 14 and 15) is checked in and copied into every bundle. After a change in Icon Composer,
`make icon` compiles it again.

## Commands
`make` lists the entry points; the build logic behind them is `cargo xtask`, in `xtask/src`.

| Command | What it does |
|---|---|
| `make build` | `cargo xtask bundle --release`: the release app with dictation, signed; VERSION unchanged |
| `make run` | `cargo xtask run`: the debug app, opened |
| `make icon` | `cargo xtask icon`: compiles the app icon, see [App icon](#app-icon); needs Xcode |
| `make lint` | `cargo fmt --check`, the workspace-hack, clippy on the workspace and on the app with `--features selftest`; warnings fail |
| `make check` | `lint`, then `cargo test --workspace`; needs no keychain, no window and no network |
| `make check-dictation` | clippy and the app's tests with the feature `dictation`, for changes that touch it |
| `make test` | `check`, then `cargo xtask selftest probe` |
| `make perf` | the timing budgets of E25 in a release build; wall-clock bounds, so only on an otherwise idle Mac |
| `make fuzz` | `cargo xtask fuzz`: the crates' fuzz targets, see [Fuzzing](#fuzzing); outside `make check` |
| `make coverage` | `cargo xtask coverage`: line coverage of the tests and the offline self-tests, see [Coverage](#coverage) |
| `make ci` | `cargo xtask ci`: formatter, `check`, coverage with every test and self-test, `perf`; seals the content, see [CI and deploy](#ci-and-deploy) |
| `make seal-explain` | `cargo xtask seal explain`: the seal of HEAD's content here and on GitHub |
| `make deploy` | notarizes the next version, commits VERSION, stages the assets and pushes `main` and its tag, a green seal along; GitHub publishes the release |
| `make release` | `cargo xtask release`: a dry run of the release, see [Release](#release); sends nothing anywhere |
| `make release-notarized` | `check`, then `cargo xtask release --notarize`: the release for real into `dist/` |
| `make test-integration` | `cargo xtask integration`: real servers, CLIs and services with the sign-ins, no paid account unless asked; see below |
| `make test-claude-latest` | `cargo xtask claude-latest`: the newest Claude CLI against the contract the app relies on, see [Newest Claude CLI](#newest-claude-cli) |

`cargo xtask` directly:
- `bundle [--release] [--selftest]` builds the package `ide-app`, lays out the bundle, signs it and prints its path.
- `run [--release]` bundles, then opens the app with `open -n`, so it runs beside an instance already open.
- `selftest [<scenario>…|--offline]` bundles the release with the selftest built in and runs each scenario, `probe` if
  none is named, every offline one with `--offline`; for a scenario that needs dictation the bundle has it as well.
- `selftest-accept <scenario> [<picture>…]` takes pictures of the last run as accepted ones, see
  [Accepted pictures](#accepted-pictures).
- `release [--notarize] [--layout]`, `universal` and `acknowledgements`, see [Release](#release).
- `integration [--list] [--paid] [--only <group>[,<group>…]]` runs the integration checks; `--list` only prints what
  would run and why the rest is skipped, and the groups that use paid accounts run only with `--paid` or by name.

`cargo xtask selftest agent-perf` measures streaming into histories of 1,000 and 10,000 cards, with answers of
roughly 10 KB and 100 KB. It reports p50, p95 and the slowest update, including height correction and drawing;
p95 must stay below 16 ms and each update below 100 ms. Samples arrive 30 ms apart, yielding to AppKit between updates. Run it on an otherwise
idle Mac. It also checks native view and selection reuse,
background tabs catching up without building cards while hidden, and text, fonts and links against a complete
Markdown render, including definitions that arrive after their references. Reports are in
`target/selftest/agent-perf.json`; it uses replayed events and makes no model requests.

## Workspace-hack
Cargo turns on the features of a shared dependency that the crates of one build need, so `cargo test -p ide-ui-icons`
and `cargo test -p ide-ui-terminal` used to build the AppKit bindings, and every crate above them, with different
features each time. The crate `ide-workspace-hack` (E194) depends on every such dependency with the features all
members together use, and every member depends on it, so each dependency is built once per profile. After adding,
removing or changing a dependency or a crate, `cargo hakari generate` rewrites its list and
`cargo hakari manage-deps` adds it to a new crate; `make lint` fails while either has something to do. Its settings
are in `.config/hakari.toml`: xtask stays outside because it also builds on Linux, and Splat's crates stay outside
because their `selftest` feature would otherwise reach the release.

## Fuzzing
The parsers that read what other programs write have fuzz targets (E95), each in a `fuzz/` folder of its crate: the
terminal's scanner and `KERN_PROCARGS2` reader, the language servers' framing with the positions they send, ACP and
Claude stream-json into the thread model, `git diff` output, the providers' server-sent events, and the answers of
local fill-in-the-middle models and Copilot that become edit predictions, and the debug adapters' messages and
`.vscode/launch.json`. They reach
crate-private parts through the feature `fuzzing`, which only the fuzz crates turn on. cargo-fuzz needs a nightly
compiler; `xtask/src/fuzz.rs` pins its date (R18), and `rustup toolchain install nightly-<date> --profile minimal`
plus `cargo install cargo-fuzz` set it up. `make fuzz` runs every target for `FUZZ_SECONDS` (60 by default),
`make fuzz ARGS=diff` one; the corpus grows in `target/fuzz/corpus/`, the recorded sessions in `tests/fixtures` start
it. A crash stops the run and leaves its input in the crate's `fuzz/artifacts/`; the fix comes with a unit test built
from that input, then the file goes.

A target checks results, not only that nothing panics. What it holds the code to, per target:

| Target | Oracle |
|---|---|
| `scan` | the scanner fed in pieces finds exactly the events it finds fed whole |
| `procargs` | arguments written into a `KERN_PROCARGS2` buffer read back as they were |
| `messages` (LSP) | a body framed again reads back as itself; a position on the text reads back in every unit; an edit makes the text a string edit at the same place would |
| `updates` (ACP), `stream_json` (Claude) | the changes the thread reports rebuild every timeline, its lookups agree with its timelines, a removed id never comes back (`ide_thread::fuzzing::Replay`) |
| `diff` | line numbers count up from the hunk's starts within its counts; the staging patch of all hunks, forwards and backwards, reads back as them |
| `stream` (providers) | events written out again read back as themselves; what OpenAI's and Gemini's readers stream is the text, reasoning and tool calls of their message. Anthropic's reader has no such oracle: a block's start carries text no delta told, and a fallback drops blocks |
| `answer` (predictions) | a cut proposal is a start of the answer without a stop, blank or trailing space, on one line inside a line; Copilot's proposals lie between characters and change the text |
| `dap_messages` | every message written again reads back as itself |
| `launch_json` | removing comments keeps every byte's place and leaves plain JSON alone; a configuration with nothing left to fill in reads back as itself |

The conversions of adapter answers into the crate's types (`convert`, `Capabilities`) and Anthropic's reader are
one-way, without an encoder or a second account of the result to compare with; for them the targets check only that
nothing panics. The DAP fuzz crate turns on serde_json's `float_roundtrip`, without which a number written and read
again may differ in its last bit.

Property tests (E96) run in `make check` with a fixed seed and a fixed number of cases: `tests/properties.rs` of
`ide-document`, and the unit test modules `property_tests` of `ide-git` (staging some hunks), `ide-project` (fuzzy
matching), `ide-terminal` (the scanner in pieces) and `ide-vim` (the replacement it hands the editor). `make fuzz`
also runs them, or `make fuzz ARGS=properties` alone, with a fresh seed and `PROPTEST_CASES` cases (10 000 by
default): proptest reads `PROPTEST_RNG_SEED` and `PROPTEST_CASES` over the values the tests fix, and the run prints
the seed it chose. A failure prints the command that repeats it with the same seed, for example
`PROPTEST_RNG_SEED=8123 PROPTEST_CASES=10000 cargo test --package ide-git --lib -- property_tests::`. proptest
shrinks the failing case and writes its own seed down: for `tests/properties.rs` into
`tests/properties.proptest-regressions` beside it, for a unit test module into the crate's `proptest-regressions/`
along the module's path, such as `crates/ide-git/proptest-regressions/staging/property_tests.txt`. That file is
checked in with the fix, and from then on every run, the fixed one of `make check` included, tries that case first. Where the shrunk case says more as
an example, it also becomes a named unit test with the values written out.

## Coverage
`make coverage` (`cargo xtask coverage`, `xtask/src/coverage.rs`) measures line coverage of `cargo test --workspace`
(unit and integration tests, not doctests: their coverage needs a nightly compiler) together with the selftest
scenarios that need no network, account, real agent CLI or person: `cargo xtask selftest --offline`, every scenario
the table in `src/selftest/scenarios.rs` marks offline. The scenarios run in one selftest bundle built with
dictation, so `dictation` is measured too; a failed test or check is reported and the run goes on. Homebrew's Rust has
no llvm-tools, so it builds with rustup's toolchain of `rust-toolchain.toml` (`rustup component add llvm-tools
--toolchain 1.99.1`) and `cargo-llvm-cov` (`brew install cargo-llvm-cov`), instrumented by `cargo llvm-cov show-env` in
`target/llvm-cov-target` with a build folder of its own, so it never touches the normal build. A run takes 17 to 20
minutes, about 15 of them the self-tests. A launch that is killed, such as the first one of `recovery`, writes no
coverage data.

Counted is every Rust file in `src/`, `crates/*/src` and `xtask/src`, except `tests/`, `benches/`, `examples/` and
`fuzz/` folders, build scripts, the unit test files cargo-llvm-cov leaves out (`tests.rs`, `*_tests.rs`), the fuzz
targets' entry points `fuzzing.rs`, and code that only exists to test the rest: the selftest scenarios and hooks
(`src/selftest*`, `*/selftest.rs`, `selftest/` folders, the terminal's `inspect.rs`) and the ACP test agent with its
recorder. xtask's selftest runner is tooling and counts. `src/decoder_main.rs` is left out too: launchd starts the
decoding service in a sandbox that cannot write coverage data; `preview` exercises it through the app. A file the build compiled without code
to run (only `mod` lines or types) counts with no lines; a file no instrumented program contains counts with its code
lines as uncovered. Beside the coverage of both together, the report gives `cargo test` alone and the self-test alone,
per crate, folder and file and in total: while the self-test is reported alone, the profiles of `cargo test` wait in
`target/llvm-cov-target/unit-profiles/`. The results are in `target/coverage/`: `coverage.txt` (the summary,
then crates, folders and files, lowest first), `files.json` (the same per file), `llvm-cov.json` (llvm-cov's export of
both), `llvm-cov-unit.json` and `llvm-cov-selftest.json` (each alone).
`make coverage ARGS=--check` fails below 90 % in total or 50 % in any file, naming them, and when `cargo test` alone
covers the app crate's `src/` (its self-test left out) other than `APP_UNIT_FLOOR` in `xtask/src/coverage.rs`: below it,
or above it by a tenth, naming the value to raise it to, a ratchet like those of `cargo xtask rules`. `--strict` also
fails the run when a test or a selftest scenario failed.

## CI and deploy
`make ci` (`cargo xtask ci`, `xtask/src/ci.rs`) is the whole pipeline. It refuses while the screen is locked (the
self-tests cannot capture windows then), runs `cargo fmt --all`, takes the fingerprint of the working tree and then,
stopping at the first failure, `make check`, `cargo xtask coverage --check --strict` and `make perf`, printing each
stage's duration. The stages run under `caffeinate -di`, so the display does not sleep and lock the screen; a stage
that fails while the screen is locked seals nothing. A complete run seals the content in a local ref: `refs/ci-seals/green/<fingerprint>` when every stage
passed, `refs/ci-seals/red/<fingerprint>` when one failed. A red seal outweighs a green one for the same fingerprint.
Content that changed during the run seals nothing.

The fingerprint (`xtask/src/seal.rs`) is the git tree of the content without `VERSION`, which only `make deploy` raises: for
the working tree through a temporary index (`read-tree HEAD`, `add --all`), so untracked files count unless they are
ignored; for a CI checkout `HEAD`'s tree. Committing the whole sealed working tree with `git add -A` therefore gives
`HEAD` the sealed fingerprint. `cargo xtask seal fingerprint [--head]` prints it, `make seal-explain` says which seals
HEAD's content has here and on origin and which paths separate it from the nearest green seal, and `cargo xtask seal
check` gives origin's verdict for HEAD as `fingerprint=` and `verdict=` lines (also into `$GITHUB_OUTPUT`), failing
for red. A seal ref points at a commit of the sealed tree without parents.

`make deploy` (`cargo xtask deploy`) refuses unless `main` is checked out and everything is committed; it does not
ask for a seal. It notarizes the next version, commits only VERSION, creates its tag and uploads all assets and their
SHA-256 manifest to a draft release. Only after the complete upload does it push `main` and the version tag
atomically, with the green seal ref when HEAD's content has one here, so the workflows find the complete draft.
An upload or push failure leaves the draft unpublished; `dist/deployment.json` lets the same source resume without
another version bump. `.github/workflows/release.yml` runs when VERSION changes, checks the commit, tag,
manifest, every checksum and the update feed, then publishes the draft as latest. A manual workflow run can retry
publication; a push without a version change creates no release. `.github/workflows/ci.yml` runs on every push to `main` and by hand: its
`seal` job (Linux) runs `cargo xtask seal check`; for content sealed green the `check` job is skipped, for content
sealed red the workflow fails, and unsealed content gets `make check` on a macOS runner, with `splat` and `lowspeak`
checked out beside it as in [Newest Claude CLI](#newest-claude-cli). The window self-tests, the coverage gate that
includes them and the timing budgets run only in the local `make ci`: they need a logged-in GUI session with screen
recording and accessibility permissions, which GitHub's runners lack. The skills `/ci` and `/deploy` in
`.claude/skills/` drive these targets: `/ci` runs `make ci` until it is green and commits the sealed tree, `/deploy`
commits the release content, releases with `make deploy` and watches both GitHub runs; it runs no `/ci` itself.

## Builds
The display name and the identifiers come from the crate `ide-product` (E18); the table shows their current values.
The identifiers never contain the display name, so renaming the app strands no settings.

| Build | Bundle | Bundle identifier | URL scheme |
|---|---|---|---|
| Debug | `target/debug/Dango Debug.app` | `local.agentide.debug` | `dango-debug` |
| Release | `target/release/Dango.app` | `local.agentide` | `dango` |
| Selftest | `target/<profile>/Dango Selftest.app` | `local.agentide.selftest` | none |
| Distribution | `target/distribution/Dango.app` | `local.agentide` | `dango` |

Each build is an app of its own, with its own defaults domain, keychain service and data folders, so a debug build
or a selftest never touches the settings of the app in use. Launch Services hands a URL to any one of the apps that
claim its scheme, so only the release claims the product's scheme; the debug build answers its own, and the selftest,
which lives only as long as a scenario, none. The executable is `Contents/MacOS/Dango` in every build.

`VERSION` holds major.minor.build and names the most recently deployed version. Only `make deploy` raises the build
number: it builds the next version in memory, then commits that exact number after successful notarization. Normal
builds, self-tests, dry runs and standalone `make release-notarized` keep VERSION unchanged. The bundle, tag, release,
update feed and committed VERSION therefore all carry the same number.

Every bundle also carries what the previews draw with (docs/splat-render.md): splat's renderer page `render.html`
and `js/` from `../splat/resources`, without the files of splat's formula field, whose licenses the acknowledgements
list, and the decoding service `Contents/XPCServices/Decoder.xpc`, the program `ide-decoder` (`src/decoder_main.rs`)
in `SERVICES`, whose identifier is the build's with `.decoder`.

## Signing
- Every build is signed with the hardened runtime and without the App Sandbox (E2), whose restrictions every child
  process would inherit: the terminal, agents, git and language servers. The decoding service is signed before the
  app, with the App Sandbox and nothing else; launchd starts it, not the supervisor.
- The entitlements are written per build to `target/<bundle identifier>.entitlements` from `entitlements` in
  `xtask/src/bundle.rs`: `com.apple.security.device.audio-input` in builds with dictation, without which the hardened
  runtime keeps the microphone closed, and nothing else.
- Local builds are signed with a self-signed certificate, `local.agentide Local Signing`, which the first bundle
  creates in the login keychain. macOS ties granted permissions, such as folder access and the keychain's "Always
  Allow", to the signature; an ad hoc signature changes with every build, the certificate does not.
  `security find-identity -p codesigning` lists it as not trusted, which is expected for a self-signed certificate.
  Delete it in Keychain Access (login, My Certificates) to have the next bundle create a new one.
- The distribution is signed with the Developer ID and the secure timestamp, see [Release](#release).

## Dictation
Dictation comes from `../lowspeak` behind the cargo feature `dictation` (docs/dictation.md). lowspeak builds
whisper.cpp with cmake and links CoreML, so only the app bundles (`cargo xtask run` and `make build` as well as the
release, `Build::dictation` in `xtask/src/bundle.rs`) and the `dictation` scenario turn the feature on; `make lint`,
`make check` and the other scenarios go without it, and so do their microphone buttons, the menu item and the fn
key. The first `cargo xtask run` therefore compiles whisper.cpp and needs cmake. `make check-dictation` checks the
code behind it. A bundle with the feature declares `NSMicrophoneUsageDescription` in its `Info.plist` and the
audio-input entitlement. The `dictation` scenario replaces
the microphone and the transcription with substitutes behind lowspeak's own seam (`Dictation::with_parts`) and the
keychain with memory, so it needs no microphone, model, key or network.

## Release
`cargo xtask release` builds what a release publishes (E2, E16), in `xtask/src/release.rs`:
1. The universal program (`cargo xtask universal`): the release with `dictation` (E46) for `aarch64-apple-darwin` and
   `x86_64-apple-darwin` through rustup, each in `target/universal/<target>` with `GGML_NATIVE=OFF` and, for Intel, the
   instruction sets every Intel Mac with macOS 14 has, as Lowspeak builds it; joined with `lipo` into
   `target/universal/ide`. It needs `../lowspeak` on its `main`, which holds the `dictation` module, and cmake.
2. The distribution bundle in `target/distribution/`: the universal program, Sparkle 2.10.1 without its XPC services
   (only a sandboxed app needs them), `SUFeedURL` and `SUPublicEDKey` in `Info.plist`, and
   `NSMicrophoneUsageDescription`, the audio-input entitlement, and
   `Contents/Resources/Acknowledgements.txt` with the license text of every crate in the program and of Sparkle
   (`cargo xtask acknowledgements` writes it to `target/` alone). XPC services listed in `SERVICES` in
   `xtask/src/bundle.rs` go into `Contents/XPCServices`, each with its own entitlements, such as the App Sandbox for a
   decoder.
3. The signature, inside out: services, Sparkle's `Autoupdate`, `Updater.app` and framework, then the app, all with
   the hardened runtime; then a check that every program has both architectures.
4. The disk image `Dango.dmg`: the app and a link to `/Applications`, compressed, signed; with `--layout` Finder
   sets the window up over `assets/dmg-background.tiff` (Splat's), which opens the image on screen and needs the
   Automation permission for Finder once.
5. The image mounted out of sight (`-nobrowse`) and checked: intact, the app signed and universal, the link in place.
6. Into the release folder: `Dango-<version>.dmg`, `Dango-<version>.md` with the English notes,
   `appcast.xml` (checked with `xmllint`) and the Homebrew Cask `dango.rb` with the image's checksum (checked with
   `ruby -c`).

Without `--notarize` it is a dry run into `target/release-dry-run/`: signed with the local certificate and without the
secure timestamp, no notarization, no stapling, the feed's EdDSA signature a placeholder with the image's real length,
VERSION unchanged. It asks Apple for nothing and the
keychain only for items' attributes, and prints what a notarized release still lacks.

Both runs check the translations before the long build: `cargo test -p ide-app --test translations` with
`IDE_I18N_RELEASE=1`, for all 35 languages whatever `IDE_I18N_LANGUAGES` says (E15). A missing, stale or doubled text
in any `Localizable.strings` stops the release; the test's output names each one.

`--notarize` stops before the long build unless everything is there, then signs with the first valid Developer ID
Application certificate, has Apple notarize the app (as a zip) and then the image, staples both tickets, has Gatekeeper
assess the image, signs it for Sparkle with the keychain's key and writes into `dist/` without changing VERSION. It
needs:
- `RELEASES` in `xtask/src/updates.rs`: the releases page in GitHub's layout; the image is downloaded from
  `<RELEASES>/download/v<version>/`, the feed from `<RELEASES>/latest/download/appcast.xml`.
- `HOMEPAGE` in `xtask/src/cask.rs`, for the Cask.
- The notarytool profile `notary`, as for Splat and Lowspeak:
  `xcrun notarytool store-credentials notary --apple-id <Apple ID> --team-id <team ID>`.
- The app's own EdDSA key for its updates, not Splat's and Lowspeak's: its private half in the login keychain under
  the account `local.agentide`, the neutral identifier, created once with
  `target/sparkle-2.10.1/bin/generate_keys --account local.agentide` (keep a backup with `-x <file>`), and the public
  half it prints in `UPDATE_PUBLIC_KEY` in `crates/ide-product/src/lib.rs` (E51). Without them the run stops and
  names these steps; a dry run puts a placeholder into `SUPublicEDKey`.
- `RELEASE_NOTES.md` and `assets/readme/<language>/RELEASE_NOTES.md` for every other language in `resources/`, new
  for the release and reviewed: notes that still begin with the comment `<!-- DRAFT` are refused, and the run names every language
  whose notes are missing. A dry run leaves the draft comment out of the feed and lists what is missing.

`make deploy` adds an identical `Dango.dmg` alias for the stable README download link and uploads the image, alias,
notes, feed, Cask and deployment manifest. Only adding the Cask to a tap stays manual. `dist/` is ignored by git;
the disk images are GitHub release assets, not source commits.

## Selftest
The app built with `--features selftest` runs scenarios without a person at the Mac. Every scenario is a row of
`SCENARIOS` in `src/selftest/scenarios.rs`: its name, entry, time limit, whether it is offline, how its launches end,
its launch arguments and whether it needs dictation. The runner asks the app for this table with
`<bundle>/Contents/MacOS/Dango --list-scenarios`, which prints one JSON object a line and ends; it keeps no list of its
own. `cargo xtask selftest` and the app agree on this:

1. The runner starts `<bundle>/Contents/MacOS/Dango` directly, once per scenario, with stdin from `/dev/null`, stdout
   on the terminal and stderr passed on to the terminal line by line, and with these variables:
   - `IDE_SELFTEST`: the scenario's name, made of letters, digits, `-` and `_`.
   - `IDE_SELFTEST_REPORT`: where the app writes its report, `target/selftest/<scenario>.json`; the runner deletes the
     file of an earlier run first.
   - `IDE_SELFTEST_HOME`: a new, empty folder in the temporary folder, only the user can enter it. The app keeps
     whatever it would keep in the user's home there, its defaults included: the standard defaults become the domain
     `<bundle id>.run` in `Library/Preferences` of that folder (the private
     `-[NSUserDefaults _initWithSuiteName:container:]` and `+setStandardUserDefaults:`), since the app's own domain
     with its window frames and divider positions is shared by every selftest app on this Mac, those of runs at the
     same time included. Web pages keep their data in memory and secrets stay in memory too.

   Variables of the runner's own environment reach the app as well, among them `IDE_SELFTEST_NO_PICTURES`, which
   skips every picture, for runs that only want the checks.
2. The app runs the scenario, writes the report and exits with status 0:
   ```json
   {"scenario": "probe", "ending": "ran", "checks": [{"name": "window opens", "ok": true, "detail": "one window, 1200×800"}]}
   ```
   It should write the file whole, to a temporary name and then renamed, since a cut-off report counts as unreadable.
   `ending` says how the scenario ended: `"ran"` to its end through `finish`, `{"skipped": "…"}` when `skip` ended it
   early on purpose, such as without a language server, or `{"gave_up": "…"}` when `give_up` ended it because
   something it needed was missing; `give_up` also records that as a failed check with the file and line it gave up
   at. `cargo xtask rules` refuses a `finish` after `return`, inside a branch or before more code, also through a
   function that ends with `finish`, so that only `give_up` and `skip` end a scenario early; a loop ends by falling
   through to its `finish`.
3. A scenario passes when the app ends by itself within its time limit with status 0 and its report is readable,
   names the scenario, says how it ended, did not give up, lists at least one check and every check is `ok`, and its
   stderr holds no failed TextKit 2
   assertion: UIFoundation logs one as `Make a symbolic breakpoint at NSText…` and carries on. The runner prints
   each check as `✓ name — detail` or `✗ name — detail`. At the limit of the scenario's row it kills the app: 120 seconds by default, 300 for `a11y` and `a11y-rtl`, and 600
   for the language server scenarios, which wait for a real rust-analyzer or gopls to index and settle before each
   step (up to 180 s each, the wait in the check's detail) and end themselves after 540.
4. The runner exits with an error if any scenario did not pass, and names it. A failed scenario's home folder stays
   for a look; a passed one's is removed.
5. A scenario whose row has `Launches::Crash`, `recovery` so far, runs twice in the same home. The
   first launch gets `IDE_SELFTEST_LAUNCH=crash` and the report file `<scenario>-crash.json`; it prepares what a crash
   should leave behind, writes its report and kills itself with `SIGKILL`, so that none of its quitting code runs. It
   passes when it ends by that signal with a report whose checks all hold. Only then does the second launch run, as
   any other scenario, and checks what came back.
6. A selftest never takes the keyboard from the person at the Mac, who keeps working while agents run selftests.
   AppKit makes an app the active one whenever it begins an alert as a sheet: `-[NSAlert
   beginSheetModalForWindow:completionHandler:]` calls `-[NSApplication activateIgnoringOtherApps:]`, and that works
   even while another app is in front. It also tries once at launch (`_doOpenUntitled`). With `IDE_SELFTEST` set, the
   shared application is therefore an `IDESelftestApplication` (`src/selftest/application.rs`), whose `activate`,
   `activateIgnoringOtherApps:` and private `_activateWithInfo:` do nothing, and so does
   `accessibilitySetFrontmostAttribute:`, through which assistive apps and AppleScript's `set frontmost` would
   activate it by way of `NSRunningApplication`. A click into a window of an inactive app makes the window server
   activate it before the app sees the click, so every window lets clicks through to what lies behind it: the app sets
   `ignoresMouseEvents` on each window when AppKit announces it is about to show
   (`NSWindowWillOrderOnScreenNotification`, posted for panels, sheets and popovers too). The activation policy
   stays regular, since a background-only app runs at a lower priority and draws more slowly; choosing the app in
   the Dock or with ⌘-Tab still activates it. The runner watches each launch through Launch Services
   (`lsappinfo listen`). If the app becomes the frontmost app anyway, for instance through `open -a`, the Dock or
   ⌘-Tab, the runner kills it at once and the scenario fails, naming the app it took the keyboard from. To find out
   how it happened, `log show` around that moment lists `SETFRONT` from `launchservicesd` and, from `WindowServer`,
   `Making … the front process` right after what caused it, such as a trackpad's button event.

   As in any inactive app, windows show and views become first responder, but no window is key. A scenario must
   therefore not depend on `isKeyWindow`. It sends its keys and clicks to the view or window itself (`keyDown:`,
   `performKeyEquivalent:`, `insertText:`, `mouseDown:`, `performClick:`), never through `CGEventPost` or anything
   else that goes through the window server.

Pictures go beside the report as `<name>.png`. A window's picture is a real capture by `screencapture`, which asks
the system's capture service; with several selftest runs capturing at the same time, each capture there took up to
ten seconds. A run therefore holds `<temporary folder>/<BUNDLE_ID>.selftest-capture.lock` while it captures, and gives a capture 15
seconds before it draws the picture itself.

Alerts and panels that a scenario answers itself, such as the question at a quit with unsaved text, are answered as
soon as they show by `when_alert_shows` of `src/selftest/helpers.rs` and the answerers built on it, which look every
50 ms from timers of the run loop for 20 s: an alert opened inside a callback of the main dispatch queue holds up
every other such callback until it closes, `main_after` included. Each alert goes to the call that began to wait
first among those it suits, so answers set up one after another answer alerts one after another; `look_at_alert`,
for a picture, leaves the alert to the next one.

A step that depends on a state waits for it with `wait_until`, `wait_for`, `wait_for_window` or `wait_for_editor` of
`src/selftest/helpers.rs`, which check every 50 ms and give up after 20 s (`wait_within` for another limit); a fixed
delay through `helpers::after` is only for what has to elapse, such as an animation or a check that nothing happens,
and is a named constant that says why. `helpers::open_project` makes, trusts and opens a scenario's folder.

### Accepted pictures
Some pictures have accepted ones in `tests/snapshots/<scenario>/<light|dark>/<picture>.png`, by the appearance the
report names. After a scenario passed, the runner compares each accepted picture with the run's: a pixel counts as
changed when a channel strays by more than 48 of 255, and a picture differs when more than 1 % of its pixels changed,
so font smoothing and the desktop showing through glass pass. A picture at another pixel size, from a display of
another scale, is scaled first. A difference fails the scenario and leaves `<picture>.diff.png` beside the report: the
accepted picture faded, changed pixels red. A scenario or appearance without accepted pictures compares nothing.
`cargo xtask selftest-accept <scenario> <picture>…` copies pictures of the last run there; without picture names it
renews those already accepted. Accepted so far: `probe-window`, `probe-settings` and `probe-markdown` of `probe`, light.

### Accessibility audit
The `a11y` scenario is the VoiceOver check of each phase (E3). It opens the main states with fixtures, recordings and
stand-ins and walks what VoiceOver gets from every window: controls without a name, images without a description,
names that say nothing (an SF Symbol's name, "Button") or repeat beside each other, roles that do not fit, controls
Tab never reaches, and code and terminal views that turn with a right-to-left interface. Then it goes through the
main tasks with key presses to the window alone: staging and commit, Open Quickly, ⌘T and the menu of new tabs, the
document beside, a prompt, comments on a
plan in its card and its document, the activity popover as its rows change, opening a folder. Each finding is
reported once, in the first state that shows it, and is a line of `target/selftest/a11y-findings.txt`:
state, problem, element, path; the window pictures are `target/selftest/a11y-*.png`.

- `a11y` runs in the Mac's language, left to right; `a11y-rtl` runs the same in Arabic, right to left, with its own
  findings (`a11y-rtl-findings.txt`) and pictures (`a11y-rtl-*.png`), and checks besides that code and terminal views
  stay left to right. `make test` runs both. The runner starts `a11y-rtl` with `-AppleLanguages (ar) -AppleTextDirection
  YES`. macOS turns an app right to left only with `AppleTextDirection` on, which it writes itself for a right-to-left
  language of the Mac or of the app (System Settings, Language & Region, Applications); the language alone, as an
  argument, keeps the app left to right. Texts still missing in `ar.lproj` show in German.
- The selftest application answers full keyboard access on for both, whatever the Mac is set to; launch
  arguments do not reach it. AppKit's buttons take the focus only with the Mac's own full keyboard access, so with it
  off a button counts as reached where the key view loop links it.
- An element's properties are read through the NSAccessibility methods first: a view's `setAccessibilityLabel:` is
  invisible to the older `accessibilityAttributeValue:`, which AppKit's cells answer.
- VoiceOver reads a control (button, pop-up, text field, image view) through its cell, which does not see what is set
  on the control: name, hide and label controls with `util::set_label`, `util::hide_from_voiceover` and
  `util::set_title_element`, which set both.
- Menus track their own keys, which reach only the active app: the ⌘T flow takes the item while the menu is open and
  sends its action once it closed. An alert begun while the app is not active takes Return off its default button;
  Return to a sheet presses the button the sheet names as default.

## Integration
`make test-integration` checks the app's parts against the real tools and services of this Mac with the owner's
sign-ins, which `make check` leaves out. Each step needs something: a program on the login shell's PATH, a sign-in, a
reachable host. Without it the step is skipped with the reason, never failed; the run fails only when a step that ran
failed. A test can also skip itself by printing `integration skip: <why>`, and a scenario whose every check says
`skipped: …` counts as skipped. `make test-integration ARGS=--list` shows the plan without running anything,
`ARGS="--only lsp,git"` runs only those groups.

The groups `claude`, `paste` and `acp` start coding agents signed in to the owner's paid accounts (R11, E53), and
`copilot` uses the owner's Copilot account. A bare `make test-integration` runs only `lsp`, `git`, `ci` and `predict`
and names the paid groups it left out; a paid group runs only
when `--only` names it, and `--paid` adds all of them. The run then prints which paid groups it starts and goes on
without asking, so it also works unattended.

| Group | What runs | Accounts |
|---|---|---|
| `lsp` | the ignored tests of `ide-lsp/tests/real_servers.rs`: gopls, rust-analyzer, TypeScript 7 through `tsc --lsp`, TypeScript 5 through vtsls, installing rust-analyzer and gopls; then `selftest lsp`, `lsp-actions` (rust-analyzer), `lsp-actions-go` (gopls) and `lsp-navigation` (rust-analyzer on this repository) | none; downloads from npm, GitHub and the Go proxy |
| `git` | checkpoints restored around an edit of the user's, in a repository and a plain folder, with the git, HOME and global configuration of the login shell | none |
| `ci` | `selftest ci-live` against a clone on github.com and one on a self-managed GitLab | none paid; reads with the token the app's CI finds through gh or glab or the login shell |
| `claude` | the ignored real-CLI test of `ide-claude` and `selftest agent-live` | Claude: two short turns with haiku |
| `paste` | `selftest paste-cli`: an image pasted into claude, codex and agy, without a prompt (Gemini CLI is no longer offered, E67) | the sign-ins of the three CLIs |
| `acp` | `ide-acp/tests/live.rs`: gemini, agy and codex-acp, each from the PATH and as the preset pins it, against one tiny prompt | Google and OpenAI accounts, a few tokens each |
| `predict` | `ide-predict/tests/real_providers.rs`: a fill-in-the-middle proposal and a cancel with a real model under Ollama (`IDE_INTEGRATION_OLLAMA_MODEL`, else `qwen2.5-coder:1.5b`) and under LM Studio's server (`IDE_INTEGRATION_LMSTUDIO_MODEL`, else `qwen2.5-coder-1.5b-instruct`) | none; nothing leaves the Mac |
| `copilot` | the same file: copilot-language-server, from the tools folder or installed with npm, proposes a completion for a made-up file; only once the user signed in to Copilot | Copilot: two completions |

The clones `ci` reads are `IDE_INTEGRATION_GITHUB_REPO`, else `~/Repos/splat`, and `IDE_INTEGRATION_GITLAB_REPO`, else
the first clone in `~/Repos` whose origin is on a self-managed GitLab the app finds a token for. `claude` takes the CLI
from `CLAUDE_BIN`, else `~/.local/bin/claude`.

The `acp` steps put `acp-record` between the client and the agent and leave each session as
`target/integration/acp/<agent>-<path|pinned>.ndjson`, one message per line as
`{"to": "agent" | "client", "message": …}`, for replay tests; a recording becomes a fixture only by copying it into
the repository. A preset's `verified` flag (E35) is set by hand, and only after its pinned step passed here with the
build at hand.

## Newest Claude CLI
The app speaks to `claude` over a protocol without a version guarantee (R7, E21). `cargo xtask claude-latest`
(`xtask/src/claude_latest.rs`) installs `@anthropic-ai/claude-code@latest` with npm into `target/claude-latest/`,
emptied first and never on the PATH, and runs ignored tests of `crates/ide-claude/tests/session.rs` against it with
`CLAUDE_BIN`:
- `the_cli_without_a_sign_in_keeps_the_recorded_contract`, always: the CLI starts with the app's whole command line in
  a home of its own with only PATH from the environment, so it finds no sign-in; it must answer `initialize` with its
  version, models and the account `tokenSource: none`, start the session and refuse the prompt as an authentication
  error, as the fixture `recorded_signed_out` does. No account, no cost; it needs Node.js and the npm registry.
- `the_real_cli_answers_a_prompt`, only with `--signed-in`: one short turn with haiku, with this Mac's sign-in or
  `CLAUDE_CODE_OAUTH_TOKEN` / `ANTHROPIC_API_KEY` in the environment; costs a few tokens.

It prints the newest version beside the one the fixtures were recorded with. When it fails, the CLI changed a part of
the contract: compare its frames with docs/claude-mapping.md, record new fixtures as described there, and adjust the
mapping.

`.github/workflows/claude-latest.yml` runs it every night on a GitHub-hosted macOS runner, with the sibling
repositories `splat` and `lowspeak` checked out next to this one from their public `main`. The default part needs no
secret. It is off until the owner enables it:
- Scheduled runs need the repository variable `CLAUDE_NIGHTLY` set to `true` (Settings › Secrets and variables ›
  Actions › Variables); "Run workflow" on the Actions page starts one at any time. macOS minutes of a private
  repository count against the account's Actions quota.
- The paid turn runs only with the secret `CLAUDE_CODE_OAUTH_TOKEN` (from `claude setup-token`) and either the
  workflow's input ticked or the variable `CLAUDE_NIGHTLY_SIGNED_IN` set to `true`.
- The checkouts of `splat` and `lowspeak` take what is pushed; local commits there that this repository needs must be
  pushed first.

## Markdown documents
What a Markdown document offers beyond the code editor (E59) lives in `src/markdown/`: the format commands
(`format.rs`, splat's `splat_markdown::commands` as source edits, each through `editor::undo_step`), pasting rich text
(`paste.rs`, `splat_import::paste_as_markdown`), the exports (`export/`) and opening in Splat (`splat_app.rs`). All
menu commands are the editor's one action `ideMarkdown:`, told apart by the item's tag; the editor calls in at four
places only: the action, its validation, `paste:` and the opening and closing of a tab. The automatic preview goes
through `preview::open_beside` and `preview::close_beside`.

An export never holds up the window. It waits up to 20 s for the formulas and diagrams the renderer has not drawn
yet, asking for them as a document of its own, and builds its file on a thread of its own: HTML through
`splat_markdown::html::to_html`, Word through `splat_markdown::docx::to_docx`, image files read and converted there.
It comes to the main thread only for AppKit's parts, each a short step: the drawn formulas and diagrams, pictures from
the web, and for a PDF the text set as the preview sets it for paper, 20 blocks a turn, after the pictures of the
folder were read as the preview reads them. The PDF's pages are laid out and drawn into the file on the export's
thread again (a TextKit 1 layout manager into a `CGPDFContext`, in the light appearance). A sheet on the window says
what runs and offers "Abbrechen", which stops the export without a file; a failure stays in the sheet until OK. Local
pictures go along only from the document's folder or its repository, as in splat; pictures from the web only from a
trusted folder (E45). The selftest scenario `markdown` covers the automatic preview, the format toggles, a paste of
HTML from a private pasteboard, the three exports, a long PDF export during which the main thread keeps turning,
cancelling and a failure in the sheet; it leaves the long PDF as `target/selftest/markdown-export.pdf`.

## Crash recovery
What the windows show is written to the state file a second after each change of windows or tabs, under
`openWindows`: per window its folder, its editor and welcome tabs and the one in front. Each editor with unsaved text
keeps a copy under `<data folder>/recovery/`, written on a lane of its own two seconds after typing rests and at most
ten seconds into steady typing, and removed once the text is saved or given up, at the latest at a quit the user
agreed to; a window close or quit the user cancels writes the copies again. At launch the windows come back
with their tabs if `reopenProjects` is set; the copies come back in any case, each in its window, unsaved, with a note
per window. A copy of a file that changed on disk since gets the bar described below. A text whose description a crash
kept from being written comes back untitled; a copy that cannot be read moves to `recovery/damaged/` and is logged.

## Window tabs
Every document is a native macOS tab (E60): `window::tabs::TabWindow` is an `NSWindow` with its own toolbar and split
view, and a folder's window (`WindowController`, a plain object that is the delegate of all its tab windows) keeps them
in one tab group under a tabbing identifier of its own, so "Merge All Windows" never mixes folders. The tab bar shows
even with one tab; each tab's title starts with an icon, drawn as a text attachment of its attributed title
(public `NSWindowTab` API): a file the icon of its type (`file_icon::image`), any other document a symbol
in its agent's or worktree's colour. The browser shows a globe, not the page's icon, which would take a fetch beside
WebKit's own. The sidebar exists once per folder and moves into the tab in front, held at the width it has in the others
until the new tab is laid out; a window of the same folder left on screen beside it, after a tab was dragged out, shows
a picture of the sidebar until it comes to the front itself; so does every tab behind, for the overview of all tabs.
`controller.window()` is the tab in front, where sheets and the keyboard focus go. A tab shows one document or two side
by side (`open_beside`); with two, each gets a header with its name and a close button, docked above the documents on
macOS 26 and later, where the bars of the title and tabs fade what lies right below them. "+" and ⌘T open the page of a
new tab (`window::chooser`), which the next document takes the place of. Closing a tab's last document closes the tab,
and the last tab the window; ⇧⌘W asks about every document first, so one that refuses keeps all tabs, and keeps the
layout of all tabs before closing them. The layout file (E24) keeps the tabs in the order of the tab bar with their
documents and the left one's share; version 1 files with panels are read as one tab per panel tab. The `panels` and
`tabs` scenarios cover this.

## Opening a folder
A folder opened while the window in front shows another one replaces that folder (E56): the window saves the old
folder's layout, closes its documents and all tabs but the one in front, ends their terminals and agent sessions and
its language servers, and opens the new folder with its own layout. Unsaved text, a file gone from disk, an agent
turn, subagent or background task that runs, or a terminal whose foreground is not its shell, in any tab, first gets
a sheet; "Alles schließen" gives the text up and ends the programs, "In neuem Fenster öffnen" leaves the window as it
is. A window that waits for the answer to a sheet keeps its folder, and the new one gets a window of its own. A folder
open in another window brings that window forward; of several folders handed to the app at once, the first replaces
and the others open beside. The `replace-folder` scenario goes through each case.

## Changes on disk
An editor without unsaved text takes a change of its file at once. One with unsaved text gets a bar above the text
instead, which stays while typing goes on and follows further changes of the file:
- "Unterschiede zeigen" opens a tab with the differences between the file and the editor.
- "Zusammenführen" merges the file's changes into the editor's text against the version both started from
  (`ide_document::merge`). A clean merge is one undo step and leaves the text unsaved; overlapping changes come in
  between Git's conflict markers, and the bar counts them and goes to the next or previous one until none is left.
- "Version von der Platte laden" takes the file's version, "Meine behalten" keeps the editor's against it.

A deleted file gets a bar with "Sichern", which writes it anew, and "Schließen", which asks first, since the editor may
hold the file's last copy: "Sichern" writes it anew, "Verwerfen" lets it go, "Abbrechen" (Return) keeps the editor
(E43). ⌘W, the tab's close button and closing the window ask the same; a quit asks it for each such editor before the
one question about the other unsaved changes.

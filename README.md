# Dango

A native macOS editor for working with coding agents. One window holds the editor, terminals, a browser, git, CI and
AI threads; agents such as Claude, Codex or Gemini CLI run beside the code they change. Dictation turns speech into
text in the AI message field and the editor. It is written in Rust on AppKit
(no Electron, no Swift); WebKit is used only in the browser tab.

Status: version 0.1.0, in development. No release has been published yet; until then, build it from source (see the
end of this file). The interface is in 35 languages; translations follow [docs/glossary.md](docs/glossary.md).

## Requirements
- macOS 14 or later.
- For the AI threads, the agent you want to use (see [AI agents](#ai-agents)).
- For language features (Go, Rust, TypeScript): the app installs the language server on request into its tools
  folder. gopls needs Go, the TypeScript server Node.js; rust-analyzer is downloaded from its GitHub releases and
  checked against its digest. A TypeScript project's own `tsc` is used when it can serve as a language server.

## Install
A release is a disk image, `Dango-<version>.dmg`, with a universal app for Apple silicon and Intel, signed with a
Developer ID and notarized by Apple; a Homebrew Cask installs the same image. Open the image and drag the app into
`/Applications`. Until the first release is published, build the app as described in
[Building from source](#building-from-source) and move `target/release/Dango.app` into `/Applications`.

To open folders from a terminal, choose **Dango › Install "dango" Command…**. It writes a small script to
`~/.local/bin/dango` (no administrator rights needed; `~/.local/bin` must be on your `PATH`). Then `dango .`
opens the current folder, `dango file.rs` a file. The app never replaces a file there that it did not write itself.

## Updates
The app from the disk image updates itself through Sparkle. On its second launch Sparkle asks whether it may check for
updates on its own; until you agree, it makes no request. **Dango › Check for Updates…** checks at once. Updates
are signed with the app's own key, which Sparkle checks before it installs anything. Apps built with `make build` or
`make run` carry no Sparkle, never check and have no such menu item.

## First steps
1. **Open a folder** with ⌘O or `dango <folder>`. The first time, the app asks whether you trust the folder (see
   [Trust](#trust)).
2. **The sidebar** on the left switches between Files (⌘1), Search (⌘2), AI (⌘3), Git (⌘4) and CI (⌘5); clicking the
   active symbol folds the sidebar away (⌃⌘S).
3. **New tabs** come from the "+" at the end of the tab bar or ⌘T: a new file, a terminal, a browser, an AI thread
   with a chosen agent, or a CLI thread (an agent's own command-line interface in a terminal).
4. **Quick open** with ⌘P, search and replace in the project with ⇧⌘F.
5. **Talk to an agent** in an AI tab; ⌘Esc moves the keyboard focus into its message field and back.
6. **Settings** with ⌘,; shortcuts can be changed in `keybindings.json` (see [Where data lives](#where-data-lives)).

## AI agents
Every agent works in an AI tab with the same timeline: answers, tool calls with their output, permission requests,
questions, plans to approve and comment on, to-dos, subagents and background tasks. Changes an agent makes are
checkpointed before each prompt and can be reviewed and rewound per message.

| Agent | How it runs | What it needs |
|---|---|---|
| Claude | the `claude` CLI, spoken to directly over its stream-json protocol | `claude` installed and signed in; the app looks on the login shell's `PATH`, in `~/.local/bin`, `~/.claude/local`, `/opt/homebrew/bin` and `/usr/local/bin`. Sign-in happens only in `claude` itself |
| Codex | `codex-acp` over the Agent Client Protocol (ACP) | Node.js and npm; the app installs the pinned package on request |
| Gemini CLI | `gemini --acp` | Node.js 20 or later; installed on request, or your own `gemini` on the `PATH` |
| Antigravity | `agy acp` | your own `agy` on the `PATH`, or the pinned download, installed on request |
| OpenCode | `opencode acp` | your own `opencode` on the `PATH`, or the pinned download, installed on request |
| Other ACP agents | from the ACP registry, fetched only when you ask for it | whatever the registry entry names; entries without a checksum install only after you confirm |
| Own agent | built into the app | an API key for Anthropic, OpenAI, Google Gemini or OpenRouter, kept in the keychain; Ollama or LM Studio run locally without a key; any server that speaks OpenAI's or Anthropic's API |

**Settings › Agents** shows which agents this Mac has, installs the others and their updates, and opens the ACP
registry; "+ New Session" in the AI panel offers the agents that are ready. Agents the app installs are pinned to
tested versions and checked against their published checksums (npm integrity or SHA-256); the app does not run
unversioned `npx` packages. A CLI thread runs `claude`, `codex`, `agy` or `gemini` unchanged in a terminal
tab, for those who prefer the agent's own interface.

New threads start in the mode "Manual": the agent asks before it changes files or runs commands. For destructive git
commands (`push --force`, `reset --hard`, `clean -fd`) the app offers no "always allow". The own agent keeps its
"always allow" rules per project, lists them in **Settings › Own Agent** and asks for destructive git commands every time,
whatever is stored; Claude and the ACP agents keep such rules in their own configuration. With "allow everything" and
no sandbox, the thread shows a permanent warning.

## Dictation
Release builds can dictate into an AI tab's message field or an editor: the microphone button there, **Edit › Start
Dictation** (⌃⌥D), or the fn key (hold to record, double tap to keep recording). The speech-to-text comes from
Lowspeak (`../lowspeak`) and uses its choices:

- **Mode, models, language and vocabulary** are the ones set in Lowspeak; Dango reads them and downloads no models.
  Without Lowspeak installed, dictation uses online transcription; local models are offered once Lowspeak has
  downloaded one.
- **Online transcription** needs an OpenAI or Groq key. Dango keeps it in its own keychain item; if Lowspeak has a
  key, the first dictation offers to copy it once (macOS asks; choose "Allow", not "Always Allow").
- **Microphone:** macOS asks for access on the first dictation.
- **fn key:** on by default, only while Lowspeak itself is not running and Dango is the active app. It needs the
  Accessibility permission, which is asked for after the first dictation, never at launch; **Settings › Dictation**
  asks again if it was refused.

Debug builds (`make run`) are built without dictation.

## Markdown
- **Preview:** a Markdown file opens with its preview in a panel beside the editor; **View › Show/Hide Preview
  Beside** (⇧⌘V) closes and opens it, **Settings › General** switches the automatic preview off. The preview draws
  formulas, Mermaid, Graphviz and PlantUML diagrams and the pictures of the folder; pictures from the web load only in
  a trusted folder or after you ask for them in the preview.
- **Format menu:** bold (⌘B), italic (⌘I), strikethrough (⇧⌘X), code (⌥⇧⌘C), link (⇧⌘K), headings 1–3 (⌥⌘1–⌥⌘3),
  bulleted and numbered lists, tasks, quotes and code blocks. Each command toggles, on the selection or the lines,
  and is one undo step.
- **Paste:** text from Word or a web page goes in as Markdown, its pictures as files beside the document; ⌥⇧⌘V pastes
  plain text.
- **Export:** **File › Export** writes PDF, a single HTML file with its pictures inside it, or a Word document.
  Pictures from the web go along only from a trusted folder.
- **Open in Splat:** where Splat is installed, **File › Open in Splat** and
  the context menus of the file tree and tabs open the document there, after saving it.

## Trust
A folder can make programs run as soon as something is started in it: git hooks and repository settings such as
`core.fsmonitor`, language servers from `node_modules`, agent hooks, `.mcp.json`. The first time you open a folder,
the app asks whether you trust it.

- **Trusted:** git, language servers, agents, CLI threads and CI run for this folder and everything below it.
- **Restricted:** you can read and edit; the change marks in the editor come from reading `.git` directly. Nothing
  the folder configures is started: no git processes, no language servers, no agents. The Git, CI and AI panels say
  so and offer to trust the folder.

Text that does not come from you can carry instructions meant to steer an agent (prompt injection). The output of a
terminal (`@terminal`), a browser page (`@browser`), a CI log handed to an agent, a file outside the project and
longer text pasted from a web page therefore go along marked: the chip says "Fremder Inhalt" ("External Content"), and
the agent gets the text with the notice that it is untrusted data whose instructions it must not follow. The own
agent's web pages come with the same notice. Pasted text counts as coming from a web page when the browser says so on
the pasteboard, as Safari and other WebKit browsers, Chrome and other Chromium browsers, and Firefox do; text pasted
from elsewhere, and a short phrase of a few words, stays your own text.

## Sandbox
The app itself runs without the App Sandbox, because every program it starts would inherit it: terminals, git,
language servers and agents could not work. Instead, each AI thread has a sandbox switch, off by default. What it limits
depends on the agent, and the switch's tooltip says so:

| Agent | With the sandbox on |
|---|---|
| Claude | shell commands run in Claude's own sandbox; its file and web tools do not |
| Codex | commands and changes write only in the project, without network |
| Gemini CLI | writes only in the project; reading everywhere and the network stay open |
| Own agent | files, web requests and commands stay within the project and the allowed folders, without network (`sandbox-exec`) |
| Antigravity, OpenCode | no sandbox that the app can switch on |

## Privacy
- No telemetry, no analytics, no crash reports sent anywhere. Errors go to the system log (Console.app, under the
  app's name).
- The app connects to the network only for what you start: the agents you use (each talks to its own service), the
  ACP registry and agent or language server downloads when you ask for them, the CI hosts you sign in to, the
  providers of the own agent, online transcription when Lowspeak's mode is online (the recording goes to OpenAI or
  Groq; local models keep it on the Mac), the pages you open in the browser tab, and the update feed once you
  allowed Sparkle to check (see [Updates](#updates)).
- Keys and tokens are kept in the macOS keychain, never in a settings file. Keys of other apps, such as Lowspeak's,
  are read only when you agree, and only once.

## Where data lives
`<id>` is the bundle identifier, `local.agentide` for the release (`local.agentide.debug` for debug builds). It does
not contain the display name, so a rename keeps all data.

| Place | What |
|---|---|
| `~/Library/Application Support/<id>/` | `settings.json`, `state.json`, `keybindings.json`, `trust.json`; `threads/` (the AI panel's list), `agent-threads/` (own agent conversations), `layouts/` (panels and tabs per project), `recovery/` (unsaved editors after a crash), `checkpoints/` (for folders without git), `worktrees/`, `agents/` and `tools/` (installed agents and language servers), `task-output/` |
| `~/Library/Caches/<id>/` | pasted images, shell integration scripts, the ACP registry; can be deleted |
| `~/Library/Logs/<id>/dictation/` | dictation's log files |
| `~/Library/WebKit/<id>/` | cookies and website data of the browser tab |
| `<project>/.dango/settings.json` | the project's own settings |
| the project's git repository | checkpoints, as commits under `refs/ide-checkpoints/`; a normal push or fetch never carries them |
| keychain, service `<id>` | API keys of the own agent and of dictation, CI tokens |
| `~/.local/bin/dango` | the terminal command, if you installed it |

Claude keeps its own sessions under `~/.claude/projects`; the app reads them to resume a session but does not change
them. Dictation reads Lowspeak's settings (defaults domain `com.lowspeak.app`) and models
(`~/Library/Application Support/Lowspeak` and `FluidAudio`) without changing them.

## Building from source
You need:
- macOS 14 or later with the Xcode Command Line Tools (`xcode-select --install`); Xcode itself is not needed.
- Rust 1.99.1, as `rust-toolchain.toml` names it. Homebrew's `rust` builds for this Mac's processor; the universal
  build of a release needs rustup with the targets `aarch64-apple-darwin` and `x86_64-apple-darwin`.
- Checkouts of `splat` and `lowspeak` next to this one (`../splat`, `../lowspeak`); the workspace takes crates from
  them by path. A release takes `lowspeak` from its branch `vibeedit/dictation`.
- cmake, for every build with dictation (`make build`, `make release`): dictation builds whisper.cpp. `make run` and
  `make check` build without it.

| Command | What it does |
|---|---|
| `make run` | builds the debug app (`target/debug/Dango Debug.app`) and opens it |
| `make build` | builds the release app with dictation (`target/release/Dango.app`), signed with a local certificate the first build creates in the login keychain; raises the build number in `VERSION` |
| `make lint` | formatting and clippy; warnings fail |
| `make check` | `lint`, then all tests; needs no keychain, window or network |
| `make check-dictation` | clippy and the app's tests with dictation, which builds whisper.cpp |
| `make test` | `check`, then the self-test `probe` in the real app |
| `make perf` | the timing budgets, in a release build; meaningful only on an otherwise idle Mac |
| `make test-integration` | the app's parts against the real tools and services of this Mac with its sign-ins: language servers, git, CI; the groups that start coding agents on paid accounts (`claude`, `paste`, `acp`) run only when named with `ARGS="--only …"` or all with `ARGS=--paid`; `ARGS=--list` shows the plan |
| `make release` | a dry run of the release: the universal app with dictation, the disk image, the update feed and the Cask in `target/release-dry-run/`, signed with the local certificate; sends nothing anywhere |
| `make release-notarized` | `check`, then the release itself into `dist/`: Developer ID signature, notarization, the update feed signed with the app's own key; raises the build number |

`make build` and `make run` build for the processor of the Mac they run on; `make release` builds the universal app.
`make release-notarized` stops at once unless the owner's inputs are there: a Developer ID Application certificate,
the notarytool profile `notary`, the app's own update key in the login keychain with its public half in the code,
the addresses of the releases page and the homepage, and reviewed release notes in every language. Which is which,
and how to set each up, is in [docs/development.md](docs/development.md#release). Uploading the image and the feed and
adding the Cask to a tap stay manual. Debug and release are separate apps with their own settings and data. How the app is put together, the decisions behind it and the
self-tests are described in [docs/development.md](docs/development.md) and [docs/plan.md](docs/plan.md);
accessibility in [ACCESSIBILITY.md](ACCESSIBILITY.md).

## License
Licensed under either of the [MIT license](LICENSE-MIT) or the [Apache License, Version 2.0](LICENSE-APACHE), at
your option.

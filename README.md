<p><img src="assets/readme-icon.png" alt="Dango app icon" width="147" height="147"></p>

# Dango

**The native macOS editor for working with coding agents.**

Dango puts the code, the agents that change it and everything around them into one window per folder: an editor with language servers and edit predictions, terminals, a browser, git, CI, a debugger and AI threads with Claude, Codex, Antigravity, OpenCode or an agent built into the app. Every agent works in the same timeline, asks before it changes files or runs commands, and leaves a checkpoint before each prompt, so you can review and rewind what it did message by message. Nothing a folder configures runs before you trust it, and text from outside reaches an agent marked as untrusted. Dictate into the editor and the message field, and the speech recognition can stay on your Mac. Dango is written in Rust on genuine AppKit controls, without Electron, speaks 35 languages, and has no telemetry.

[![Download Dango for macOS](https://img.shields.io/badge/Download-Dango_for_macOS-0A84FF?style=for-the-badge&logo=apple&logoColor=white)](https://github.com/sventmueller/dango/releases/latest/download/Dango.dmg)

**[Download Dango.dmg](https://github.com/sventmueller/dango/releases/latest/download/Dango.dmg)**: signed and notarized by Apple, for macOS 14 or later on Apple Silicon and Intel. The link becomes available with the first release.

**Found a bug?** [Open an issue](https://github.com/sventmueller/dango/issues/new) and say which agent and which kind of project it happened with.

[![Rust 2024 edition](https://img.shields.io/badge/Rust-2024_edition-000000?logo=rust&logoColor=white)](https://www.rust-lang.org) [![macOS 14 or later](https://img.shields.io/badge/macOS-14%2B-000000?logo=apple&logoColor=white)](https://www.apple.com/macos/) ![Universal: Apple Silicon and Intel](https://img.shields.io/badge/Universal-Apple_Silicon_%7C_Intel-333333?logo=apple&logoColor=white) [![AppKit via objc2](https://img.shields.io/badge/AppKit-objc2_0.6-147EFB?logo=apple&logoColor=white)](https://github.com/madsmtm/objc2) [![Agent Client Protocol](https://img.shields.io/badge/ACP-Agent_Client_Protocol-6E56CF)](https://agentclientprotocol.com) [![Language Server Protocol](https://img.shields.io/badge/LSP-3.17-0078D4)](https://microsoft.github.io/language-server-protocol/) [![Debug Adapter Protocol](https://img.shields.io/badge/DAP-debugger-0078D4)](https://microsoft.github.io/debug-adapter-protocol/) [![Terminal: Tapa term-vt](https://img.shields.io/badge/terminal-Tapa_term--vt-F46D01)](https://github.com/sventmueller/tapa) ![35 interface languages](https://img.shields.io/badge/languages-35-4C9A2A) ![No telemetry](https://img.shields.io/badge/telemetry-none-4C9A2A) ![Signed with Hardened Runtime](https://img.shields.io/badge/Hardened_Runtime-signed-4C9A2A) [![License: MIT OR Apache-2.0](https://img.shields.io/badge/license-MIT_OR_Apache--2.0-4C9A2A)](#license)

[Bahasa Indonesia](assets/readme/id/README.md) · [Bahasa Melayu](assets/readme/ms/README.md) · [Català](assets/readme/ca/README.md) · [Čeština](assets/readme/cs/README.md) · [Dansk](assets/readme/da/README.md) · [Deutsch](assets/readme/de/README.md) · **English** · [Español](assets/readme/es/README.md) · [Français](assets/readme/fr/README.md) · [Hrvatski](assets/readme/hr/README.md) · [Italiano](assets/readme/it/README.md) · [Magyar](assets/readme/hu/README.md) · [Nederlands](assets/readme/nl/README.md) · [Norsk](assets/readme/no/README.md) · [Polski](assets/readme/pl/README.md) · [Português (Brasil)](assets/readme/pt_BR/README.md) · [Português (Portugal)](assets/readme/pt_PT/README.md) · [Română](assets/readme/ro/README.md) · [Slovenčina](assets/readme/sk/README.md) · [Slovenščina](assets/readme/sl/README.md) · [Suomi](assets/readme/fi/README.md) · [Svenska](assets/readme/sv/README.md) · [Tiếng Việt](assets/readme/vi/README.md) · [Türkçe](assets/readme/tr/README.md) · [Ελληνικά](assets/readme/el/README.md) · [Русский](assets/readme/ru/README.md) · [Українська](assets/readme/uk/README.md) · [עברית](assets/readme/he/README.md)‎ · [العربية](assets/readme/ar/README.md)‎ · [हिन्दी](assets/readme/hi/README.md) · [ไทย](assets/readme/th/README.md) · [한국어](assets/readme/ko/README.md) · [日本語](assets/readme/ja/README.md) · [简体中文](assets/readme/zh-Hans/README.md) · [繁體中文](assets/readme/zh-Hant/README.md)

## Contents

- [Features at a glance](#features-at-a-glance)
- [Requirements](#requirements)
- [Installation](#installation)
- [Getting started](#getting-started)
- [AI agents](#ai-agents)
  - [The timeline](#the-timeline)
  - [Permissions and modes](#permissions-and-modes)
  - [Checkpoints and worktrees](#checkpoints-and-worktrees)
- [Editor](#editor)
- [Language servers](#language-servers)
- [Tests](#tests)
- [Debugger](#debugger)
- [Terminal](#terminal)
- [Browser](#browser)
- [Git](#git)
- [CI](#ci)
- [Containers](#containers)
- [Markdown](#markdown)
- [Dictation](#dictation)
- [Trust](#trust)
- [Sandbox](#sandbox)
- [Appearance and accessibility](#appearance-and-accessibility)
- [Languages](#languages)
- [Keyboard shortcuts](#keyboard-shortcuts)
- [Privacy and security](#privacy-and-security)
- [Where data lives](#where-data-lives)
- [Known limitations](#known-limitations)
- [Building from source](#building-from-source)
- [License](#license)

## Features at a glance

- **Every agent in one timeline.** Claude through its own `claude` command, Codex, Antigravity and OpenCode through the Agent Client Protocol, more from the ACP registry, and an agent built into the app for Anthropic, OpenAI, Google Gemini, OpenRouter, Ollama and LM Studio. Answers, tool calls, permission requests, plans, to-dos, subagents and background tasks look the same whichever agent runs.
- **Review and rewind.** Before each prompt Dango keeps a checkpoint. What an agent changed can be reviewed and rewound per message, and a thread can work in a git worktree of its own and hand its work over by merge or cherry-pick.
- **One window per folder.** Files, terminals, browser pages, AI threads, diffs and previews are macOS tabs of that window. The sidebar holds Files, Search, Agents, Git, Tests, Debug, CI and Containers.
- **A real code editor.** Syntax colours, several carets, folding, `.editorconfig`, language servers for 21 languages with completion, go to definition, references, rename, quick fixes and hierarchies, and edit predictions from GitHub Copilot or a local model.
- **Debugger, git and CI built in.** Breakpoints and stepping for Rust, C, C++, Swift, Go, JavaScript, TypeScript and Python; staging single hunks, branches, stashes, conflicts and a commit graph; runs, jobs and logs from GitHub Actions and GitLab CI.
- **Careful with your folders.** A folder you have not trusted starts nothing: no git processes, language servers, hooks or agents. Destructive git commands are never allowed for good, and commits are checked for secrets.
- **Dictation.** Speak into the editor and the message field, online through OpenAI or Groq or locally with Whisper and Parakeet.
- **Native to the bone.** AppKit windows, tabs, menus and toolbars, TextKit 2 for the editor, Metal for the terminal, WebKit only in the browser tab. Dark mode, Increase Contrast and VoiceOver work as in every Mac app.
- **In your language.** The interface comes in 35 languages: Arabic, Catalan, Chinese (Simplified), Chinese (Traditional), Croatian, Czech, Danish, Dutch, English, Finnish, French, German, Greek, Hebrew, Hindi, Hungarian, Indonesian, Italian, Japanese, Korean, Malay, Norwegian, Polish, Portuguese (Brazil), Portuguese (Portugal), Romanian, Russian, Slovak, Slovenian, Spanish, Swedish, Thai, Turkish, Ukrainian and Vietnamese.

## Requirements

- macOS 14 Sonoma or later, on Apple Silicon or Intel. Dango is developed and tested on macOS 27.
- For the AI threads, the agent you want to use; see [AI agents](#ai-agents).
- For language servers and debug adapters, which Dango installs when you ask: Go for gopls and `dlv`; Node.js for the TypeScript, Python, PHP, JSON, YAML, CSS, HTML and shell servers and the JavaScript debugger; the Xcode Command Line Tools for clangd, sourcekit-lsp and `lldb-dap`. rust-analyzer is downloaded from its GitHub releases.
- For edit predictions, a GitHub Copilot plan (the free one will do), or a fill-in-the-middle model in Ollama or LM Studio.

## Installation

**From the disk image.** Each release is a disk image, `Dango-<version>.dmg`, with one universal app for Apple Silicon and Intel, signed with a Developer ID and notarized by Apple. Open it and drag Dango onto the Applications folder. A Homebrew Cask installs the same image. No release is published yet.

**Updates.** Dango from the disk image keeps itself up to date with [Sparkle](https://sparkle-project.org). On the second launch it asks whether it may look for updates on its own; until you agree, it makes no request. **Dango › Check for Updates…** looks right away. Updates are signed with the app's own key, which Sparkle checks before it installs anything. A build of your own has no updater.

**From source.** See [Building from source](#building-from-source), then move `target/release/Dango.app` into `/Applications`.

**The terminal command.** **Dango › Install “dango” Command…** writes a small script to `~/.local/bin/dango`, without administrator rights; `~/.local/bin` must be on your `PATH`. Then `dango .` opens the current folder and `dango file.rs` a file. Dango never replaces a file there that it did not write itself.

## Getting started

1. **Open a folder** with ⌘O or `dango <folder>`. The first time, Dango asks whether you trust it (see [Trust](#trust)).
2. **The sidebar** switches between Files (⌘1), Search (⌘2), Agents (⌘3), Git (⌘4), Tests (⌘5), Debug (⌘6), CI (⌘7) and Containers (⌘8); a right-click hides the ones you do not use, and ⌃⌘S folds the sidebar away.
3. **Every document is a tab** of the folder's window, macOS's own tabs. The “+” at the end of the tab bar and ⌘T offer a new file, a terminal, the browser and each installed agent, also in a git worktree of its own; holding the “+” shows the same as a menu.
4. **Open Quickly** with ⌘P finds files, and with `#` the project's symbols; ⇧⌘F searches and replaces in the project.
5. **Talk to an agent** in an AI tab. ⌘Esc moves the keyboard focus into its message field and back.
6. **Settings** with ⌘, hold the tabs General, CI, Agents, Own Agent, Language server, Dictation and Prediction.

The toolbar holds back and forward through the places you edited (⌃- and ⌃⇧-), the branch, Open Quickly and the Activity overview, which lists everything that runs. **Open to the Side** in the context menu of a file shows a second document beside the first. After a restart, or a crash, every window comes back with its tabs, and editors with unsaved text come back with it. Images, audio, video, fonts and Office documents open in a tab through Quick Look, PDFs through PDFKit.

## AI agents

| Agent | How it runs | What it needs |
|---|---|---|
| Claude | the `claude` command, spoken to directly over its stream-json protocol | `claude` installed and signed in; Dango looks on the login shell's `PATH`, in `~/.local/bin`, `~/.claude/local`, `/opt/homebrew/bin` and `/usr/local/bin`. Signing in happens only in `claude` itself |
| Codex | `codex-acp` over the Agent Client Protocol (ACP) | Node.js and npm; Dango installs the pinned package when you ask |
| Antigravity | `agy acp` | your own `agy` on the `PATH`, or the pinned download, installed when you ask |
| OpenCode | `opencode acp` | your own `opencode` on the `PATH`, or the pinned download, installed when you ask |
| Other ACP agents | from the ACP registry, fetched only when you ask for it | whatever the registry entry names; entries without a checksum install only after you confirm |
| Own agent | built into the app, with tools for files, search, commands, diagnostics and web requests | an API key for Anthropic, OpenAI, Google Gemini or OpenRouter, kept in the keychain; Ollama or LM Studio on this Mac without a key; any server that speaks OpenAI's or Anthropic's API |

**Settings › Agents** shows which agents this Mac has, installs the others and their updates, and opens the ACP registry. Agents Dango installs are pinned to tested versions and checked against their published checksums (npm integrity or SHA-256); Dango runs no unversioned `npx` packages.

### The timeline

Every agent works in an AI tab with the same timeline: answers, tool calls with their output, permission requests, questions, plans to approve and comment on (⌥⌘K), to-dos, subagents and background tasks. Two buttons in the tab's header list the running subagents and background processes. Mention a file, `@terminal` or `@browser` to hand the agent its content. A green dot on the tab shows a session at work, a red one a session waiting for you, and a notification says when it finishes or waits while you look elsewhere. For Claude, the status line shows the context and the usage.

### Permissions and modes

New threads start in the mode “Manual”: the agent asks before it changes files or runs commands. For destructive git commands (`push --force`, `reset --hard`, `clean -fd`) Dango offers no “always allow”. The own agent keeps its “always allow” rules per project, lists them in **Settings › Own Agent** and asks for destructive git commands every time, whatever is stored; Claude and the ACP agents keep such rules in their own configuration. With “allow everything” and no sandbox, the thread shows a permanent warning.

### Checkpoints and worktrees

Before each prompt Dango keeps a checkpoint, as a commit under `refs/ide-checkpoints/` in the project's repository, or in its own folder for a project without git; a normal push or fetch never carries them. What an agent changed can be reviewed and rewound per message. A thread can work in a git worktree of its own: the Git panel shows what it changed and takes the work over by merge or cherry-pick.

## Editor

- **Syntax colours, line numbers and change marks** against the last commit, find and replace, Open Quickly (⌘P) and project search (⇧⌘F).
- **Indentation** follows the file and `.editorconfig`: tabs or spaces, automatic indentation, bracket pairs.
- **Several carets:** ⌥-click, ⌥-drag for a column, ⌃⇧↑ and ⌃⇧↓, ⌘D for the next occurrence.
- **Folding** (⌥⌘← and ⌥⌘→, with ⇧ for everything), toggling comments (⌘/), go to line (⌃G), visible whitespace and line wrapping (⌥Z).
- **Files keep their encoding**, byte order mark and line endings. A file changed on disk reloads when the editor holds no unsaved text; otherwise a bar offers the differences and a merge.
- **Edit predictions.** While you type, a suggestion appears in grey, and Tab takes it. It comes from GitHub Copilot or from a local model in Ollama or LM Studio, as chosen in **Settings › Prediction**; a local model keeps your code on this Mac.
- **Minimap and Sticky Scroll** (Editor menu): a narrow picture of the whole file beside the text, and the header lines of the blocks the top line is in, held above it.
- **Diagnostics at the line's end** (View menu): each line's most serious message after a second's rest, with a symbol for its severity.
- **Live preview of static pages:** Editor › Open in Browser Tab serves a trusted folder's pages from Dango itself and reloads the tab when a file is saved.
- **Prettier** formats JavaScript, TypeScript, CSS, HTML, Markdown and more where the project configures it, with the project's own `node_modules/.bin/prettier`; the setting `formatter` picks the formatter per language.
- **Vim mode** (Editor › Vim Mode): Normal, Insert, Visual and Replace mode, counts, operators with motions and text objects, `.`, registers, marks and macros. `u` and ⌃R use the editor's undo, and an insert pass is one step. `"+` and `"*` are the clipboard; the setting `vimClipboard` makes the unnamed register the clipboard too. `/`, `?`, `n`, `N`, `*`, `:s` and `:%s` search with Rust's regular expressions, not Vim's patterns: no `\v`, and `\b` in place of `\<` and `\>`. Ex commands are `:w`, `:q`, `:wq`, `:x`, `:noh` and `:<line>`. In Normal mode Vim takes ⌃G from go to line.
- **Settings sync** through a folder in iCloud Drive (**Settings › General**): your settings and shortcuts follow you to your other Macs; paths, local addresses and consents stay on each.

## Language servers

Go, Rust, TypeScript and JavaScript, Python, C, C++ and Objective-C, Swift, PHP, Ruby, C#, Java, shell scripts, JSON, YAML, CSS, HTML, TOML, Markdown and Lua, and servers of your own from **Settings › Language server**. Dango installs a server when you ask, into its own tools folder, and uses a TypeScript project's own `tsc` when it can serve as a language server.

What they bring: diagnostics (⌘' and ⇧⌘' move between them), hover (⌃⌘I), completion (⌥Esc), signature help, go to definition (⌃⌘J), type definition and implementation, references (⌃⇧⌘F), call and type hierarchies (⌃⇧⌘H), rename (⌃⌘E), formatting (⌥⇧F), semantic colours, folding and the outline (⇧⌘O); types and parameter names inside the line; quick fixes and refactorings behind a light bulb (⌥⌘A); the project's symbols with ⌥⌘O.

Beside a file's own server, ESLint checks JavaScript and TypeScript of projects that use it, with its quick fixes, and Emmet's abbreviations appear in completion for HTML, CSS and JSX. Dango installs both when you ask; **Settings › General** fixes ESLint's problems on save if you want.

## Tests

The Tests panel lists a folder's tests by package and file with their state: `cargo test` or cargo-nextest, Go, pytest, Vitest and Jest. A symbol at each test in the gutter runs it, ⌥-click debugs it, and a failed assert marks its line. The Debug menu runs the test at the caret (⌃⌥⌘U), the last tests again (⌃⌥⌘G) or all of them, and debugs the test at the caret. Tests run in trusted folders only, since they run the project's code; `cargo test` lists its tests when you ask, as listing builds every test program.

## Debugger

Breakpoints, also with conditions and as logpoints, stepping, threads with their call stacks, variables and watched expressions in the inspector (⌥⌘I), values beside the code while the program is paused, and a debug console that evaluates in the paused frame.

| Language | Debug adapter |
|---|---|
| Rust, C, C++, Swift | `lldb-dap` from the Xcode Command Line Tools |
| Go | `dlv` |
| JavaScript, TypeScript | the JavaScript debugger, on Node.js |
| Python | debugpy |

A project's `.vscode/launch.json` is read. The Debug menu uses Xcode's shortcuts: ⌘\\ sets a breakpoint, ⌘Y turns all of them on or off, ⌃⌘Y continues, F6, F7 and F8 step over, into and out, ⌘. stops.

## Terminal

Terminals in tabs (⌃\`) with the login shell's environment, drawn with Metal. ⌘K clears the screen. Pasting or dropping an image hands it to a coding agent running in the terminal, or pastes the path of a saved copy. ⌘-click opens links and file names. A development server the terminal starts can be opened in the browser tab.

## Browser

A browser tab with address field (⌘L), back and forward (⌘[ and ⌘]), reload (⌘R), search, downloads that ask first, and the Web Inspector. `@browser` hands the page to an agent, marked as text from outside.

## Git

The Git panel stages and discards files or single hunks, commits, commits and pushes or syncs, switches and creates branches, keeps stashes, resolves merge conflicts and draws a graph of the recent commits. Blame shows who last changed the line under the cursor, or a column for the whole file. A button writes the commit message with AI; the diff goes out only after gitleaks found no secret in it, and every commit is checked for secrets as well.

## CI

GitHub Actions and GitLab CI, also GitHub Enterprise Server and self-managed GitLab:

- runs, jobs and logs of the current branch;
- re-running failed jobs, a single job or, on GitHub, a whole run; starting manual jobs or approving waiting deployments; starting a new pipeline or a workflow that may be started by hand; cancelling;
- the open pull requests or merge requests and the issues, which open in the browser tab;
- a failed job's log handed to an agent, marked as text from outside.

The token comes from the keychain, `gh` or `glab`, the login shell's environment, or one you enter in **Settings › CI**.

## Containers

The Containers panel shows Docker's Compose projects with their services, single containers, images and volumes, and follows Docker's events as they happen. A Compose file of the folder appears as a project even before any of its containers exists. Containers start, stop and restart; their logs and a shell open in terminal tabs, and a published port opens in the browser tab. Deleting a container, an image or a volume, and cleaning up unused data, ask first. Compose's up, down and pull run in trusted folders only, since Compose builds and runs what the project describes. The panel's menu chooses the Docker context for the window without changing the one your shells use; the same commands are in the Containers menu. It needs the `docker` command line, Docker 24.0.6 or later.

## Markdown

- **Preview.** A Markdown file opens with its preview beside it; **View › Show/Hide Preview Beside** (⇧⌘V) closes and opens it, and **Settings › General** switches the automatic preview off. The preview draws formulas, Mermaid, Graphviz and PlantUML diagrams and the folder's pictures; pictures from the web load only in a trusted folder or after you ask for them in the preview.
- **Format menu:** bold (⌘B), italic (⌘I), strikethrough (⇧⌘X), code (⌥⇧⌘C), link (⇧⌘K), headings 1–3 (⌥⌘1–⌥⌘3), bulleted and numbered lists, tasks, quotes and code blocks. Each command toggles, on the selection or the lines, and is one undo step.
- **Paste.** Text from Word or a web page arrives as Markdown, its pictures as files beside the document; ⌥⇧⌘V pastes plain text.
- **Export.** **File › Export** writes PDF, a single HTML file with its pictures inside, or a Word document. Pictures from the web go along only from a trusted folder.
- **Open in Splat.** Where [Splat](https://github.com/sventmueller/splat) is installed, **File › Open in Splat** and the context menus of the file tree and the tabs open the document there, after saving it.

## Dictation

Dictate into an AI tab's message field or the editor: the microphone button there, **Edit › Start Dictation** (⌃⌥D), or the fn key (hold to record, double-tap to keep recording). The speech recognition is the one of [Lowspeak](https://github.com/sventmueller/lowspeak); Lowspeak itself does not need to be installed.

- **Settings › Dictation** sets the mode and model (online with OpenAI or Groq, locally with Whisper or Parakeet), the language, the post-processing, the microphone and your own vocabulary.
- **Local models** are downloaded from Hugging Face when you choose one, with progress and a way to cancel. They land in the folders Lowspeak uses, so either app uses what the other downloaded. A local model keeps the recording on this Mac.
- **Online transcription** needs an OpenAI or Groq key, which Dango keeps in its own keychain item.
- **From Lowspeak.** If Lowspeak is set up differently from the defaults, the Dictation tab offers once to take its settings over, and its keys on request; macOS asks before Dango reads one (choose “Allow”, not “Always Allow”).
- **Permissions.** macOS asks for the microphone on the first dictation. The fn key works while Dango is the active app and Lowspeak is not running; it needs the Accessibility permission, which Dango asks for after the first dictation, never at launch.

## Trust

A folder can make programs run as soon as something is started in it: git hooks and repository settings such as `core.fsmonitor`, language servers from `node_modules`, agent hooks, `.mcp.json`. The first time you open a folder, Dango asks whether you trust it.

- **Trusted:** git, language servers, debuggers, agents and CI run for this folder and everything below it.
- **Restricted:** you can read and edit; the change marks in the editor come from reading `.git` directly. Nothing the folder configures is started. The Git, CI and AI panels say so and offer to trust the folder.

Text that does not come from you can carry instructions meant to steer an agent (prompt injection). The output of a terminal (`@terminal`), a browser page (`@browser`), a CI log handed to an agent, a file outside the project and longer text pasted from a web page therefore go along marked: the chip says “External Content”, and the agent gets the text with the notice that it is untrusted data whose instructions it must not follow. The own agent's web pages come with the same notice. Pasted text counts as coming from a web page when the browser says so on the pasteboard, as Safari, Chrome and Firefox do; text from elsewhere, and a short phrase of a few words, stays your own.

## Sandbox

Dango itself runs without the App Sandbox, because every program it starts would inherit it: terminals, git, language servers and agents could not work. Instead, each AI thread has a sandbox switch, off by default. What it limits depends on the agent, and the switch's tooltip says so:

| Agent | With the sandbox on |
|---|---|
| Claude | shell commands run in Claude's own sandbox; its file and web tools do not |
| Codex | commands and changes write only in the project, without network |
| Own agent | files, web requests and commands stay within the project and the allowed folders, without network (`sandbox-exec`) |
| Antigravity, OpenCode | no sandbox that Dango can switch on |

## Appearance and accessibility

- **Dark mode** follows the system; **Increase Contrast** and **Reduce Motion** are respected.
- **VoiceOver** names the sidebar, the tabs, the cards of the AI timeline, Git and CI rows, the debugger and the terminal's text. Breakpoints are never told apart by colour alone.
- **Keyboard.** Every command has a shortcut, and all of them can be changed in `keybindings.json`; ⌥⌘\` moves to the next panel.
- **Right to left.** In Arabic and Hebrew the window is mirrored, while code views stay left to right.

What Dango adds on top of AppKit, area by area, is in [ACCESSIBILITY.md](ACCESSIBILITY.md).

## Languages

The interface is translated into 35 languages: Arabic, Catalan, Chinese (Simplified), Chinese (Traditional), Croatian, Czech, Danish, Dutch, English, Finnish, French, German, Greek, Hebrew, Hindi, Hungarian, Indonesian, Italian, Japanese, Korean, Malay, Norwegian, Polish, Portuguese (Brazil), Portuguese (Portugal), Romanian, Russian, Slovak, Slovenian, Spanish, Swedish, Thai, Turkish, Ukrainian and Vietnamese.

Dango follows the language order in **System Settings › General › Language & Region**; you can also pick a language for Dango alone there under **Applications**. Without a translation for your language, it uses English. The terms follow one glossary, [docs/glossary.md](docs/glossary.md), and Apple's own translations, so Dango reads like the rest of your Mac.

## Keyboard shortcuts

| Action | Shortcut |
|---|---|
| Open folder, new window, close window | ⌘O, ⇧⌘N, ⇧⌘W |
| New file, save, save as | ⌘N, ⌘S, ⇧⌘S |
| New tab, close tab, next and previous tab | ⌘T, ⌘W, ⇧⌘] and ⇧⌘[ |
| Open Quickly, search in the project | ⌘P, ⇧⌘F |
| Files, Search, Agents, Git, Tests, Debug, CI, Containers | ⌘1 to ⌘8 |
| Show or hide the sidebar, next panel | ⌃⌘S, ⌥⌘\` |
| Back and forward through edited places | ⌃- and ⌃⇧- |
| Toggle comment, go to line | ⌘/, ⌃G |
| Next occurrence, caret above and below | ⌘D, ⌃⇧↑ and ⌃⇧↓ |
| Fold, unfold, fold all, unfold all | ⌥⌘←, ⌥⌘→, ⇧⌥⌘←, ⇧⌥⌘→ |
| Line wrapping, inspector | ⌥Z, ⌥⌘I |
| Completion, hover, go to definition | ⌥Esc, ⌃⌘I, ⌃⌘J |
| References, call hierarchy, rename | ⌃⇧⌘F, ⌃⇧⌘H, ⌃⌘E |
| Symbols in the file, in the project | ⇧⌘O, ⌥⌘O |
| Quick fix, format | ⌥⌘A, ⌥⇧F |
| Next and previous problem | ⌘' and ⇧⌘' |
| Markdown preview, paste as plain text | ⇧⌘V, ⌥⇧⌘V |
| Bold, italic, strikethrough, code, link | ⌘B, ⌘I, ⇧⌘X, ⌥⇧⌘C, ⇧⌘K |
| Heading 1 to 3 | ⌥⌘1 to ⌥⌘3 |
| New terminal, clear terminal | ⌃\`, ⌘K |
| Focus the agent's message field, comment on a plan | ⌘Esc, ⌥⌘K |
| Address field, reload, back, forward (browser) | ⌘L, ⌘R, ⌘[, ⌘] |
| Start or stop dictation | ⌃⌥D |
| Breakpoint, all breakpoints on or off | ⌘\\, ⌘Y |
| Run the test at the caret, run the last tests again | ⌃⌥⌘U, ⌃⌥⌘G |
| Continue, step over, into, out, stop | ⌃⌘Y, F6, F7, F8, ⌘. |
| Settings | ⌘, |

`keybindings.json` in the app's data folder (see [Where data lives](#where-data-lives)) changes any of them.

## Privacy and security

- **No telemetry,** no analytics, no crash reports sent anywhere. Errors go to the system log (Console.app, under the app's name).
- **The network only for what you start:** the agents you use, each talking to its own service; the ACP registry and the downloads of agents, language servers, debug adapters and dictation models when you ask for them; the CI hosts you sign in to; edit predictions from Copilot if you chose it; the providers of the own agent; online transcription if you chose it; the pages you open in the browser tab; and the update check once you allowed it.
- **Code that leaves the Mac** does so only where you chose a service for it: an agent, Copilot's predictions, an AI-written commit message. The diff for a commit message goes out only after gitleaks found no secret in it.
- **Keys and tokens** stay in the macOS keychain, never in a settings file. Keys of other apps, such as Lowspeak's, are read only when you agree, and only once.
- **Pinned downloads.** Agents, language servers and debug adapters Dango installs are pinned to tested versions and checked against their checksums.
- **Code signing** with the Hardened Runtime; updates install only with the app's own signature.

## Where data lives

`<id>` is the bundle identifier, `local.agentide` for the release (`local.agentide.debug` for debug builds). It does not contain the display name, so a rename keeps all data.

| Place | What |
|---|---|
| `~/Library/Application Support/<id>/` | `settings.json`, `state.json`, `keybindings.json`, `trust.json`; `threads/` (the AI panel's list), `agent-threads/` (own agent conversations), `layouts/` (tabs per project), `recovery/` (unsaved editors after a crash), `checkpoints/` (for folders without git), `worktrees/`, `agents/` and `tools/` (installed agents, language servers and debug adapters), `task-output/` |
| `~/Library/Caches/<id>/` | pasted images, shell integration scripts, the ACP registry; can be deleted |
| `~/Library/Logs/<id>/dictation/` | dictation's log files |
| `~/Library/Application Support/Lowspeak` and `FluidAudio` | local dictation models, shared with Lowspeak |
| `~/Library/WebKit/<id>/` | cookies and website data of the browser tab |
| `<project>/.dango/settings.json` | the project's own settings |
| the project's git repository | checkpoints, as commits under `refs/ide-checkpoints/`; a normal push or fetch never carries them |
| keychain, service `<id>` | API keys of the own agent and of dictation, CI tokens |
| `~/.local/bin/dango` | the terminal command, if you installed it |

Claude keeps its own sessions under `~/.claude/projects`; Dango reads them to resume a session but does not change them.

## Known limitations

- **No App Sandbox** for the app itself, since terminals, git, language servers and agents would inherit it; a folder must be trusted before anything it configures runs. Antigravity and OpenCode have no sandbox Dango can switch on.
- **Claude's usage figures and context breakdown** rely on parts of its protocol marked experimental and are hidden when they are missing.
- **Rewinding the conversation** works for Claude only; for the other agents only the code is rewound.
- **Signing in to CI through the browser** is off; use `gh`, `glab`, a token or the environment.
- **Gemini CLI is not offered:** Google ended its sign-in with a personal account and points to Antigravity. Earlier Gemini threads stay readable.

## Building from source

You need:

- macOS 14 or later with the Xcode Command Line Tools (`xcode-select --install`); Xcode itself is not needed.
- Rust as `rust-toolchain.toml` names it. Homebrew's `rust` builds for this Mac's processor; the universal build of a release needs rustup with the targets `aarch64-apple-darwin` and `x86_64-apple-darwin`.
- Checkouts of [splat](https://github.com/sventmueller/splat) and [lowspeak](https://github.com/sventmueller/lowspeak) next to this one (`../splat`, `../lowspeak`), both on their `main` branch; the workspace takes crates from them by path.
- cmake, for every build with dictation (`make run`, `make build`, `make release`), which builds whisper.cpp. `make check` builds without it.

| Command | What it does |
|---|---|
| `make run` | builds the debug app (`target/debug/Dango Debug.app`) and opens it |
| `make build` | builds the release app (`target/release/Dango.app`), signed with a local certificate the first build creates in the login keychain; leaves the version in `Cargo.toml` unchanged |
| `make lint` | formatting and clippy; warnings fail |
| `make check` | `lint`, then the tests of the changed crates and of those that depend on them (`ALL=1`: all tests); needs no keychain, window or network |
| `make test` | `check`, then the self-test `probe` and the accessibility audit, left to right and right to left, in the real app |
| `make perf` | the timing budgets, in a release build; meaningful only on an otherwise idle Mac |
| `make release` | a dry run of the release into `target/release-dry-run/`, signed with the local certificate; sends nothing anywhere |
| `make release-notarized` | `check`, then the current version into `dist/`: Developer ID signature, notarization, the update feed signed with the app's own key; leaves the version in `Cargo.toml` unchanged |
| `make deploy` | on clean `main`, with or without a CI seal: notarizes the next version, commits it in `Cargo.toml` and the lock files, uploads the release assets, pushes the code and version tag, a green CI seal along when there is one, and then publishes the release itself |

Debug and release are separate apps with their own settings and data. Building, testing, the self-tests, the release and the rules the code follows are described in [docs/development.md](docs/development.md), the decisions behind it in [docs/plan.md](docs/plan.md). The release notes are [RELEASE_NOTES.md](RELEASE_NOTES.md), translated beside each README in `assets/readme/<language>/`.

## License

Dango is licensed under either of

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE))
- MIT License ([LICENSE-MIT](LICENSE-MIT))

at your option.

Unless you explicitly state otherwise, any contribution intentionally submitted for inclusion in Dango by you, as defined in the Apache-2.0 license, shall be dual licensed as above, without any additional terms or conditions.

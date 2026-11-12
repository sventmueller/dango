<!-- DRAFT for the owner's review. Check each point against the release build, adjust what differs, then delete this
comment; `cargo xtask release --notarize` refuses notes that still carry it. To check before publishing: the interface
languages (only German and English so far), whether the Markdown preview and the decoder service have landed, and
whether browser sign-in for CI is on (it needs the registered OAuth apps). -->
Dango 0.1.0 is the first release: a native macOS editor for working with coding agents. Editor, terminals, a
browser, git, CI and AI threads share one window; the app is written in Rust on AppKit, and WebKit appears only in the
browser tab.

## Requirements
- macOS 14 or later, on Apple silicon or Intel.
- The AI threads need the agent you choose: Claude needs the `claude` command, signed in; Codex and Gemini CLI need
  Node.js (Gemini CLI version 20 or later), and the app installs their pinned packages when you ask; Antigravity and
  OpenCode use your own `agy` or `opencode`, or a pinned download; the own agent needs an API key, or Ollama or
  LM Studio on this Mac.
- Language features for Go need Go, for TypeScript Node.js; the app installs gopls, rust-analyzer and the TypeScript
  server when you ask.

## Editor
- Syntax colours, line numbers, change marks against the last commit, find and replace, Open Quickly (⌘P) and project
  search (⇧⌘F).
- Automatic indentation, tabs or spaces from `.editorconfig`, bracket pairs, toggling comments, folding, go to line,
  visible whitespace and line wrapping.
- Several carets: ⌥-click, ⌥-drag for a column, ⌘D for the next occurrence.
- Files keep their encoding, byte order mark and line endings. A file changed on disk reloads when the editor holds no
  unsaved text; otherwise a bar offers the differences and a merge.

## Language servers
Go, Rust and TypeScript: diagnostics, hover, completion, signature help, go to definition, references, rename,
formatting, semantic colours, folding and the outline. A TypeScript project's own `tsc` is
used when it can serve as a language server.

## Terminal
Terminals in tabs and panels, with the login shell's environment. Pasting or dropping an image hands it to a coding
agent running in the terminal, or pastes the path of a saved copy. ⌘-click opens links and file names. A development
server the terminal starts can be opened in the browser tab.

## Browser
A browser tab with address field, back and forward, search, downloads with a question first, and the Web Inspector.

## Git
A Git panel with staging and discarding of files or single hunks, commits, branches, push, pull and fetch, merge
conflicts, and a check for secrets before a commit. An AI thread can work in a git worktree of its own; the panel shows
what it changed and takes the work over by merge or cherry-pick.

## CI
GitHub Actions and GitLab CI, also GitHub Enterprise Server and self-managed GitLab: runs, jobs and logs of the current
branch; re-running failed jobs, a single job or (on GitHub) a whole run, starting manual jobs or approving waiting
deployments, starting a new pipeline or a workflow that may be started by hand, and cancelling. The open merge requests
or pull requests and issues list in the same panel and open in the browser. A failed job's log can go to an agent,
marked as text from outside. The token comes from the keychain, `gh` or `glab`, the login shell's environment, or one
you enter.

## AI
Every agent works in an AI tab with the same timeline: answers, tool calls with their output, permission requests,
questions, plans to approve and comment on, to-dos, subagents and background tasks. Before each prompt the app keeps a
checkpoint; changes can be reviewed and rewound per message. New threads start in the mode "Manual", and destructive
git commands are never allowed for good. An overview in the toolbar lists everything that runs, and a notification
and the Dock icon say when a session finishes or waits while you look elsewhere.
- **Claude** through the `claude` command, unchanged, with sessions, subagents, background tasks, context and usage.
- **ACP agents:** Codex, Gemini CLI, Antigravity and OpenCode, and more from the ACP registry when you ask for it;
  installed agents are pinned to tested versions and checked against their checksums.
- **Own agent:** Anthropic, OpenAI, Google Gemini, OpenRouter, Ollama, LM Studio and servers that speak OpenAI's or
  Anthropic's API, with tools for files, search, commands, diagnostics and web requests; keys stay in the keychain.
- A sandbox switch per thread limits what an agent's commands may do, as far as the agent supports it.

## Panels and recovery
Tabs can be opened in panels to the right or below and dragged between them; the layout is kept per project. After a
crash the windows come back with their tabs, and editors with unsaved text come back with it.

## Dictation
Dictation into the AI message field and the editor through Lowspeak's speech recognition: the microphone button,
⌃⌥D, or the fn key while Dango is the active app. It uses Lowspeak's settings and models; online transcription needs
an OpenAI or Groq key.

## Accessibility
VoiceOver names for the sidebar, tabs, cards of the AI timeline, Git and CI rows, and the terminal's text; full keyboard
control with shortcuts you can change in `keybindings.json`; Increase Contrast and Reduce Motion; code views stay left
to right in right-to-left interfaces. Details are in ACCESSIBILITY.md.

## Privacy
No telemetry, no analytics and no crash reports. The app connects only for what you start: the agents you use, the
downloads you ask for, the CI hosts you sign in to, online transcription if you chose it, and the pages you open.
Keys and tokens stay in the macOS keychain.

## Known limits
- The interface is in German and English.
- No Markdown preview yet; formulas and diagrams in AI answers show as code.
- Sign-in to CI through the browser is off; use `gh`, `glab`, a token or the environment.
- The app runs without the App Sandbox, since terminals, git, language servers and agents would inherit it; a folder
  must be trusted before anything it configures runs. Antigravity and OpenCode have no sandbox the app can switch on.
- Claude's usage figures and context breakdown rely on parts of its protocol marked experimental and are hidden when
  they are missing. Rewinding the conversation works for Claude only; for the other agents only the code is rewound.
- Accessibility gaps: in a right-to-left interface the tab bar's "+" stays at the right edge; the Activity overview
  loses the keyboard focus every second; Space does nothing on a section row of the Git list; commenting on a plan
  needs the mouse.

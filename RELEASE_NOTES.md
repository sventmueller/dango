<!-- DRAFT for the owner's review. Check each point against the release build, adjust what differs, then delete this
comment; `cargo xtask release --notarize` refuses notes that still carry it. To check before publishing: whether
browser sign-in for CI is on (it needs the registered OAuth apps), and that release-notes/<language>.md still say the
same as this file. -->
This is the first release of Dango, a native macOS editor for working with coding agents. Editor, terminals, a
browser, git, CI, a debugger and AI threads share one window per folder. The app is written in Rust on AppKit;
WebKit appears only in the browser tab. The interface is in 35 languages.

## Requirements
- macOS 14 or later, on Apple silicon or Intel.
- For the AI threads, the agent you choose: Claude needs the `claude` command, signed in; Codex needs Node.js, and
  the app installs its pinned package when you ask; Antigravity and OpenCode use your own `agy` or `opencode`, or a
  pinned download; the own agent needs an API key, or Ollama or LM Studio on this Mac.
- Language servers and debug adapters are installed when you ask. gopls and `dlv` need Go; the TypeScript, Python,
  JSON, YAML, CSS, HTML and shell servers and the JavaScript debugger need Node.js; clangd, sourcekit-lsp and
  `lldb-dap` come with the Xcode Command Line Tools.
- Edit predictions need a GitHub Copilot plan (the free one will do), or a fill-in-the-middle model in Ollama or
  LM Studio.

## Window
- Every file, terminal, browser page, AI thread, diff and preview is a macOS tab of the folder's window. The "+" at
  the end of the tab bar and ⌘T offer a new file, a terminal, the browser and each installed agent, also in a git
  worktree of its own.
- The sidebar switches between Files, Search, Agents, Git, CI and Debug (⌘1–⌘6); a right-click hides the ones you do
  not use. The toolbar holds back and forward through the places you edited (⌃- and ⌃⇧-), the branch, Open Quickly and
  the Activity overview.
- "Open to the Side" shows a second document beside the first. After a restart, or a crash, every window comes back
  with its tabs, and editors with unsaved text come back with it.
- Images, audio, video, fonts and Office documents open in a tab through Quick Look, PDFs through PDFKit.

## Editor
- Syntax colours, line numbers, change marks against the last commit, find and replace, Open Quickly (⌘P) and project
  search (⇧⌘F).
- Automatic indentation, tabs or spaces from `.editorconfig`, bracket pairs, toggling comments, folding, go to line,
  visible whitespace and line wrapping.
- Several carets: ⌥-click, ⌥-drag for a column, ⌘D for the next occurrence.
- Files keep their encoding, byte order mark and line endings. A file changed on disk reloads when the editor holds no
  unsaved text; otherwise a bar offers the differences and a merge.
- **Edit predictions:** while you type, a suggestion appears in grey; Tab takes it. They come from GitHub Copilot or
  from a local model in Ollama or LM Studio, whichever you choose in the settings; a local model keeps your code on
  this Mac.

## Language servers
Go, Rust, TypeScript and JavaScript, Python, C, C++ and Objective-C, Swift, shell scripts, JSON, YAML, CSS, HTML,
TOML, Markdown and Lua, and servers of your own from the settings. Diagnostics, hover, completion, signature help,
go to definition, type definition and implementation, references, call and type hierarchies, rename, formatting,
semantic colours, folding and the outline; types and parameter names inside the line; quick fixes and refactorings
behind a light bulb (⌥⌘A); the project's symbols with ⌥⌘O or `#` in Open Quickly. A TypeScript project's own `tsc`
is used when it can serve as a language server.

## Debugger
Breakpoints, also with conditions, stepping, threads with their call stacks, variables and watched expressions in the
inspector (⌥⌘I), values beside the code while the program is paused, and a debug console that evaluates in the paused
frame. Rust, C, C++ and Swift through `lldb-dap`, Go through `dlv`, JavaScript and TypeScript through the JavaScript
debugger, Python through debugpy. A project's `.vscode/launch.json` is read. The Debug menu uses Xcode's shortcuts.

## Terminal
Terminals in tabs with the login shell's environment, drawn with Metal. Pasting or dropping an image hands it to a
coding agent running in the terminal, or pastes the path of a saved copy. ⌘-click opens links and file names. A
development server the terminal starts can be opened in the browser tab.

## Markdown
A Markdown file opens with its preview beside it (⇧⌘V), with formulas, Mermaid, Graphviz and PlantUML diagrams and
the folder's pictures. A Format menu for bold, italic, links, headings, lists, quotes and code blocks; text pasted
from Word or a web page arrives as Markdown. **File › Export** writes PDF, a single HTML file or a Word document.
Where Splat is installed, **File › Open in Splat** opens the document there.

## Browser
A browser tab with address field, back and forward, search, downloads with a question first, and the Web Inspector.

## Git
A Git panel with staging and discarding of files or single hunks, commits, commit and push or sync, branches, stashes,
merge conflicts, and a graph of the recent commits. Blame shows who last changed the line under the cursor, or a
column for the whole file. A button writes the commit message with AI; the diff goes out only after gitleaks found no
secret in it, and every commit is checked for secrets as well. An AI thread can work in a git worktree of its own; the
panel shows what it changed and takes the work over by merge or cherry-pick.

## CI
GitHub Actions and GitLab CI, also GitHub Enterprise Server and self-managed GitLab: runs, jobs and logs of the current
branch; re-running failed jobs, a single job or (on GitHub) a whole run, starting manual jobs or approving waiting
deployments, starting a new pipeline or a workflow that may be started by hand, and cancelling. The open merge
requests or pull requests and issues list in the same panel and open in the browser tab. A failed job's log can go to
an agent, marked as text from outside. The token comes from the keychain, `gh` or `glab`, the login shell's
environment, or one you enter.

## AI
Every agent works in an AI tab with the same timeline: answers, tool calls with their output, permission requests,
questions, plans to approve and comment on, to-dos, subagents and background tasks; two buttons in the tab's header
list the running subagents and background processes. Before each prompt the app keeps a checkpoint; changes can be
reviewed and rewound per message. New threads start in the mode "Manual", and destructive git commands are never
allowed for good. The Activity overview lists everything that runs. A green dot on the tab shows a session at work, a
red one a session waiting for you, and a notification says when it finishes or waits while you look elsewhere.
- **Claude** through the `claude` command, unchanged, with sessions, subagents, background tasks, context and usage.
- **ACP agents:** Codex, Antigravity and OpenCode, and more from the ACP registry when you ask for it; installed
  agents are pinned to tested versions and checked against their checksums.
- **Own agent:** Anthropic, OpenAI, Google Gemini, OpenRouter, Ollama, LM Studio and servers that speak OpenAI's or
  Anthropic's API, with tools for files, search, commands, diagnostics and web requests; keys stay in the keychain.
- A sandbox switch per thread limits what an agent's commands may do, as far as the agent supports it.
- Text from outside, such as terminal output, a web page or a CI log, goes to an agent marked as untrusted.

## Dictation
Dictation into the AI message field and the editor through Lowspeak's speech recognition: the microphone button,
⌃⌥D, or the fn key while Dango is the active app. It uses Lowspeak's settings and models; online transcription needs
an OpenAI or Groq key.

## Updates
The app updates itself through Sparkle. On the second launch it asks whether it may check on its own; until you
agree, it makes no request. **Dango › Check for Updates…** checks at once.

## Accessibility
VoiceOver names for the sidebar, tabs, the cards of the AI timeline, Git and CI rows, the debugger and the terminal's
text; full keyboard control with shortcuts you can change in `keybindings.json`; Increase Contrast and Reduce
Motion; code views stay left to right in right-to-left interfaces. Details are in ACCESSIBILITY.md.

## Privacy
No telemetry, no analytics and no crash reports. The app connects only for what you start: the agents you use, the
downloads you ask for, the CI hosts you sign in to, edit predictions from Copilot if you chose it, online
transcription if you chose it, the pages you open, and the update check once you allowed it. Keys and tokens stay in
the macOS keychain.

## Known limits
- Sign-in to CI through the browser is off; use `gh`, `glab`, a token or the environment.
- The app runs without the App Sandbox, since terminals, git, language servers and agents would inherit it; a folder
  must be trusted before anything it configures runs. Antigravity and OpenCode have no sandbox the app can switch on.
- Claude's usage figures and context breakdown rely on parts of its protocol marked experimental and are hidden when
  they are missing. Rewinding the conversation works for Claude only; for the other agents only the code is rewound.
- Gemini CLI is not offered: Google ended its sign-in with a personal account and points to Antigravity. Earlier
  Gemini threads stay readable.

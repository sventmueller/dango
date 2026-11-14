# Thread-locals

Every `thread_local!` the code had when B22 began ([phase-11.md](phase-11.md), E97), with what it holds, how long
that lives and where it went. Open objects, running work and app state live in the app context (`src/context.rs`),
which the app delegate creates and hands to the window controllers by `Rc`; an answer from a worker thread carries
only ids and the result, and finds its object through the context on the main thread, or nothing once it closed
(Review C11).

The UI crates split off in B19 (E99, `crates/ide-ui-*`) cannot see the app context. Each declares a `Host` trait and
an install-once hook (a `OnceLock`, like `splat_i18n::install`); `src/host.rs` implements the traits and `src/main.rs`
installs them at launch. A crate reaches its state, an `Rc` the app context owns, and windows by their id only
through its `Host`. The hooks are not thread-locals and need no row here.

A thread-local may stay only for one of these reasons:

- **cache**: it holds nothing the app decides on, only what it could make again: colours, fonts, pictures, a setting
  as last read, or a stateless object that menu items and buttons need alive because they hold their target weakly
  and that aims at no particular window.
- **once per process**: macOS wants the object exactly once in the app, such as the `WKWebsiteDataStore`.
- **self-test**: the self-test's own record of a run, built only with the `selftest` feature; it is not app state,
  and `selftest::prepare` records checks before the app delegate exists.

`make lint` runs `cargo xtask thread-locals`, which fails for every `thread_local!` static that this table does not
list with a target starting with "stays", and for every such row whose static is gone. A new thread-local therefore
needs a row here first, and a reason from the list above.

| File | Static | Holds | Lives | Target |
|---|---|---|---|---|
| `crates/ide-claude/src/session.rs` | `LATER` | events of a Claude session that wait until the state lock is released | until the session's reader thread empties it | into the session: `Shared::later` (a mutex); the crate knows no app context |
| `crates/ide-ui-browser/src/dialogs.rs` | `ASKING` | a page's open panel with text fields, for the self-test | while it shows | AppContext: `browser` (`ide_ui_browser::State`, reached through its `Host`, only with `selftest`) |
| `crates/ide-ui-browser/src/downloads.rs` | `DOWNLOADS` | the download delegate with its list of running downloads | app | AppContext: `browser` (`ide_ui_browser::State`, reached through its `Host`) |
| `crates/ide-ui-browser/src/tabs.rs` | `SERIAL` | counter for the keys of tabs without an address | app | AppContext: `browser` (`ide_ui_browser::State`, reached through its `Host`) |
| `crates/ide-ui-browser/src/tabs.rs` | `TABS` | the browser tabs of every window | from opening to closing | AppContext: `browser` (`ide_ui_browser::State`, reached through its `Host`) |
| `crates/ide-ui-browser/src/trust.rs` | `ACCEPTED` | the certificates the user accepted | app | AppContext: `browser` (`ide_ui_browser::State`, reached through its `Host`) |
| `crates/ide-ui-browser/src/trust.rs` | `REFUSED` | certificates of development servers that macOS refused | app | AppContext: `browser` (`ide_ui_browser::State`, reached through its `Host`) |
| `crates/ide-ui-browser/src/web.rs` | `STORE` | the browser tabs' `WKWebsiteDataStore` | app | stays: once per process |
| `crates/ide-ui-common/src/style.rs` | `ACCENT` | the terracotta as a dynamic colour | app | stays: cache |
| `crates/ide-ui-common/src/style.rs` | `ACCENTS` | accent colours by base colour | app | stays: cache |
| `crates/ide-ui-common/src/style.rs` | `TOKEN_COLORS` | colours by token kind | app, until the appearance changes | stays: cache |
| `crates/ide-ui-dictation/src/hub/fn_key.rs` | `FN` | what fn does: hotkey, gesture, running recording, observers | app | AppContext: `dictation` (`ide_ui_dictation::State`, reached through its `Host`; nur mit `dictation`) |
| `crates/ide-ui-dictation/src/hub.rs` | `HUB` | the dictations: fields, running recognition, keys, questions | app | AppContext: `dictation` (`ide_ui_dictation::State`, reached through its `Host`; nur mit `dictation`) |
| `crates/ide-ui-dictation/src/hub.rs` | `TARGET` | action target of the dictation buttons and menu item, without state | app (menu items hold their target weakly) | stays: cache (stateless object that aims at no window) |
| `crates/ide-ui-export/src/lib.rs` | `FAILED` | the sheets of failed exports | until the user closes them | AppContext: `exports` (`ide_ui_export::State`, reached through its `Host`) |
| `crates/ide-ui-export/src/lib.rs` | `NEXT` | the next export number | app | AppContext: `exports` (`ide_ui_export::State`, reached through its `Host`) |
| `crates/ide-ui-export/src/lib.rs` | `RUNS` | the running exports | until they end | AppContext: `exports` (`ide_ui_export::State`, reached through its `Host`) |
| `crates/ide-ui-icons/src/logo.rs` | `IMAGES` | drawn logos by name | app | stays: cache |
| `crates/ide-ui-terminal/src/gpu.rs` | `SHARED` | Metal device, queue and pipeline of every terminal | app | stays: once per process |
| `crates/ide-ui-terminal/src/host.rs` | `VIEWS` | the terminal views of every window by id | from opening to closing | AppContext: `terminal_views` (`ide_ui_terminal::State`, reached through its `Host`) |
| `src/activity/mod.rs` | `ACTIVITIES` | the activity items of every window's toolbar | from a window's first toolbar on | AppContext: `activities` |
| `crates/ide-ui-agent/src/actions.rs` | `ALERTS` | the self-test's answers to the rewind's questions and the questions seen | self-test run | AppContext: `agent.rewind_alerts` (nur mit `selftest`) |
| `crates/ide-ui-agent/src/context.rs` | `NEXT` | the next id for them | app | AppContext: `agent.page_selections` |
| `crates/ide-ui-agent/src/context.rs` | `WAITING` | callbacks waiting for a page to say what is selected, by id | until the page answers | AppContext: `agent.page_selections` |
| `crates/ide-ui-agent/src/context_view.rs` | `GAUGES` | the context gauge of each AI tab by tab key | while the tab is open | AppContext: `agent.context_gauges` |
| `crates/ide-ui-agent/src/context_view.rs` | `OPENER` | action target of the context gauges, without state | app (buttons hold their target weakly) | stays: cache (stateless object that aims at no window) |
| `crates/ide-ui-agent/src/context_view.rs` | `SHOWN` | the breakdown popover on view | while it shows | AppContext: `agent.context_gauges` |
| `crates/ide-ui-agent/src/enlarge.rs` | `OPENER` | action target of the enlarge buttons, without state | app (buttons hold their target weakly) | stays: cache (stateless object that aims at no window) |
| `crates/ide-ui-agent/src/enlarge.rs` | `SHOWN` | the picture shown large | while it shows | AppContext: `agent.enlarged_picture` |
| `crates/ide-ui-agent/src/figures.rs` | `ASKED` | the formulas and diagrams asked of the renderer, with their receiver | until the answers came | AppContext: `agent.figures` |
| `crates/ide-ui-agent/src/figures.rs` | `ASKING` | whether a request is out | until the answer | AppContext: `agent.figures` |
| `crates/ide-ui-agent/src/figures.rs` | `COLLECT` | the figures a card uses while it is built | one call of `collect` | AppContext: `agent.figures` |
| `crates/ide-ui-agent/src/mod.rs` | `TABS` | every open AI tab | from opening to closing | AppContext: `agent.agent_tabs` |
| `crates/ide-ui-agent/src/plan/ask.rs` | `ASK` | the question about a plan on view | while it shows | AppContext: `agent.plan.ask` |
| `crates/ide-ui-agent/src/plan/doc.rs` | `DOCS` | the open plan documents | from opening to closing | AppContext: `agent.plan.docs` |
| `crates/ide-ui-agent/src/plan/mod.rs` | `LAST_NOTE` | the id the last plan comment got | app | AppContext: `agent.plan.notes` |
| `crates/ide-ui-agent/src/usage.rs` | `REPORT` | the last answer to `get_usage` with its time | until it is stale (2 min) | AppContext: `agent.usage`; the `ASKING` flag (a static `AtomicBool`) moves along |
| `crates/ide-ui-agent/src/agent_panel/agents.rs` | `OFFERED` | the agents the "new session" menu offers, by tag | until the menu is filled again | AppContext: `agent_panels.offered_agents` |
| `crates/ide-ui-agent/src/agent_panel/mod.rs` | `PANELS` | the agent panels of every window | from creation until replaced | AppContext: `agent_panels`; callbacks now carry an id instead of the address |
| `src/ci_panel/log_tab.rs` | `TABS` | the CI log tabs of every window by id | from opening to closing | AppContext: `ci_log_tabs` |
| `src/ci_panel/panel.rs` | `PANELS` | the CI panels of every window | from creation until replaced | AppContext: `ci_panels` |
| `src/editor/blame.rs` | `AT_LINE_END` | the setting "blame at the line end" as last chosen | app | stays: cache (of the setting) |
| `src/editor/blame.rs` | `CACHE` | blame results by file and commit | app, the oldest drop out | stays: cache |
| `src/editor/compare.rs` | `SHOWN` | the text views of open comparison tabs | while the tab is open | AppContext: `editor.comparisons` |
| `src/editor/insets.rs` | `FORCED_CONTRAST` | whether editors draw as with increased contrast, for the self-test | self-test run | AppContext: `editor.forced_contrast` (nur mit `selftest`) |
| `src/editor/mod.rs` | `OPEN` | the open editors of every window | from opening to closing | AppContext: `editors` |
| `src/editor/recovery.rs` | `DUE` | which copies of unsaved text are due | until they are written | AppContext: `editor.recovery_copies` |
| `src/editor/recovery.rs` | `TIMER` | when the timer for the due copies fires | until it fires | AppContext: `editor.recovery_copies` |
| `src/editor/style.rs` | `STYLED_FONTS` | fonts by size and style | app | stays: cache |
| `src/editor/style.rs` | `TOKEN_COLORS` | colours by token kind | app | stays: cache |
| `src/editor/whitespace.rs` | `SHOWN` | the setting "show whitespace" as last chosen | app | stays: cache (of the setting) |
| `crates/ide-ui-git/src/commands.rs` | `COMMANDS` | action target of the Git menu, without state; acts on the window in front | app (menu items hold their target weakly) | stays: cache (stateless object that aims at no particular window) |
| `crates/ide-ui-git/src/diff.rs` | `TABS` | the diff tabs of every window by id | from opening to closing | AppContext: `git.diff_tabs` |
| `crates/ide-ui-git/src/panel.rs` | `PANELS` | the Git panels of every window | from creation until replaced | AppContext: `git.panels` |
| `crates/ide-ui-git/src/toolbar.rs` | `SHOWN` | the toolbars' branch items with their window | while the toolbar exists | AppContext: `git.toolbar_items` |
| `src/lsp/actions.rs` | `CHOOSER` | the menu that offers them | while it is open | AppContext: `code_actions` |
| `src/lsp/actions.rs` | `SHOWN` | the code actions on view | while the menu is open | AppContext: `code_actions` |
| `src/lsp/complete.rs` | `LIST` | the completion list on view | while it shows | AppContext: `completion` |
| `src/lsp/inlay.rs` | `ASKED` | the last inlay hint request of each editor | until the answer comes | AppContext: `inlay_requests` |
| `src/lsp/inlay.rs` | `CHOSEN` | the inlay hint settings as last chosen | app | stays: cache (of the setting) |
| `src/lsp/inlay.rs` | `NEXT` | the next id for them | app | AppContext: `inlay_requests` |
| `src/lsp/inlay.rs` | `SHOWN` | the document revision of the hints shown, by editor | while the editor is open | AppContext: `inlay_requests` |
| `src/lsp/mod.rs` | `CUSTOM` | the language servers the user set up, read from the settings | until `servers_changed` | stays: cache (of the setting) |
| `src/lsp/mod.rs` | `NEXT_ID` | the next id for them | app | AppContext: `lsp_workspaces` |
| `src/lsp/mod.rs` | `WORKSPACES` | the language server workspaces of every window | until their window closes | AppContext: `lsp_workspaces` |
| `src/lsp/navigate.rs` | `SHEET` | the sheet of locations on view | while it shows | AppContext: `location_sheet` |
| `src/lsp/project_symbols.rs` | `LEASES` | the servers lent to the project symbol search, by folder | until the lease runs out (5 min) | AppContext: `server_leases` |
| `src/lsp/rename.rs` | `FORMATTING` | running format-on-save requests by editor | until the answer | AppContext: `formatting` |
| `src/lsp/signature.rs` | `HELP` | the signature help on view | while it shows | AppContext: `signature_help` |
| `src/markdown/splat_app.rs` | `TARGET` | action target of the "open in splat" menu items, without state | app (menu items hold their target weakly) | stays: cache (stateless object that aims at no window) |
| `src/navigation.rs` | `GROUPS` | the toolbars' arrow groups with their window | while the toolbar exists | AppContext: `navigation` |
| `src/navigation.rs` | `HISTORIES` | the back and forward history of each window | while the window is open | AppContext: `navigation` |
| `src/navigation.rs` | `NAVIGATOR` | action target of the toolbar's arrows | app | AppContext: `navigation` (aims at a window through `GROUPS`) |
| `src/preview/mod.rs` | `LOOKING` | whether comparing the picture files is scheduled | while a preview is open | AppContext: `previews` |
| `src/preview/mod.rs` | `NEXT_KEY` | the next key for them | app | AppContext: `previews` |
| `src/preview/mod.rs` | `OPEN` | the open Markdown previews of every window | from opening to closing | AppContext: `previews` |
| `src/selftest.rs` | `CHECKS` | the self-test's checks | self-test run, from before the app delegate exists (`prepare`) | stays: self-test |
| `src/selftest/a11y.rs` | `LEFT_OUT` | the controls Tab leaves out on purpose | self-test run | stays: self-test (written from every step of the scenario's walk, which chains plain function steps) |
| `src/selftest/a11y.rs` | `REPORTED` | the accessibility findings reported so far | self-test run | stays: self-test (written from every step of the scenario's walk, which chains plain function steps) |
| `src/selftest/git/message.rs` | `STUB` | the stub provider for commit messages | self-test run | stays: self-test (the stub server lives for the whole scenario and many plain function steps read it) |
| `src/session.rs` | `QUITTING` | whether the app quits | from the quit on | AppContext: `session` |
| `src/session.rs` | `WRITE_PENDING` | whether writing down the windows is scheduled | until it is written | AppContext: `session` |
| `src/settings.rs` | `WINDOW` | the settings window with its controller | from the first opening on | AppContext: `settings.settings_window` |
| `src/settings/agents.rs` | `PROGRESS` | how far running installations got, by agent | until the installation ends | AppContext: `settings.agent_registry` |
| `src/settings/agents.rs` | `SHOWN` | the registry's ids in the order of its buttons | until it is filled again | AppContext: `settings.agent_registry` |
| `src/settings/agents.rs` | `WINDOW` | the agent registry window | from the first opening on | AppContext: `settings.agent_registry` |
| `src/settings/commit_message.rs` | `SHOWN` | the "commit messages" part of the settings on view | while it shows | AppContext: `settings.commit_message_settings` |
| `src/settings/editor.rs` | `TAB` | the "editor" part of the settings on view | while it shows | AppContext: `settings.editor_settings` |
| `src/updates.rs` | `CONTROLLER` | the Sparkle updater | app | stays: once per process |
| `src/window/mod.rs` | `WINDOWS` | the open window controllers | from opening to just after closing | AppContext: `windows` (a `Registry` with an id per window) |
| `src/window/new_tab_button.rs` | `BELOW_PLUS` | whether the last menu stood below the "+", for the self-test | until read | AppContext: `new_tab_menu` |
| `src/window/new_tab_button.rs` | `MONITOR` | the local event monitor for the "+" of the tab bars | app | stays: once per process |
| `src/window/new_tab_button.rs` | `RECORD_ONLY` | whether the menu is only recorded, for the self-test | self-test run | AppContext: `new_tab_menu` |
| `src/window/new_tab_button.rs` | `SHOWN` | the menu of the "+" shown last | until the next | AppContext: `new_tab_menu` |
| `src/window/search.rs` | `NEXT_ID` | the next id for them | app | AppContext: `search_panels` |
| `src/window/search.rs` | `PANELS` | the search panels of every window | while their window is open | AppContext: `search_panels` |
| `src/window/search/hierarchy.rs` | `NEXT_ID` | the next id for them | app | AppContext: `hierarchy_panels` |
| `src/window/search/hierarchy.rs` | `PANELS` | the call and type hierarchy panels of every window | while their window is open | AppContext: `hierarchy_panels` |

## Statics

Process-wide statics whose type keeps state that changes (a mutex, a once-cell, an atomic, a lane and the like) in
`src/` and `crates/ide-ui-*`. They bypass the app context like a thread-local, so the same rule holds: what the app
decides on lives in `AppContext`; a static may stay only for one of these reasons:

- **install-once hook**: a UI crate's `Host`, as described above.
- **once per process**: something the app has exactly once, such as the login shell's environment or the git program,
  which worker threads read and the main thread's context cannot hand them.
- **worker threads**: a lane of worker threads, or what code on them shares, which an `Rc` context cannot reach.
- **cache**: as above, only what could be made again.
- **serial**: a counter that hands out unique numbers off the main thread and holds nothing else.

Code built only for tests or the self-test (`#[cfg(test)]`, `#[cfg(feature = "selftest")]`, `src/selftest/`, a file's
`selftest_probe.rs`) is not listed. `make lint` runs `cargo xtask rules`, which fails for a stateful static this table does not list and for a row
whose static is gone.

| File | Static | Holds | Reason |
|---|---|---|---|

| `crates/ide-ui-agent/src/host.rs` | `HOST` | the AI tab's `Host` | install-once hook |
| `crates/ide-ui-agent/src/notify.rs` | `ASKED` | whether macOS was asked to allow notifications | once per process |
| `crates/ide-ui-agent/src/notify.rs` | `NOTIFIED` | how many notifications were posted, which the self-test reads | serial |
| `crates/ide-ui-agent/src/own_agent/mod.rs` | `CACHE` | the models each endpoint offered last | cache |
| `crates/ide-ui-agent/src/own_agent/mod.rs` | `STORE` | the app's store of secrets, handed over from the app context, where the keys are | once per process |
| `crates/ide-ui-browser/src/host.rs` | `HOST` | the browser's `Host` | install-once hook |
| `crates/ide-ui-browser/src/web.rs` | `NAME` | Safari's part of the user agent | cache |
| `crates/ide-ui-common/src/window.rs` | `HOST` | the `WindowHost` the feature crates reach windows through | install-once hook |
| `crates/ide-ui-debug/src/host.rs` | `HOST` | the debugger's `Host` | install-once hook |
| `crates/ide-ui-dictation/src/hub.rs` | `HOST` | the dictation's `Host` | install-once hook |
| `crates/ide-ui-export/src/lib.rs` | `HOST` | the export's `Host` | install-once hook |
| `crates/ide-ui-git/src/host.rs` | `HOST` | the Git panel's `Host` | install-once hook |
| `crates/ide-ui-git/src/message/claude.rs` | `NEXT` | the number of the next empty folder `claude` runs in | serial |
| `crates/ide-ui-git/src/work.rs` | `GIT` | the git program and its environment | once per process |
| `crates/ide-ui-git/src/work.rs` | `LOCAL` | the thread of local git work | worker threads |
| `crates/ide-ui-git/src/worktrees/read.rs` | `CACHE` | worktree readings and how often each was marked changed, shared with the Git panel's thread | cache |
| `crates/ide-ui-terminal/src/host.rs` | `HOST` | the terminal's `Host` | install-once hook |
| `crates/ide-ui-terminal/src/host.rs` | `NEXT_ID` | the next terminal view id | serial |
| `crates/ide-ui-terminal/src/host.rs` | `OPTION_AS_META` | the setting ⌥ as Meta as last chosen | cache (of the setting) |
| `crates/ide-ui-terminal/src/render.rs` | `MADE` | the next id of a set of terminal fonts | serial |
| `src/activity/mod.rs` | `WATCH` | whether the supervisor is watched already | once per process |
| `src/ci_panel/login.rs` | `CALLBACK_PORT` | the login's local callback port while one listens on it | worker threads |
| `src/ci_panel/work.rs` | `AUTH` | the CI tokens' source | once per process |
| `src/ci_panel/work.rs` | `LANE` | the CI panel's worker threads | worker threads |
| `src/ci_panel/work.rs` | `NETWORK` | the CI panel's HTTP transport | once per process |
| `src/ci_panel/work.rs` | `STORE` | the app's store of secrets, handed over from the app context, where the tokens are | once per process |
| `src/editor/recovery.rs` | `COPIES` | the thread that writes copies of unsaved text | worker threads |
| `src/lsp/edits.rs` | `EDITS` | the thread that applies a server's workspace edits | worker threads |
| `src/environment.rs` | `ENVIRONMENT` | the login shell's environment | once per process |
| `src/environment.rs` | `READY` | whether that environment is read, for threads that wait for it | once per process |
| `src/lsp/mod.rs` | `REGISTRY` | the language server registry, built once the environment is known | once per process |
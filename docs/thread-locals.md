# Thread-locals

Every `thread_local!` the code had when B22 began ([phase-11.md](phase-11.md), E97), with what it holds, how long
that lives and where it went. Open objects, running work and app state live in the app context (`src/context.rs`),
which the app delegate creates and hands to the window controllers by `Rc`; an answer from a worker thread carries
only ids and the result, and finds its object through the context on the main thread, or nothing once it closed
(Review C11).

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
| `crates/ide-ui-common/src/style.rs` | `ACCENT` | the terracotta as a dynamic colour | app | stays: cache |
| `crates/ide-ui-common/src/style.rs` | `ACCENTS` | accent colours by base colour | app | stays: cache |
| `crates/ide-ui-common/src/style.rs` | `TOKEN_COLORS` | colours by token kind | app, until the appearance changes | stays: cache |
| `crates/ide-ui-export/src/lib.rs` | `FAILED` | the sheets of failed exports | until the user closes them | AppContext: `exports` (`ide_ui_export::State`, reached through its `Host`) |
| `crates/ide-ui-export/src/lib.rs` | `NEXT` | the next export number | app | AppContext: `exports` (`ide_ui_export::State`, reached through its `Host`) |
| `crates/ide-ui-export/src/lib.rs` | `RUNS` | the running exports | until they end | AppContext: `exports` (`ide_ui_export::State`, reached through its `Host`) |
| `crates/ide-ui-icons/src/logo.rs` | `IMAGES` | drawn logos by name | app | stays: cache |
| `crates/ide-ui-terminal/src/gpu.rs` | `SHARED` | Metal device, queue and pipeline of every terminal | app | stays: once per process |
| `src/activity/mod.rs` | `ACTIVITIES` | the activity items of every window's toolbar | from a window's first toolbar on | AppContext: `activities` |
| `src/agent/actions.rs` | `ALERTS` | the self-test's answers to the rewind's questions and the questions seen | self-test run | AppContext: `agent.rewind_alerts` (nur mit `selftest`) |
| `src/agent/context.rs` | `NEXT` | the next id for them | app | AppContext: `agent.page_selections` |
| `src/agent/context.rs` | `WAITING` | callbacks waiting for a page to say what is selected, by id | until the page answers | AppContext: `agent.page_selections` |
| `src/agent/context_view.rs` | `GAUGES` | the context gauge of each AI tab by tab key | while the tab is open | AppContext: `agent.context_gauges` |
| `src/agent/context_view.rs` | `OPENER` | action target of the context gauges, without state | app (buttons hold their target weakly) | stays: cache (stateless object that aims at no window) |
| `src/agent/context_view.rs` | `SHOWN` | the breakdown popover on view | while it shows | AppContext: `agent.context_gauges` |
| `src/agent/enlarge.rs` | `OPENER` | action target of the enlarge buttons, without state | app (buttons hold their target weakly) | stays: cache (stateless object that aims at no window) |
| `src/agent/enlarge.rs` | `SHOWN` | the picture shown large | while it shows | AppContext: `agent.enlarged_picture` |
| `src/agent/figures.rs` | `ASKED` | the formulas and diagrams asked of the renderer, with their receiver | until the answers came | AppContext: `agent.figures` |
| `src/agent/figures.rs` | `ASKING` | whether a request is out | until the answer | AppContext: `agent.figures` |
| `src/agent/figures.rs` | `COLLECT` | the figures a card uses while it is built | one call of `collect` | AppContext: `agent.figures` |
| `src/agent/mod.rs` | `TABS` | every open AI tab | from opening to closing | AppContext: `agent.agent_tabs` |
| `src/agent/plan/ask.rs` | `ASK` | the question about a plan on view | while it shows | AppContext: `agent.plan.ask` |
| `src/agent/plan/doc.rs` | `DOCS` | the open plan documents | from opening to closing | AppContext: `agent.plan.docs` |
| `src/agent/plan/mod.rs` | `LAST_NOTE` | the id the last plan comment got | app | AppContext: `agent.plan.notes` |
| `src/agent/usage.rs` | `REPORT` | the last answer to `get_usage` with its time | until it is stale (2 min) | AppContext: `agent.usage`; the `ASKING` flag (a static `AtomicBool`) moves along |
| `src/agent_panel/agents.rs` | `OFFERED` | the agents the "new session" menu offers, by tag | until the menu is filled again | AppContext: `agent_panels.offered_agents` |
| `src/agent_panel/mod.rs` | `PANELS` | the agent panels of every window | from creation until replaced | AppContext: `agent_panels`; callbacks now carry an id instead of the address |
| `src/browser/dialogs.rs` | `ASKING` | a page's open panel with text fields, for the self-test | while it shows | AppContext: `browser_dialogs` (nur mit `selftest`) |
| `src/browser/downloads.rs` | `DOWNLOADS` | the download delegate with its list of running downloads | app | AppContext: `downloads` |
| `src/browser/mod.rs` | `SERIAL` | counter for the keys of tabs without an address | app | AppContext: `browser_tabs` |
| `src/browser/mod.rs` | `TABS` | the browser tabs of every window | from opening to closing | AppContext: `browser_tabs` |
| `src/browser/trust.rs` | `ACCEPTED` | the certificates the user accepted | app | AppContext: `certificates` |
| `src/browser/trust.rs` | `REFUSED` | certificates of development servers that macOS refused | app | AppContext: `certificates` |
| `src/browser/web.rs` | `STORE` | the browser tabs' `WKWebsiteDataStore` | app | stays: once per process |
| `src/ci_panel/log_tab.rs` | `TABS` | the CI log tabs of every window by id | from opening to closing | AppContext: `ci_log_tabs` |
| `src/ci_panel/panel.rs` | `PANELS` | the CI panels of every window | from creation until replaced | AppContext: `ci_panels` |
| `src/dictation/fn_key.rs` | `FN` | what fn does: hotkey, gesture, running recording, observers | app | AppContext: `dictation.fn_key` (nur mit `dictation`) |
| `src/dictation/mod.rs` | `HUB` | the dictations: fields, running recognition, keys, questions | app | AppContext: `dictation.hub` (nur mit `dictation`) |
| `src/dictation/mod.rs` | `TARGET` | action target of the dictation buttons and menu item, without state | app (menu items hold their target weakly) | stays: cache (stateless object that aims at no window) |
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
| `src/git_panel/commands.rs` | `COMMANDS` | action target of the Git menu, without state; acts on the window in front | app (menu items hold their target weakly) | stays: cache (stateless object that aims at no particular window) |
| `src/git_panel/diff.rs` | `TABS` | the diff tabs of every window by id | from opening to closing | AppContext: `git.diff_tabs` |
| `src/git_panel/panel.rs` | `PANELS` | the Git panels of every window | from creation until replaced | AppContext: `git.panels` |
| `src/git_panel/toolbar.rs` | `SHOWN` | the toolbars' branch items with their window | while the toolbar exists | AppContext: `git.toolbar_items` |
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
| `src/selftest/a11y.rs` | `LEFT_OUT` | the controls Tab leaves out on purpose | self-test run | stays: self-test |
| `src/selftest/a11y.rs` | `REPORTED` | the accessibility findings reported so far | self-test run | stays: self-test |
| `src/selftest/a11y/states.rs` | `AI_TAB` | the AI tab that plays the recording | self-test run | stays: self-test |
| `src/selftest/agent_perf.rs` | `BENCH` | the running benchmark | self-test run | stays: self-test |
| `src/selftest/blame.rs` | `HEAD` | the commit at the scenario's start | self-test run | stays: self-test |
| `src/selftest/browser_dev.rs` | `TLS_SERVER` | the TLS test server and its input | self-test run | stays: self-test |
| `src/selftest/git/flows.rs` | `FETCH_STARTED` | when the fetch began | self-test run | stays: self-test |
| `src/selftest/git/message.rs` | `STUB` | the stub provider for commit messages | self-test run | stays: self-test |
| `src/selftest/git/sheets.rs` | `TOP_BEFORE` | the newest commit's subject before the refused ones | self-test run | stays: self-test |
| `src/selftest/insets.rs` | `CLICKED` | how often an inset was clicked | self-test run | stays: self-test |
| `src/selftest/markdown.rs` | `EXPORTED` | how each export ended | self-test run | stays: self-test |
| `src/selftest/markdown.rs` | `LAST` | how the last export ended | self-test run | stays: self-test |
| `src/selftest/markdown.rs` | `TICKS` | turns of the main thread during an export | self-test run | stays: self-test |
| `src/session.rs` | `QUITTING` | whether the app quits | from the quit on | AppContext: `session` |
| `src/session.rs` | `WRITE_PENDING` | whether writing down the windows is scheduled | until it is written | AppContext: `session` |
| `src/settings.rs` | `WINDOW` | the settings window with its controller | from the first opening on | AppContext: `settings.settings_window` |
| `src/settings/agents.rs` | `PROGRESS` | how far running installations got, by agent | until the installation ends | AppContext: `settings.agent_registry` |
| `src/settings/agents.rs` | `SHOWN` | the registry's ids in the order of its buttons | until it is filled again | AppContext: `settings.agent_registry` |
| `src/settings/agents.rs` | `WINDOW` | the agent registry window | from the first opening on | AppContext: `settings.agent_registry` |
| `src/settings/commit_message.rs` | `SHOWN` | the "commit messages" part of the settings on view | while it shows | AppContext: `settings.commit_message_settings` |
| `src/settings/editor.rs` | `TAB` | the "editor" part of the settings on view | while it shows | AppContext: `settings.editor_settings` |
| `src/terminal/mod.rs` | `VIEWS` | the terminal views of every window by id | from opening to closing | AppContext: `terminal_views` |
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

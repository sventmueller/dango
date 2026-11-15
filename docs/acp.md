# ACP agents → thread model

How `ide-acp` connects agents over the Agent Client Protocol (E4, E5, E19) and turns their session updates into
`ide_thread::Update`s. The table is the specification of the tests in `crates/ide-acp/tests/session.rs`, which run
the session against the test agent of `crates/ide-acp/src/test_agent.rs`; that agent speaks the protocol line by
line without the SDK, so the tests also check what the client puts on the wire.

## Crate version (E4, R2)

`agent-client-protocol = "=5.2.0"` (schema 1.17.0), chosen on 2026-11-11, replacing 2.2.0 (chosen 2026-11-09):

| Release | Date | Why not / why |
|---|---|---|
| **5.2.0** | 2026-11-10 | Since 5.0 the stable v1 API follows semver, so breaks wait for a major release; only `unstable_*` features may change in a minor, which is why the pin is exact. Zed, the protocol's authors, builds on 5.x. ACP v1 on the wire is the same as in 2.2. |
| 3.0–3.6 | 2026-10-06 – 10-28 | Seven minors in three weeks; skipped. |
| 2.2.0 | 2026-09-18 | The first choice: the most used release then. Held a request from the agent that names a session and has no handler unanswered, which a handler of our own had to answer. |

What the move to 5.x changed here: the connection is built with `Builder::new(Client)`, without the SDK's v1
guard, because the guard turns an agent of another protocol version into `-32600` that names the version only in
its text; `initialize` checks the version itself and reports `SessionError::Version(n)`. Requests without a handler
(`fs/*`, `terminal/*`, unknown) are answered `-32601` by the SDK, so the handler of our own is gone. A responder the
client drops is answered `-32603` by the SDK. Raising the version means re-reading the migration guides
(`md/migration-v5.md` in the SDK's repository and later ones) and running the tests here and `make test-integration`
against the real agents.

### Unstable features (E146)

The overlays switch on only `unstable_session_fork`: `/btw` forks the session (`session/fork`), prompts the fork,
collects its `agent_message_chunk`s apart from the thread, refuses every permission request of the fork and closes
it (`session/close`) where the agent announces that. `/status` reads what `initialize` and the session reported, so
`unstable_auth_status` and `unstable_session_status` stay off. Stable and used when announced: `logout`
(`agentCapabilities.auth.logout`). An agent without one of them gets no such command. The exact pin catches a change
of the unstable feature in a minor release of the SDK.

The client hands MCP servers to `session/new` and `session/load` (`ide-acp/src/session/extras.rs`): the project's
`.mcp.json` with `${VAR}` and `${VAR:-default}` filled from the login shell (only in a trusted folder, E22, and
switched off per project in the app's state) and the app's own list (`acpMcpServers` in the state, the values of
environment and headers in the store of secrets under `mcp:<name>`, R42), only with the transports the agent
announces (`mcpCapabilities.http`, `.sse`; stdio always, the program resolved to an absolute path). A change applies
from the next session start.

## Process

| Item | Value |
|---|---|
| Start | the preset's own command on the login shell's PATH (`gemini --acp`, `agy acp`, `opencode acp`, `codex-acp`, `claude-agent-acp`), else the version the app installed (`<data>/agents/<id>/<version>/`) |
| Environment | the login shell's (E10, E23) plus the entry's own variables; folders not trusted run nothing (E22, the app checks before starting) |
| Supervisor | `Kind::Agent`, label = the agent's name; stop = close the connection, then SIGTERM/SIGKILL to the group |
| Initialize | protocol version 1, `clientInfo`; the client offers no file system and no terminal, so agents use their own tools |
| Session | `session/load` with the agent's session id when continuing and the agent offers `loadSession` (the thread is cleared first, since the agent replays the conversation), else `session/new` with the project folder and, for a sandboxed Claude, `_meta.claudeCode.options.sandbox` |
| Sign-in | `session/new` answered with `-32000` → `Event::Failed(SessionError::SignIn)`; the methods are in the capabilities; `authenticate(method)` opens the session after it |
| Protocol version | an agent answering with another version than 1 → `Event::Failed(SessionError::Version(n))` |

## Capabilities

Read from `initialize` and the session's answers; nothing is assumed. `_meta` of the initialize response, the agent
capabilities and their session and prompt parts go into `Capabilities::unknown` as name and JSON text, for the UI to
show. Unknown sign-in methods and option kinds are left out; unknown option classes stay `Category::Other(name)`.

| Agent says | Capabilities |
|---|---|
| `agentInfo` | `agent` (name, title, version) |
| `loadSession`, `sessionCapabilities.list/resume` | `load_session`, `list_sessions`, `resume_session` |
| `promptCapabilities.image/audio/embeddedContext` | `images`, `audio`, `embedded_context` |
| `authMethods` | `auth_methods` (agent-run, or terminal with args and env) |
| `modes` of the session | `modes`, `mode` |
| `configOptions` of the session, `config_option_update` | `options`; `model()` = class `model`, `thought_level()` = class `thought_level` |
| `available_commands_update` | `commands` |

## Prompt

| User message | Content blocks |
|---|---|
| Text | `text` |
| Image | `image` (base64) when `images`, else left out |
| File, whole or lines | `resource` with the text when `embedded_context` and ≤ 512 KB (lines `#L5-10`), else `resource_link` (plus a text naming the lines) |
| Selection | `resource` with the selected text when `embedded_context`, else a fenced `text` with path and lines |
| Text from outside the user (E22) | `text` from `ide_thread::external_text`: the notice that it is untrusted data, then the text between `<external>` markers |

## Session updates

Keys carry the mapper's generation (`message-<g>-<n>`), so a session that continues on a thread an earlier session
filled never overwrites its entries.

| Update | Thread |
|---|---|
| `agent_message_chunk` | `Put AgentMessage` (key `message-<g>-<n>`) once, then `Append`; a new `messageId` or any other update starts a new entry and `Finish`es the open one |
| `agent_thought_chunk` | the same with `Thought` (key `thought-<g>-<n>`) |
| `user_message_chunk` (replayed by `session/load`) | `Put UserMessage` (key `user-<g>-<n>`), text growing chunk by chunk |
| `tool_call` | `Put ToolCall`, key = `toolCallId`; kind, status, title, `name` (else the title), `rawInput` → input; view: `diff` content → `Edit` (old text) or `Write` (none), `execute` with `command` → `Command`, `read` → `Read` (path or first location), `search` → `Search`, `fetch` → `Fetch`; started when not pending, ended when completed or failed |
| `tool_call_update` | `Tool` patch of the same; an unknown id is treated as a new `tool_call` |
| tool output | first `diff` content → `ToolOutput::Diff` with hunks computed from old and new text; else the `content` texts → `Text`; else `rawOutput` as string → `Text`, with `stdout`/`stderr` → `Terminal` |
| `plan` | `Todos(list)` and `Put Todos` (key `plan-<g>-<turn>`, replaced within the turn) |
| `current_mode_update` | `Mode(id)` |
| `config_option_update` | `Model`, `Effort`, `Mode` from the options classed so, by value id |
| `session_info_update` with a title | `Title` |
| `usage_update` | `Usage`: context used and size; cumulative cost in the reported currency or unit, including credits (USD also fills `cost_usd`) |
| `available_commands_update` | capabilities only |
| anything else (unstable or unknown) | ignored |

## Turns, permissions, errors

| Event | Thread |
|---|---|
| Prompt sent | `Put UserMessage`, `Status(Running)` |
| `session/prompt` answered | open text `Finish`ed, `Status(Idle)`; token report from `usage`, else `_meta.quota.token_count` (Codex camelCase or Gemini snake_case); stop reason `cancelled` → `Notice::Interrupted`, any other than `end_turn` → `Notice::Error { Turn, <reason> }` |
| `session/prompt` failed | `Notice::Error { Api, <message> }`, `Status(Idle)` |
| `session/request_permission` | the tool call updated from the request; `Put PermissionRequest` (key `permission-pN`) with name, title and input of the tool call, options in the agent's order (`allow_once` → `AllowOnce`, `allow_always` → `AllowAlways { rule: <name> }`, rejects → `Deny`); `Status(RequiresAction)` |
| Answer | `Choice::Option(n)` → `selected` with the agent's option id, `Choice::Dismiss` → `cancelled`; `Resolve Decided(Allowed { always } \| Denied)`; `Status(Running)` once none is open |
| Request without options | answered `cancelled` at once (E21) |
| Cancel | open permission requests answered `cancelled` and `Resolve Withdrawn`, then `session/cancel` |
| Any other request from the agent (`fs/*`, `terminal/*`, unknown) | answered `method not found` |
| Process ends unasked | open requests withdrawn; while a turn ran or the connection failed: `Notice::Error { Process, <end of stderr> }`, `Status(Idle)`; `Event::Ended` |

Token reports are displayed as the latest reported usage, with the agent's total and breakdown preserved.
They are not accumulated: ACP's experimental `PromptResponse.usage` has
[conflicting turn/session semantics](https://github.com/agentclientprotocol/agent-client-protocol/issues/1860)
in the schema and agents. Cache and reasoning counts may overlap input/output; neither is added to the reported
total. A token report does not imply context occupancy or subscription limits. In particular, the Codex/Gemini
`quota.token_count` extension reports tokens, not a five-hour or weekly allowance
([Codex](https://github.com/agentclientprotocol/codex-acp/blob/main/src/QuotaMeta.ts),
[Gemini](https://github.com/google-gemini/gemini-cli/blob/main/packages/cli/src/acp/acpSession.ts)). Missing data stays unknown.
The account usage cache of the native Claude connection is only used by its own tabs.

## Register and installation (E5, R8)

The registry (`https://cdn.agentclientprotocol.com/registry/v1/latest/registry.json`) is fetched only when the user
asks and kept in `<cache>/acp-registry.json`; entries keep only this Mac's build (`darwin-aarch64` or
`darwin-x86_64`), distributions this build does not know are listed in `Entry::unknown`.

| Preset | Own command | Pinned install (registry of 2026-11-09) | Sandbox (E19) | What it limits |
|---|---|---|---|---|
| Codex | `codex-acp` | npm `@agentclientprotocol/codex-acp@2.6.0`, integrity checked in the lock file | `INITIAL_AGENT_MODE=workspace-write` (codex-acp has no `-s`; the mode carries the policy of `codex -s workspace-write`) | commands and edits write only in the project, no network |
| Gemini CLI | `gemini --acp` | npm `@google/gemini-cli@0.67.0`, Node ≥ 20 | `--sandbox` | macOS profile `permissive-open`: writes only in the project, reading and network open |
| Antigravity | `agy acp` (official way) | `agy-acp-server` 1.4.5 archive, SHA-256 | none known | — |
| OpenCode | `opencode acp` | archive 2.2.6, SHA-256 | none known | — |
| Claude (ACP fallback, E21) | `claude-agent-acp` | archive 0.102.0, SHA-256 | `_meta.claudeCode.options.sandbox` = `{enabled, failIfUnavailable}` | shell commands only |

Rules:

- A preset installs its pinned version with its checksum. It counts as a tested combination only once
  `make test-integration` verified it and its `verified` flag is set; until then (all presets as of 2026-11-10) it
  asks for confirmation like a registry entry. Raising a pin resets the flag until the new version is verified.
- A registry entry installs only an exact version (`1.2.3`, not `latest` or a range): its archive for this Mac, else
  its npm package (`npm install --prefix`, npm checks its registry's hashes), else its uv package (`uv tool install
  pkg==version` into the agent's folder). Every registry entry asks for confirmation, as it was not tried with this
  build; one without a checksum says so in the question.
- Archives are checked against their SHA-256 before unpacking (`ditto` for zip, `tar` otherwise); the program must
  lie inside the agent's folder.
- Each agent keeps the current and the previous version; `Installs::rollback` swaps them, so an update can be taken
  back and the step undone; older versions are removed.
- Node, npm and uv are looked up on the login shell's PATH; a missing or too old one is named before anything is
  downloaded.

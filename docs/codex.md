# Codex over its app-server (E147)

Dango runs Codex through `codex app-server`, the protocol the Codex SDK builds on, and falls back to the
`codex-acp` adapter (`docs/acp.md`) when that is not possible. The crate is `crates/ide-codex`; the AI tab's side is
`crates/ide-ui-agent/src/codex.rs` and the overlays' backend `dialogs/backend/codex.rs`.

## Route

| Item | Value |
|---|---|
| Choice | The Codex entry of the agents (preset `codex-acp`); there is no second Codex agent |
| Tested range | Codex `0.167.0` and newer (`ide_codex::TESTED`); `codex --version` is read through the supervisor before each start |
| Fallback | `codex` missing from the login shell's PATH, too old or unreadable: the tab starts `codex-acp` over ACP |
| Thread | Its agent stays `codex-acp`, so a restored thread opens the same choice; the Codex thread id is the thread's session id and goes to `thread/resume` |
| Self-test | The app itself is the server with `IDE_CODEX_TEST_SERVER` (`ide_codex::test_server`); `set_test_codex` replaces the route, and without it no Codex on the Mac starts |

## Framing and handshake

- One JSON-RPC 2.0 message per line on stdio, **without** the `"jsonrpc"` member (`wire.rs`). Not `ide-rpc`, which
  frames with `Content-Length`.
- `initialize` with `clientInfo` and `capabilities.experimentalApi: false` (`true` on the shared server, see Remote
  Control), then the notification `initialized`. Requests before that are refused by the server.
- Then `account/read`, `thread/start` or `thread/resume` (cwd, model, effort, approval policy, sandbox) and
  `model/list`.

## Turns

| Dango | Codex |
|---|---|
| Send | `turn/start` with the text and `localImage`/`mention` inputs, `clientUserMessageId` |
| Stop | `turn/interrupt` |
| Model, effort | the next `turn/start`'s overrides (`set_model`, `set_effort`) |
| Mode | a preset of approval policy and sandbox (`ide_codex::MODES`: `read-only`, `auto`, `full-access`, the ids `codex-acp` uses) |
| Approval cards | `item/commandExecution/requestApproval`, `item/fileChange/requestApproval`, `item/permissions/requestApproval`; the answer's `decision` is `accept`, `acceptForSession`, `decline` or `cancel` |
| Questions | `item/tool/requestUserInput`, `mcpServer/elicitation/request` |
| Usage | `thread/tokenUsage/updated`, `account/rateLimits/updated` |
| Rewind (I6) | each user message is chained to its turn; rewinding before a later prompt sends `thread/revert {threadId, beforeTurnId}` and resumes the cut thread, before the first prompt a new thread starts; files go back through Dango's own checkpoints (E40), as `thread/revert` leaves them |
| History | `thread/list` of the folder after each start, kept in `codex-threads.json` of the data folder; the Agents panel lists the threads the app has not seen and continues them with Codex |

The items `agentMessage`, `reasoning`, `plan`, `commandExecution`, `fileChange`, `mcpToolCall`, `webSearch` and
`contextCompaction` become the thread model's entries (`mapping.rs`, `mapping/cards.rs`). Tool cards carry the names
Claude Code uses (`Bash`, `Edit`, `Write`, `WebSearch`), so every agent's cards read alike.

## Overlays (E146)

| Command | Codex |
|---|---|
| `/status` | release, account and plan, model, effort, folder, `account/rateLimits/read` (without limits for a key or a provider) |
| `/export` | Dango's own thread as text |
| `/mcp` | `mcpServerStatus/list`; Reconnect `config/mcpServer/reload`; Sign in `mcpServer/oauth/login`; Add `config/value/write` on `mcp_servers.<name>` and a reload. Switching and removing stay in `config.toml` (greyed) |
| `/config` | the model, effort and mode as choices; `config.toml` to open |
| `/memory` | `AGENTS.md` of the project |
| `/skills` | `skills/list`, `skills/config/write` |
| `/logout` | `account/read`, `account/logout` |
| `/bug` | `feedback/upload` with the logs, after a confirmation that names what goes to OpenAI (R43) |

Not offered: `/btw` (a fork's turn would need a second thread on the connection), `/plugins` (`plugin/list` is
marked as in development), `/permissions` and `/hooks`.

## Remote Control (E148)

The phone reaches only Codex's shared app-server, the daemon `codex app-server daemon` manages. Remote Control belongs
to it, so it is on or off for every Codex session of the Mac, unlike Claude's, which is one session's.

| Item | Codex |
|---|---|
| Shared server | `codex app-server proxy` hands stdio through to the daemon's socket byte for byte (`Launch::shared`). The socket speaks WebSocket (`ws.rs`): the client sends the upgrade, waits for `101`, as the daemon never answers a connection whose frames came earlier, then sends each message as a masked text frame and answers pings; `-c` overrides do not reach the daemon |
| When | the daemon's saved choice `remoteControlEnabled` in `$CODEX_HOME/app-server-daemon/settings.json` is on (`ide_codex::remote::wanted`); `codex app-server daemon start` makes sure it runs, and if it does not start, the tab keeps a server of its own |
| On | the phone in the tab's head runs `codex remote-control start --json`, which starts the daemon, turns Remote Control on and saves that; a tab with a server of its own then moves to the daemon once the agent is done, resuming its thread there |
| State | `remoteControl/status/changed`, sent after `initialize` and on each change: `disabled`, `connecting`, `connected` with the `serverName` the phone shows, `errored` |
| Pairing | `codex remote-control pair --json`: `manualPairingCode` and `expiresAt`, shown in an alert |
| Off | `remoteControl/disable`, saved for the daemon's next start; of the experimental API, which `initialize` asks for only on the shared server |
| Other clients | every connection hears the threads other clients start after it connected (`thread/started` with `parentThreadId`); the session leaves the requests and turns of a thread that is neither its own nor descends from it to that client, unanswered |
| Process | the daemon is no child of the supervisor. Started by Dango it inherits the login shell's environment (E10, E23); one the Codex app or a terminal started keeps its own, which the commands Codex runs then get (R46) |
| Self-test | the stand-in answers `app-server proxy` as the shared server over WebSocket, in split frames with a ping, refusing frames that come before its answer to the upgrade, and `remote-control start`, `remote-control pair` and `app-server daemon start` as the CLI, with the saved choice in its `CODEX_HOME` |

## Schemas

`crates/ide-codex/tests/fixtures/schema/` holds the JSON schemas `codex app-server generate-json-schema` wrote for
0.167.0; the tests check what the crate sends and reads against them. A newer release is taken as long as its
handshake succeeds; the protocol is marked experimental (R41).

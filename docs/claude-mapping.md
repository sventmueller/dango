# Claude stream-json → thread model

How `ide-claude` turns the frames of the `claude` CLI into `ide_thread::Update`s (E4, E21). The table is also the
specification of the replay tests in `crates/ide-claude/tests/replay.rs`; every row names the fixture that covers it.
Shapes follow `@anthropic-ai/claude-agent-sdk@0.3.319` (`sdk.d.ts`) and were checked against recordings of CLI
2.1.319.

Tier: **core** — acceptance depends on it; **exp.** — experimental (CLI marks it so, or the format is internal); the
UI hides the element when it fails.

## Process

| Item | Value |
|---|---|
| Command | `claude --print --input-format=stream-json --output-format=stream-json --verbose --include-partial-messages --include-hook-events --permission-prompt-tool=stdio --replay-user-messages` |
| Options | `--model=`, `--permission-mode=` (new threads: `default`, E27), `--effort=`, `--thinking=adaptive\|disabled` or `--max-thinking-tokens=N`, `--thinking-display=summarized` (default), `--resume=<session>` with optional `--resume-session-at=<message uuid>`, `--resume-drops-turn=<prompt uuid>` (only with both before it) and `--fork-session` (rewinding the conversation as a branch, I6), `--allowedTools=`, `--disallowedTools=`, `--add-dir=`, `--no-session-persistence` (only when `Launch::persist` is off), sandbox (E19, off by default): `--settings={"sandbox":{"enabled":true,"failIfUnavailable":true}}` |
| Environment | the login shell's (`ide_process::shell_env`), without `CLAUDE_CODE_ENTRYPOINT` so the CLI never takes the app for the VS Code extension; `CLAUDE_CODE_ENABLE_SDK_FILE_CHECKPOINTING=true` only with `Launch::file_checkpoints` (needed by `rewind_files`) |
| Supervisor | `Kind::Agent`, label `claude`; graceful stop = close stdin (`on_stop`), then SIGTERM/SIGKILL to the group |
| First frame | `initialize` with `perTaskStopAffordance: true` (an interrupt spares background tasks, E23), `agentProgressSummaries`, `forwardSubagentText` |
| Framing | one JSON object per line both ways; lines that are not JSON objects are skipped |
| Credentials | never read or written; a missing sign-in becomes `Event::Failed(SessionError::SignInRequired)` and the UI suggests running `claude` in a terminal |

## Entry keys

Updates address entries by key, so later frames can change them in place.

| Entry | Key |
|---|---|
| User message | `user:<uuid>` (uuid the app sends, which `rewind_files` names) |
| Streamed text / thought | `<message id>:<content block index>` |
| Text / thought without a stream | `<message id>:f<n>` |
| Tool call, to-do list, question, plan approval | the `tool_use` id |
| Permission request | `perm:<request id>` |
| Retry notice | `retry:<turn>` (one per turn, updated in place) |
| Hook notice | `hook:<hook id>` |
| Rate-limit notice | `limit:<window>:<status>` |

Lanes: frames with `parent_tool_use_id = P` go to `Lane::Subagent(P)`; `P` is also the subagent's key.

## Resume points (I6)

Every frame of the main conversation with a `uuid` — the prompts the app sends, `assistant` frames, `user` frames
(tool results, and also those the timeline leaves out, such as synthetic ones) — is a link of the CLI's chain; the
transcript stores the same uuids. The mapper keeps the chain in order and sends `Update::Chain` for the entries a
frame belongs to:

| Field | Meaning | Use |
|---|---|---|
| `Entry::uuid` | the entry's latest frame: a prompt's own uuid, a text or thought block's `assistant` frame, a tool call's `tool_result` frame (its `assistant` frame until the result comes) | a prompt's `drops_turn` (see below) |
| `Entry::follows` | the frame before the entry's first frame, kept from the first time it is set | a prompt's `fork_at` (see below); none for the conversation's first prompt |
| `Thread::resume_point(id)` | the entry's own `uuid`, else the nearest earlier one of the main timeline | locating an entry without a frame, such as a notice, in the conversation |

Subagent lanes, notices and permission requests carry no frame. Retracted frames
(`supersedes`, refusal fallback) leave the chain; `conversation_reset` starts it afresh. `UserMessage::message_id`
stays the prompt's uuid, which `rewind_files` names.

The conversation is rewound only to just before a prompt, never inside a turn, and always guarded:

| Step | API | CLI |
|---|---|---|
| Rewind point | `Thread::rewind_before(prompt) -> Option<Rewind { fork_at, drops_turn }>`: `fork_at` = the prompt's `follows`, the kept turn's last frame; `drops_turn` = the prompt's uuid. None for other entries, subagent lanes and the first prompt (a new session starts instead) | — |
| Launch | `Launch::rewind(session, &rewind)` sets `resume`, `resume_at`, `resume_drops_turn`, `fork_session` | `--resume=… --resume-session-at=<fork_at> --resume-drops-turn=<drops_turn> --fork-session` |
| Refusal | `result` `error_during_execution` whose error starts with `Resume rejected by --resume-drops-turn:` → Notice `RewindRejected` (not an error), `Event::Failed(SessionError::RewindRejected)`; the process ends as expected (no `Process` notice) | deterministic: resume plainly (without the three rewind fields), never retry |

The kept turn's last frame is the last chain frame on stdout before the prompt: a normal turn's final `assistant`
frame; an interrupted turn's tool results and the `[Request interrupted by user for tool use]` marker, which the
timeline does not show. CLI 2.1.319 accepted both with the guard (`recorded_question`,
`recorded_interrupted_tools`); the `prompt_snapshot` attachment the transcript keeps between a turn and the next
prompt is skipped by the guard. Not covered: turns that end on a `structured_output` attachment (`--json-schema`, MCP
end-turn tools), whose last entry never appears on stdout; the app uses neither, and the guard refuses rather than
losing that output.

## Frames from the CLI

| Frame (`type` / `subtype`) | Thread update | Tier | Fixture |
|---|---|---|---|
| `system/init` | `Session`, `Model`, `Mode`, `Effort`; capabilities (`tools`, `slash_commands`, `agents`, `skills`, `mcp_servers`, `capabilities`, version) | core | plain_thinking, future |
| `stream_event` `message_start` | remembers the message id per lane; `Usage.context_used` = input + cache tokens (main lane) | core | plain_thinking |
| `stream_event` `content_block_start` text / thinking | `Put` AgentMessage / Thought, `streaming: true` | core | plain_thinking |
| `stream_event` `content_block_start` tool_use | `Put` ToolCall `Pending` (or the to-do / question / plan entry) | core | recorded_* |
| `stream_event` `content_block_delta` text_delta / thinking_delta | `Append` | core | plain_thinking |
| `stream_event` `content_block_delta` input_json_delta, signature_delta | ignored (the complete input arrives with `assistant`) | — | recorded_* |
| `stream_event` `content_block_stop` | `Finish` (thought gets its duration) | core | plain_thinking, interrupt |
| `stream_event` `message_delta` | `Usage.context_used` = input + output (main lane) | core | plain_thinking |
| `assistant` text / thinking block | `Finish` with the final text when the block streamed (matched by message id and order), else `Put` | core | plain_thinking, recorded_* |
| `assistant` tool_use / server_tool_use | ToolCall: kind, title, typed view, input, `InProgress`, `started`; `Agent`/`Task` also `Subagent` (description, type, background) | core | recorded_* |
| `assistant` `TodoWrite` | `Put` Todos entry; `Todos` on the thread (main lane) | core | plan_todos |
| `assistant` `AskUserQuestion` | `Put` Question (`Pending`, no request id yet) | core | recorded_question |
| `assistant` `ExitPlanMode` | `Put` PlanApproval with the plan markdown | core | plan_todos |
| `assistant` with `error` | Notice Error (`Auth` for `authentication_failed`, `oauth_org_not_allowed`, `account_on_hold`, `verification_required`, `cloud_credential_error`; else `Api`); auth also `Event::Failed(SignInRequired)` | core | recorded_signed_out |
| `assistant` `supersedes` | `Remove` the entries of the named frames | core | — |
| `assistant` / `user` frame of the main lane with `uuid` | `Chain { key, uuid, follows }` for each entry the frame fills (see Resume points) | core | recorded_question, recorded_edit_diff, plain_thinking |
| `user` tool_result | ToolCall `Completed`/`Failed`, `ended`, elapsed, output: Bash → Terminal (stdout, stderr, interrupted); Edit/MultiEdit/Write/NotebookEdit → Diff from `structuredPatch` (a created file as added lines); else Text; Grep/Glob/ToolSearch → result count; `backgroundTaskId` → `background_task` | core | recorded_* |
| `user` tool_result of `Agent`/`Task` | Subagent final status, tokens, tool uses, duration from `tool_use_result` | core | recorded_subagent_background |
| `user` tool_result of AskUserQuestion / ExitPlanMode | `Resolve` Answered (from `answers`) / Approved; `is_error` → Denied / Rejected. A plan the app answered in this session keeps that answer; only a plan read back from a transcript is resolved from the result, with `stop` when the text is `PLAN_REJECT_MESSAGE` and no feedback for the app's fixed messages | core | recorded_question, plan_todos |
| `user` text with `parent_tool_use_id` | `Put` UserMessage in the subagent lane (its prompt) | core | recorded_subagent_background |
| `user` text in the main lane | ignored live (the app put the message itself); used by `Mapper::for_history` | exp. | — |
| `user` with `isReplay` of a prompt the app sent (its uuid) | ignored | core | — |
| `user` with `isReplay` of any other prompt of the main lane (typed on claude.ai through Remote Control), without `isSynthetic`, with no `origin` or `origin.kind` `human`/`remote`, text not starting with `<` | `Put` UserMessage `user:<uuid>`, chained, `Status::Running`, as if the app had sent it | exp. | — |
| `user` with `isSynthetic`, and echoes of task notifications or CLI tags | ignored | — | — |
| `system/bridge_state` | Remote Control: `failed`/`policy_disabled` of the current connection (`bridge_epoch`) → `Failed(detail)`; `connected` after that → `Connected` again; others ignored | exp. | — |
| `system/status` | `Activity` Requesting / Compacting / none; `Mode` from `permissionMode`; `compact_result: failed` → Notice Error `Compaction` | core | compaction |
| `system/compact_boundary` | Notice Compaction (manual/auto, tokens before/after, `freed_tokens()`); `context_used` = post tokens | core | compaction |
| `system/api_retry` | Notice Retrying (attempt, max, delay, error, HTTP status), one per turn | core | api_retry |
| `rate_limit_event` | `RateLimit` per window (status, utilization 0–1, reset time); not `allowed` → also a RateLimit notice | core | rate_limit |
| `system/task_started` `local_agent` | `Subagent` (task id, type, description, Running, backgrounded, cancellable, parent from `parent_task_id`) | core | recorded_subagent_background, late_runs |
| `system/task_started` other types | links the tool call to its background task; refreshes the background items | core | recorded_subagent_background |
| `system/task_progress` | Subagent elapsed, tokens, tool uses, last tool, summary | core | recorded_subagent_background, late_runs |
| `system/task_updated` | Subagent status (`killed` → Stopped), end, error → summary; other tasks: BackgroundItem status | core | recorded_subagent_background, late_runs |
| `system/task_notification` | Subagent final (status, summary, usage, not cancellable); other tasks: BackgroundItem status and output file | core | recorded_subagent_background |
| `system/background_tasks_changed` | `Background` — REPLACE: the payload is the whole set; start time, tool call and output stay with an item that remains | core | recorded_subagent_background, late_runs |
| run-id rule (all `task_*`, background entries) | a frame naming a run that sorts before the run already known for the task is late and ignored; a background set naming an older run is ignored whole | core | late_runs |
| `tool_progress` | ToolCall `InProgress`, elapsed, start | core | — |
| `tool_use_summary` | `summary` on each preceding tool call | core | — |
| `system/hook_started` / `hook_progress` / `hook_response` | Notice Hook (name, event, output or stderr, exit code, Running/Success/Error/Cancelled) | exp. | hooks |
| `system/session_state_changed` | `Status` idle / running / requires_action (authoritative when present) | core | hooks |
| `result` refusing a guarded rewind (error starts with `Resume rejected by --resume-drops-turn:`) | Notice `RewindRejected` with the CLI's explanation, `Status` Idle; `Event::Failed(SessionError::RewindRejected)` | core | recorded_rewind_rejected |
| `result` | finishes open streams; `Usage`: cost, token totals over `modelUsage`, context size from the model's `contextWindow`, `models` (per model, most expensive first) and a `TurnUsage` (see below); error → Notice Error `Turn` (`Auth` when it asks for `/login`, once per turn); `terminal_reason` `aborted_*` → open tool calls `Failed` and Notice Interrupted; `Activity` none, `Status` Idle; the session then asks `get_context_usage` | core | plain_thinking, interrupt, future, context_cost |
| `system/informational` | Notice Info with level | core | hooks |
| `system/local_command_output` | AgentMessage | core | — |
| `system/notification` | Notice Info | core | — |
| `system/permission_denied` | ToolCall `Failed` (its tool_result follows) | core | — |
| `system/model_refusal_fallback` / `_no_fallback` | `Remove` retracted frames, Notice Info warning | core | — |
| `system/session_writes_lost` | Notice Error `Disk` | core | — |
| `system/commands_changed` | capabilities: slash commands replaced | core | — |
| `conversation_reset` | `Clear`, background items re-sent | core | hooks |
| `auth_status` with `error` | Notice Error `Auth` | core | — |
| `system/thinking_tokens`, `prompt_suggestion`, `keep_alive`, `command_lifecycle`, `system/memory_recall`, `files_persisted`, `plugin_install`, `elicitation_complete`, `mirror_error`, `worker_shutting_down`, `control_request_progress` | ignored | — | recorded_*, plain_thinking |
| unknown `type`, unknown `system` subtype, known type whose fields do not fit, non-JSON line | ignored; the frame is kept as raw JSON (`Message::Unknown`); the session goes on (R7) | core | future |

Process end (stdout closed): open streams finished, waiting requests withdrawn, running tool calls `Failed`, running
subagents `Stopped`, background set emptied (the level is per process), `Status` Idle. An end the app did not ask for
(neither `close_input`, `stop`, the supervisor nor a refused rewind) adds Notice Error `Process` with the last stderr line; stderr
asking for a login also raises `SignInRequired`.

## Control requests from the CLI

| Request | Handling | Tier |
|---|---|---|
| `can_use_tool` (any tool) | `Put` PermissionRequest `perm:<id>`: tool, title, description, reason, blocked path, input (editable), options AllowOnce / AllowAlways per allow suggestion (rule text + scope; none when `suppress_always_allow_rule`) / Deny, `default_to_no`; tool call back to `Pending`; `Status` RequiresAction | core |
| `can_use_tool` AskUserQuestion | the Question entry gets the request id (`Pending`) | core |
| `can_use_tool` ExitPlanMode | the PlanApproval entry gets the request id (`Pending`) | core |
| `control_cancel_request` | `Resolve` Withdrawn | core |
| anything else (`hook_callback`, `mcp_message`, `elicitation`, `request_user_dialog`, unknown) | answered with an error response, never allowed (E21) | core |

Answers (`Session::answer`, `answer_questions`, `answer_plan`):

| Decision | Response payload | Thread |
|---|---|---|
| Allow once | `behavior: allow`, `updatedInput` = the input, edited or not, `toolUseID`, `decisionClassification: user_temporary` | Allowed { edited }; tool call `InProgress` |
| Allow always | as above plus `updatedPermissions` = the chosen suggestions verbatim, `user_permanent` | Allowed { always } |
| Deny | `behavior: deny`, `message`, optional `interrupt`, `user_reject` | Denied |
| Questions | allow with `updatedInput.answers = { question: "label, label" }` | Answered |
| Plan (`PlanAnswer`) | approve = allow; revise = deny with the feedback as message (the user's comments, each under the passage it quotes as a markdown blockquote); reject = deny with `PLAN_REJECT_MESSAGE` and `interrupt` | Approved / Rejected { feedback, stop } |

## Control requests the app sends

| Method | Request | Answer | Timeout | Thread update | Tier |
|---|---|---|---|---|---|
| (start) | `initialize` | `InitializeResult` (commands, models, account, version) | 60 s | capabilities; `tokenSource: none` on Anthropic's API without a key → `SignInRequired`; `Usage.subscription` from the account: a `subscriptionType` on Anthropic's own API without an API key (E57); a replay reads it from the recorded answer too | core |
| `interrupt` | `interrupt` | `InterruptResult { still_queued }` | 10 s | via the `result` that follows | core |
| `set_permission_mode` | `set_permission_mode { mode }` | `()` | 10 s | `Mode` | core |
| `set_model` | `set_model { model }` | `()` | 10 s | `Model` | core |
| `set_effort` | `apply_flag_settings { settings: { effortLevel } }` | `()` | 10 s | `Effort` | core |
| `stop_task` | `stop_task { task_id }` | `()` | 10 s | via `task_updated` / `task_notification` | core |
| `background` | `background_tasks { tool_use_id? }` | `bool` (`backgrounded`, true when absent) | 10 s | via `task_*` and `background_tasks_changed` | core |
| `task_output` | `get_task_output { task_id }` | `TaskOutput { output, total_bytes, truncated }` | 10 s | BackgroundItem output tail | core |
| `context_usage` (and by itself after every `result`, as `summary`) | `get_context_usage { detail: summary\|full }` | `ContextUsage` (categories, totals, memory files, MCP tools, agents, skills, built-in tools, messages) | 30 s | `context_used`, `context_size`, `breakdown` (`exact` for `full`); a refusal stops the asking after turns | exp. |
| `usage` (and at the start of a resumed session) | `get_usage { skip_behaviors: true }` | `UsageReport` (session cost and model totals, subscription, windows, raw) | 30 s | the session's totals a resumed session's next turn is counted from | exp. |
| `rewind_files` | `rewind_files { user_message_id, dry_run }` | `RewindResult` | 30 s | — (I6 checkpoints stay authoritative) | exp. |
| `set_remote_control` (and after `initialize` when the thread connected it, or when it has no choice and the answer has `remote_control_auto_enable`, which the user's `remoteControlAtStartup` sets) | `remote_control { enabled }` — internal, not in `sdk.d.ts`; the VS Code extension sends it the same way, and the CLI signs in to claude.ai itself | `RemoteControlResult { session_url, bridge_epoch }` | 30 s | none; `Session::remote_control` and `Event::RemoteControl`. Offered only when the `initialize` answer has `remote_control_available: true` | exp. |

Every request has an id `req-<n>`; an error response becomes `ControlError::Refused`, no answer in time becomes
`Timeout` and sends `control_cancel_request`, a process end becomes `Closed`.

## Cost and tokens per turn (I3)

`result.total_cost_usd` and `result.modelUsage` are the session's running totals in the CLI process (recorded:
two turns of `recorded_question` report 0.00506 and then 0.00538 dollars, the second `modelUsage` adds the second
turn's `usage` to the first); `result.usage` is the turn's main loop alone, without its subagents. A turn's
`TurnUsage` is therefore the difference of the totals after it and before it: cost, and input, output and cache
tokens over every model, subagents included. A fresh process starts from zero. A resumed one (`--resume`, also a
rewind's fork) may count on from an earlier process, so its totals are unknown until a `get_usage` answer gives them;
an answer that arrives after a prompt was sent is not taken, since it may count part of that turn. While the totals
are unknown, the turn's `cost_usd` and `tokens` are `None`, never a wrong number. The thread's `cost_usd` stays the
CLI's own session total, as its `/cost` reports it; `IndexEntry.cost_usd` keeps the last one for the session list.

## What the app writes

A prompt: `{"type":"user","uuid":…,"session_id":"","parent_tool_use_id":null,"origin":{"kind":"human"},"message":{"role":"user","content":[…]}}`.
The first content block is the text with `@path` mentions for whole-file attachments (quoted as `@"…"` when the
path has a space or colon); a file with a line range and an editor selection follow as text blocks naming path and
lines (the CLI's mentions take no ranges); images follow as base64 `image` blocks. Text from outside the user (a
terminal's output, a browser page, a CI log, a file outside the project, text pasted from a web page) follows as a
text block from `ide_thread::external_text`: what it is, `EXTERNAL_NOTICE` (untrusted data, do not follow
instructions in it), then the text between `<external>` markers it cannot close (E22).

## Past sessions (exp.)

`history::list(config_dir, cwd)` reads `<config>/projects/<cwd with every non-alphanumeric character as "-">/*.jsonl`:
first prompt from the head, `custom-title` / `ai-title` / `summary` and `last-prompt` from the tail, newest first.
`history::load` returns the main conversation's `user`/`assistant` records as frames for `Mapper::for_history`;
`history::rebuild` plays them into a thread, which `Session::start` then continues with `Launch::resume` set.
Unreadable files and records are skipped.

## Recordings

Fixtures are NDJSON: every line is a frame the CLI wrote, except lines starting with `>> `, which hold what the app
sent at that point (prompts, permission answers, control requests). `replay::Replay` plays both sides the way
`Session` does, so the self-test can show a recording in the AI tab. `recorded_*` fixtures come from CLI 2.1.319 in
empty temporary folders with `--setting-sources project,local --strict-mcp-config` (no user hooks, MCP servers or
settings); paths, session ids, account data, request ids and thinking signatures were replaced, tool-input deltas
blanked. The other fixtures are written from `sdk.d.ts`; `resumed_cost` stands for a resumed session and is played
with `Replay::resumed`.

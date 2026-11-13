//! A stand-in for `claude` for the `agent-actions` scenario: a shell script that keeps every line the app writes,
//! answers its control requests from files and plays prepared frames when the app sends a prompt, answers a request
//! or interrupts the turn. No real CLI starts and nothing leaves the Mac.

use std::path::{Path, PathBuf};

use serde_json::{Value, json};

/// The session id the stand-in's frames name.
pub const SESSION: &str = "51e1f7e5-4000-4000-8000-0000000000aa";

const SCRIPT: &str = r#"#!/bin/sh
dir='@DIR@'
printf '%s\n' "$@" > "$dir/args.txt"
turns=0
answers=0
while IFS= read -r line; do
  printf '%s\n' "$line" >> "$dir/written.ndjson"
  case "$line" in
    *'"type":"control_request"'*)
      id=$(printf '%s\n' "$line" | /usr/bin/sed -n 's/.*"request_id":"\([^"]*\)".*/\1/p')
      payload='{}'
      refused=''
      after=''
      for kind in initialize interrupt set_permission_mode set_model apply_flag_settings stop_task background_tasks get_task_output get_context_usage get_usage rewind_files remote_control; do
        case "$line" in
          *"\"subtype\":\"$kind\""*)
            [ -f "$dir/refuse-$kind.txt" ] && refused=$(/bin/cat "$dir/refuse-$kind.txt")
            [ -f "$dir/answer-$kind.json" ] && payload=$(/bin/cat "$dir/answer-$kind.json")
            after="$dir/on-$kind.ndjson"
            break;;
        esac
      done
      if [ -n "$refused" ]; then
        printf '{"type":"control_response","response":{"subtype":"error","request_id":"%s","error":"%s"}}\n' "$id" "$refused"
      else
        printf '{"type":"control_response","response":{"subtype":"success","request_id":"%s","response":%s}}\n' "$id" "$payload"
      fi
      [ -n "$after" ] && [ -f "$after" ] && /bin/cat "$after"
      ;;
    *'"type":"control_response"'*)
      answers=$((answers + 1))
      [ -f "$dir/after-$answers.ndjson" ] && /bin/cat "$dir/after-$answers.ndjson"
      ;;
    *'"type":"user"'*)
      turns=$((turns + 1))
      [ -f "$dir/turn-$turns.ndjson" ] && /bin/cat "$dir/turn-$turns.ndjson"
      [ -f "$dir/turn-$turns.ndjson.then" ] && /bin/mv "$dir/turn-$turns.ndjson.then" "$dir/turn-$turns.ndjson"
      ;;
  esac
done
"#;

/// The stand-in's folder: the script, the answers and frames it gives, and what it saw.
pub struct StandIn {
    pub dir: PathBuf,
}

impl StandIn {
    /// Writes the script and its answers into `dir`.
    pub fn new(dir: &Path, cwd: &Path) -> std::io::Result<StandIn> {
        let _ = std::fs::remove_dir_all(dir);
        std::fs::create_dir_all(dir)?;
        let stand_in = StandIn { dir: dir.to_path_buf() };
        let script = stand_in.program();
        std::fs::write(&script, SCRIPT.replace("@DIR@", &dir.to_string_lossy()))?;
        std::fs::set_permissions(&script, std::os::unix::fs::PermissionsExt::from_mode(0o755))?;
        stand_in.write("answer-initialize.json", &initialize().to_string())?;
        stand_in.write("refuse-set_permission_mode.txt", "plan mode is not available here")?;
        stand_in.write(
            "answer-remote_control.json",
            &json!({"session_url": "https://claude.ai/code/session_selftest", "bridge_epoch": 1}).to_string(),
        )?;
        stand_in.write(
            "answer-get_task_output.json",
            &json!({"output": "tick 1\ntick 2\n", "total_bytes": 14, "truncated": false}).to_string(),
        )?;
        stand_in.write(
            "answer-background_tasks.json",
            &json!({"backgrounded": true}).to_string(),
        )?;
        stand_in.write(
            "answer-get_usage.json",
            &json!({"session": {"total_cost_usd": 0.42}, "subscription_type": "max",
                    "rate_limits": {"five_hour": {"utilization": 42.0, "resets_at": "2026-11-09T22:00:00Z"}}})
            .to_string(),
        )?;
        stand_in.write(
            "answer-rewind_files.json",
            &json!({"canRewind": true, "filesChanged": ["hello.txt"], "insertions": 1, "deletions": 1}).to_string(),
        )?;
        stand_in.frames("turn-1.ndjson", &first_turn(cwd))?;
        stand_in.frames("after-1.ndjson", &after_rules(cwd))?;
        stand_in.frames("after-2.ndjson", &after_fetch())?;
        stand_in.frames("after-3.ndjson", &after_always())?;
        stand_in.frames("after-4.ndjson", &after_deny())?;
        stand_in.frames("after-5.ndjson", &after_question())?;
        stand_in.frames("after-6.ndjson", &after_words())?;
        stand_in.frames("after-7.ndjson", &after_plan())?;
        stand_in.frames("on-interrupt.ndjson", &[result(9)])?;
        stand_in.frames("turn-2.ndjson", &second_turn())?;
        stand_in.frames("turn-3.ndjson", &[text(22, "Ich sehe die Anhänge."), result(23)])?;
        Ok(stand_in)
    }

    /// The program that stands in for `claude`.
    pub fn program(&self) -> PathBuf {
        self.dir.join("claude")
    }

    fn write(&self, name: &str, text: &str) -> std::io::Result<()> {
        std::fs::write(self.dir.join(name), text)
    }

    fn frames(&self, name: &str, frames: &[Value]) -> std::io::Result<()> {
        let text: String = frames.iter().map(|f| format!("{f}\n")).collect();
        self.write(name, &text)
    }

    /// The arguments the program was last started with.
    pub fn args(&self) -> Vec<String> {
        std::fs::read_to_string(self.dir.join("args.txt"))
            .unwrap_or_default()
            .lines()
            .map(str::to_string)
            .collect()
    }

    /// Every line the app wrote to the program, as JSON.
    pub fn written(&self) -> Vec<Value> {
        std::fs::read_to_string(self.dir.join("written.ndjson"))
            .unwrap_or_default()
            .lines()
            .filter_map(|l| serde_json::from_str(l).ok())
            .collect()
    }

    /// The app's answer to the program's request `id`.
    pub fn answer_to(&self, id: &str) -> Option<Value> {
        self.written().into_iter().find_map(|frame| {
            let response = frame.get("response")?;
            (frame["type"] == "control_response" && response["request_id"] == id).then(|| response["response"].clone())
        })
    }

    /// The bodies of the control requests of `subtype` the app sent.
    pub fn requests(&self, subtype: &str) -> Vec<Value> {
        self.written()
            .into_iter()
            .filter(|f| f["type"] == "control_request" && f["request"]["subtype"] == subtype)
            .map(|f| f["request"].clone())
            .collect()
    }

    /// The prompts the app sent, by their text.
    pub fn prompts(&self) -> Vec<String> {
        self.written()
            .into_iter()
            .filter(|f| f["type"] == "user")
            .filter_map(|f| {
                f["message"]["content"]
                    .as_array()?
                    .iter()
                    .find_map(|c| c["text"].as_str().map(str::to_string))
            })
            .collect()
    }
}

fn initialize() -> Value {
    json!({
        "commands": [
            {"name": "deploy", "description": "Deploy to staging", "argumentHint": "<env>"},
            {"name": "compact", "description": "Compact the conversation"}
        ],
        "models": [
            {"value": "default", "displayName": "Opus", "description": "Most capable",
             "supportedEffortLevels": ["low", "high", "max"]},
            {"value": "haiku", "displayName": "Haiku", "description": "Fastest"}
        ],
        "account": {"email": "selftest@example.org", "subscriptionType": "max", "tokenSource": "claude.ai"},
        "output_style": "default",
        "remote_control_available": true,
        "remote_control_auto_enable": false
    })
}

fn uuid(n: u32) -> String {
    format!("51e1f7e5-5000-4000-8000-{n:012}")
}

fn init(cwd: &Path) -> Value {
    json!({"type": "system", "subtype": "init", "cwd": cwd, "session_id": SESSION,
           "tools": ["AskUserQuestion", "Bash", "ExitPlanMode", "Task", "WebFetch", "Write"], "mcp_servers": [],
           "model": "claude-opus-5-5", "permissionMode": "default", "slash_commands": ["deploy", "compact"],
           "apiKeySource": "none", "claude_code_version": "2.1.319", "output_style": "default", "uuid": uuid(1)})
}

pub(super) fn tool_use(n: u32, call: &str, name: &str, input: &Value) -> Value {
    json!({"type": "assistant", "message": {"model": "claude-opus-5-5", "id": format!("msg_actions_{n}"),
           "type": "message", "role": "assistant",
           "content": [{"type": "tool_use", "id": call, "name": name, "input": input}],
           "stop_reason": null, "usage": {"input_tokens": 1, "output_tokens": 1}},
           "parent_tool_use_id": null, "session_id": SESSION, "uuid": uuid(n)})
}

fn text(n: u32, words: &str) -> Value {
    json!({"type": "assistant", "message": {"model": "claude-opus-5-5", "id": format!("msg_actions_{n}"),
           "type": "message", "role": "assistant", "content": [{"type": "text", "text": words}],
           "stop_reason": null, "usage": {"input_tokens": 1, "output_tokens": 1}},
           "parent_tool_use_id": null, "session_id": SESSION, "uuid": uuid(n)})
}

fn ask(id: &str, call: &str, tool: &str, input: &Value, suggestions: &Value) -> Value {
    json!({"type": "control_request", "request_id": id, "request": {"subtype": "can_use_tool", "tool_name": tool,
           "display_name": tool, "input": input, "permission_suggestions": suggestions, "tool_use_id": call,
           "requires_user_interaction": matches!(tool, "AskUserQuestion" | "ExitPlanMode")}})
}

pub(super) fn tool_result(n: u32, call: &str, content: &str) -> Value {
    json!({"type": "user", "message": {"role": "user",
           "content": [{"tool_use_id": call, "type": "tool_result", "content": content}]},
           "parent_tool_use_id": null, "session_id": SESSION, "uuid": uuid(n)})
}

fn rule(tool: &str, content: &str, destination: &str) -> Value {
    json!({"type": "addRules", "rules": [{"toolName": tool, "ruleContent": content}], "behavior": "allow",
           "destination": destination})
}

fn result(n: u32) -> Value {
    json!({"type": "result", "subtype": "success", "is_error": false, "duration_ms": 900, "duration_api_ms": 800,
           "num_turns": 1, "result": "ok", "stop_reason": "end_turn", "session_id": SESSION,
           "total_cost_usd": 0.0123, "usage": {"input_tokens": 100, "output_tokens": 20}, "uuid": uuid(n)})
}

/// The question the stand-in asks: one answer picked, then several ticked with words of the user's own.
pub const COLOUR: &str = "Welche Farbe?";
pub const TESTS: &str = "Welche Tests?";

/// The second request: two multiple-choice questions the user answers only in words, sent with one button.
pub const AREAS: &str = "Welche Bereiche?";
pub const EXTRAS: &str = "Welche Extras?";

/// Write asks with two rules to store.
fn first_turn(cwd: &Path) -> Vec<Value> {
    let input = json!({"file_path": cwd.join("hello.txt"), "content": "hello there\n"});
    vec![
        init(cwd),
        tool_use(2, "toolu_write", "Write", &input),
        ask(
            "p1",
            "toolu_write",
            "Write",
            &input,
            &json!([
                rule("Write", "hello.txt", "localSettings"),
                rule("Write", "hello.txt", "userSettings")
            ]),
        ),
    ]
}

/// WebFetch asks; its input is edited as JSON.
fn after_rules(cwd: &Path) -> Vec<Value> {
    let input = json!({"url": "https://example.org", "prompt": "summarize"});
    vec![
        tool_result(
            3,
            "toolu_write",
            &format!("File written: {}", cwd.join("hello.txt").display()),
        ),
        tool_use(4, "toolu_fetch", "WebFetch", &input),
        ask("p2", "toolu_fetch", "WebFetch", &input, &json!([])),
    ]
}

/// A command asks with one rule to store.
fn after_fetch() -> Vec<Value> {
    let input = json!({"command": "ls -la", "description": "List the files"});
    vec![
        tool_result(5, "toolu_fetch", "Example Domain"),
        tool_use(6, "toolu_ls", "Bash", &input),
        ask(
            "p3",
            "toolu_ls",
            "Bash",
            &input,
            &json!([rule("Bash", "ls:*", "localSettings")]),
        ),
    ]
}

/// A command the user refuses.
fn after_always() -> Vec<Value> {
    let input = json!({"command": "rm -rf build", "description": "Remove the build"});
    vec![
        tool_result(7, "toolu_ls", "hello.txt"),
        tool_use(8, "toolu_rm", "Bash", &input),
        ask("p4", "toolu_rm", "Bash", &input, &json!([])),
    ]
}

/// Two questions in one request.
fn after_deny() -> Vec<Value> {
    let input = json!({"questions": [
        {"question": COLOUR, "header": "Farbe", "multiSelect": false,
         "options": [{"label": "red", "description": "Rot"}, {"label": "blue", "description": "Blau"}]},
        {"question": TESTS, "header": "Tests", "multiSelect": true,
         "options": [{"label": "unit", "description": "Unit"}, {"label": "ui", "description": "UI"},
                     {"label": "perf", "description": "Perf"}]}
    ]});
    vec![
        tool_result(10, "toolu_rm", "The user denied this tool call."),
        tool_use(11, "toolu_ask", "AskUserQuestion", &input),
        ask("q1", "toolu_ask", "AskUserQuestion", &input, &json!([])),
    ]
}

/// Two multiple-choice questions in one request.
fn after_question() -> Vec<Value> {
    let input = json!({"questions": [
        {"question": AREAS, "header": "Bereiche", "multiSelect": true,
         "options": [{"label": "api", "description": "API"}, {"label": "ui", "description": "UI"}]},
        {"question": EXTRAS, "header": "Extras", "multiSelect": true,
         "options": [{"label": "docs", "description": "Doku"}, {"label": "perf", "description": "Leistung"}]}
    ]});
    vec![
        tool_result(12, "toolu_ask", "answered"),
        tool_use(40, "toolu_ask2", "AskUserQuestion", &input),
        ask("q2", "toolu_ask2", "AskUserQuestion", &input, &json!([])),
    ]
}

/// A plan to revise, then a background command and a subagent that run on.
fn after_words() -> Vec<Value> {
    let plan = json!({"plan": "## Plan\n1. Tests schreiben\n2. Parser reparieren"});
    vec![
        tool_result(41, "toolu_ask2", "answered"),
        tool_use(13, "toolu_plan", "ExitPlanMode", &plan),
        ask("x1", "toolu_plan", "ExitPlanMode", &plan, &json!([])),
    ]
}

/// What the stand-in plays after the plan's answer.
fn after_plan() -> Vec<Value> {
    vec![
        tool_result(14, "toolu_plan", "revise"),
        tool_use(
            15,
            "toolu_bg",
            "Bash",
            &json!({"command": "sleep 30", "description": "Warten", "run_in_background": true}),
        ),
        json!({"type": "system", "subtype": "task_started", "task_id": "bg1", "tool_use_id": "toolu_bg",
               "description": "Warten", "is_backgrounded": true, "task_type": "local_bash",
               "session_id": SESSION, "uuid": uuid(16)}),
        tool_use(
            19,
            "toolu_fg",
            "Bash",
            &json!({"command": "cargo test", "description": "Tests laufen lassen"}),
        ),
        tool_use(
            17,
            "toolu_sub",
            "Task",
            &json!({"description": "Tests ansehen", "prompt": "Sieh dir die Tests an", "subagent_type": "general-purpose"}),
        ),
        json!({"type": "system", "subtype": "task_started", "task_id": "sub1", "tool_use_id": "toolu_sub",
               "description": "Tests ansehen", "subagent_type": "general-purpose", "task_type": "local_agent",
               "session_id": SESSION, "uuid": uuid(18)}),
    ]
}

/// Plays `frames` for the first prompt of the next start of the program, then `then` for those after it.
pub fn next_start(stand_in: &StandIn, frames: &[Value], then: &[Value]) -> std::io::Result<()> {
    stand_in.frames("turn-1.ndjson", frames)?;
    stand_in.frames("turn-1.ndjson.then", then)
}

/// The CLI's answer when it cannot resume at the point a rewind asked for.
pub fn rewind_rejected() -> Value {
    json!({"type": "result", "subtype": "error_during_execution", "is_error": true, "num_turns": 0,
           "duration_ms": 0, "duration_api_ms": 0, "stop_reason": null, "session_id": SESSION, "total_cost_usd": 0,
           "usage": {"input_tokens": 0, "output_tokens": 0}, "uuid": uuid(30),
           "errors": ["Resume rejected by --resume-drops-turn: resuming here would discard entries not attributable to the turn"]})
}

/// A short answer that ends the turn.
pub fn short_answer() -> Vec<Value> {
    vec![text(31, "Erledigt."), result(32)]
}

/// The prompt that waited in the queue.
fn second_turn() -> Vec<Value> {
    vec![text(20, "Die Tests laufen."), result(21)]
}

/// A turn that calls every kind of tool the timeline shows in a card of its own, each with an answer.
pub fn tools_turn(cwd: &Path) -> Vec<Value> {
    let lines: String = (1..=12).map(|n| format!("row {n}\n")).collect();
    let calls = [
        (
            "t_grep",
            "Grep",
            json!({"pattern": "fn main", "path": cwd.join("src"), "glob": "*.rs"}),
            "src/main.rs:1".to_string(),
        ),
        (
            "t_search",
            "WebSearch",
            json!({"query": "objc2 menus"}),
            "3 results".to_string(),
        ),
        (
            "t_mcp",
            "mcp__db__query",
            json!({"sql": "select 1\nfrom accounts\nwhere id = 7"}),
            lines.clone(),
        ),
        (
            "t_other",
            "Frobnicate",
            json!({"level": 3, "note": "a long note that does not fit on one line of the card at all, so it folds"}),
            "done".to_string(),
        ),
        (
            "t_read",
            "Read",
            json!({"file_path": cwd.join("README.md"), "offset": 2, "limit": 5}),
            "# Actions".to_string(),
        ),
        (
            "t_fetch",
            "WebFetch",
            json!({"url": "https://example.org/docs", "prompt": "summarize"}),
            lines,
        ),
    ];
    let mut frames = Vec::new();
    for (n, (call, name, input, output)) in (40..).step_by(2).zip(calls) {
        frames.push(tool_use(n, call, name, &input));
        frames.push(tool_result(n + 1, call, &output));
    }
    frames
}

//! The bodies of responses and events turned into the crate's types; what is missing or of the wrong type is left
//! out instead of failing the whole answer, since adapters differ in what they fill in.

use std::path::PathBuf;

use serde_json::Value;

use crate::types::{
    Breakpoint, Evaluation, FrameId, Output, OutputCategory, Program, Scope, Source, StackFrame, StartMethod, Stop,
    StopReason, Thread, ThreadId, Variable, VariablesRef,
};

fn string(value: &Value, key: &str) -> Option<String> {
    value.get(key).and_then(Value::as_str).map(str::to_string)
}

fn int(value: &Value, key: &str) -> Option<i64> {
    value.get(key).and_then(Value::as_i64)
}

fn line(value: &Value, key: &str) -> Option<u32> {
    value
        .get(key)
        .and_then(Value::as_u64)
        .and_then(|n| u32::try_from(n).ok())
}

fn list<'a>(value: &'a Value, key: &str) -> &'a [Value] {
    value.get(key).and_then(Value::as_array).map_or(&[], Vec::as_slice)
}

pub(crate) fn source(value: &Value) -> Option<Source> {
    if !value.is_object() {
        return None;
    }
    Some(Source {
        name: string(value, "name"),
        path: string(value, "path").filter(|p| !p.is_empty()).map(PathBuf::from),
        reference: int(value, "sourceReference").filter(|r| *r > 0),
        hint: string(value, "presentationHint"),
    })
}

pub(crate) fn threads(body: &Value) -> Vec<Thread> {
    list(body, "threads")
        .iter()
        .filter_map(|thread| {
            Some(Thread {
                id: ThreadId(int(thread, "id")?),
                name: string(thread, "name").unwrap_or_default(),
            })
        })
        .collect()
}

pub(crate) fn stack_frames(body: &Value) -> Vec<StackFrame> {
    list(body, "stackFrames")
        .iter()
        .filter_map(|frame| {
            Some(StackFrame {
                id: FrameId(int(frame, "id")?),
                name: string(frame, "name").unwrap_or_default(),
                source: frame.get("source").and_then(source),
                line: line(frame, "line").unwrap_or(0),
                column: line(frame, "column").unwrap_or(0),
                hint: string(frame, "presentationHint"),
            })
        })
        .collect()
}

pub(crate) fn scopes(body: &Value) -> Vec<Scope> {
    list(body, "scopes")
        .iter()
        .filter_map(|scope| {
            Some(Scope {
                name: string(scope, "name")?,
                variables: VariablesRef(int(scope, "variablesReference").unwrap_or(0)),
                expensive: scope.get("expensive").and_then(Value::as_bool).unwrap_or(false),
                hint: string(scope, "presentationHint"),
            })
        })
        .collect()
}

pub(crate) fn variables(body: &Value) -> Vec<Variable> {
    list(body, "variables")
        .iter()
        .filter_map(|variable| {
            Some(Variable {
                name: string(variable, "name")?,
                value: string(variable, "value").unwrap_or_default(),
                type_name: string(variable, "type").filter(|t| !t.is_empty()),
                children: VariablesRef(int(variable, "variablesReference").unwrap_or(0)),
                evaluate_name: string(variable, "evaluateName"),
            })
        })
        .collect()
}

pub(crate) fn evaluation(body: &Value) -> Evaluation {
    Evaluation {
        result: string(body, "result").unwrap_or_default(),
        type_name: string(body, "type").filter(|t| !t.is_empty()),
        children: VariablesRef(int(body, "variablesReference").unwrap_or(0)),
    }
}

pub(crate) fn breakpoint(value: &Value) -> Breakpoint {
    Breakpoint {
        id: int(value, "id"),
        verified: value.get("verified").and_then(Value::as_bool).unwrap_or(false),
        message: string(value, "message"),
        path: value.get("source").and_then(source).and_then(|s| s.path),
        line: line(value, "line"),
        column: line(value, "column"),
    }
}

pub(crate) fn breakpoints(body: &Value) -> Vec<Breakpoint> {
    list(body, "breakpoints").iter().map(breakpoint).collect()
}

pub(crate) fn stop(body: &Value) -> Stop {
    Stop {
        reason: StopReason::from_wire(&string(body, "reason").unwrap_or_default()),
        description: string(body, "description"),
        text: string(body, "text"),
        thread: int(body, "threadId").map(ThreadId),
        all_threads: body.get("allThreadsStopped").and_then(Value::as_bool).unwrap_or(false),
        hit_breakpoints: list(body, "hitBreakpointIds")
            .iter()
            .filter_map(Value::as_i64)
            .collect(),
    }
}

/// The console line of an `output` event; `None` for telemetry, which is not for the user.
pub(crate) fn output(body: &Value) -> Option<Output> {
    let category = match string(body, "category").as_deref() {
        None | Some("console") => OutputCategory::Console,
        Some("important") => OutputCategory::Important,
        Some("stdout") => OutputCategory::Stdout,
        Some("stderr") => OutputCategory::Stderr,
        Some("telemetry") => return None,
        Some(other) => OutputCategory::Other(other.to_string()),
    };
    Some(Output {
        category,
        text: string(body, "output").unwrap_or_default(),
        source: body.get("source").and_then(source),
        line: line(body, "line"),
    })
}

pub(crate) fn program(body: &Value) -> Program {
    Program {
        name: string(body, "name").unwrap_or_default(),
        pid: body
            .get("systemProcessId")
            .and_then(Value::as_u64)
            .and_then(|pid| u32::try_from(pid).ok())
            .filter(|pid| *pid > 0),
        start: match string(body, "startMethod").as_deref() {
            Some("launch") => Some(StartMethod::Launch),
            Some("attach") => Some(StartMethod::Attach),
            Some("attachForSuspendedLaunch") => Some(StartMethod::AttachForSuspendedLaunch),
            _ => None,
        },
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn missing_or_mistyped_fields_leave_out_the_item_or_the_field_not_the_answer() {
        let body = json!({"stackFrames": [
            {"id": 1, "name": "main", "line": 4, "column": 5, "source": {"name": "main.rs", "path": "/p/main.rs"}},
            {"name": "no id"},
            {"id": 2, "name": "start", "line": -1, "source": "not an object"},
        ]});
        let frames = stack_frames(&body);
        assert_eq!(frames.len(), 2);
        assert_eq!(
            frames[0].source.as_ref().unwrap().path,
            Some(PathBuf::from("/p/main.rs"))
        );
        assert_eq!((frames[1].line, frames[1].source.clone()), (0, None));
        assert!(stack_frames(&json!({"stackFrames": "x"})).is_empty());
    }

    #[test]
    fn telemetry_is_not_shown_and_other_categories_are_kept() {
        assert!(output(&json!({"category": "telemetry", "output": "x"})).is_none());
        let line = output(&json!({"output": "hi\n"})).unwrap();
        assert_eq!((line.category, line.text.as_str()), (OutputCategory::Console, "hi\n"));
        let line = output(&json!({"category": "stderr", "output": "e"})).unwrap();
        assert_eq!(line.category, OutputCategory::Stderr);
    }
}

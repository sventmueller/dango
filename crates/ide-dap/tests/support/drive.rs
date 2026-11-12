//! One debugging session on `tests/support/rust_program`, driven the same way against lldb-dap and against its
//! recording, so the replay checks what the real adapter answered.

use std::path::Path;
use std::sync::mpsc::Receiver;
use std::time::Duration;

use ide_dap::{EvaluateContext, Event, Session, Stop, StopReason, ThreadId, wait};

pub const WAIT: Duration = Duration::from_secs(60);

/// The line of `total += …` in the program, where the breakpoint goes.
pub const LOOP_LINE: u32 = 10;

/// What the session showed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Observed {
    pub stop_line: u32,
    pub function: String,
    pub point: Vec<(String, String)>,
    pub total_at_first_stop: String,
    pub line_after_step: u32,
    pub total_at_second_stop: String,
    pub printed: String,
    pub exit_code: Option<i64>,
}

fn stopped(rx: &Receiver<Event>) -> Stop {
    loop {
        match rx.recv_timeout(WAIT).expect("the program did not stop") {
            Event::Stopped(stop) => return stop,
            Event::Ended(end) => panic!("the session ended: {end:?}"),
            _ => {}
        }
    }
}

fn top(session: &Session, thread: ThreadId) -> ide_dap::StackFrame {
    wait(|r| session.stack_trace(thread, r)).unwrap().remove(0)
}

fn total(session: &Session, thread: ThreadId) -> String {
    let frame = top(session, thread).id;
    wait(|r| session.evaluate("total", Some(frame), EvaluateContext::Watch, r))
        .unwrap()
        .result
}

/// Stops at the loop, looks around, steps, continues to the next round, removes the breakpoint and runs to the end.
pub fn drive(session: &Session, rx: &Receiver<Event>, source: &Path) -> Observed {
    let stop = stopped(rx);
    assert_eq!(stop.reason, StopReason::Breakpoint);
    let threads = wait(|r| session.threads(r)).unwrap();
    let thread = stop.thread.unwrap_or(threads[0].id);
    let frame = top(session, thread);
    let scopes = wait(|r| session.scopes(frame.id, r)).unwrap();
    let locals = wait(|r| session.variables(scopes[0].variables, r)).unwrap();
    let point = locals.iter().find(|v| v.name == "point").expect("no local point");
    let fields = wait(|r| session.variables(point.children, r))
        .unwrap()
        .into_iter()
        .map(|v| (v.name, v.value))
        .collect();
    let total_at_first_stop = total(session, thread);
    wait(|r| session.next(thread, r)).unwrap();
    assert_eq!(stopped(rx).reason, StopReason::Step);
    let line_after_step = top(session, thread).line;
    wait(|r| session.resume(thread, r)).unwrap();
    stopped(rx);
    let total_at_second_stop = total(session, thread);
    wait(|r| session.set_breakpoints(source, Vec::new(), r)).unwrap();
    wait(|r| session.resume(thread, r)).unwrap();
    let mut printed = String::new();
    let mut exit_code = None;
    loop {
        match rx.recv_timeout(WAIT).expect("the session did not end") {
            Event::Output(output) if output.category == ide_dap::OutputCategory::Stdout => {
                printed.push_str(&output.text)
            }
            Event::Exited(code) => exit_code = Some(code),
            Event::Ended(_) => break,
            _ => {}
        }
    }
    Observed {
        stop_line: frame.line,
        function: frame.name,
        point: fields,
        total_at_first_stop,
        line_after_step,
        total_at_second_stop,
        printed,
        exit_code,
    }
}

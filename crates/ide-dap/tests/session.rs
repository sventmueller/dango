//! Sessions against the fake adapter on threads of the test: start, breakpoints of every kind, inspecting, stepping,
//! the terminal request and the end.

use std::sync::mpsc::{Receiver, channel};
use std::time::Duration;

use ide_dap::fake::Script;
use ide_dap::{
    Adapter, EndReason, Error, EvaluateContext, Event, FrameId, OutputCategory, Request, Session, Setup,
    SourceBreakpoint, State, StopReason, ThreadId, wait,
};
use ide_process::Supervisor;

const WAIT: Duration = Duration::from_secs(5);
const MAIN: ThreadId = ThreadId(1);

fn start(script: &Script, setup: Setup) -> (Session, Receiver<Event>) {
    let (tx, rx) = channel();
    let session = Session::start(
        &Supervisor::new(),
        Adapter::fake(script.clone()),
        script.launch_config(),
        setup,
        move |_, event| {
            let _ = tx.send(event);
        },
    )
    .unwrap();
    (session, rx)
}

fn setup(script: &Script, breakpoints: Vec<SourceBreakpoint>) -> Setup {
    let mut setup = Setup::default();
    setup.breakpoints.insert(script.path.clone(), breakpoints);
    setup
}

/// The next event `pick` takes, skipping the others.
fn next<T>(rx: &Receiver<Event>, pick: impl Fn(Event) -> Option<T>) -> T {
    loop {
        let event = rx.recv_timeout(WAIT).expect("no such event in time");
        if let Some(found) = pick(event) {
            return found;
        }
    }
}

fn stopped(rx: &Receiver<Event>) -> ide_dap::Stop {
    next(rx, |e| match e {
        Event::Stopped(stop) => Some(stop),
        _ => None,
    })
}

fn line_now(session: &Session) -> u32 {
    wait(|r| session.stack_trace(MAIN, r)).unwrap()[0].line
}

#[test]
fn a_breakpoint_stops_the_program_and_its_state_can_be_read() {
    let script = Script::default();
    let (session, rx) = start(
        &script,
        setup(&script, vec![script.breakpoint(4), script.breakpoint(40)]),
    );
    let capabilities = next(&rx, |e| match e {
        Event::Initialized(capabilities) => Some(capabilities),
        _ => None,
    });
    assert!(capabilities.conditional_breakpoints && capabilities.restart);
    assert_eq!(capabilities.exception_filters[0].filter, "panic");
    let set = next(&rx, |e| match e {
        Event::BreakpointsSet { breakpoints, .. } => Some(breakpoints),
        _ => None,
    });
    assert_eq!(set.iter().map(|b| b.verified).collect::<Vec<_>>(), [true, false]);
    let stop = stopped(&rx);
    assert_eq!(
        (stop.reason, stop.hit_breakpoints.clone()),
        (StopReason::Breakpoint, vec![set[0].id.unwrap()])
    );
    assert!(matches!(session.state(), State::Stopped(_)));
    assert_eq!(session.request(), Request::Launch);

    let threads = wait(|r| session.threads(r)).unwrap();
    assert_eq!(
        threads.iter().map(|t| t.name.as_str()).collect::<Vec<_>>(),
        ["main", "worker"]
    );
    let frames = wait(|r| session.stack_trace(MAIN, r)).unwrap();
    assert_eq!((frames.len(), frames[0].line), (2, 4));
    assert_eq!(frames[0].source.as_ref().unwrap().path.as_ref(), Some(&script.path));
    let scopes = wait(|r| session.scopes(frames[0].id, r)).unwrap();
    assert!(scopes[1].expensive);
    let locals = wait(|r| session.variables(scopes[0].variables, r)).unwrap();
    let point = locals.iter().find(|v| v.name == "point").unwrap();
    let fields = wait(|r| session.variables(point.children, r)).unwrap();
    let inner = fields.iter().find(|v| v.name == "inner").unwrap();
    let deepest = wait(|r| session.variables(inner.children, r)).unwrap();
    assert_eq!((deepest[0].name.as_str(), deepest[0].value.as_str()), ("z", "3"));
    assert!(wait(|r| session.variables(deepest[0].children, r)).unwrap().is_empty());

    let value = wait(|r| session.evaluate("point.x", Some(FrameId(1)), EvaluateContext::Hover, r)).unwrap();
    assert_eq!(value.result, "1");
    let unknown = wait(|r| session.evaluate("nope", Some(FrameId(1)), EvaluateContext::Repl, r));
    assert!(matches!(unknown, Err(Error::Adapter { ref message, .. }) if message.contains("nope")));
}

#[test]
fn steps_move_through_the_lines_and_continuing_runs_to_the_end() {
    let script = Script::default();
    let (session, rx) = start(&script, setup(&script, vec![script.breakpoint(2)]));
    stopped(&rx);
    wait(|r| session.next(MAIN, r)).unwrap();
    assert_eq!(stopped(&rx).reason, StopReason::Step);
    assert_eq!(line_now(&session), 3);
    wait(|r| session.step_in(MAIN, r)).unwrap();
    stopped(&rx);
    assert_eq!(line_now(&session), 4);
    assert_eq!(
        wait(|r| session.pause(MAIN, r)),
        Err(Error::NotStopped),
        "pausing a stopped program"
    );
    wait(|r| session.resume(MAIN, r)).unwrap();
    let code = next(&rx, |e| match e {
        Event::Exited(code) => Some(code),
        _ => None,
    });
    let end = next(&rx, |e| match e {
        Event::Ended(end) => Some(end),
        _ => None,
    });
    assert_eq!((code, end.reason, end.exit_code), (0, EndReason::Finished, Some(0)));
    assert_eq!(wait(|r| session.threads(r)), Err(Error::Ended));
    assert_eq!(wait(|r| session.next(MAIN, r)), Err(Error::NotStopped));
}

#[test]
fn conditions_hit_counts_and_log_points_decide_whether_the_program_stops() {
    let script = Script {
        lines: vec![1, 2, 1, 2, 1, 2, 3],
        ..Script::default()
    };
    let never = SourceBreakpoint {
        line: 1,
        condition: Some("false".into()),
        ..SourceBreakpoint::default()
    };
    let third = SourceBreakpoint {
        line: 2,
        hit_condition: Some("3".into()),
        ..SourceBreakpoint::default()
    };
    let log = SourceBreakpoint {
        line: 3,
        log_message: Some("hits so far: {hits}".into()),
        ..SourceBreakpoint::default()
    };
    let mut setup = setup(&script, vec![never, third]);
    setup.breakpoints.get_mut(&script.path).unwrap().push(log);
    let (session, rx) = start(&script, setup);
    stopped(&rx);
    let hits = wait(|r| session.evaluate("hits", None, EvaluateContext::Watch, r)).unwrap();
    assert_eq!(hits.result, "6", "line 1 three times, line 2 three times");
    wait(|r| session.resume(MAIN, r)).unwrap();
    let logged = next(&rx, |e| match e {
        Event::Output(output) if output.category == OutputCategory::Console => Some(output),
        _ => None,
    });
    assert_eq!((logged.text.as_str(), logged.line), ("hits so far: 1\n", Some(3)));
    assert!(session.wait_until_ended(WAIT));
}

#[test]
fn a_panic_stops_only_while_its_filter_is_on() {
    let script = Script {
        panics: true,
        ..Script::default()
    };
    let (session, rx) = start(&script, Setup::default());
    let stop = stopped(&rx);
    assert_eq!(
        (stop.reason, stop.text.as_deref()),
        (StopReason::Exception, Some("fake panic"))
    );
    session.stop();
    assert!(session.wait_until_ended(WAIT));

    let off = Setup {
        exception_filters: Some(Vec::new()),
        ..Setup::default()
    };
    let (session, _rx) = start(&script, off);
    assert!(session.wait_until_ended(WAIT));
    assert_eq!(session.end().unwrap().reason, EndReason::Finished);
}

#[test]
fn breakpoints_set_while_stopped_take_effect_and_restart_follows_the_capability() {
    let script = Script::default();
    let (session, rx) = start(&script, setup(&script, vec![script.breakpoint(2)]));
    stopped(&rx);
    let set = wait(|r| session.set_breakpoints(&script.path, vec![script.breakpoint(6)], r)).unwrap();
    assert!(set[0].verified);
    wait(|r| session.resume(MAIN, r)).unwrap();
    stopped(&rx);
    assert_eq!(line_now(&session), 6);
    wait(|r| session.restart(r)).unwrap();
    stopped(&rx);
    assert_eq!(
        line_now(&session),
        6,
        "after the restart the program runs from the start to line 6 again"
    );
    assert_eq!(session.setup().breakpoints[&script.path], vec![script.breakpoint(6)]);

    let no_restart = Script {
        restart: false,
        ..Script::default()
    };
    let (session, rx) = start(&no_restart, setup(&no_restart, vec![no_restart.breakpoint(2)]));
    stopped(&rx);
    assert!(!session.capabilities().restart);
    assert_eq!(wait(|r| session.restart(r)), Err(Error::Unsupported));
}

#[test]
fn the_terminal_request_reaches_the_app_and_its_answer_the_adapter() {
    let script = Script {
        run_in_terminal: true,
        ..Script::default()
    };
    let (session, rx) = start(&script, setup(&script, vec![script.breakpoint(5)]));
    let (request, answer) = next(&rx, |e| match e {
        Event::RunInTerminal(request, answer) => Some((request, answer)),
        _ => None,
    });
    assert_eq!(request.args, [script.path.display().to_string()]);
    assert!(request.env.contains(&("UNSET".to_string(), None)));
    answer.started(Some(4321), None);
    let program = next(&rx, |e| match e {
        Event::Program(program) => Some(program),
        _ => None,
    });
    assert_eq!(program.pid, Some(4321));
    stopped(&rx);
    session.stop();
    assert!(session.wait_until_ended(WAIT));
    assert_eq!(session.end().unwrap().reason, EndReason::Stopped);
}

#[test]
fn stopping_and_dropping_end_the_session_once() {
    let script = Script::default();
    let (session, rx) = start(&script, setup(&script, vec![script.breakpoint(3)]));
    stopped(&rx);
    session.stop();
    session.stop();
    assert!(session.wait_until_ended(WAIT));
    let ends: Vec<Event> = rx.try_iter().filter(|e| matches!(e, Event::Ended(_))).collect();
    assert_eq!(ends.len(), 1);
    assert_eq!(session.end().unwrap().reason, EndReason::Stopped);

    let (session, rx) = start(&script, setup(&script, vec![script.breakpoint(3)]));
    stopped(&rx);
    drop(session);
    let end = next(&rx, |e| match e {
        Event::Ended(end) => Some(end),
        _ => None,
    });
    assert_eq!(end.reason, EndReason::Stopped);
}

#[test]
fn a_slow_launch_keeps_the_session_starting_and_stopping_then_ends_it() {
    let script = Script {
        launch_delay_ms: 300,
        ..Script::default()
    };
    let (session, rx) = start(&script, setup(&script, vec![script.breakpoint(2)]));
    assert_eq!(session.state(), State::Starting);
    stopped(&rx);

    let slow = Script {
        launch_delay_ms: 60_000,
        ..Script::default()
    };
    let (session, _rx) = start(&slow, Setup::default());
    std::thread::sleep(Duration::from_millis(100));
    assert_eq!(session.state(), State::Starting);
    let begun = std::time::Instant::now();
    assert!(session.stop_and_wait(WAIT));
    assert_eq!(session.end().unwrap().reason, EndReason::Stopped);
    assert!(begun.elapsed() < Duration::from_secs(3), "{:?}", begun.elapsed());
}

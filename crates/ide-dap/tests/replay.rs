//! Recorded sessions played back through the replay adapter: one recorded here from the fake adapter, which checks
//! the playback itself, and `fixtures/lldb_rust.ndjson`, recorded from lldb-dap on `support/rust_program` by
//! `IDE_DAP_RECORD=… cargo test -p ide-dap --test real_adapters -- --ignored lldb_dap`.

#![allow(clippy::unwrap_used, reason = "test helpers, which allow-unwrap-in-tests misses")]

#[path = "support/drive.rs"]
mod drive;

use std::path::Path;
use std::sync::mpsc::channel;

use ide_dap::fake::Script;
use ide_dap::{Adapter, AdapterKind, EndReason, Session, Setup, SourceBreakpoint, replay};
use ide_process::Supervisor;

fn setup(path: &Path, line: u32) -> Setup {
    let mut setup = Setup::default();
    let breakpoint = SourceBreakpoint {
        line,
        ..SourceBreakpoint::default()
    };
    setup.breakpoints.insert(path.to_path_buf(), vec![breakpoint]);
    setup
}

fn run(adapter: Adapter, config: ide_dap::LaunchConfig, setup: Setup, source: &Path) -> (drive::Observed, Session) {
    let (tx, rx) = channel();
    let session = Session::start(&Supervisor::new(), adapter, config, setup, move |_, event| {
        let _ = tx.send(event);
    })
    .unwrap();
    let observed = drive::drive(&session, &rx, source);
    assert!(session.wait_until_ended(drive::WAIT));
    (observed, session)
}

#[test]
fn a_recorded_session_plays_back_as_it_happened() {
    let script = Script {
        lines: vec![1, 2, 3, 2, 3, 2, 3, 4],
        ..Script::default()
    };
    let dir = ide_core::testing::scratch("dap-replay");
    let file = dir.join("recording.ndjson");
    let adapter = Adapter::fake(script.clone()).record_to(&file);
    let line = 3;
    let (live, _) = run(adapter, script.launch_config(), setup(&script.path, line), &script.path);
    let recording = std::fs::read_to_string(&file).unwrap();
    let adapter = Adapter::replay(AdapterKind::Fake, &recording).unwrap();
    let (played, session) = run(adapter, script.launch_config(), setup(&script.path, line), &script.path);
    assert_eq!(played, live);
    assert_eq!(session.end().unwrap().reason, EndReason::Finished);
    assert_eq!(live.point[0], ("x".to_string(), "1".to_string()));
}

#[test]
fn a_request_the_recording_lacks_is_refused() {
    let adapter = Adapter::replay(AdapterKind::Fake, "").unwrap();
    let script = Script::default();
    let session = Session::start(
        &Supervisor::new(),
        adapter,
        script.launch_config(),
        Setup::default(),
        |_, _| {},
    )
    .unwrap();
    assert!(session.wait_until_ended(drive::WAIT));
    assert!(matches!(session.end().unwrap().reason, EndReason::Failed(ref why) if why.contains("initialize")));
    assert!(replay::parse("not json\n{\"from\":\"adapter\"}\n").is_empty());
}

#[test]
fn lldb_dap_on_a_rust_program_plays_back_from_its_recording() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/lldb_rust.ndjson");
    let recording = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    let lines = replay::parse(&recording);
    assert_eq!(
        lines.len(),
        recording.lines().count(),
        "every line of the recording reads"
    );
    let launch = lines
        .iter()
        .find_map(|line| match &line.message {
            ide_dap::protocol::Message::Request { command, arguments, .. } if command == "launch" => {
                arguments.as_object().cloned()
            }
            _ => None,
        })
        .expect("no launch in the recording");
    let config = ide_dap::LaunchConfig {
        name: "dap-sample".into(),
        kind: AdapterKind::Lldb,
        request: ide_dap::Request::Launch,
        arguments: launch,
        process_pick: false,
    };
    let source = Path::new("/tmp/dap-sample/src/main.rs");
    let adapter = Adapter::replay(AdapterKind::Lldb, &recording).unwrap();
    let (observed, session) = run(adapter, config, setup(source, drive::LOOP_LINE), source);
    assert_eq!(observed.stop_line, drive::LOOP_LINE);
    assert!(observed.function.contains("main"), "{}", observed.function);
    assert_eq!(
        observed.point,
        [("x".to_string(), "1".to_string()), ("y".to_string(), "2".to_string())]
    );
    assert_eq!(
        (
            observed.total_at_first_stop.as_str(),
            observed.total_at_second_stop.as_str()
        ),
        ("0", "3")
    );
    assert_eq!((observed.printed.trim_end(), observed.exit_code), ("total 12", Some(0)));
    assert_eq!(session.end().unwrap().reason, EndReason::Finished);
    let capabilities = session.capabilities();
    assert!(capabilities.conditional_breakpoints && capabilities.log_points);
    assert!(!capabilities.exception_filters.is_empty());
}

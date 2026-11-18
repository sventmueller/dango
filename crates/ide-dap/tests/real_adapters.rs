//! The real debug adapters of this Mac, each launching and attaching to a small program; ignored in `make check`
//! and run by `make test-integration` where the adapter is installed. `IDE_DAP_RECORD=<file>` keeps the lldb-dap
//! session as a recording for `tests/replay.rs`.

#[path = "support/drive.rs"]
mod drive;

use ide_core::testing::scratch;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::mpsc::{Receiver, channel};
use std::time::{Duration, Instant};

use drive::{LOOP_LINE, WAIT};
use ide_dap::{
    AdapterKind, Adapters, EndReason, Event, LaunchConfig, Session, Setup, SourceBreakpoint, State, current_program,
    start,
};
use ide_net::Cancel;
use ide_process::Supervisor;
use serde_json::json;

fn env() -> Vec<(String, String)> {
    std::env::vars().collect()
}

/// The sample Rust program copied into a folder of its own, so its build stays out of the repository.
fn rust_program() -> PathBuf {
    let dir = scratch("rust");
    let from = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/support/rust_program");
    std::fs::create_dir_all(dir.join("src")).unwrap();
    for file in ["Cargo.toml", "src/main.rs"] {
        std::fs::copy(from.join(file), dir.join(file)).unwrap();
    }
    dir.keep()
}

fn breakpoint_at(path: &Path, line: u32) -> Setup {
    let mut setup = Setup::default();
    let breakpoint = SourceBreakpoint {
        line,
        ..SourceBreakpoint::default()
    };
    setup.breakpoints.insert(path.to_path_buf(), vec![breakpoint]);
    setup
}

fn begin(adapters: &Adapters, root: &Path, config: LaunchConfig, setup: Setup) -> (Session, Receiver<Event>) {
    let (tx, rx) = channel();
    let session = start(
        adapters,
        root,
        true,
        config,
        setup,
        &Cancel::new(),
        &mut |_| {},
        move |_, event| {
            let _ = tx.send(event);
        },
    )
    .unwrap_or_else(|e| panic!("{e}"));
    (session, rx)
}

/// A program the user started in a terminal, in a group of its own, which the test ends itself.
struct Foreign(Child);

impl Foreign {
    fn running(&mut self) -> bool {
        self.0.try_wait().unwrap().is_none()
    }
}

impl Drop for Foreign {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

fn foreign(program: &Path, args: &[&str], dir: &Path) -> Foreign {
    use std::os::unix::process::CommandExt;
    let child = Command::new(program)
        .args(args)
        .current_dir(dir)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .process_group(0)
        .spawn()
        .unwrap();
    Foreign(child)
}

/// Attaches, waits until the adapter has the program, detaches, and checks the program still runs.
fn attach_and_detach(adapters: &Adapters, root: &Path, config: LaunchConfig, program: &mut Foreign) {
    let (session, _rx) = begin(adapters, root, config, Setup::default());
    assert!(
        session.wait_for(WAIT, |s| matches!(s, State::Running | State::Stopped(_))),
        "{:?}",
        session.state()
    );
    assert!(session.stop_and_wait(WAIT));
    assert_eq!(session.end().unwrap().reason, EndReason::Stopped);
    std::thread::sleep(Duration::from_millis(500));
    assert!(program.running(), "detaching ended the attached program");
}

#[test]
#[ignore = "needs lldb-dap from Apple's developer tools and cargo; run by make test-integration"]
fn lldb_dap_launches_and_attaches_to_a_rust_program() {
    let root = rust_program();
    let adapters = Adapters::new(&Supervisor::new(), env());
    let main = root.join("src/main.rs");
    let config = current_program(&adapters, &root, true, &main, "rust", &Cancel::new(), &mut |_| {}).unwrap();
    let program = PathBuf::from(config.arguments["program"].as_str().unwrap());
    let recording = std::env::var_os("IDE_DAP_RECORD").map(PathBuf::from);
    let (tx, rx) = channel();
    let mut adapter = adapters.find(AdapterKind::Lldb, &root).unwrap();
    if let Some(path) = &recording {
        adapter = adapter.record_to(path);
    }
    let session = Session::start(
        adapters.supervisor(),
        adapter,
        config,
        breakpoint_at(&main, LOOP_LINE),
        move |_, event| {
            let _ = tx.send(event);
        },
    )
    .unwrap();
    let observed = drive::drive(&session, &rx, &main);
    assert_eq!(observed.stop_line, LOOP_LINE);
    assert!(observed.function.contains("main"), "{}", observed.function);
    assert_eq!(
        observed.point,
        [("x".to_string(), "1".to_string()), ("y".to_string(), "2".to_string())]
    );
    assert_eq!(observed.total_at_first_stop, "0");
    assert_eq!(observed.total_at_second_stop, "3");
    assert_eq!((observed.printed.trim_end(), observed.exit_code), ("total 12", Some(0)));
    assert!(session.wait_until_ended(WAIT));
    assert_eq!(session.end().unwrap().reason, EndReason::Finished);
    if let Some(path) = &recording {
        anonymize(path, &root);
    }

    let mut waiting = foreign(&program, &["--wait"], &root);
    std::thread::sleep(Duration::from_millis(300));
    let attach = LaunchConfig::attach(AdapterKind::Lldb, waiting.0.id(), "attach");
    attach_and_detach(&adapters, &root, attach, &mut waiting);
}

#[test]
#[ignore = "needs dlv and go; run by make test-integration"]
fn dlv_launches_and_attaches_to_a_go_program() {
    let root = scratch("go");
    std::fs::write(root.join("go.mod"), "module sample\n\ngo 1.21\n").unwrap();
    let source = "package main\n\nimport (\n\t\"fmt\"\n\t\"os\"\n\t\"time\"\n)\n\nfunc main() {\n\ttotal := 0\n\
                  \tfor i := 0; i < 3; i++ {\n\t\ttotal += i\n\t}\n\tfmt.Println(\"total\", total)\n\
                  \tif len(os.Args) > 1 {\n\t\ttime.Sleep(120 * time.Second)\n\t}\n}\n";
    let main = root.join("main.go");
    std::fs::write(&main, source).unwrap();
    let adapters = Adapters::new(&Supervisor::new(), env());
    let config = current_program(&adapters, &root, true, &main, "go", &Cancel::new(), &mut |_| {}).unwrap();
    let (session, rx) = begin(&adapters, &root, config, breakpoint_at(&main, 12));
    let stop = stopped(&rx);
    let frames = ide_dap::wait(|r| session.stack_trace(stop.thread.unwrap(), r)).unwrap();
    assert_eq!(frames[0].line, 12);
    assert!(session.stop_and_wait(WAIT));

    let go = which("go").unwrap();
    let built = Command::new(&go)
        .args(["build", "-o", "sample", "."])
        .current_dir(&root)
        .status()
        .unwrap();
    assert!(built.success());
    let mut waiting = foreign(&root.join("sample"), &["--wait"], &root);
    std::thread::sleep(Duration::from_millis(300));
    let attach = LaunchConfig::attach(AdapterKind::Go, waiting.0.id(), "attach");
    attach_and_detach(&adapters, &root, attach, &mut waiting);
}

#[test]
#[ignore = "needs node and downloads js-debug; run by make test-integration"]
fn js_debug_launches_and_attaches_to_a_node_program() {
    let root = scratch("node");
    let tools = scratch("node-tools");
    let main = root.join("main.js");
    std::fs::write(
        &main,
        "let total = 0;\nfor (let i = 0; i < 3; i++) {\n  total += i;\n}\nconsole.log('total', total);\n\
         if (process.argv.includes('--wait')) setTimeout(() => {}, 120000);\n",
    )
    .unwrap();
    let adapters = Adapters::with_tools(&Supervisor::new(), env(), tools.to_path_buf());
    adapters
        .install(
            ide_dap::Install::JsDebug,
            &ide_net::Http::new("debug adapter installer"),
            &Cancel::new(),
            &mut |_| {},
        )
        .unwrap();
    let config = current_program(&adapters, &root, true, &main, "javascript", &Cancel::new(), &mut |_| {}).unwrap();
    let (session, rx) = begin(&adapters, &root, config, breakpoint_at(&main, 3));
    // js-debug debugs each Node process in a child session of the one started here.
    let stop = stopped(&rx);
    let child = session.children().into_iter().next().expect("no child session");
    let frames = ide_dap::wait(|r| child.stack_trace(stop.thread.unwrap(), r)).unwrap();
    assert_eq!(frames[0].line, 3);
    assert!(session.stop_and_wait(WAIT));
    assert!(child.wait_until_ended(WAIT));

    let port = ide_dap::free_port().unwrap();
    let node = which("node").unwrap();
    let inspect = format!("--inspect=127.0.0.1:{port}");
    let mut waiting = foreign(&node, &[&inspect, main.to_str().unwrap(), "--wait"], &root);
    std::thread::sleep(Duration::from_millis(500));
    let mut attach = LaunchConfig::attach(AdapterKind::JsDebug, waiting.0.id(), "attach");
    attach.arguments.remove("processId");
    attach.arguments.insert("port".into(), json!(port));
    attach_and_detach(&adapters, &root, attach, &mut waiting);
}

/// Replaces the folders of this Mac in a recording with fixed ones, so the fixture names no user and no temporary
/// folder.
fn anonymize(path: &Path, root: &Path) {
    let mut text = std::fs::read_to_string(path).unwrap();
    let canonical = std::fs::canonicalize(root).unwrap();
    for (from, to) in [
        (canonical.display().to_string(), "/tmp/dap-sample".to_string()),
        (root.display().to_string(), "/tmp/dap-sample".to_string()),
        (std::env::var("HOME").unwrap(), "/Users/me".to_string()),
    ] {
        text = text.replace(&from, &to);
    }
    std::fs::write(path, text).unwrap();
}

fn stopped(rx: &Receiver<Event>) -> ide_dap::Stop {
    let deadline = Instant::now() + WAIT;
    while Instant::now() < deadline {
        match rx.recv_timeout(Duration::from_millis(200)) {
            Ok(Event::Stopped(stop)) => return stop,
            Ok(Event::Ended(end)) => panic!("ended: {end:?}"),
            _ => {}
        }
    }
    panic!("the program did not stop");
}

fn which(name: &str) -> Option<PathBuf> {
    std::env::var("PATH")
        .ok()?
        .split(':')
        .map(|dir| Path::new(dir).join(name))
        .find(|p| p.is_file())
}

//! Who owns which process (Review C7), with the fake adapter as a process under the supervisor: a launched program
//! ends with its session and with a crashed adapter, an attached program and foreign processes keep running.

use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Child, Command};
use std::sync::mpsc::{Receiver, channel};
use std::time::{Duration, Instant};

use ide_dap::fake::Script;
use ide_dap::{Adapter, AdapterKind, EndReason, Event, LaunchConfig, Session, Setup, State};
use ide_process::Supervisor;
use serde_json::json;

const WAIT: Duration = Duration::from_secs(10);

fn alive(pid: u32) -> bool {
    let pid = libc::pid_t::try_from(pid).unwrap();
    // SAFETY: Signal 0 only asks whether the process exists.
    unsafe { libc::kill(pid, 0) == 0 }
}

fn gone_within(pid: u32, timeout: Duration) -> bool {
    let deadline = Instant::now() + timeout;
    while alive(pid) && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(20));
    }
    !alive(pid)
}

/// A program of somebody else's, in a process group of its own, as a program the user started in a shell is.
struct Foreign(Child);

impl Foreign {
    fn start(own_group: bool) -> Foreign {
        let mut command = Command::new("/bin/sleep");
        command.arg("60");
        if own_group {
            command.process_group(0);
        }
        Foreign(command.spawn().unwrap())
    }

    fn pid(&self) -> u32 {
        self.0.id()
    }

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

fn script_file(script: &Script, name: &str) -> PathBuf {
    let path = std::env::temp_dir().join(format!("ide-dap-ownership-{}-{name}.json", std::process::id()));
    std::fs::write(&path, serde_json::to_vec(script).unwrap()).unwrap();
    path
}

fn adapter(script: &Script, name: &str) -> Adapter {
    let file = script_file(script, name);
    let env: Vec<(String, String)> = std::env::vars().collect();
    Adapter::command(
        AdapterKind::Fake,
        Path::new(env!("CARGO_BIN_EXE_ide-dap-fake-adapter")),
        &[file.display().to_string()],
        env,
        &std::env::temp_dir(),
    )
}

/// A session that stops at line 3 and stays there.
fn start(supervisor: &Supervisor, script: &Script, name: &str, config: LaunchConfig) -> (Session, Receiver<Event>) {
    let mut setup = Setup::default();
    setup
        .breakpoints
        .insert(script.path.clone(), vec![script.breakpoint(3)]);
    let (tx, rx) = channel();
    let session = Session::start(supervisor, adapter(script, name), config, setup, move |_, event| {
        let _ = tx.send(event);
    })
    .unwrap();
    assert!(
        session.wait_for(WAIT, |s| matches!(s, State::Stopped(_))),
        "{:?}",
        session.state()
    );
    (session, rx)
}

fn launching(script: &Script, program: &[&str]) -> LaunchConfig {
    let mut config = script.launch_config();
    config.arguments.insert("spawn".into(), json!(program));
    config
}

fn program_pid(rx: &Receiver<Event>) -> u32 {
    let deadline = Instant::now() + WAIT;
    while Instant::now() < deadline {
        if let Ok(Event::Program(program)) = rx.recv_timeout(Duration::from_millis(100))
            && let Some(pid) = program.pid
        {
            return pid;
        }
    }
    panic!("no program");
}

#[test]
fn detaching_leaves_the_attached_program_and_foreign_processes_running() {
    let supervisor = Supervisor::new();
    let script = Script::default();
    let mut attached = Foreign::start(true);
    let mut bystander = Foreign::start(false);
    let config = script.attach_config(attached.pid());
    let (session, _rx) = start(&supervisor, &script, "detach", config);
    let adapter = supervisor.running()[0].pid;
    session.stop();
    assert!(session.wait_until_ended(WAIT));
    assert_eq!(session.end().unwrap().reason, EndReason::Stopped);
    assert!(gone_within(adapter, WAIT), "the adapter belongs to the app and ends");
    std::thread::sleep(Duration::from_millis(300));
    assert!(attached.running(), "the attached program was signalled");
    assert!(bystander.running(), "a foreign process was signalled");
}

#[test]
fn quitting_the_app_detaches_attached_sessions_through_the_supervisor() {
    let supervisor = Supervisor::new();
    let script = Script::default();
    let mut attached = Foreign::start(true);
    let config = script.attach_config(attached.pid());
    let (session, _rx) = start(&supervisor, &script, "quit", config);
    supervisor.shutdown(Duration::from_secs(2));
    assert!(supervisor.running().is_empty());
    assert!(session.wait_until_ended(WAIT));
    std::thread::sleep(Duration::from_millis(300));
    assert!(attached.running(), "the attached program was signalled");
}

#[test]
fn a_launched_program_ends_with_its_session() {
    let supervisor = Supervisor::new();
    let script = Script::default();
    let (session, rx) = start(
        &supervisor,
        &script,
        "launch",
        launching(&script, &["/bin/sleep", "60"]),
    );
    let program = program_pid(&rx);
    assert!(alive(program));
    session.stop();
    assert!(session.wait_until_ended(WAIT));
    assert!(gone_within(program, WAIT), "the launched program outlived its session");
    assert!(supervisor.running().is_empty());
}

#[test]
fn without_terminate_the_launched_program_still_ends_through_disconnect() {
    let supervisor = Supervisor::new();
    let script = Script {
        terminate: false,
        ..Script::default()
    };
    let (session, rx) = start(
        &supervisor,
        &script,
        "disconnect",
        launching(&script, &["/bin/sleep", "60"]),
    );
    let program = program_pid(&rx);
    assert!(session.stop_and_wait(WAIT));
    assert!(gone_within(program, WAIT));
}

#[test]
fn an_adapter_crash_ends_the_session_with_a_message_and_the_launched_program_with_its_group() {
    let supervisor = Supervisor::new();
    let script = Script::default();
    let (session, rx) = start(&supervisor, &script, "crash", launching(&script, &["/bin/sleep", "60"]));
    let program = program_pid(&rx);
    let adapter = supervisor.running()[0].pid;
    // Only the adapter itself, as a crash would end it; its program stays behind in its group.
    let status = Command::new("/bin/kill")
        .args(["-9", &adapter.to_string()])
        .status()
        .unwrap();
    assert!(status.success());
    assert!(session.wait_until_ended(WAIT));
    let end = session.end().unwrap();
    assert!(
        matches!(
            end.reason,
            EndReason::AdapterCrashed {
                ending: Some(ide_process::Ending::Signal(9)),
                ..
            }
        ),
        "{end:?}"
    );
    assert!(
        gone_within(program, WAIT),
        "the launched program outlived the crashed adapter"
    );
}

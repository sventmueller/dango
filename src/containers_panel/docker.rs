//! Running `docker` for the Containers panel: every call through the supervisor with the login shell's environment
//! (E10, E23), the reads on a worker thread, `docker events` as a long-running process whose lines arrive one by one.

use std::io::{BufRead, BufReader, Read};
use std::process::{Command, Stdio};
use std::time::Duration;

use ide_process::{Ending, Kind, Supervisor};

/// How long a single call may take.
const LIMIT: Duration = Duration::from_secs(120);

/// The `docker` program on the login shell's PATH.
pub fn program(env: &[(String, String)]) -> Option<std::path::PathBuf> {
    env.iter().find(|(k, _)| k == "PATH").and_then(|(_, path)| {
        path.split(':')
            .map(|d| std::path::Path::new(d).join("docker"))
            .find(|p| p.is_file())
    })
}

fn command(program: &std::path::Path, args: &ide_docker::Command, env: &[(String, String)]) -> Command {
    let mut process = Command::new(program);
    process
        .args(&args.args)
        .env_clear()
        .envs(env.iter().map(|(k, v)| (k, v)))
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    if let Some(cwd) = &args.cwd {
        process.current_dir(cwd);
    }
    process
}

/// Runs `args` to its end on the calling thread: its output, or its error output as the error.
pub fn run(args: &ide_docker::Command, env: &[(String, String)]) -> Result<String, String> {
    let program = program(env).ok_or_else(|| "docker".to_string())?;
    let label = format!("docker {}", args.args.get(2).cloned().unwrap_or_default());
    let mut spawned = Supervisor::global()
        .spawn(command(&program, args, env), Kind::Tool, label)
        .map_err(|e| e.to_string())?;
    let read = |pipe: Option<Box<dyn Read + Send>>| {
        std::thread::spawn(move || {
            let mut out = String::new();
            if let Some(mut pipe) = pipe {
                let _ = pipe.read_to_string(&mut out);
            }
            out
        })
    };
    let stdout = read(spawned.stdout.take().map(|p| Box::new(p) as Box<dyn Read + Send>));
    let stderr = read(spawned.stderr.take().map(|p| Box::new(p) as Box<dyn Read + Send>));
    let ending = Supervisor::global().wait(spawned.id, LIMIT);
    if ending.is_none() {
        Supervisor::global().stop(spawned.id, Duration::from_secs(1));
    }
    let (out, err) = (stdout.join().unwrap_or_default(), stderr.join().unwrap_or_default());
    match ending {
        Some(Ending::Code(0)) => Ok(out),
        _ => Err(if err.trim().is_empty() { out } else { err }.trim().to_string()),
    }
}

/// Starts `docker events` and calls `line` with each line it prints and `ended` once it ends, both on its thread.
pub fn watch(
    args: &ide_docker::Command,
    env: &[(String, String)],
    line: impl Fn(&str) + Send + 'static,
    ended: impl FnOnce(String) + Send + 'static,
) -> Result<ide_process::ProcessId, String> {
    let program = program(env).ok_or_else(|| "docker".to_string())?;
    let mut spawned = Supervisor::global()
        .spawn(command(&program, args, env), Kind::Tool, "docker events")
        .map_err(|e| e.to_string())?;
    let id = spawned.id;
    let stdout = spawned.stdout.take();
    let mut stderr = spawned.stderr.take();
    std::thread::Builder::new()
        .name("docker-events".into())
        .spawn(move || {
            if let Some(stdout) = stdout {
                for read in BufReader::new(stdout).lines() {
                    match read {
                        Ok(text) => line(&text),
                        Err(_) => break,
                    }
                }
            }
            let mut error = String::new();
            if let Some(stderr) = stderr.as_mut() {
                let _ = stderr.read_to_string(&mut error);
            }
            ended(error.trim().to_string());
        })
        .map_err(|e| e.to_string())?;
    Ok(id)
}

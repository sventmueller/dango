//! The user's running processes, to pick one to attach to.

use std::path::Path;
use std::time::Duration;

use ide_process::Supervisor;

use crate::helper::Helper;

/// A running process.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Process {
    /// Its number.
    pub pid: u32,
    /// The name of its program.
    pub name: String,
    /// Its command line.
    pub command: String,
}

/// The processes of the current user except the app itself, newest first; blocks while `ps` runs.
pub fn processes(supervisor: &Supervisor, env: &[(String, String)]) -> std::io::Result<Vec<Process>> {
    let helper = Helper {
        supervisor,
        env,
        timeout: Duration::from_secs(10),
        cancel: None,
    };
    // Names and command lines may hold spaces, so each comes last in a listing of its own.
    let mut lists = Vec::new();
    for columns in ["pid=,uid=,ucomm=", "pid=,args="] {
        let output = helper.run(
            Path::new("/bin/ps"),
            &["-axww", "-o", columns],
            Path::new("/"),
            &mut |_| {},
        )?;
        if !output.succeeded() {
            return Err(std::io::Error::other(output.stderr));
        }
        lists.push(output.stdout);
    }
    // SAFETY: getuid only reads the calling process's user id and cannot fail.
    let uid = unsafe { libc::getuid() };
    Ok(parse(&lists[0], &lists[1], uid, std::process::id()))
}

/// Splits off the first `n` fields of `line` and returns them with the rest, which may hold spaces.
fn fields(line: &str, n: usize) -> Option<(Vec<&str>, &str)> {
    let mut rest = line.trim_start();
    let mut first = Vec::with_capacity(n);
    for _ in 0..n {
        let end = rest.find(char::is_whitespace)?;
        first.push(&rest[..end]);
        rest = rest[end..].trim_start();
    }
    Some((first, rest.trim_end()))
}

/// The processes of `uid` without `own`, from the listings `pid uid name` and `pid command`.
fn parse(names: &str, commands: &str, uid: u32, own: u32) -> Vec<Process> {
    let commands: std::collections::HashMap<u32, &str> = commands
        .lines()
        .filter_map(|line| {
            let (first, rest) = fields(line, 1)?;
            Some((first[0].parse().ok()?, rest))
        })
        .collect();
    let mut list: Vec<Process> = names
        .lines()
        .filter_map(|line| {
            let (first, name) = fields(line, 2)?;
            let pid: u32 = first[0].parse().ok()?;
            let owner: u32 = first[1].parse().ok()?;
            (owner == uid && pid != own).then(|| Process {
                pid,
                name: name.to_string(),
                command: commands
                    .get(&pid)
                    .map_or_else(|| name.to_string(), |c| (*c).to_string()),
            })
        })
        .collect();
    list.sort_by_key(|p| std::cmp::Reverse(p.pid));
    list
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_the_users_processes_are_listed_without_the_app() {
        let names = "  1     0 launchd\n  501   501 zsh\n  777   501 ide\n  902   501 Google Chrome Helper\ngarbage\n";
        let commands = "1 /sbin/launchd\n501 -zsh\n902 /Applications/Google Chrome.app/Helper --type=gpu\n";
        let list = parse(names, commands, 501, 777);
        let found: Vec<(u32, &str)> = list.iter().map(|p| (p.pid, p.name.as_str())).collect();
        assert_eq!(found, [(902, "Google Chrome Helper"), (501, "zsh")]);
        assert_eq!(list[0].command, "/Applications/Google Chrome.app/Helper --type=gpu");
    }

    #[test]
    fn the_list_of_this_mac_holds_this_test() {
        let env: Vec<(String, String)> = std::env::vars().collect();
        let list = processes(&Supervisor::new(), &env).unwrap();
        assert!(!list.is_empty());
        assert!(list.iter().all(|p| p.pid != std::process::id()));
    }
}

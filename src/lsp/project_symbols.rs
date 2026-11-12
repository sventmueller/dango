//! The project's symbols for Open Quickly (E85): a search asks the servers of every language the folder holds, which
//! it starts itself and keeps through a lease, whether an editor uses them or not (review C17).
//!
//! The first search in a while surveys the folder off the main thread: the languages of its files that ignore rules
//! do not hide, each with the project roots the registry's markers find. Their servers start two at a time, only in a
//! trusted folder (E22). A new query cancels what the previous one still waits for.

use std::cell::RefCell;
use std::collections::{HashMap, HashSet, VecDeque};
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::Arc;
use std::time::Instant;

use ide_lsp::{CustomServer, Language, ProjectSymbol, Server, Status, Ticket, uri};

use super::lease::IDLE_STOP;
use super::{State, custom_servers, end_later, slot, slot_at, workspace, workspace_or_new};
use crate::util::main_soon;

/// Servers that may start at the same time for a search.
const STARTING_AT_ONCE: usize = 2;

/// Symbols one answer lists at most.
const LIMIT: usize = 100;

/// The protocol's error for an answer the server dropped because the document changed meanwhile.
const CONTENT_MODIFIED: i64 = -32801;

/// A symbol a search found, with the server that knows more about it.
#[derive(Clone)]
pub struct FoundSymbol {
    pub symbol: ProjectSymbol,
    pub path: PathBuf,
    server: Server,
}

/// What a search has found so far.
pub struct SymbolAnswer {
    pub symbols: Vec<FoundSymbol>,
    /// Whether a server still starts or indexes, so that finding nothing may change.
    pub indexing: bool,
    /// Whether the folder is not trusted, so no server runs for it.
    pub untrusted: bool,
}

enum Plan {
    Surveying,
    Ready(Vec<(Language, PathBuf)>),
}

/// The servers the search of one folder holds, and the query it waits on.
struct Lease {
    plan: Plan,
    queue: VecDeque<(Language, PathBuf)>,
    /// Slots that were new when the search made them and have not finished starting.
    starting: Vec<u64>,
    slots: Vec<u64>,
    last: Instant,
    generation: u64,
    tickets: Vec<Ticket>,
    waiting: usize,
    found: Vec<FoundSymbol>,
    query: String,
    answer: Option<Rc<dyn Fn(SymbolAnswer)>>,
}

thread_local! {
    static LEASES: RefCell<HashMap<PathBuf, Lease>> = RefCell::new(HashMap::new());
}

/// Searches the symbols of the window folder `folder` for `query`; `answer` hears what was found each time a server
/// answers, on the main thread, until the next search of the folder.
pub fn search_symbols(folder: &Path, query: &str, answer: impl Fn(SymbolAnswer) + 'static) {
    if !crate::project::is_trusted(folder) {
        answer(SymbolAnswer {
            symbols: vec![],
            indexing: false,
            untrusted: true,
        });
        return;
    }
    let now = Instant::now();
    let survey = LEASES.with(|leases| {
        let mut leases = leases.borrow_mut();
        let fresh = leases.get(folder).is_some_and(|lease| now < lease.last + IDLE_STOP);
        let lease = leases.entry(folder.to_path_buf()).or_insert_with(|| Lease {
            plan: Plan::Surveying,
            queue: VecDeque::new(),
            starting: vec![],
            slots: vec![],
            last: now,
            generation: 0,
            tickets: vec![],
            waiting: 0,
            found: vec![],
            query: String::new(),
            answer: None,
        });
        lease.last = now;
        for ticket in lease.tickets.drain(..) {
            ticket.cancel();
        }
        lease.generation += 1;
        lease.waiting = 0;
        lease.found.clear();
        lease.query = query.trim().to_string();
        lease.answer = Some(Rc::new(answer));
        if fresh {
            return false;
        }
        lease.plan = Plan::Surveying;
        true
    });
    if survey {
        start_survey(folder);
    }
    renew(folder, now);
    pump(folder);
    ask(folder);
}

/// Surveys `folder` off the main thread and queues the servers it needs.
fn start_survey(folder: &Path) {
    let custom = custom_servers();
    let folder = folder.to_path_buf();
    let _ = std::thread::Builder::new()
        .name("lsp-symbol-survey".into())
        .spawn(move || {
            let plan = survey(&folder, &custom);
            main_soon(move || surveyed(&folder, plan));
        });
}

fn surveyed(folder: &Path, plan: Vec<(Language, PathBuf)>) {
    LEASES.with(|leases| {
        if let Some(lease) = leases.borrow_mut().get_mut(folder) {
            lease.plan = Plan::Ready(plan);
        }
    });
    renew(folder, Instant::now());
    pump(folder);
    ask(folder);
}

/// The servers a symbol search needs in `folder`: one per language that has files there that ignore rules do not hide
/// and whose servers search symbols, and per project root of those files.
fn survey(folder: &Path, custom: &[Arc<CustomServer>]) -> Vec<(Language, PathBuf)> {
    let Ok(index) = ide_project::FileIndex::build(folder, &ide_project::Cancel::new()) else {
        return vec![];
    };
    let mut directories: HashSet<(Language, PathBuf)> = HashSet::new();
    for file in index.files() {
        let path = folder.join(file);
        let Some((language, _)) = Language::detect(&path, custom) else {
            continue;
        };
        if language.searches_project() {
            let directory = path.parent().unwrap_or(folder).to_path_buf();
            directories.insert((language, directory));
        }
    }
    let mut plan: Vec<(Language, PathBuf)> = directories
        .into_iter()
        .map(|(language, directory)| {
            let root = ide_lsp::root(&language, &directory, folder);
            (language, root)
        })
        .collect::<HashSet<_>>()
        .into_iter()
        .collect();
    plan.sort_by(|a, b| (a.0.name(), &a.1).cmp(&(b.0.name(), &b.1)));
    plan
}

/// Renews the lease of every slot the search of `folder` holds and queues the planned servers that do not run.
fn renew(folder: &Path, now: Instant) {
    LEASES.with(|leases| {
        let mut leases = leases.borrow_mut();
        let Some(lease) = leases.get_mut(folder) else { return };
        lease.slots.retain(|id| slot(folder, *id).is_some());
        for id in &lease.slots {
            if let Some(slot) = slot(folder, *id) {
                let mut hold = slot.hold.get();
                hold.lease(now);
                slot.hold.set(hold);
                end_later(folder, &slot);
            }
        }
        let Plan::Ready(plan) = &lease.plan else { return };
        let running: Vec<(Language, PathBuf)> = workspace(folder)
            .map(|w| {
                w.slots
                    .borrow()
                    .iter()
                    .map(|s| (s.language.clone(), s.root.clone()))
                    .collect()
            })
            .unwrap_or_default();
        for wanted in plan {
            if !running.contains(wanted) && !lease.queue.contains(wanted) {
                lease.queue.push_back(wanted.clone());
            }
        }
    });
}

/// Starts queued servers while fewer than two start, and lets go of servers that cannot search symbols.
pub(super) fn pump(folder: &Path) {
    LEASES.with(|leases| {
        let mut leases = leases.borrow_mut();
        let Some(lease) = leases.get_mut(folder) else { return };
        lease
            .starting
            .retain(|id| slot(folder, *id).is_some_and(|s| starting(&s.state.borrow())));
        lease.slots.retain(|id| {
            let Some(slot) = slot(folder, *id) else { return false };
            let useless = matches!(&*slot.state.borrow(), State::Running(server)
                if server.status() == Status::Running && !server.offers("workspaceSymbolProvider", "workspace/symbol"));
            if useless {
                let mut hold = slot.hold.get();
                hold.end_lease();
                slot.hold.set(hold);
                end_later(folder, &slot);
            }
            !useless
        });
        while lease.starting.len() < STARTING_AT_ONCE
            && let Some((language, root)) = lease.queue.pop_front()
        {
            let workspace = workspace_or_new(folder);
            let (slot, new) = slot_at(&workspace, &language, root);
            let mut hold = slot.hold.get();
            hold.lease(lease.last);
            slot.hold.set(hold);
            end_later(folder, &slot);
            if !lease.slots.contains(&slot.id) {
                lease.slots.push(slot.id);
            }
            if new {
                lease.starting.push(slot.id);
            }
        }
    });
}

/// Whether a slot in `state` is still on its way to answering requests.
fn starting(state: &State) -> bool {
    match state {
        State::Finding => true,
        State::Running(server) => matches!(server.status(), Status::Starting | Status::Restarting { .. }),
        State::Missing(_) | State::Installing(..) | State::Failed(_) => false,
    }
}

/// Whether a server of the search of `folder` still starts or indexes, or the survey is not done.
fn indexing(lease: &Lease, folder: &Path) -> bool {
    if matches!(lease.plan, Plan::Surveying) || !lease.queue.is_empty() {
        return true;
    }
    lease.slots.iter().filter_map(|id| slot(folder, *id)).any(|slot| {
        starting(&slot.state.borrow())
            || matches!(&*slot.state.borrow(), State::Running(_)) && !slot.progress.borrow().is_empty()
    })
}

/// Sends the query of the search of `folder` to every running server it holds, or answers at once without any.
fn ask(folder: &Path) {
    let (query, generation, servers) = LEASES.with(|leases| {
        let leases = leases.borrow();
        let Some(lease) = leases.get(folder) else {
            return (String::new(), 0, vec![]);
        };
        let servers: Vec<Server> = lease
            .slots
            .iter()
            .filter_map(|id| slot(folder, *id))
            .filter_map(|slot| match &*slot.state.borrow() {
                State::Running(server) if server.status() == Status::Running => Some(server.clone()),
                _ => None,
            })
            .collect();
        (lease.query.clone(), lease.generation, servers)
    });
    if query.is_empty() {
        deliver(folder);
        return;
    }
    let mut tickets = vec![];
    for server in servers {
        let (folder, asking) = (folder.to_path_buf(), server.clone());
        let asked = server.project_symbols(&query, move |answer| {
            main_soon(move || arrived(&folder, generation, &asking, answer));
        });
        if let Ok(ticket) = asked {
            tickets.push(ticket);
        }
    }
    LEASES.with(|leases| {
        if let Some(lease) = leases.borrow_mut().get_mut(folder)
            && lease.generation == generation
        {
            lease.waiting = tickets.len();
            lease.tickets = tickets;
        }
    });
    deliver(folder);
}

fn arrived(folder: &Path, generation: u64, server: &Server, answer: Result<Vec<ProjectSymbol>, ide_lsp::Error>) {
    let current = LEASES.with(|leases| {
        let mut leases = leases.borrow_mut();
        let Some(lease) = leases.get_mut(folder).filter(|l| l.generation == generation) else {
            return false;
        };
        lease.waiting = lease.waiting.saturating_sub(1);
        match answer {
            Ok(symbols) => lease.found.extend(symbols.into_iter().filter_map(|symbol| {
                Some(FoundSymbol {
                    path: uri::to_path(&symbol.uri)?,
                    symbol,
                    server: server.clone(),
                })
            })),
            Err(ide_lsp::Error::Cancelled) => {}
            Err(ide_lsp::Error::Server { code, .. }) if code == CONTENT_MODIFIED => {}
            Err(error) => crate::diag::report("searching the project's symbols", error),
        }
        true
    });
    if current {
        deliver(folder);
    }
}

/// Hands what the search of `folder` found so far, best matches first, to its listener.
fn deliver(folder: &Path) {
    let delivered = LEASES.with(|leases| {
        let leases = leases.borrow();
        let lease = leases.get(folder)?;
        let mut symbols = lease.found.clone();
        let query = lease.query.to_lowercase();
        symbols.sort_by_cached_key(|found| (rank(&found.symbol.name, &lease.query, &query), found.symbol.name.len()));
        symbols.truncate(LIMIT);
        let waiting = lease.waiting > 0 && symbols.is_empty();
        let answer = SymbolAnswer {
            symbols,
            indexing: waiting || indexing(lease, folder),
            untrusted: false,
        };
        Some((lease.answer.clone()?, answer))
    });
    if let Some((listener, answer)) = delivered {
        listener(answer);
    }
}

/// How well `name` matches `query`: the exact name first, then without case, then names that start with it, then
/// those that contain it, then whatever else the server found.
fn rank(name: &str, query: &str, lowered: &str) -> u8 {
    let name_lowered = name.to_lowercase();
    if name == query {
        0
    } else if name_lowered == lowered {
        1
    } else if name_lowered.starts_with(lowered) {
        2
    } else if name_lowered.contains(lowered) {
        3
    } else {
        4
    }
}

/// Opens the place of `found` in the window of `folder`, asking its server for the range first when it left it out.
pub fn open_symbol(folder: &Path, found: &FoundSymbol) {
    if let Some(range) = found.symbol.range {
        super::navigate::show(folder, &found.path, Some(range));
        return;
    }
    let (folder, path) = (folder.to_path_buf(), found.path.clone());
    let fallback = (folder.clone(), path.clone());
    let asked = found
        .server
        .resolve_project_symbol(found.symbol.clone(), move |resolved| {
            let range = resolved.ok().and_then(|s| s.range);
            main_soon(move || {
                super::navigate::show(&folder, &path, range);
            });
        });
    if asked.is_err() {
        super::navigate::show(&fallback.0, &fallback.1, None);
    }
}

/// The window of `folder` closed: its search lets go of everything.
pub(super) fn forget(folder: &Path) {
    LEASES.with(|leases| {
        if let Some(lease) = leases.borrow_mut().remove(folder) {
            for ticket in lease.tickets {
                ticket.cancel();
            }
        }
    });
}

/// The servers the symbol search of `folder` holds, each with its language, whether it runs, and how many editors use
/// it, for the self-test.
#[cfg(feature = "selftest")]
pub fn held(folder: &Path) -> Vec<(String, bool, usize)> {
    let ids = LEASES.with(|leases| leases.borrow().get(folder).map(|l| l.slots.clone()).unwrap_or_default());
    ids.into_iter()
        .filter_map(|id| slot(folder, id))
        .map(|slot| {
            let running = matches!(&*slot.state.borrow(), State::Running(server) if server.status() == Status::Running);
            (
                slot.language.name().to_string(),
                running,
                super::views_of(folder, slot.id).len(),
            )
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write(root: &Path, relative: &str, text: &str) {
        let path = root.join(relative);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, text).unwrap();
    }

    #[test]
    fn a_folder_with_three_languages_gets_a_server_per_language_and_root() {
        let folder = std::env::temp_dir().join(format!("ide-symbol-survey-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&folder);
        write(&folder, "Cargo.toml", "[package]\nname = \"a\"\n");
        write(&folder, "src/main.rs", "fn main() {}\n");
        write(&folder, "src/deep/more.rs", "fn more() {}\n");
        write(&folder, "go/go.mod", "module example.com/go\n");
        write(&folder, "go/cmd/main.go", "package main\n");
        write(&folder, "web/package.json", "{}\n");
        write(&folder, "web/src/index.ts", "export {};\n");
        write(&folder, ".gitignore", "build/\n");
        write(&folder, "build/generated.py", "x = 1\n");
        let plan = survey(&folder, &[]);
        let _ = std::fs::remove_dir_all(&folder);
        assert_eq!(
            plan,
            vec![
                (Language::Go, folder.join("go")),
                (Language::Rust, folder.clone()),
                (Language::TypeScript, folder.join("web")),
            ],
            "no Python from the ignored folder, no servers for TOML and JSON"
        );
    }

    #[test]
    fn exact_names_come_before_prefixes_and_other_matches() {
        let mut names = vec!["parse_all", "Parse", "reparse", "parse", "other"];
        names.sort_by_key(|n| (rank(n, "parse", "parse"), n.len()));
        assert_eq!(names, ["parse", "Parse", "parse_all", "reparse", "other"]);
    }
}

//! The project's symbols for Open Quickly (E85): a search asks the servers of every language the folder holds, which
//! it starts itself and keeps through a lease, whether an editor uses them or not (review C17).
//!
//! The first search in a while surveys the folder off the main thread: the languages of its files that ignore rules
//! do not hide, each with the project roots the registry's markers find. Their servers start two at a time, only in a
//! trusted folder (E22). A new query cancels what the previous one still waits for.

use std::cell::RefCell;
use std::collections::{HashMap, VecDeque};
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::time::Instant;

use ide_lsp::{Language, ProjectSymbol, Server, Status, Ticket, uri};

use super::lease::IDLE_STOP;
use super::{State, custom_servers, end_later, slot, slot_at, workspace, workspace_or_new};
use ide_ui_common::util::main_soon;

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

/// The language servers lent to the project symbol search, by folder.
#[derive(Default)]
pub(crate) struct Leases {
    leases: RefCell<HashMap<PathBuf, Lease>>,
}

fn with_leases<R>(f: impl FnOnce(&RefCell<HashMap<PathBuf, Lease>>) -> R) -> R {
    f(&crate::host::state().server_leases.leases)
}

/// Searches the symbols of the window folder `folder` for `query`; `answer` hears what was found each time a server
/// answers, on the main thread, until the next search of the folder.
pub fn search_symbols(folder: &Path, query: &str, answer: impl Fn(SymbolAnswer) + 'static) {
    if !crate::host::is_trusted(folder) {
        answer(SymbolAnswer {
            symbols: vec![],
            indexing: false,
            untrusted: true,
        });
        return;
    }
    let now = Instant::now();
    let survey = with_leases(|leases| {
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
    let failed = folder.clone();
    if let Err(why) = ide_ui_common::util::spawn("lsp-symbol-survey", move || {
        let plan = ide_lsp::symbol_search::survey(&folder, &custom);
        main_soon(move || surveyed(&folder, plan));
    }) {
        ide_ui_common::diag::report("surveying a folder for symbol search", why);
        surveyed(&failed, Vec::new());
    }
}

fn surveyed(folder: &Path, plan: Vec<(Language, PathBuf)>) {
    with_leases(|leases| {
        if let Some(lease) = leases.borrow_mut().get_mut(folder) {
            lease.plan = Plan::Ready(plan);
        }
    });
    renew(folder, Instant::now());
    pump(folder);
    ask(folder);
}

/// Renews the lease of every slot the search of `folder` holds and queues the planned servers that do not run.
fn renew(folder: &Path, now: Instant) {
    with_leases(|leases| {
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
    with_leases(|leases| {
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
    let (query, generation, servers) = with_leases(|leases| {
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
    with_leases(|leases| {
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
    let current = with_leases(|leases| {
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
            Err(error) => ide_ui_common::diag::report("searching the project's symbols", error),
        }
        true
    });
    if current {
        deliver(folder);
    }
}

/// Hands what the search of `folder` found so far, best matches first, to its listener.
fn deliver(folder: &Path) {
    let delivered = with_leases(|leases| {
        let leases = leases.borrow();
        let lease = leases.get(folder)?;
        let mut symbols = lease.found.clone();
        let query = lease.query.to_lowercase();
        symbols.sort_by_cached_key(|found| {
            (
                ide_lsp::symbol_search::rank(&found.symbol.name, &lease.query, &query),
                found.symbol.name.len(),
            )
        });
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
    with_leases(|leases| {
        if let Some(lease) = leases.borrow_mut().remove(folder) {
            for ticket in lease.tickets {
                ticket.cancel();
            }
        }
    });
}

#[cfg(feature = "selftest")]
mod selftest_probe;
#[cfg(feature = "selftest")]
pub use selftest_probe::*;

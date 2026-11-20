//! The project's symbols for Open Quickly (E85): a search asks the servers of every language the folder holds, which
//! it starts itself and keeps through a lease, whether an editor uses them or not (review C17).
//!
//! The first search in a while surveys the folder off the main thread: the languages of its files that ignore rules
//! do not hide, each with the project roots the registry's markers find. Their servers start two at a time, only in a
//! trusted folder (E22). A new query cancels what the previous one still waits for. TypeScript's server knows only
//! the projects of its open files, so the search opens a few of them on it and closes them when it ends.

use std::cell::RefCell;
use std::collections::{HashMap, VecDeque};
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::time::Instant;

use ide_lsp::symbol_search::Planned;
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
    /// Whether it belongs to other people's code, such as a standard library or an installed package.
    pub dependency: bool,
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
    Ready(Vec<Planned>),
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
    /// The language of the file the user works on, whose symbols come first among equal matches.
    prefer: Option<Language>,
    answer: Option<Rc<dyn Fn(SymbolAnswer)>>,
    /// Files the search opened on a server, by slot, which it closes when it ends.
    seeded: Vec<Opened>,
    /// Slots whose files are being read to open them.
    seeding: Vec<u64>,
}

/// The language servers lent to the project symbol search, by folder.
#[derive(Default)]
pub(crate) struct Leases {
    leases: RefCell<HashMap<PathBuf, Lease>>,
}

fn with_leases<R>(f: impl FnOnce(&RefCell<HashMap<PathBuf, Lease>>) -> R) -> R {
    f(&crate::host::state().server_leases.leases)
}

/// Searches the symbols of the window folder `folder` for `query`, those in the language of `active` first among
/// equal matches; `answer` hears what was found each time a server answers, on the main thread, until the next search
/// of the folder.
pub fn search_symbols(folder: &Path, query: &str, active: Option<&Path>, answer: impl Fn(SymbolAnswer) + 'static) {
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
            prefer: None,
            answer: None,
            seeded: vec![],
            seeding: vec![],
        });
        lease.last = now;
        for ticket in lease.tickets.drain(..) {
            ticket.cancel();
        }
        lease.generation += 1;
        lease.waiting = 0;
        lease.found.clear();
        lease.query = query.trim().to_string();
        lease.prefer = active.and_then(super::language_of).map(|(language, _)| language);
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

fn surveyed(folder: &Path, plan: Vec<Planned>) {
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
        for planned in plan {
            let wanted = (planned.language.clone(), planned.root.clone());
            if !running.contains(&wanted) && !lease.queue.contains(&wanted) {
                lease.queue.push_back(wanted);
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
    if matches!(lease.plan, Plan::Surveying) || !lease.queue.is_empty() || !lease.seeding.is_empty() {
        return true;
    }
    lease.slots.iter().filter_map(|id| slot(folder, *id)).any(|slot| {
        starting(&slot.state.borrow())
            || matches!(&*slot.state.borrow(), State::Running(_)) && !slot.progress.borrow().is_empty()
    })
}

/// Files a search opened on the server of slot `slot`.
struct Opened {
    slot: u64,
    server: Server,
    paths: Vec<PathBuf>,
}

/// Files to open on a server, each with its language id.
type Seeds = Vec<(PathBuf, String)>;

/// Sends the query of the search of `folder` to every running server it holds, or answers at once without any.
fn ask(folder: &Path) {
    ask_after(folder, None);
}

/// Asks as `ask` does, once the files in `opened` were opened for the search.
fn ask_after(folder: &Path, opened: Option<Opened>) {
    let (query, generation, servers, wanted, late) = with_leases(|leases| {
        let mut leases = leases.borrow_mut();
        let Some(lease) = leases.get_mut(folder) else {
            return (String::new(), 0, vec![], vec![], opened);
        };
        let mut late = None;
        if let Some(opened) = opened {
            lease.seeding.retain(|slot| *slot != opened.slot);
            if lease.answer.is_some() {
                lease.seeded.push(opened);
            } else {
                late = Some(opened);
            }
        }
        let running: Vec<(u64, Server, Seeds)> = lease
            .slots
            .iter()
            .filter_map(|id| slot(folder, *id))
            .filter_map(|slot| match &*slot.state.borrow() {
                State::Running(server) if server.status() == Status::Running => Some((
                    slot.id,
                    server.clone(),
                    planned_seeds(&lease.plan, &slot.language, &slot.root),
                )),
                _ => None,
            })
            .collect();
        let wanted: Vec<(u64, Server, Seeds)> = if lease.query.is_empty() || lease.answer.is_none() {
            vec![]
        } else {
            running
                .iter()
                .filter(|(id, _, seeds)| {
                    !seeds.is_empty()
                        && !lease.seeding.contains(id)
                        && !lease.seeded.iter().any(|opened| opened.slot == *id)
                })
                .cloned()
                .collect()
        };
        lease.seeding.extend(wanted.iter().map(|(id, ..)| *id));
        let servers = running.into_iter().map(|(_, server, _)| server).collect();
        (lease.query.clone(), lease.generation, servers, wanted, late)
    });
    if let Some(late) = late {
        close_opened(late);
    }
    if query.is_empty() {
        deliver(folder);
        return;
    }
    for (slot, server, seeds) in wanted {
        seed(folder, slot, server, seeds);
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

/// The files the survey `plan` wants open on the server of `language` for `root`.
fn planned_seeds(plan: &Plan, language: &Language, root: &Path) -> Seeds {
    let Plan::Ready(plan) = plan else { return vec![] };
    plan.iter()
        .find(|p| p.language == *language && p.root == root)
        .map(|p| p.seeds.clone())
        .unwrap_or_default()
}

/// Reads `seeds` off the main thread and opens them on `server`, the server of slot `slot`, then asks again.
fn seed(folder: &Path, slot: u64, server: Server, seeds: Seeds) {
    let folder = folder.to_path_buf();
    let failed = (folder.clone(), server.clone());
    if let Err(why) = ide_ui_common::util::spawn("lsp-symbol-seed", move || {
        let read: Vec<(PathBuf, String, String)> = seeds
            .into_iter()
            .filter_map(|(path, language_id)| {
                let text = std::fs::read_to_string(&path).ok()?;
                Some((path, language_id, text))
            })
            .collect();
        main_soon(move || {
            let paths = read
                .into_iter()
                .filter(|(path, language_id, text)| server.open_borrowed(&uri::from_path(path), language_id, text))
                .map(|(path, ..)| path)
                .collect();
            ask_after(&folder, Some(Opened { slot, server, paths }));
        });
    }) {
        ide_ui_common::diag::report("reading files for the symbol search", why);
        let (folder, server) = failed;
        ask_after(
            &folder,
            Some(Opened {
                slot,
                server,
                paths: vec![],
            }),
        );
    }
}

fn close_opened(opened: Opened) {
    for path in opened.paths {
        opened.server.close_borrowed(&uri::from_path(&path));
    }
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
                let path = uri::to_path(&symbol.uri)?;
                Some(FoundSymbol {
                    dependency: ide_lsp::symbol_search::is_dependency(&path, folder),
                    path,
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
            let other_language = lease
                .prefer
                .as_ref()
                .is_some_and(|prefer| super::language_of(&found.path).is_none_or(|(language, _)| language != *prefer));
            (
                ide_lsp::symbol_search::rank(&found.symbol.name, &lease.query, &query),
                found.dependency,
                other_language,
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

/// The search of `folder` is over for now: it closes the files it opened that no editor took over.
pub fn end_symbol_search(folder: &Path) {
    let_go(folder, false);
}

/// The window of `folder` closed: its search lets go of everything.
pub(super) fn forget(folder: &Path) {
    let_go(folder, true);
}

fn let_go(folder: &Path, closed: bool) {
    let (tickets, seeded) = with_leases(|leases| {
        let mut leases = leases.borrow_mut();
        if closed {
            return leases
                .remove(folder)
                .map(|lease| (lease.tickets, lease.seeded))
                .unwrap_or_default();
        }
        let Some(lease) = leases.get_mut(folder) else {
            return (vec![], vec![]);
        };
        lease.answer = None;
        (vec![], std::mem::take(&mut lease.seeded))
    });
    for ticket in tickets {
        ticket.cancel();
    }
    for opened in seeded {
        close_opened(opened);
    }
}

#[cfg(feature = "selftest")]
mod selftest_probe;
#[cfg(feature = "selftest")]
pub use selftest_probe::*;

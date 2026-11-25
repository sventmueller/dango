//! The self-test's hooks of `project_symbols.rs`.

use super::*;

/// The servers the symbol search of `folder` holds, each with its language, whether it runs, and how many editors use
/// it, for the self-test.
pub fn held(folder: &Path) -> Vec<(String, bool, usize)> {
    let state = crate::host::state();
    let ids = state
        .server_leases
        .leases
        .borrow()
        .get(folder)
        .map(|l| l.slots.clone())
        .unwrap_or_default();
    ids.into_iter()
        .filter_map(|id| slot(&state, folder, id))
        .map(|slot| {
            let running = matches!(&*slot.state.borrow(), State::Running(server) if server.status() == Status::Running);
            (
                slot.language.name().to_string(),
                running,
                super::super::views_of(folder, slot.id).len(),
            )
        })
        .collect()
}

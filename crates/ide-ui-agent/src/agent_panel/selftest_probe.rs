//! What the self-test reads from and does with the parts of `mod.rs`, kept out of its production code.

#[allow(
    unused_imports,
    reason = "the parts moved here use only some of their module's names"
)]
use super::*;

/// The configuration folder of Claude Code, where it keeps its sessions.
pub fn claude_config_dir() -> std::path::PathBuf {
    config_dir()
}

/// The quiet texts of the session rows of `controller`'s panel, for the self-test.
pub fn session_details(controller: &(impl AsWindow + ?Sized)) -> Vec<String> {
    let controller = &controller.as_window();
    panel_of(controller).map_or_else(Vec::new, |panel| {
        panel.reload();
        panel.sessions_source.session_details()
    })
}

/// The context menu of row `row` of the session list of `controller`'s panel, for the self-test.
pub fn session_menu(controller: &(impl AsWindow + ?Sized), row: isize) -> Option<Retained<objc2_app_kit::NSMenu>> {
    let controller = &controller.as_window();
    let panel = panel_of(controller)?;
    panel.reload();
    panel.sessions_source.context_menu(row)
}

/// Opens the session of row `row` of `controller`'s panel, as a click does, for the self-test.
pub fn open_session_row(controller: &(impl AsWindow + ?Sized), row: usize) {
    let controller = &controller.as_window();
    if let Some(panel) = panel_of(controller) {
        panel.reload();
        panel.sessions_source.open_row(row);
    }
}

/// What the self-test reads from the panel.
pub struct Inspection {
    /// The rows top to bottom: "AI" or "CLI", whether the session is open, and its title.
    pub rows: Vec<(&'static str, bool, String)>,
    /// The titles of the "new session" menu.
    pub new_session: Vec<String>,
}

/// In the panel's coordinates, where the content of each row of the list begins and where it ends.
pub fn edges(controller: &(impl AsWindow + ?Sized)) -> Option<Vec<(f64, f64)>> {
    let controller = &controller.as_window();
    let panel = panel_of(controller)?;
    panel.reload();
    panel.view.layoutSubtreeIfNeeded();
    let frame = |view: &NSView| view.convertRect_toView(view.bounds(), Some(&panel.view));
    let mut rows = Vec::new();
    for row in 0..panel.sessions.numberOfRows() {
        let Some(cell) = panel.sessions.viewAtColumn_row_makeIfNecessary(0, row, true) else {
            continue;
        };
        cell.layoutSubtreeIfNeeded();
        let views = cell
            .subviews()
            .iter()
            .find_map(|view| view.downcast::<NSStackView>().ok())
            .map(|stack| stack.arrangedSubviews());
        if let Some((first, last)) = views.and_then(|v| v.firstObject().zip(v.lastObject())) {
            let last = frame(&last);
            rows.push((frame(&first).origin.x, last.origin.x + last.size.width));
        }
    }
    Some(rows)
}

pub fn inspect(controller: &(impl AsWindow + ?Sized)) -> Option<Inspection> {
    let controller = &controller.as_window();
    let panel = panel_of(controller)?;
    panel.reload();
    Some(Inspection {
        rows: panel.sessions_source.rows(),
        new_session: panel.agents.menu_titles(),
    })
}

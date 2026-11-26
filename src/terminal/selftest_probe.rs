//! The self-test's hooks of `terminal/mod.rs`.

pub(crate) use super::state::Heard;
use super::*;

/// The tab that shows the terminal view `id`.
pub(crate) fn tab_of_view(id: u64) -> Option<TabId> {
    crate::context::app().terminals().tab_of(id)
}

/// The dev server banner above the terminal view `id`.
pub(crate) fn dev_server_banner(id: u64) -> Option<Retained<dev_server::DevServerBanner>> {
    crate::context::app().terminals().banner_of(id)
}

/// What the tab of a terminal shows of its program's bell, notifications and progress.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Signals {
    /// Whether the app keeps the red dot for the tab, after a bell or a notification the user has not seen.
    pub(crate) waits: bool,
    /// Whether the tab shows a red dot.
    pub(crate) red_dot: bool,
    /// The progress the program reported last, until it ended.
    pub(crate) progress: Option<term_session::event::Progress>,
    /// The ring the tab shows beside its title.
    pub(crate) ring: Option<Ring>,
}

/// What the tab of the terminal view `id` shows of its program's signals.
pub(crate) fn signals_of_view(id: u64) -> Option<Signals> {
    let cx = crate::context::app();
    let terminals = cx.terminals();
    let (waits, progress) = terminals.signals_of(id)?;
    let tab = terminals.tab_of(id)?;
    let controller = cx.windows().get(find(id)?.window_id().get())?;
    let red_dot = controller
        .tab_dot(tab)
        .is_some_and(|color| *color == *objc2_app_kit::NSColor::systemRedColor());
    Some(Signals {
        waits,
        red_dot,
        progress,
        ring: controller.tab_ring(tab),
    })
}

/// The notifications the terminals' programs asked for while out of sight, oldest first.
pub(crate) fn heard_notifications() -> Vec<Heard> {
    crate::context::app().terminals().heard()
}

/// How many notifications the app posted since it started, or would have where posting is not allowed (R12).
pub(crate) fn notifications_posted() -> usize {
    ide_ui_common::notify::posted()
}

/// The progress bar the Dock icon shows.
pub(crate) fn dock_progress() -> Option<Ring> {
    let mtm = objc2::MainThreadMarker::new()?;
    crate::context::app().terminals().dock_ring(mtm)
}

//! The times the CI list shows, how long ago a run or merge request changed and how long a job runs, kept current in
//! the cells it built instead of building them again.

use std::time::Duration;

use objc2::DefinedClass;
use objc2::rc::Retained;
use objc2_app_kit::{NSAccessibility, NSTextField, NSUserInterfaceItemIdentification, NSView};
use objc2_foundation::NSString;

use super::model::{Key, Phase};
use super::panel::{CiPanel, with_panel};
use super::rows::key_of;
use crate::util::{main_after, ns};

/// The identifier of a cell's second line.
pub(super) const DETAIL: &str = "ideCiDetail";
/// The identifier of the value at a cell's end.
pub(super) const VALUE: &str = "ideCiValue";

/// How often the times advance while a job runs, whose duration shows seconds.
const RUNNING_TICK: Duration = Duration::from_secs(1);
/// How often they advance otherwise, when only ages in minutes and more show.
const IDLE_TICK: Duration = Duration::from_secs(30);

impl CiPanel {
    /// Keeps the times of the rows on screen advancing while the list is in view, unless that runs already.
    pub(crate) fn start_clock(&self) {
        if !self.ivars().ticking.replace(true) {
            self.plan_tick();
        }
    }

    fn plan_tick(&self) {
        let running = self.read().runs.iter().any(|r| {
            r.jobs
                .iter()
                .any(|j| j.started.is_some() && j.finished.is_none() && j.state.is_active())
        });
        let id = self.id();
        main_after(if running { RUNNING_TICK } else { IDLE_TICK }, move || {
            with_panel(id, CiPanel::tick);
        });
    }

    fn tick(&self) {
        if ide_ui_common::panel::out_of_view(&self.ui().page) || self.read().phase != Phase::Ready {
            self.ivars().ticking.set(false);
            return;
        }
        self.retime_shown();
        self.plan_tick();
    }

    /// Brings the times of the rows on screen up to now; the others catch up when they show again.
    pub(crate) fn retime_shown(&self) {
        let ivars = self.ivars();
        ivars.clock.set(ivars.clock.get() + 1);
        let outline = &self.ui().outline;
        let shown = outline.rowsInRect(outline.visibleRect());
        for row in shown.location..shown.location + shown.length {
            let row = row.cast_signed();
            let Some(key) = outline.itemAtRow(row).and_then(|item| key_of(&item)) else {
                continue;
            };
            if let Some(cell) = outline.viewAtColumn_row_makeIfNecessary(0, row, false) {
                self.retime(&key, &cell);
            }
        }
    }

    /// Notes that the cell of `key` shows the times as of now.
    pub(crate) fn note_timed(&self, key: &Key) {
        let ivars = self.ivars();
        ivars.timed.borrow_mut().insert(*key, ivars.clock.get());
    }

    /// Whether the cell of `key` shows times older than the last tick.
    pub(crate) fn is_stale(&self, key: &Key) -> bool {
        self.ivars().timed.borrow().get(key) != Some(&self.ivars().clock.get())
    }

    /// Forgets which cells show the times as of now, so each brings them up to now the next time it shows.
    pub(crate) fn forget_times(&self) {
        let ivars = self.ivars();
        ivars.timed.borrow_mut().clear();
        ivars.clock.set(ivars.clock.get() + 1);
    }

    /// Puts the times of `key`'s row as of now into its `cell`.
    pub(crate) fn retime(&self, key: &Key, cell: &NSView) {
        self.note_timed(key);
        if matches!(key, Key::Stage(..) | Key::Step(..)) {
            return;
        }
        let Some(texts) = self.texts(key) else { return };
        let mut moved = false;
        for (id, text) in [(DETAIL, &texts.detail), (VALUE, &texts.value)] {
            if let (Some(field), Some(text)) = (labelled(cell, &ns(id)), text)
                && field.stringValue().to_string() != *text
            {
                field.setStringValue(&ns(text));
                moved = true;
            }
        }
        if moved {
            let spoken = ns(&texts.spoken());
            cell.setAccessibilityLabel(Some(&spoken));
            cell.setToolTip(Some(&spoken));
        }
    }
}

/// The label below `view` with the identifier `id`.
fn labelled(view: &NSView, id: &NSString) -> Option<Retained<NSTextField>> {
    for sub in view.subviews() {
        if sub.identifier().is_some_and(|own| own.isEqualToString(id)) {
            return sub.downcast::<NSTextField>().ok();
        }
        if let Some(found) = labelled(&sub, id) {
            return Some(found);
        }
    }
    None
}

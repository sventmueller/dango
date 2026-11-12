//! What the self-test reads from a timeline: the rows built, their heights, the cards' texts and buttons.

use objc2::rc::Retained;
use objc2_app_kit::{NSAccessibility, NSView};

use super::timeline::{BOTTOM, TOP, Timeline};

impl Timeline {
    /// How many cards were built so far.
    pub fn built(&self) -> usize {
        self.built.get()
    }

    /// Checks the geometry of visible rows against the heights published to AppKit.
    pub fn height_discrepancy(&self) -> f64 {
        let range = self.table.rowsInRect(self.table.visibleRect());
        (range.location..range.location + range.length)
            .filter(|row| *row < self.count())
            .map(|row| {
                (self.table.rectOfRow(isize::try_from(row).unwrap_or(0)).size.height - self.row_height(row)).abs()
            })
            .fold(0.0, f64::max)
    }

    /// Includes deferred height correction and drawing in the streaming timing budget.
    pub fn settle(&self) {
        if let Some(window) = self.scroll.window() {
            window.layoutIfNeeded();
        }
        self.width_changed();
        for _ in 0..32 {
            self.flush_corrections();
            if let Some(window) = self.scroll.window() {
                window.displayIfNeeded();
            }
            if self.corrections.borrow().is_empty() {
                break;
            }
        }
    }

    /// The current answer's text view, for checking reuse and selection during streaming.
    pub fn streaming_text(&self) -> Option<Retained<objc2_app_kit::NSTextView>> {
        let row = isize::try_from(self.count().checked_sub(1)?).ok()?;
        let cell = self.table.viewAtColumn_row_makeIfNecessary(0, row, false)?;
        let text = cell
            .subviews()
            .firstObject()?
            .downcast::<super::text::CardText>()
            .ok()?;
        Some(text.into_super())
    }

    /// The rows in view whose height differs from what their card needs, as `row: table height / card height`.
    pub fn misfits(&self) -> Vec<String> {
        let range = self.table.rowsInRect(self.table.visibleRect());
        (range.location..range.location + range.length)
            .filter_map(|row| {
                let rect = self.table.rectOfRow(isize::try_from(row).ok()?);
                let view = self
                    .table
                    .viewAtColumn_row_makeIfNecessary(0, isize::try_from(row).ok()?, false)?;
                let card = view.subviews().firstObject()?.fittingSize().height + TOP + BOTTOM;
                ((rect.size.height - card).abs() > 1.5).then(|| format!("{row}: {} / {card}", rect.size.height))
            })
            .collect()
    }

    /// What the permission editors hold.
    pub fn editor_texts(&self) -> Vec<String> {
        let inputs = self.inputs.borrow();
        inputs.editors.keys().filter_map(|id| inputs.editor_text(*id)).collect()
    }

    /// The titles of the buttons on the card of the last row.
    pub fn last_card_buttons(&self) -> Vec<String> {
        fn collect(view: &NSView, out: &mut Vec<String>) {
            if let Some(button) = view.downcast_ref::<objc2_app_kit::NSButton>() {
                out.push(button.title().to_string());
            }
            for child in view.subviews().iter() {
                collect(&child, out);
            }
        }
        let last = self.table.numberOfRows() - 1;
        let Some(view) = (last >= 0)
            .then(|| self.table.viewAtColumn_row_makeIfNecessary(0, last, true))
            .flatten()
        else {
            return Vec::new();
        };
        let mut out = Vec::new();
        collect(&view, &mut out);
        out
    }

    /// The cards in view, as their VoiceOver summaries.
    pub fn visible_summaries(&self) -> Vec<String> {
        let range = self.table.rowsInRect(self.table.visibleRect());
        (range.location..range.location + range.length)
            .filter_map(|row| {
                let view = self
                    .table
                    .viewAtColumn_row_makeIfNecessary(0, isize::try_from(row).ok()?, false)?;
                let card = view.subviews().firstObject()?;
                card.accessibilityLabel().map(|l| l.to_string())
            })
            .collect()
    }
}

//! What the self-test reads from and does with the parts of `tab.rs`, kept out of its production code.

#[allow(
    unused_imports,
    reason = "the parts moved here use only some of their module's names"
)]
use super::*;

impl AgentTab {
    /// The texts of the empty state, top to bottom, with whether it holds anything else, such as an image.
    pub fn empty_state(&self) -> (Vec<String>, bool) {
        let views = self.empty.arrangedSubviews();
        let texts = views
            .iter()
            .filter_map(|view| view.downcast::<objc2_app_kit::NSTextField>().ok())
            .map(|field| field.stringValue().to_string())
            .collect();
        let other = views
            .iter()
            .any(|view| view.downcast_ref::<objc2_app_kit::NSTextField>().is_none());
        (texts, other)
    }

    /// Clicks across the composer and on its margin: whether each put the keyboard focus into its text.
    pub fn composer_clicks(&self) -> Vec<bool> {
        self.composer.clicks()
    }

    /// The width of the column the conversation reads in.
    pub fn column_width(&self) -> f64 {
        // SAFETY: Asking for the superview only reads; the column holds the timeline while the tab lives.
        unsafe { self.timeline.view().superview() }.map_or(0.0, |column| column.frame().size.width)
    }

    /// The width of the composer's card and how far its middle is off the column's.
    pub fn composer_place(&self) -> (f64, f64) {
        let card = self.composer.card.frame();
        // SAFETY: Asking for the superview only reads; the column holds the timeline while the tab lives.
        let column = unsafe { self.timeline.view().superview() };
        let middle = |frame: NSRect| frame.origin.x + frame.size.width / 2.0;
        let off = column.map_or(f64::NAN, |column| {
            let card = self
                .composer
                .card
                .convertRect_toView(self.composer.card.bounds(), Some(&column));
            middle(card) - column.bounds().size.width / 2.0
        });
        (card.size.width, off)
    }

    /// Whether a `claude` process runs for the tab.
    pub fn is_live(&self) -> bool {
        self.state.borrow().source().is_live()
    }
}

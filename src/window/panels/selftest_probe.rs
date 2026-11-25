//! What the self-test reads from and does with the parts of `panels.rs`, kept out of its production code.

#[allow(
    unused_imports,
    reason = "the parts moved here use only some of their module's names"
)]
use super::*;

/// Where the parts of a panel's title row sit, in the panel's coordinates.
pub(crate) struct TitleRow {
    pub(crate) title_start: f64,
    pub(crate) title_middle: f64,
    pub(crate) buttons_middle: f64,
    pub(crate) page_top: f64,
    pub(crate) buttons_start: f64,
    pub(crate) buttons_end: f64,
    pub(crate) text_end: f64,
    pub(crate) width: f64,
    /// The symbol configuration of each button in a title row's stack of buttons, and the stack's spacing.
    pub(crate) button_symbols: Vec<Option<Retained<objc2_app_kit::NSImageSymbolConfiguration>>>,
    pub(crate) button_spacing: f64,
}

impl Panels {
    pub(crate) fn title(&self) -> String {
        self.header.stringValue().to_string()
    }

    /// Whether the title row shows, and the texts and buttons of the shown page.
    pub(crate) fn seen(&self) -> (bool, Vec<String>) {
        fn texts(view: &NSView, out: &mut Vec<String>) {
            if view.isHidden() {
                return;
            }
            if let Some(field) = view.downcast_ref::<NSTextField>() {
                out.push(field.stringValue().to_string());
            } else if let Some(button) = view.downcast_ref::<objc2_app_kit::NSButton>() {
                // Once drawn, a button holds a private NSButtonTextField with its title, which would count it twice.
                out.push(button.title().to_string());
                return;
            }
            for child in view.subviews().iter() {
                texts(&child, out);
            }
        }
        let mut out = Vec::new();
        if let Some(page) = self.shown_page() {
            texts(&page, &mut out);
        }
        (!self.header.isHidden(), out)
    }

    /// The geometry of the title row, in the panel's coordinates.
    pub(crate) fn title_row(&self) -> TitleRow {
        use objc2_foundation::NSRect;
        self.view.layoutSubtreeIfNeeded();
        let in_panel = |view: &NSView| -> NSRect { view.convertRect_toView(view.bounds(), Some(&self.view)) };
        let middle = |rect: NSRect| rect.origin.y + rect.size.height / 2.0;
        let title = in_panel(&self.header);
        let buttons = self.accessory.arrangedSubviews();
        let first = buttons.firstObject().map(|view| in_panel(&view));
        let last = buttons.lastObject().map(|view| in_panel(&view));
        let page = self.shown_page().map_or(f64::NAN, |page| {
            let rect = in_panel(&page);
            if self.view.isFlipped() {
                rect.origin.y
            } else {
                rect.origin.y + rect.size.height
            }
        });
        let inner = buttons
            .firstObject()
            .and_then(|view| view.downcast::<NSStackView>().ok());
        TitleRow {
            button_symbols: inner
                .as_ref()
                .map(|stack| {
                    stack
                        .arrangedSubviews()
                        .iter()
                        .map(|view| {
                            view.downcast::<objc2_app_kit::NSButton>()
                                .ok()
                                .and_then(|button| button.symbolConfiguration())
                        })
                        .collect()
                })
                .unwrap_or_default(),
            button_spacing: inner.map_or(f64::NAN, |stack| stack.spacing()),
            title_start: title.origin.x,
            title_middle: middle(title),
            buttons_middle: first.map_or(f64::NAN, middle),
            page_top: page,
            buttons_start: first.map_or(f64::NAN, |rect| rect.origin.x),
            buttons_end: last.map_or(f64::NAN, |rect| rect.origin.x + rect.size.width),
            text_end: title.origin.x + self.header.intrinsicContentSize().width,
            width: self.view.bounds().size.width,
        }
    }
}

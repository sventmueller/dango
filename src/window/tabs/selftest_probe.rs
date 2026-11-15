//! What the self-test reads from and does with the parts of `tabs.rs`, kept out of its production code.

#[allow(
    unused_imports,
    reason = "the parts moved here use only some of their module's names"
)]
use super::*;

impl Document {
    pub fn is_dirty(&self) -> bool {
        self.dirty.get()
    }
}

impl TabWindow {
    /// The share of the first of two parts side by side.
    pub fn share(&self) -> Option<f64> {
        self.panes.first_share()
    }

    /// Gives the first of two parts side by side `share` of the width, which it keeps as the tab changes size.
    pub fn set_share(&self, share: f64) {
        self.panes.set_first_share(share);
    }

    /// What the documents' badges say, left to right: the name and the close button's label of those that show.
    pub fn badges(&self) -> Vec<Option<(String, String)>> {
        use objc2_app_kit::NSAccessibility;
        self.documents()
            .iter()
            .map(|doc| {
                (!doc.badge.isHidden()).then(|| {
                    let label = doc
                        .close
                        .accessibilityLabel()
                        .map(|l| l.to_string())
                        .unwrap_or_default();
                    (doc.badge_title.stringValue().to_string(), label)
                })
            })
            .collect()
    }

    /// The close button on the badge of the document `id`.
    pub fn badge_close(&self, id: super::TabId) -> Option<Retained<NSButton>> {
        self.document(id).map(|doc| doc.close.clone())
    }

    /// Drags the divider to `share` with the mouse: the drag and the release wait in the event queue, which the split
    /// view's tracking of the press reads.
    pub fn drag_share(&self, share: f64) {
        use objc2_app_kit::{NSApplication, NSEvent, NSEventModifierFlags, NSEventType};

        let Some(area) = self.panes.root_area() else { return };
        let Some(left) = area.subviews().iter().next().map(|first| first.frame().size.width) else {
            return;
        };
        let bounds = area.bounds();
        let target = bounds.size.width * share;
        let y = bounds.size.height / 2.0;
        let event = |kind: NSEventType, x: f64| {
            NSEvent::mouseEventWithType_location_modifierFlags_timestamp_windowNumber_context_eventNumber_clickCount_pressure(
                kind,
                area.convertPoint_toView(NSPoint::new(x, y), None),
                NSEventModifierFlags::empty(),
                0.0,
                self.window.windowNumber(),
                None,
                0,
                1,
                1.0,
            )
        };
        let app = NSApplication::sharedApplication(self.window.mtm());
        for (kind, x) in [
            (NSEventType::LeftMouseDragged, (left + target) / 2.0),
            (NSEventType::LeftMouseDragged, target),
            (NSEventType::LeftMouseUp, target),
        ] {
            if let Some(event) = event(kind, x) {
                app.postEvent_atStart(&event, false);
            }
        }
        if let Some(down) = event(NSEventType::LeftMouseDown, left + 0.5) {
            // SAFETY: The split view takes the press, which lives for the call.
            let () = unsafe { objc2::msg_send![&*area, mouseDown: &*down] };
        }
    }

    /// The narrowest and widest the user may drag the sidebar to now.
    pub fn sidebar_range(&self) -> (f64, f64) {
        (self.sidebar.minimumThickness(), self.sidebar.maximumThickness())
    }

    /// Folds the sidebar away or back, as the menu's command does.
    pub fn toggle_sidebar(&self) {
        // SAFETY: The action takes no sender.
        unsafe { self.split.toggleSidebar(None) };
    }

    /// Whether the tab shows the picture of the sidebar.
    pub fn shows_picture(&self) -> bool {
        !self.picture.isHidden()
    }
}

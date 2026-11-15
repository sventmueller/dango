//! What the self-test reads from and does with the parts of `sidebar.rs`, kept out of its production code.

#[allow(
    unused_imports,
    reason = "the parts moved here use only some of their module's names"
)]
use super::*;

impl WindowController {
    /// The context menu of `panel`'s symbol in the activity bar.
    pub(crate) fn activity_menu(&self, panel: Panel) -> Option<Retained<objc2_app_kit::NSMenu>> {
        self.ivars().sidebar.activity.borrow().as_ref()?.menu_of(panel)
    }

    /// The panels whose symbol the activity bar shows.
    pub(crate) fn shown_symbols(&self) -> Vec<Panel> {
        self.ivars()
            .sidebar
            .activity
            .borrow()
            .as_ref()
            .map(ActivityBar::shown)
            .unwrap_or_default()
    }

    /// How many symbols the activity bar has.
    pub(crate) fn activity_button_count(&self) -> usize {
        self.ivars()
            .sidebar
            .activity
            .borrow()
            .as_ref()
            .map_or(0, ActivityBar::button_count)
    }

    /// The title of the panel that shows.
    pub(crate) fn panel_title(&self) -> String {
        self.ivars()
            .sidebar
            .panels
            .borrow()
            .as_ref()
            .map(Panels::title)
            .unwrap_or_default()
    }

    /// The tooltip of `panel`'s symbol, with the marks that show on any symbol.
    pub(crate) fn symbol_tip(&self, panel: Panel) -> (Option<String>, usize) {
        let activity = self.ivars().sidebar.activity.borrow();
        activity.as_ref().map_or((None, 0), |a| (a.tip(panel), a.marks()))
    }

    /// The window the sidebar is in now.
    pub(crate) fn sidebar_window(&self) -> Option<Retained<objc2_app_kit::NSWindow>> {
        self.ivars().sidebar.view.borrow().as_ref()?.window()
    }

    /// Gives the sidebar `width`, as a drag of its edge does.
    pub(crate) fn set_sidebar_width(&self, width: f64) {
        self.ivars()
            .sidebar
            .panel_width
            .set((width - ACTIVITY_WIDTH).clamp(PANEL_MIN_WIDTH, PANEL_MAX_WIDTH));
        if let Some(tab) = self.current_tab() {
            self.fit_sidebar(&tab, tab.sidebar_collapsed());
        }
    }

    /// Whether the shown panel's title row shows, and the texts and buttons of its page.
    pub(crate) fn panel_seen(&self) -> Option<(bool, Vec<String>)> {
        self.ivars().sidebar.panels.borrow().as_ref().map(Panels::seen)
    }

    /// The shown panel's title and the geometry of its title row.
    pub(crate) fn panel_title_row(&self) -> Option<(String, crate::window::panels::TitleRow)> {
        let panels = self.ivars().sidebar.panels.borrow();
        let panels = panels.as_ref()?;
        Some((panels.title(), panels.title_row()))
    }

    /// In window coordinates, y up: the top of the tab bar, of the sidebar's safe area, and of the activity bar.
    pub(crate) fn sidebar_top(&self) -> (f64, f64, f64) {
        fn tab_bar(view: &NSView) -> Option<Retained<NSView>> {
            if view.class().name().to_bytes() == b"NSTabBar" {
                return Some(objc2::Message::retain(view));
            }
            view.subviews().iter().find_map(|child| tab_bar(&child))
        }
        let top = |view: &NSView, rect: objc2_foundation::NSRect| {
            let rect = view.convertRect_toView(rect, None);
            rect.origin.y + rect.size.height
        };
        let window = self.window();
        // SAFETY: Asking for the superview only reads; the frame view holds the content view while the window shows.
        let frame = window
            .as_ref()
            .and_then(|w| w.contentView())
            .and_then(|v| unsafe { v.superview() });
        let tabs = frame
            .and_then(|f| tab_bar(&f))
            .map_or(f64::NAN, |bar| top(&bar, bar.bounds()));
        let sidebar = self.ivars().sidebar.view.borrow();
        let (safe, activity) = sidebar.as_ref().map_or((f64::NAN, f64::NAN), |sidebar| {
            sidebar.layoutSubtreeIfNeeded();
            let safe = top(sidebar, sidebar.safeAreaLayoutGuide().frame());
            let activity = self.ivars().sidebar.activity.borrow();
            (
                safe,
                activity.as_ref().map_or(f64::NAN, |a| top(a.view(), a.view().bounds())),
            )
        });
        (tabs, safe, activity)
    }

    /// Lays the sidebar out below a toolbar that reaches `toolbar` down from its top, as one in full screen does.
    pub(crate) fn push_sidebar_below_toolbar(&self, toolbar: f64) {
        self.keep_sidebar_below_toolbar(toolbar);
        if let Some(sidebar) = self.ivars().sidebar.view.borrow().as_ref() {
            sidebar.layoutSubtreeIfNeeded();
        }
    }

    /// The button of the files panel's header that sends `action`.
    pub(crate) fn files_header_button(&self, action: objc2::runtime::Sel) -> Option<Retained<objc2_app_kit::NSButton>> {
        fn find(view: &NSView, action: objc2::runtime::Sel) -> Option<Retained<objc2_app_kit::NSButton>> {
            if let Some(button) = view.downcast_ref::<objc2_app_kit::NSButton>()
                && button.action() == Some(action)
            {
                return Some(objc2::Message::retain(button));
            }
            view.subviews().iter().find_map(|child| find(&child, action))
        }
        let panels = self.ivars().sidebar.panels.borrow();
        find(panels.as_ref()?.view(), action)
    }
}

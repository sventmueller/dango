//! The sidebar of a window, one per folder (E60): the activity bar and the panel beside it, which move into the window
//! tab in front at the width they had; which panel shows, the symbols of the activity bar and their marks, keeping it
//! below the toolbar, and moving the focus into a panel.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use objc2::rc::Retained;
use objc2::{DefinedClass, MainThreadOnly};
use objc2_app_kit::{NSLayoutConstraint, NSView};

use super::activity_bar::ActivityBar;
use super::panels::Panels;
use super::tabs::{self, TabWindow};
use super::{Panel, WindowController};
use crate::util::{activate, auto_layout};
#[cfg(feature = "selftest")]
mod selftest_probe;

/// How far the activity bar and the panel reach above the sidebar's safe area, which puts their first symbol and title
/// level with the top edge of the tabs, which start where the safe area does.
pub(crate) const SIDEBAR_RAISE: f64 = 11.0;
/// How far the panel reaches under the activity bar, so that its content keeps the same distance from the symbols as
/// the symbols keep from the window's edge.
const PANEL_UNDER_ACTIVITY: f64 = 8.5;
/// The width of the activity bar at the default interface size, which stays when the panel beside it is closed.
pub(crate) const ACTIVITY_WIDTH: f64 = 44.0;
pub(super) const PANEL_MIN_WIDTH: f64 = 200.0;
pub(super) const PANEL_MAX_WIDTH: f64 = 560.0;
pub(crate) const PANEL_DEFAULT_WIDTH: f64 = 260.0;

/// How long a panel's page stays in the sidebar after another panel showed, so switching back and forth is cheap and
/// a move of the sidebar into another window tab later moves only the page that shows.
const LEAVE_OUT: std::time::Duration = std::time::Duration::from_secs(2);

/// The width of the activity bar at the interface size the user chose.
pub(crate) fn activity_width() -> f64 {
    ide_ui_common::ui_size::scaled(ACTIVITY_WIDTH)
}

/// The folder's sidebar: its views, which panel shows, whether the panel beside the activity bar is open and how wide.
pub(super) struct State {
    panel: Cell<Option<Panel>>,
    panel_open: Cell<bool>,
    panel_width: Cell<f64>,
    activity: RefCell<Option<ActivityBar>>,
    panels: RefCell<Option<Panels>>,
    /// The activity bar and the panel beside it, which the window tab in front shows.
    view: RefCell<Option<Retained<NSView>>>,
    /// Keeps the sidebar's top below the toolbar, which in full screen reaches further down than its safe area.
    below_toolbar: RefCell<Option<Retained<NSLayoutConstraint>>>,
}

impl Default for State {
    fn default() -> State {
        State {
            panel: Cell::default(),
            panel_open: Cell::default(),
            panel_width: Cell::new(PANEL_DEFAULT_WIDTH),
            activity: RefCell::default(),
            panels: RefCell::default(),
            view: RefCell::default(),
            below_toolbar: RefCell::default(),
        }
    }
}

/// `panel` unless its symbol is among `hidden`, else the first panel whose symbol shows.
pub(super) fn first_shown(panel: Panel, hidden: &[Panel]) -> Panel {
    if !hidden.contains(&panel) {
        return panel;
    }
    Panel::ALL.into_iter().find(|p| !hidden.contains(p)).unwrap_or(panel)
}

impl WindowController {
    /// Builds the sidebar, the activity bar and the panel beside it, which moves into the window tab in front.
    pub(super) fn build_sidebar(&self) {
        let mtm = self.mtm();
        let sidebar_view = NSView::new(mtm);
        let activity = ActivityBar::new(mtm, self);
        let panels = Panels::new(mtm, self, self.project().as_deref());
        sidebar_view.addSubview(panels.view());
        sidebar_view.addSubview(activity.view());
        let guide = sidebar_view.safeAreaLayoutGuide();
        let activity_width = activity.view().widthAnchor().constraintEqualToConstant(ACTIVITY_WIDTH);
        auto_layout(&[activity.view(), panels.view()]);
        let under_title_bar = activity
            .view()
            .topAnchor()
            .constraintEqualToAnchor_constant(&guide.topAnchor(), -SIDEBAR_RAISE);
        // Below required, so that the toolbar of full screen, which the safe area leaves out, pushes the sidebar down.
        under_title_bar.setPriority(999.0);
        activate([
            activity
                .view()
                .leadingAnchor()
                .constraintEqualToAnchor(&sidebar_view.leadingAnchor()),
            under_title_bar,
            activity
                .view()
                .bottomAnchor()
                .constraintEqualToAnchor(&sidebar_view.bottomAnchor()),
            activity_width.clone(),
            panels
                .view()
                .leadingAnchor()
                .constraintEqualToAnchor_constant(&activity.view().trailingAnchor(), -PANEL_UNDER_ACTIVITY),
            panels
                .view()
                .trailingAnchor()
                .constraintEqualToAnchor(&sidebar_view.trailingAnchor()),
            panels
                .view()
                .topAnchor()
                .constraintEqualToAnchor(&activity.view().topAnchor()),
            panels
                .view()
                .bottomAnchor()
                .constraintEqualToAnchor(&sidebar_view.bottomAnchor()),
        ]);
        let below_toolbar = activity
            .view()
            .topAnchor()
            .constraintGreaterThanOrEqualToAnchor_constant(&sidebar_view.topAnchor(), -SIDEBAR_RAISE);
        below_toolbar.setActive(true);
        self.follow_interface_size(&activity_width);
        let state = &self.ivars().sidebar;
        *state.below_toolbar.borrow_mut() = Some(below_toolbar);
        *state.activity.borrow_mut() = Some(activity);
        *state.panels.borrow_mut() = Some(panels);
        *state.view.borrow_mut() = Some(sidebar_view);
    }

    /// Keeps the activity bar, held at its width by `width`, at the interface size, and the sidebar in front as wide as
    /// the activity bar and the panel beside it then are.
    fn follow_interface_size(&self, width: &NSLayoutConstraint) {
        let controller = objc2::rc::Weak::from(self);
        let first = Cell::new(true);
        ide_ui_common::ui_size::keep(width, ide_ui_common::ui_size::Aspect::Length, move |width| {
            width.setConstant(activity_width());
            if !first.replace(false)
                && let Some(controller) = controller.load()
                && let Some(tab) = controller.current_tab()
            {
                controller.fit_sidebar(&tab, tab.sidebar_collapsed());
            }
        });
    }

    /// Moves the sidebar's first symbol and title below the toolbar of the window tab in front, as far as it reaches now.
    pub(super) fn follow_toolbar(&self) {
        let reach = self.current_tab().map_or(0.0, |tab| tab.toolbar_reach());
        self.keep_sidebar_below_toolbar(reach);
    }

    /// Keeps the sidebar's first symbol and title clear of a toolbar that reaches `toolbar` down from the sidebar's top.
    fn keep_sidebar_below_toolbar(&self, toolbar: f64) {
        if let Some(constraint) = self.ivars().sidebar.below_toolbar.borrow().as_ref() {
            constraint.setConstant(toolbar - SIDEBAR_RAISE);
        }
    }

    /// The sidebar leaves `previous` for `next`: its width and the inspector's are kept, `previous` keeps a picture of
    /// it while its window stays on screen beside, and the panels let go of the pages that do not show. Whether the
    /// sidebar of `previous` was folded away.
    pub(super) fn sidebar_leaves(&self, previous: &TabWindow, next: &TabWindow) -> bool {
        let collapsed = previous.sidebar_collapsed();
        self.remember_sidebar_width(previous);
        self.remember_inspector(previous);
        match self.ivars().sidebar.view.borrow().as_ref() {
            Some(view) if stays_on_screen(previous, next) => previous.keep_picture_of(view),
            _ => previous.forget_picture(),
        }
        if let Some(panels) = self.ivars().sidebar.panels.borrow().as_ref() {
            panels.leave_out_hidden();
        }
        collapsed
    }

    /// The sidebar moves into `tab`, which came to the front, at the width it had, folded away when `collapsed`; the
    /// inspector follows it.
    pub(super) fn sidebar_joins(&self, tab: &Rc<TabWindow>, collapsed: bool) {
        if let Some(view) = self.ivars().sidebar.view.borrow().as_ref() {
            tab.adopt_sidebar(view);
        }
        self.fit_sidebar(tab, collapsed);
        self.inspector_follows(tab);
    }

    /// Gives `tab`, on screen beside the tab in front, a picture of the sidebar, unless it has one.
    pub(super) fn picture_sidebar_in(&self, tab: &TabWindow) {
        if tab.has_picture() {
            return;
        }
        if let Some(view) = self.ivars().sidebar.view.borrow().as_ref() {
            tab.keep_picture_of(view);
        }
    }

    /// Shows the folder's tree with `buttons` in the title row of the files panel, as a folder opens.
    pub(super) fn set_files_page(&self, page: &NSView, buttons: &NSView) {
        if let Some(panels) = self.ivars().sidebar.panels.borrow().as_ref() {
            panels.set_files(page, buttons);
        }
    }

    /// Tells the panels the window shows another folder now.
    pub(super) fn panels_follow_project(&self) {
        if let Some(panels) = self.ivars().sidebar.panels.borrow().as_ref() {
            panels.set_project(self.project().as_deref());
        }
    }

    /// The width of the panel beside the activity bar, as the user left it.
    pub(super) fn panel_width(&self) -> f64 {
        self.ivars().sidebar.panel_width.get()
    }

    /// Takes the panel width of the last save, `width` of the whole sidebar.
    pub(super) fn set_saved_panel_width(&self, width: f64) {
        self.ivars()
            .sidebar
            .panel_width
            .set((width - activity_width()).clamp(PANEL_MIN_WIDTH, PANEL_MAX_WIDTH));
    }

    /// The panel picked last, whether or not the sidebar shows it.
    pub(super) fn picked_panel(&self) -> Option<Panel> {
        self.ivars().sidebar.panel.get()
    }

    /// Whether the panel beside the activity bar is open.
    pub(super) fn panel_open(&self) -> bool {
        self.ivars().sidebar.panel_open.get()
    }

    /// Counts the panel beside the activity bar as open, which a new window starts with before it shows.
    pub(super) fn start_with_panel_open(&self) {
        self.ivars().sidebar.panel_open.set(true);
    }

    /// Keeps the width the user gave the sidebar in `tab`.
    pub(super) fn remember_sidebar_width(&self, tab: &TabWindow) {
        let ivars = &self.ivars().sidebar;
        let width = tab.sidebar_width();
        if ivars.panel_open.get() && tab.window.isVisible() && width > activity_width() + PANEL_MIN_WIDTH - 1.0 {
            ivars
                .panel_width
                .set((width - activity_width()).clamp(PANEL_MIN_WIDTH, PANEL_MAX_WIDTH));
        }
    }

    /// Gives the sidebar of `tab` the width it has in the other tabs, which a tab that has not shown yet takes only
    /// held there; the user may drag it again once the tab is laid out.
    pub(super) fn fit_sidebar(&self, tab: &TabWindow, collapsed: bool) {
        let ivars = &self.ivars().sidebar;
        let open = ivars.panel_open.get();
        let width = if open {
            activity_width() + ivars.panel_width.get()
        } else {
            activity_width()
        };
        // A tab laid out at that width already keeps it, and pinning it again would lay out its window anew; one that
        // came laid out so, as a new window tab does, still needs the range the user may drag it in.
        if tab.sidebar_collapsed() == collapsed && (tab.sidebar_width() - width).abs() < 0.5 {
            if open {
                tab.release_sidebar(activity_width() + PANEL_MIN_WIDTH, activity_width() + PANEL_MAX_WIDTH);
            } else {
                tab.release_sidebar(width, width);
            }
            return;
        }
        tab.pin_sidebar(width, collapsed);
        if open {
            // The tab itself, not the one in front then: another may come to the front meanwhile, and the held one
            // would stay held.
            let window = Retained::as_ptr(&tab.window) as usize;
            self.later(move |controller| {
                if controller.panel_open()
                    && let Some(tab) = controller.tab_at(window)
                {
                    tab.release_sidebar(activity_width() + PANEL_MIN_WIDTH, activity_width() + PANEL_MAX_WIDTH);
                }
            });
        }
    }

    /// Shows `panel`, opening the sidebar if it is closed; the panel that already shows stays open (E61).
    pub(crate) fn activate_panel(&self, panel: Panel) {
        if !self.panel_open() {
            self.set_panel_open(true);
        }
        self.show_panel(panel);
    }

    pub(super) fn show_panel(&self, panel: Panel) {
        let state = &self.ivars().sidebar;
        state.panel.set(Some(panel));
        if let Some(panels) = state.panels.borrow().as_ref() {
            panels.show(panel);
        }
        let id = self.id();
        crate::util::main_after(LEAVE_OUT, move || {
            if let Some(controller) = crate::host::window(id)
                && let Some(panels) = controller.ivars().sidebar.panels.borrow().as_ref()
            {
                panels.leave_out_hidden();
            }
        });
        if let Some(activity) = state.activity.borrow().as_ref() {
            activity.select(Some(panel));
        }
        self.panel_came_into_view();
    }

    /// The panel the sidebar shows came into view: the panels that read their data only once shown read it now.
    fn panel_came_into_view(&self) {
        match self.shown_panel() {
            Some(Panel::Containers) => crate::containers_panel::shown(self),
            Some(Panel::Debug) => crate::debug::shown(self),
            Some(Panel::Ci) => crate::ci_panel::shown(self),
            _ => {}
        }
    }

    /// Whether the user hid the activity bar's symbol of `panel` (E80).
    pub(crate) fn symbol_hidden(panel: Panel) -> bool {
        super::activity_bar::hidden().contains(&panel)
    }

    /// The panel the sidebar shows, if it is open.
    pub(crate) fn shown_panel(&self) -> Option<Panel> {
        let ivars = &self.ivars().sidebar;
        ivars.panel_open.get().then(|| ivars.panel.get()).flatten()
    }

    /// Hides the activity bar's symbols of `hidden`; a hidden panel that showed gives way to the first that still has
    /// its symbol.
    pub(super) fn leave_out_symbols(&self, hidden: &[Panel]) {
        let ivars = &self.ivars().sidebar;
        if let Some(activity) = ivars.activity.borrow().as_ref() {
            activity.leave_out(hidden);
        }
        if let Some(shown) = ivars.panel.get().filter(|panel| hidden.contains(panel)) {
            let open = ivars.panel_open.get();
            self.show_panel(first_shown(shown, hidden));
            if !open && let Some(activity) = ivars.activity.borrow().as_ref() {
                activity.select(None);
            }
        }
    }

    /// Puts the keyboard focus into the panel beside the activity bar while it shows, else back into the active tab.
    pub(super) fn focus_panel(&self) {
        let Some(window) = self.window() else { return };
        let ivars = &self.ivars().sidebar;
        let page = ivars.panels.borrow().as_ref().and_then(Panels::shown_page);
        let target = page
            .as_deref()
            .filter(|_| ivars.panel_open.get())
            .and_then(tabs::first_responder_in);
        if let Some(view) = target {
            window.makeFirstResponder(Some(&view));
            return;
        }
        let focus = window
            .firstResponder()
            .and_then(|responder| Retained::downcast::<NSView>(responder).ok());
        let in_panel = ivars
            .panels
            .borrow()
            .as_ref()
            .is_some_and(|panels| focus.is_some_and(|focus| focus.isDescendantOf(panels.view())));
        if in_panel
            && let Some(tab) = self.current_tab()
            && let Some(document) = tab.focused()
        {
            tab.activate(document.id);
        }
    }

    /// Opens or closes the panel beside the activity bar, which stays.
    pub(super) fn set_panel_open(&self, open: bool) {
        let ivars = &self.ivars().sidebar;
        let Some(tab) = self.current_tab() else { return };
        if let Some(panels) = ivars.panels.borrow().as_ref() {
            panels.view().setHidden(!open);
        }
        if !open {
            self.remember_sidebar_width(&tab);
        }
        ivars.panel_open.set(open);
        if open {
            self.panel_came_into_view();
        }
        self.fit_sidebar(&tab, tab.sidebar_collapsed());
        if let Some(activity) = ivars.activity.borrow().as_ref() {
            activity.select(open.then(|| ivars.panel.get()).flatten());
        }
    }

    /// Names `words`, such as a failed CI run, in the tooltip of `panel`'s symbol; `None` names only the panel.
    pub(crate) fn set_symbol_status(&self, panel: Panel, words: Option<&str>) {
        if let Some(activity) = self.ivars().sidebar.activity.borrow().as_ref() {
            activity.set_status(panel, words);
        }
    }

    /// Names `count` changes in the tooltip of `panel`'s symbol.
    pub(crate) fn set_symbol_count(&self, panel: Panel, count: usize) {
        if let Some(activity) = self.ivars().sidebar.activity.borrow().as_ref() {
            activity.set_count(panel, count);
        }
    }

    /// Shows `view` as the page of `panel`.
    pub(crate) fn set_panel_page(&self, panel: Panel, view: &NSView) {
        if let Some(panels) = self.ivars().sidebar.panels.borrow().as_ref() {
            panels.set_page(panel, view);
        }
    }

    /// Shows `view` at the end of the title row of `panel`.
    pub(crate) fn set_panel_accessory(&self, panel: Panel, view: &NSView) {
        if let Some(panels) = self.ivars().sidebar.panels.borrow().as_ref() {
            panels.set_accessory(panel, view);
        }
    }
}

/// Whether the window of `previous` stays on screen once `next` comes to the front: it is visible and not in the tab
/// group of `next`, whose selected window is the only one of the group on screen.
fn stays_on_screen(previous: &TabWindow, next: &TabWindow) -> bool {
    let group = |tab: &TabWindow| tab.window.tabGroup().map(|group| Retained::as_ptr(&group));
    previous.window.isVisible() && (group(previous).is_none() || group(previous) != group(next))
}

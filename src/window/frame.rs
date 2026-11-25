//! Where a folder's window is and how its sidebar looks, kept in the defaults for the next time the folder opens; a
//! folder that opens for the first time takes what the window saved last had.

use objc2::MainThreadOnly;
use objc2::rc::Retained;
use objc2_app_kit::{NSScreen, NSWindow};
use objc2_foundation::{NSPoint, NSRect, NSString, NSUserDefaults};

use super::{Panel, WindowController, tabs};
use crate::util::ns;

/// The name the window saved last is also kept under, for a folder without a name of its own.
const LAST: &str = "ProjectWindow Last";
/// How far a window that takes the last frame moves off a window already there.
const CASCADE: f64 = 22.0;

/// How the sidebar of a window looked when it was saved.
pub(crate) struct Sidebar {
    pub(crate) panel: Panel,
    /// Whether the panel beside the activity bar showed.
    pub(crate) open: bool,
    /// Whether the whole sidebar was folded away.
    pub(crate) collapsed: bool,
}

fn key(kind: &str, name: &str) -> Retained<NSString> {
    ns(&format!("{kind} {name}"))
}

/// The name of the folder's own values if it has them, else of the window saved last, else none.
fn saved_name(name: &str) -> Option<String> {
    let defaults = NSUserDefaults::standardUserDefaults();
    [name, LAST]
        .into_iter()
        .find(|name| defaults.objectForKey(&key("Sidebar", name)).is_some())
        .map(str::to_string)
}

impl WindowController {
    /// The name the window's frame and sidebar are kept under, the folder's.
    fn frame_name(&self) -> String {
        match self.project() {
            Some(path) => format!("ProjectWindow {}", path.display()),
            None => "ProjectWindow".into(),
        }
    }

    /// Puts the window where it was for this folder, else where the window saved last was, a step aside if a window
    /// is there, else at its default size in the middle; a frame no screen shows any more moves onto the main one.
    /// The sidebar gets the width it had.
    /// Puts the window back to the frame saved for its folder, after tabs that joined later resized the tab group.
    pub(in crate::window) fn reapply_saved_frame(&self) {
        if let Some(window) = self.window()
            && window.setFrameUsingName(&ns(&self.frame_name()))
        {
            keep_on_screen(&window);
        }
    }

    pub(super) fn restore_frame(&self, window: &NSWindow) {
        let name = self.frame_name();
        if !window.setFrameUsingName(&ns(&name)) {
            if window.setFrameUsingName(&ns(LAST)) {
                step_aside(self.context(), window);
            } else {
                window.setContentSize(tabs::default_size());
                window.center();
            }
        }
        keep_on_screen(window);
        if let Some(saved) = saved_name(&name) {
            self.set_saved_panel_width(NSUserDefaults::standardUserDefaults().doubleForKey(&key("Sidebar", &saved)));
            let defaults = NSUserDefaults::standardUserDefaults();
            self.set_saved_inspector(
                defaults.boolForKey(&key("Inspector open", &saved)),
                defaults.doubleForKey(&key("Inspector", &saved)),
            );
        }
    }

    /// How the sidebar looked when the window of this folder, or else the window saved last, was saved.
    pub(super) fn saved_sidebar(&self) -> Sidebar {
        let defaults = NSUserDefaults::standardUserDefaults();
        let Some(name) = saved_name(&self.frame_name()) else {
            return Sidebar {
                panel: Panel::Files,
                open: true,
                collapsed: false,
            };
        };
        let panel = defaults.integerForKey(&key("Panel", &name));
        let open = key("Panel open", &name);
        Sidebar {
            panel: Panel::from_tag(panel).unwrap_or(Panel::Files),
            open: defaults.objectForKey(&open).is_none() || defaults.boolForKey(&open),
            collapsed: defaults.boolForKey(&key("Sidebar collapsed", &name)),
        }
    }

    /// Keeps where the window is and how its sidebar looks, for the next time the folder opens, and as the last
    /// window's for a folder that opens for the first time.
    pub(super) fn save_frame(&self) {
        let Some(tab) = self.current_tab() else { return };
        self.remember_sidebar_width(&tab);
        let defaults = NSUserDefaults::standardUserDefaults();
        let (inspector_shown, inspector_width) = self.saved_inspector();
        for name in [self.frame_name().as_str(), LAST] {
            defaults.setBool_forKey(inspector_shown, &key("Inspector open", name));
            defaults.setDouble_forKey(inspector_width, &key("Inspector", name));
            tab.window.saveFrameUsingName(&ns(name));
            defaults.setDouble_forKey(
                super::sidebar::activity_width() + self.panel_width(),
                &key("Sidebar", name),
            );
            if let Some(panel) = self.picked_panel() {
                defaults.setInteger_forKey(panel.tag(), &key("Panel", name));
            }
            defaults.setBool_forKey(self.panel_open(), &key("Panel open", name));
            defaults.setBool_forKey(tab.sidebar_collapsed(), &key("Sidebar collapsed", name));
        }
    }
}

/// Moves `window` down and right while another window of the app has its top left corner.
fn step_aside(cx: &crate::context::AppContext, window: &NSWindow) {
    let others: Vec<NSPoint> = cx
        .windows()
        .all()
        .iter()
        .filter_map(|controller| controller.window())
        .filter(|other| !std::ptr::eq(&**other, window))
        .map(|other| top_left(other.frame()))
        .collect();
    let mut frame = window.frame();
    while others.iter().any(|other| near(*other, top_left(frame))) {
        frame.origin.x += CASCADE;
        frame.origin.y -= CASCADE;
    }
    window.setFrame_display(frame, false);
}

fn top_left(frame: NSRect) -> NSPoint {
    NSPoint::new(frame.origin.x, frame.origin.y + frame.size.height)
}

fn near(a: NSPoint, b: NSPoint) -> bool {
    (a.x - b.x).abs() < 1.0 && (a.y - b.y).abs() < 1.0
}

/// Moves `window` onto the main screen, at most as large as it, when no screen shows its title bar.
fn keep_on_screen(window: &NSWindow) {
    let frame = window.frame();
    let title_bar = NSRect::new(
        NSPoint::new(frame.origin.x, frame.origin.y + frame.size.height - 30.0),
        objc2_foundation::NSSize::new(frame.size.width, 30.0),
    );
    let screens = NSScreen::screens(window.mtm());
    let shown = screens
        .iter()
        .any(|screen| objc2_foundation::NSIntersectsRect(screen.visibleFrame(), title_bar));
    if shown {
        return;
    }
    let Some(main) = NSScreen::mainScreen(window.mtm()) else {
        return;
    };
    let visible = main.visibleFrame();
    let size = objc2_foundation::NSSize::new(
        frame.size.width.min(visible.size.width),
        frame.size.height.min(visible.size.height),
    );
    window.setFrame_display(NSRect::new(visible.origin, size), false);
    window.center();
}

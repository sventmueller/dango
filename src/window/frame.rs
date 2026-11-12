//! Where a folder's window is and how its sidebar looks, kept in the defaults for the next time the folder opens; a
//! folder that opens for the first time takes what the window saved last had.

use objc2::rc::Retained;
use objc2::{DefinedClass, MainThreadOnly};
use objc2_app_kit::{NSScreen, NSWindow};
use objc2_foundation::{NSPoint, NSRect, NSString, NSUserDefaults};

use super::{ACTIVITY_WIDTH, PANEL_MAX_WIDTH, PANEL_MIN_WIDTH, Panel, WindowController, tabs};
use crate::util::ns;

/// The name the window saved last is also kept under, for a folder without a name of its own.
const LAST: &str = "ProjectWindow Last";
/// How far a window that takes the last frame moves off a window already there.
const CASCADE: f64 = 22.0;

/// How the sidebar of a window looked when it was saved.
pub struct Sidebar {
    pub panel: Panel,
    /// Whether the panel beside the activity bar showed.
    pub open: bool,
    /// Whether the whole sidebar was folded away.
    pub collapsed: bool,
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
    /// Puts the window where it was for this folder, else where the window saved last was, a step aside if a window
    /// is there, else at its default size in the middle; a frame no screen shows any more moves onto the main one.
    /// The sidebar gets the width it had.
    pub(super) fn restore_frame(&self, window: &NSWindow) {
        let name = self.frame_name();
        if !window.setFrameUsingName(&ns(&name)) {
            if window.setFrameUsingName(&ns(LAST)) {
                step_aside(window);
            } else {
                window.setContentSize(tabs::default_size());
                window.center();
            }
        }
        keep_on_screen(window);
        if let Some(saved) = saved_name(&name) {
            let width = NSUserDefaults::standardUserDefaults().doubleForKey(&key("Sidebar", &saved)) - ACTIVITY_WIDTH;
            self.ivars()
                .panel_width
                .set(width.clamp(PANEL_MIN_WIDTH, PANEL_MAX_WIDTH));
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
        let ivars = self.ivars();
        let defaults = NSUserDefaults::standardUserDefaults();
        for name in [self.frame_name().as_str(), LAST] {
            tab.window.saveFrameUsingName(&ns(name));
            defaults.setDouble_forKey(ACTIVITY_WIDTH + ivars.panel_width.get(), &key("Sidebar", name));
            if let Some(panel) = ivars.panel.get() {
                defaults.setInteger_forKey(panel.tag(), &key("Panel", name));
            }
            defaults.setBool_forKey(ivars.panel_open.get(), &key("Panel open", name));
            defaults.setBool_forKey(tab.sidebar_collapsed(), &key("Sidebar collapsed", name));
        }
    }
}

/// Moves `window` down and right while another window of the app has its top left corner.
fn step_aside(window: &NSWindow) {
    let others: Vec<NSPoint> = super::windows()
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

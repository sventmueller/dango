//! The page of a new tab, which ⌘T and the tab bar's "+" open (E17, E60): what a tab can show, as buttons. The
//! document chosen takes the page's place in its tab.

use objc2::rc::{Retained, Weak};
use objc2::runtime::AnyObject;
use objc2::{DefinedClass, MainThreadMarker, MainThreadOnly, define_class, msg_send, sel};
use objc2_app_kit::{
    NSAccessibility, NSApplication, NSBezelStyle, NSButton, NSCellImagePosition, NSControlSize, NSFont,
    NSFontWeightSemibold, NSLayoutAttribute, NSMenu, NSStackView, NSUserInterfaceLayoutOrientation, NSView,
};
use objc2_foundation::{NSObjectProtocol, NSPoint};
use splat_i18n::t;

use super::WindowController;
use super::tabs::{fill_new_tab_menu, new_tab_menu};
use crate::util::{guarded, label, ns, set_action};

pub struct PageIvars {
    controller: Weak<WindowController>,
    menu: Retained<NSMenu>,
}

define_class!(
    // SAFETY: NSView has no subclassing requirements and `NewTabPage` does not implement Drop.
    #[unsafe(super(NSView))]
    #[thread_kind = MainThreadOnly]
    #[name = "IDENewTabPage"]
    #[ivars = PageIvars]
    pub struct NewTabPage;

    unsafe impl NSObjectProtocol for NewTabPage {}

    impl NewTabPage {
        #[unsafe(method(ideChooseNewTab:))]
        fn choose_action(&self, sender: Option<&AnyObject>) {
            guarded((), || {
                let button = sender.and_then(|s| s.downcast_ref::<NSButton>());
                if let Some(button) = button {
                    self.choose(button);
                }
            });
        }
    }
);

impl NewTabPage {
    /// Opens what the entry of `button` stands for, or its menu below it.
    fn choose(&self, button: &NSButton) {
        let Some(controller) = self.ivars().controller.load() else {
            return;
        };
        let menu = &self.ivars().menu;
        let Some(entry) = menu.itemWithTag(button.tag()) else {
            return;
        };
        if let Some(submenu) = entry.submenu() {
            fill_new_tab_menu(&controller, menu);
            let below = NSPoint::new(0.0, button.frame().size.height + 4.0);
            submenu.popUpMenuPositioningItem_atLocation_inView(None, below, Some(button));
            return;
        }
        if let Some(action) = entry.action() {
            let app = NSApplication::sharedApplication(self.mtm());
            // SAFETY: The window controller answers each action of the menu with a method that takes the sender.
            unsafe { app.sendAction_to_from(action, Some(&controller), Some(&entry)) };
        }
    }

    /// The titles of the page's entries, for the self-test.
    #[cfg(feature = "selftest")]
    pub fn titles(&self) -> Vec<String> {
        self.ivars()
            .menu
            .itemArray()
            .iter()
            .filter(|item| !item.isSeparatorItem())
            .map(|item| item.title().to_string())
            .collect()
    }
}

/// The page of a new tab of `controller`'s folder.
pub fn view(mtm: MainThreadMarker, controller: &WindowController) -> Retained<NewTabPage> {
    let menu = new_tab_menu(mtm);
    let ivars = PageIvars {
        controller: Weak::from(controller),
        menu: menu.clone(),
    };
    // SAFETY: NSView's initializer, called once on the newly allocated object.
    let page: Retained<NewTabPage> = unsafe { msg_send![super(NewTabPage::alloc(mtm).set_ivars(ivars)), init] };
    let stack = NSStackView::new(mtm);
    stack.setOrientation(NSUserInterfaceLayoutOrientation::Vertical);
    stack.setAlignment(NSLayoutAttribute::Leading);
    stack.setSpacing(8.0);
    let title = label(mtm, t("Neuer Tab"));
    // SAFETY: AppKit sets its font weight constants when it loads.
    title.setFont(Some(&NSFont::systemFontOfSize_weight(22.0, unsafe {
        NSFontWeightSemibold
    })));
    stack.addArrangedSubview(&title);
    stack.setCustomSpacing_afterView(16.0, &title);
    for (index, entry) in menu.itemArray().iter().enumerate() {
        if entry.isSeparatorItem() {
            continue;
        }
        entry.setTag(isize::try_from(index).unwrap_or(-1) + TAG_BASE);
        let name = if entry.submenu().is_some() {
            format!("{} ▸", entry.title())
        } else {
            entry.title().to_string()
        };
        // SAFETY: The page answers the action with a method that takes the sender, and is set as the target below.
        let button = unsafe { NSButton::buttonWithTitle_target_action(&ns(&name), None, None, mtm) };
        set_action(&button, &page, sel!(ideChooseNewTab:));
        button.setTag(entry.tag());
        button.setBezelStyle(NSBezelStyle::Push);
        button.setControlSize(NSControlSize::Large);
        button.setImage(entry.image().as_deref());
        button.setImagePosition(NSCellImagePosition::ImageLeading);
        button.setEnabled(entry.isEnabled());
        stack.addArrangedSubview(&button);
        button
            .widthAnchor()
            .constraintGreaterThanOrEqualToConstant(220.0)
            .setActive(true);
    }
    page.addSubview(&stack);
    stack.setTranslatesAutoresizingMaskIntoConstraints(false);
    for constraint in [
        stack.centerXAnchor().constraintEqualToAnchor(&page.centerXAnchor()),
        stack
            .centerYAnchor()
            .constraintEqualToAnchor_constant(&page.centerYAnchor(), -30.0),
    ] {
        constraint.setActive(true);
    }
    page.setAccessibilityElement(false);
    page
}

/// Where the tags of the page's entries start, clear of the tags some entries' actions read.
const TAG_BASE: isize = 1000;

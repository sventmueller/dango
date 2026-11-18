//! The Leitstand's menu items (E175, F66; no menu of its own): "Ablage ▸ Neuer Auftrag …" and the submenu "Ablage ▸
//! Auftrag" with the commands of the order in front, "Software-Factory-Einstellungen …" among them, greyed out where they
//! cannot run (E101). The order in front is the order tab's, else the one selected in the Leitstand tab or the panel.

use std::rc::{Rc, Weak};

use ide_factory::OrderId;
use ide_ui_common::util::{guarded, item, separator, set_target, submenu, tagged};
use ide_ui_common::window::{Tabs, Window};
use objc2::rc::Retained;
use objc2::runtime::{AnyObject, NSObject};
use objc2::{DefinedClass, MainThreadMarker, MainThreadOnly, define_class, msg_send, sel};
use objc2_app_kit::NSMenuItem;
use objc2_foundation::NSObjectProtocol;
use splat_i18n::t;

use crate::actions::Target;
use crate::host::{State, host};
use crate::model::detail::Command;

define_class!(
    // SAFETY: NSObject has no subclassing requirements and `OrderCommands` does not implement Drop.
    #[unsafe(super(NSObject))]
    #[thread_kind = MainThreadOnly]
    #[name = "IDEFactoryOrderCommands"]
    #[ivars = Weak<State>]
    pub(crate) struct OrderCommands;

    unsafe impl NSObjectProtocol for OrderCommands {}

    impl OrderCommands {
        #[unsafe(method(ideFactoryMenuNewOrder:))]
        fn new_order(&self, _sender: Option<&AnyObject>) {
            guarded((), || {
                if let Some(target) = self.front() {
                    crate::sheet::show(&target);
                }
            });
        }

        #[unsafe(method(ideFactoryMenuOpen:))]
        fn open(&self, _sender: Option<&AnyObject>) {
            guarded((), || {
                if let Some((target, id)) = self.front_order() {
                    crate::order::open(&target, &id);
                }
            });
        }

        #[unsafe(method(ideFactoryMenuCommand:))]
        fn command(&self, sender: Option<&AnyObject>) {
            guarded((), || {
                let command = sender
                    .and_then(|s| s.downcast_ref::<NSMenuItem>())
                    .and_then(|item| Command::of_tag(item.tag()));
                if let (Some(command), Some((target, id))) = (command, self.front_order()) {
                    crate::actions::run_on(&target, &id, command);
                }
            });
        }

        #[unsafe(method(ideFactoryMenuBoard:))]
        fn board(&self, _sender: Option<&AnyObject>) {
            guarded((), || {
                if let Some(target) = self.front() {
                    crate::board::show(&target);
                }
            });
        }

        #[unsafe(method(ideFactoryMenuSettings:))]
        fn settings(&self, _sender: Option<&AnyObject>) {
            guarded((), || {
                if let Some(target) = self.front() {
                    crate::project::show(&target);
                }
            });
        }

        #[unsafe(method(validateMenuItem:))]
        fn validate_menu_item(&self, item: &NSMenuItem) -> bool {
            guarded(false, || self.validate(item))
        }
    }
);

impl OrderCommands {
    pub(crate) fn new(mtm: MainThreadMarker, state: &Rc<State>) -> Retained<OrderCommands> {
        let this = OrderCommands::alloc(mtm).set_ivars(Rc::downgrade(state));
        // SAFETY: NSObject's initializer, called once on the newly allocated object.
        unsafe { msg_send![super(this), init] }
    }

    /// The window in front with a folder, where the commands act.
    fn front(&self) -> Option<Target> {
        let state = self.ivars().upgrade()?;
        let window = Window::frontmost()?;
        let panel = state.panels.of(window)?;
        Some(panel.target())
    }

    /// The order in front of the window in front: its tab's, else the one selected in the Leitstand tab or the panel.
    fn front_order(&self) -> Option<(Target, OrderId)> {
        let state = self.ivars().upgrade()?;
        let target = self.front()?;
        let window = target.window();
        let selected = window.selected_tab();
        let in_tab = state
            .orders
            .live()
            .into_iter()
            .find(|tab| tab.tab().is_some() && tab.tab() == selected && tab.root() == target.root())
            .map(|tab| tab.order().clone());
        let in_board = || {
            state
                .boards
                .live()
                .into_iter()
                .find(|tab| tab.tab().is_some() && tab.tab() == selected && tab.root() == target.root())
                .and_then(|tab| tab.selected_order())
        };
        let in_panel = || state.panels.of(window).and_then(|panel| panel.selected_order());
        let id = in_tab.or_else(in_board).or_else(in_panel)?;
        Some((target, id))
    }

    fn validate(&self, item: &NSMenuItem) -> bool {
        let Some(action) = item.action() else { return false };
        if action == sel!(ideFactoryMenuCommand:) {
            let Some(command) = Command::of_tag(item.tag()) else {
                return false;
            };
            return self
                .front_order()
                .and_then(|(target, id)| host().order(target.root(), &id))
                .is_some_and(|detail| detail.can(command));
        }
        if action == sel!(ideFactoryMenuOpen:) {
            return self.front_order().is_some();
        }
        self.front().is_some()
    }
}

/// "Neuer Auftrag …" and the submenu "Auftrag", for the File menu; `key` gives a command's shortcut.
pub(crate) fn items(
    mtm: MainThreadMarker,
    key: &dyn Fn(&str) -> String,
    target: &OrderCommands,
) -> Vec<Retained<NSMenuItem>> {
    let entry = |title: &str, action: objc2::runtime::Sel, command: &str| {
        let entry = item(mtm, title, Some(action), &key(command));
        set_target(&entry, target);
        entry
    };
    let mut order = vec![
        entry(t("Auftrag öffnen"), sel!(ideFactoryMenuOpen:), "order.open"),
        separator(mtm),
    ];
    for command in Command::ALL {
        let entry = tagged(mtm, command.title(), sel!(ideFactoryMenuCommand:), "", command.tag());
        set_target(&entry, target);
        order.push(entry);
    }
    order.extend([
        separator(mtm),
        entry(
            t("Software-Factory-Tab zeigen"),
            sel!(ideFactoryMenuBoard:),
            "order.board",
        ),
        entry(
            t("Software-Factory-Einstellungen …"),
            sel!(ideFactoryMenuSettings:),
            "order.settings",
        ),
    ]);
    vec![
        entry(t("Neuer Auftrag …"), sel!(ideFactoryMenuNewOrder:), "order.new"),
        submenu(mtm, t("Auftrag"), order).0,
    ]
}

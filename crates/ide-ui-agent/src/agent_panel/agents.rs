//! The panel's "+ Neue Sitzung": the agents a session can start with now, and the way to the settings' Agents tab,
//! where agents are installed and managed (E58).

use std::cell::RefCell;

use ide_acp::Installs;
use ide_acp::register;
use objc2::rc::Retained;
use objc2::runtime::AnyObject;
use objc2::{DefinedClass, MainThreadMarker, MainThreadOnly, define_class, msg_send, sel};
use objc2_app_kit::{NSButton, NSColor, NSMenu, NSMenuItem, NSView};
use objc2_foundation::{NSObject, NSObjectProtocol, NSPoint};
use splat_i18n::t;

use crate::{AcpAgent, Agent, Availability};
use ide_ui_common::util::{guarded, item, set_action};
use ide_ui_common::window::Window;

/// Agents installed from the registry that the "new session" menu offers, by its tags.
#[derive(Default)]
pub struct OfferedAgents {
    offered: RefCell<Vec<String>>,
}

fn with_offered<R>(f: impl FnOnce(&RefCell<Vec<String>>) -> R) -> R {
    f(&crate::host::state().agent_panels.offered_agents.offered)
}

pub struct TargetIvars {
    controller: Window,
}

define_class!(
    // SAFETY: NSObject has no subclassing requirements and `Target` does not implement Drop.
    #[unsafe(super(NSObject))]
    #[thread_kind = MainThreadOnly]
    #[name = "IDEAgentsTarget"]
    #[ivars = TargetIvars]
    pub struct Target;

    unsafe impl NSObjectProtocol for Target {}

    impl Target {
        #[unsafe(method(ideAgentsNewSession:))]
        fn new_session(&self, sender: Option<&NSView>) {
            guarded((), || {
                if let Some(view) = sender {
                    let menu = session_menu(self);
                    let below = NSPoint::new(0.0, view.frame().size.height + 4.0);
                    menu.popUpMenuPositioningItem_atLocation_inView(None, below, Some(view));
                }
            });
        }

        #[unsafe(method(ideAgentsOpen:))]
        fn open(&self, sender: Option<&AnyObject>) {
            guarded((), || {
                let id = with_offered(|o| usize::try_from(tag(sender)).ok().and_then(|n| o.borrow().get(n).cloned()));
                if let (Some(controller), Some(agent)) = (self.ivars().controller.load(), id.and_then(|id| AcpAgent::find(&id))) {
                    crate::open_agent(&controller, Agent::Acp(agent), None);
                }
            });
        }

        #[unsafe(method(ideAgentsManage:))]
        fn manage(&self, _sender: Option<&AnyObject>) {
            guarded((), || crate::host::host().show_agents_settings(self.mtm()));
        }
    }
);

fn tag(sender: Option<&AnyObject>) -> isize {
    // SAFETY: Every sender of these actions is a menu item, which answers `tag`.
    sender.map_or(-1, |s| unsafe { msg_send![s, tag] })
}

/// The panel's way to start a session.
pub struct Agents {
    target: Retained<Target>,
    mtm: MainThreadMarker,
}

impl Agents {
    pub fn new(controller: &Window) -> Agents {
        let mtm = controller.mtm();
        let this = Target::alloc(mtm).set_ivars(TargetIvars {
            controller: *controller,
        });
        // SAFETY: NSObject's initializer, called once on the newly allocated object.
        let target: Retained<Target> = unsafe { msg_send![super(this), init] };
        Agents { target, mtm }
    }

    /// The button that opens a new session, with the choice of agent.
    pub fn new_session_button(&self) -> Retained<NSButton> {
        let button =
            ide_ui_common::util::borderless_button(self.mtm, t("+ Neue Sitzung"), None, sel!(ideAgentsNewSession:));
        set_action(&button, &self.target, sel!(ideAgentsNewSession:));
        button.setFont(Some(&crate::style::small()));
        button.setContentTintColor(Some(&NSColor::linkColor()));
        button
    }
}

/// Whether Claude's own program is there; while the login shell's environment is still read, it counts as there.
fn claude_usable() -> bool {
    ide_ui_common::window::try_environment(std::time::Duration::ZERO)
        .is_none_or(|env| crate::claude_program(&env).is_some())
}

/// The agents a session can start with now: Claude, the own agent once it has a model, the presets that are ready
/// and the agents installed from the registry; then the way to the settings that install the others.
fn session_menu(target: &Target) -> Retained<NSMenu> {
    let mtm = target.mtm();
    let menu = NSMenu::new(mtm);
    if claude_usable() {
        menu.addItem(&item(mtm, "Claude", Some(sel!(ideNewClaudeThread:)), ""));
    }
    if crate::own_agent::default_choice().is_some() {
        menu.addItem(&item(mtm, t("Eigener Agent"), Some(sel!(ideNewOwnThread:)), ""));
    }
    let env = &ide_ui_common::window::environment(std::time::Duration::ZERO)[..];
    for (n, agent) in AcpAgent::presets().into_iter().enumerate() {
        if agent.offered()
            && matches!(
                crate::availability(&agent, env, std::path::Path::new("/")),
                Availability::Ready(_)
            )
        {
            let entry = item(mtm, &agent.name, Some(sel!(ideNewAcpThread:)), "");
            entry.setTag(isize::try_from(n).unwrap_or(-1));
            menu.addItem(&entry);
        }
    }
    let register = crate::cached_register().map(|(r, _)| r).unwrap_or_default();
    let installs = Installs::new(Installs::default_root());
    let installed: Vec<(String, String)> = register
        .entries
        .iter()
        .filter(|e| register::preset(&e.id).is_none() && installs.current(&e.id).is_some())
        .map(|e| (e.id.clone(), e.name.clone()))
        .collect();
    for (n, (_, name)) in installed.iter().enumerate() {
        let entry = item(mtm, name, Some(sel!(ideAgentsOpen:)), "");
        ide_ui_common::util::set_target(&entry, target);
        entry.setTag(isize::try_from(n).unwrap_or(-1));
        menu.addItem(&entry);
    }
    with_offered(|o| *o.borrow_mut() = installed.into_iter().map(|(id, _)| id).collect());
    if menu.numberOfItems() > 0 {
        menu.addItem(&NSMenuItem::separatorItem(mtm));
    }
    let manage = item(mtm, t("Agenten verwalten …"), Some(sel!(ideAgentsManage:)), "");
    ide_ui_common::util::set_target(&manage, target);
    menu.addItem(&manage);
    menu
}

#[cfg(feature = "selftest")]
mod selftest_probe;

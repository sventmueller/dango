//! The Terminal tab (E199): Tapa's switches of the input field, the blocks and the pasteboard, grouped and worded as
//! in Tapa's settings window, and ⌥ as Meta. A switch only writes its setting; the open terminals take it up when the
//! settings change (`crate::terminal::settings_changed`).

use std::cell::RefCell;

use ide_core::settings::{Key, Settings, keys};
use objc2::rc::Retained;
use objc2::runtime::{AnyObject, NSObject, Sel};
use objc2::{DefinedClass, MainThreadMarker, MainThreadOnly, define_class, msg_send, sel};
use objc2_app_kit::{NSButton, NSControlStateValueOff, NSControlStateValueOn, NSView};
use objc2_foundation::NSObjectProtocol;
use splat_i18n::t;

use super::form::{self, Row};
use crate::util::{guarded, ns, set_action};

pub(crate) struct TerminalIvars {
    /// Each switch with its setting, once the tab was built.
    shown: RefCell<Vec<(Retained<NSButton>, Key<bool>)>>,
}

define_class!(
    // SAFETY: NSObject has no subclassing requirements and `TerminalSettings` does not implement Drop.
    #[unsafe(super(NSObject))]
    #[thread_kind = MainThreadOnly]
    #[name = "IDETerminalSettings"]
    #[ivars = TerminalIvars]
    pub(crate) struct TerminalSettings;

    unsafe impl NSObjectProtocol for TerminalSettings {}

    impl TerminalSettings {
        #[unsafe(method(ideToggleTerminalInputField:))]
        fn toggle_input_field(&self, sender: Option<&AnyObject>) {
            guarded((), || {
                store(sender, &keys::TERMINAL_INPUT_FIELD);
            });
        }

        #[unsafe(method(ideToggleTerminalInputVim:))]
        fn toggle_input_vim(&self, sender: Option<&AnyObject>) {
            guarded((), || {
                store(sender, &keys::TERMINAL_INPUT_VIM);
            });
        }

        #[unsafe(method(ideToggleTerminalSuggestions:))]
        fn toggle_suggestions(&self, sender: Option<&AnyObject>) {
            guarded((), || {
                store(sender, &keys::TERMINAL_SUGGESTIONS);
            });
        }

        #[unsafe(method(ideToggleTerminalHighlighting:))]
        fn toggle_highlighting(&self, sender: Option<&AnyObject>) {
            guarded((), || {
                store(sender, &keys::TERMINAL_HIGHLIGHTING);
            });
        }

        #[unsafe(method(ideToggleTerminalUnderlineErrors:))]
        fn toggle_underline_errors(&self, sender: Option<&AnyObject>) {
            guarded((), || {
                store(sender, &keys::TERMINAL_UNDERLINE_ERRORS);
            });
        }

        #[unsafe(method(ideToggleTerminalBlocks:))]
        fn toggle_blocks(&self, sender: Option<&AnyObject>) {
            guarded((), || {
                store(sender, &keys::TERMINAL_BLOCKS);
            });
        }

        #[unsafe(method(ideToggleTerminalCopyOnSelect:))]
        fn toggle_copy_on_select(&self, sender: Option<&AnyObject>) {
            guarded((), || {
                store(sender, &keys::TERMINAL_COPY_ON_SELECT);
            });
        }

        #[unsafe(method(ideToggleTerminalPasteProtection:))]
        fn toggle_paste_protection(&self, sender: Option<&AnyObject>) {
            guarded((), || {
                store(sender, &keys::TERMINAL_PASTE_PROTECTION);
            });
        }

        #[unsafe(method(ideToggleOptionAsMeta:))]
        fn toggle_option_as_meta(&self, sender: Option<&AnyObject>) {
            guarded((), || crate::terminal::set_option_as_meta(store(sender, &keys::OPTION_AS_META)));
        }
    }
);

/// A switch of the tab.
pub(super) struct Switch {
    /// The title of the group the switch opens; `None` where it goes on in the group above.
    pub(super) group: Option<&'static str>,
    pub(super) title: &'static str,
    pub(super) key: Key<bool>,
    pub(super) action: Sel,
}

/// The tab's switches in their order: the input field, the blocks, the pasteboard and the keyboard.
pub(super) fn switches() -> [Switch; 9] {
    let switch = |group: Option<&'static str>, title: &'static str, key: Key<bool>, action: Sel| Switch {
        group,
        title,
        key,
        action,
    };
    [
        switch(
            Some(t("Neue Terminals:")),
            t("Mit Eingabefeld"),
            keys::TERMINAL_INPUT_FIELD,
            sel!(ideToggleTerminalInputField:),
        ),
        switch(
            None,
            t("Vim-Tasten im Eingabefeld"),
            keys::TERMINAL_INPUT_VIM,
            sel!(ideToggleTerminalInputVim:),
        ),
        switch(
            None,
            t("Vorschläge beim Tippen"),
            keys::TERMINAL_SUGGESTIONS,
            sel!(ideToggleTerminalSuggestions:),
        ),
        switch(
            None,
            t("Befehle einfärben"),
            keys::TERMINAL_HIGHLIGHTING,
            sel!(ideToggleTerminalHighlighting:),
        ),
        switch(
            None,
            t("Unbekannte Befehle unterstreichen"),
            keys::TERMINAL_UNDERLINE_ERRORS,
            sel!(ideToggleTerminalUnderlineErrors:),
        ),
        switch(
            Some(t("Shell:")),
            t("Befehle als Blöcke markieren"),
            keys::TERMINAL_BLOCKS,
            sel!(ideToggleTerminalBlocks:),
        ),
        switch(
            Some(t("Zwischenablage:")),
            t("Auswahl sofort kopieren"),
            keys::TERMINAL_COPY_ON_SELECT,
            sel!(ideToggleTerminalCopyOnSelect:),
        ),
        switch(
            None,
            t("Mehrzeiliges Einsetzen vorher zeigen"),
            keys::TERMINAL_PASTE_PROTECTION,
            sel!(ideToggleTerminalPasteProtection:),
        ),
        switch(
            Some(t("Tastatur:")),
            t("Wahltaste als Meta-Taste"),
            keys::OPTION_AS_META,
            sel!(ideToggleOptionAsMeta:),
        ),
    ]
}

/// Stores the state of the checkbox `sender` as `key`, and returns it.
fn store(sender: Option<&AnyObject>, key: &Key<bool>) -> bool {
    let on = sender
        .and_then(|s| s.downcast_ref::<NSButton>())
        .is_some_and(|b| b.state() == NSControlStateValueOn);
    if let Err(error) = Settings::load_user().set(key, &on) {
        crate::diag::report("settings", error);
    }
    on
}

fn state(on: bool) -> isize {
    if on {
        NSControlStateValueOn
    } else {
        NSControlStateValueOff
    }
}

impl TerminalSettings {
    pub(crate) fn new(mtm: MainThreadMarker) -> Retained<TerminalSettings> {
        let this = Self::alloc(mtm).set_ivars(TerminalIvars {
            shown: RefCell::new(Vec::new()),
        });
        // SAFETY: NSObject's initializer, called once on the newly allocated object.
        unsafe { msg_send![super(this), init] }
    }

    /// The tab, each switch titled by its group and showing its setting.
    pub(crate) fn view(&self) -> Retained<NSView> {
        let mtm = self.mtm();
        let settings = Settings::load_user();
        let mut rows = Vec::new();
        let mut shown = Vec::new();
        for switch in switches() {
            // SAFETY: The checkbox sends its action to this object, which answers it, set right below.
            let button = unsafe { NSButton::checkboxWithTitle_target_action(&ns(switch.title), None, None, mtm) };
            set_action(&button, self, switch.action);
            button.setState(state(settings.get(&switch.key)));
            rows.push(match switch.group {
                Some(group) => Row::new(mtm, group, &button),
                None => Row::under(&button),
            });
            shown.push((button, switch.key));
        }
        *self.ivars().shown.borrow_mut() = shown;
        form::page(mtm, &form::grid(mtm, &rows))
    }

    /// Shows the settings as they are now, which the sync or a paste warning's "do not ask again" may have changed.
    pub(crate) fn reload(&self) {
        let settings = Settings::load_user();
        for (button, key) in self.ivars().shown.borrow().iter() {
            button.setState(state(settings.get(key)));
        }
    }
}

//! The editor's switches in the General tab: inlay hints with the languages that leave them out (E83), and blame at
//! the end of the cursor's line (E86). The View menu flips the same settings.

use std::cell::RefCell;

use objc2::rc::Retained;
use objc2::runtime::{AnyObject, NSObject};
use objc2::{DefinedClass, MainThreadMarker, MainThreadOnly, define_class, msg_send, sel};
use objc2_app_kit::{
    NSButton, NSControlStateValueOff, NSControlStateValueOn, NSMenuItem, NSPopUpButton, NSStackView, NSView,
};
use objc2_foundation::NSObjectProtocol;
use splat_i18n::t;

use crate::util::{guarded, label, ns, set_action};

pub struct EditorIvars {
    views: RefCell<Option<Views>>,
}

struct Views {
    hints: Retained<NSButton>,
    languages: Retained<NSPopUpButton>,
    blame: Retained<NSButton>,
}

define_class!(
    // SAFETY: NSObject has no subclassing requirements and `EditorSettings` does not implement Drop.
    #[unsafe(super(NSObject))]
    #[thread_kind = MainThreadOnly]
    #[name = "IDEEditorSettings"]
    #[ivars = EditorIvars]
    pub struct EditorSettings;

    unsafe impl NSObjectProtocol for EditorSettings {}

    impl EditorSettings {
        #[unsafe(method(ideToggleInlayHints:))]
        fn toggle_hints(&self, _sender: Option<&AnyObject>) {
            guarded((), || {
                crate::lsp::inlay::toggle();
                self.reload();
            });
        }

        #[unsafe(method(ideToggleInlayLanguage:))]
        fn toggle_language(&self, sender: Option<&AnyObject>) {
            guarded((), || {
                let tag = sender.and_then(|s| s.downcast_ref::<NSMenuItem>()).map_or(-1, |item| item.tag());
                let language = usize::try_from(tag).ok().and_then(|i| ide_lsp::BUILT_IN.get(i));
                if let Some(language) = language {
                    crate::lsp::inlay::toggle_language(language.name());
                }
                self.reload();
            });
        }

        #[unsafe(method(ideToggleBlameAtLineEnd:))]
        fn toggle_blame(&self, sender: Option<&AnyObject>) {
            guarded((), || {
                let on = sender
                    .and_then(|s| s.downcast_ref::<NSButton>())
                    .is_some_and(|b| b.state() == NSControlStateValueOn);
                crate::editor::set_blame_at_line_end(on);
            });
        }
    }
);

/// The editor part of the settings on view, which its controls send their actions to.
#[derive(Default)]
pub struct EditorSettingsShown {
    tab: RefCell<Option<Retained<EditorSettings>>>,
}

fn with_tab<R>(f: impl FnOnce(&RefCell<Option<Retained<EditorSettings>>>) -> R) -> R {
    f(&crate::context::app().settings.editor_settings.tab)
}

fn state(on: bool) -> isize {
    if on {
        NSControlStateValueOn
    } else {
        NSControlStateValueOff
    }
}

impl EditorSettings {
    fn new(mtm: MainThreadMarker) -> Retained<EditorSettings> {
        let this = Self::alloc(mtm).set_ivars(EditorIvars {
            views: RefCell::new(None),
        });
        // SAFETY: NSObject's initializer, called once on the newly allocated object.
        unsafe { msg_send![super(this), init] }
    }

    /// Shows the settings as they are now, which the View menu may have changed.
    fn reload(&self) {
        let views = self.ivars().views.borrow();
        let Some(views) = views.as_ref() else { return };
        let on = crate::lsp::inlay::enabled();
        views.hints.setState(state(on));
        views.languages.setEnabled(on);
        let items = views.languages.itemArray();
        for (index, language) in ide_lsp::BUILT_IN.iter().enumerate() {
            if let Some(item) = items.iter().nth(index + 1) {
                item.setState(state(crate::lsp::inlay::enabled_for(language.name())));
            }
        }
        views.blame.setState(state(crate::editor::blame_at_line_end()));
    }
}

/// The rows of the General tab for the editor's switches, each a title and its controls.
pub fn rows(mtm: MainThreadMarker) -> Vec<Vec<Retained<NSView>>> {
    let target = EditorSettings::new(mtm);
    // SAFETY: The checkbox sends its action to the target, which answers it, set right below.
    let hints = unsafe { NSButton::checkboxWithTitle_target_action(&ns(t("Im Editor zeigen")), None, None, mtm) };
    set_action(&hints, &target, sel!(ideToggleInlayHints:));
    let languages = NSPopUpButton::new(mtm);
    languages.setPullsDown(true);
    languages.addItemWithTitle(&ns(t("Sprachen")));
    for (index, language) in ide_lsp::BUILT_IN.iter().enumerate() {
        languages.addItemWithTitle(&ns(language.name()));
        if let Some(item) = languages.lastItem() {
            item.setTag(isize::try_from(index).unwrap_or(0));
            // SAFETY: The target set right below answers the action, which `set_target` checks.
            unsafe { item.setAction(Some(sel!(ideToggleInlayLanguage:))) };
            crate::util::set_target(&item, &target);
        }
    }
    let hint_row = NSStackView::stackViewWithViews(
        &objc2_foundation::NSArray::from_retained_slice(&[
            hints.clone().into_super().into_super(),
            languages.clone().into_super().into_super().into_super(),
        ]),
        mtm,
    );
    hint_row.setSpacing(12.0);
    // SAFETY: The checkbox sends its action to the target, which answers it, set right below.
    let blame =
        unsafe { NSButton::checkboxWithTitle_target_action(&ns(t("Am Ende der Cursorzeile zeigen")), None, None, mtm) };
    set_action(&blame, &target, sel!(ideToggleBlameAtLineEnd:));
    blame.setToolTip(Some(&ns(t(
        "Zeigt nach kurzer Ruhe, wer die Zeile zuletzt geändert hat, nur in vertrauten Ordnern.",
    ))));
    *target.ivars().views.borrow_mut() = Some(Views {
        hints,
        languages,
        blame: blame.clone(),
    });
    target.reload();
    with_tab(|tab| *tab.borrow_mut() = Some(target));
    vec![
        vec![
            label(mtm, t("Inlay-Hinweise:")).into_super().into_super(),
            hint_row.into_super(),
        ],
        vec![
            label(mtm, t("Blame:")).into_super().into_super(),
            blame.into_super().into_super(),
        ],
    ]
}

/// Shows the editor's switches as the settings are now, when the window comes to the front.
pub fn reload() {
    with_tab(|tab| {
        if let Some(tab) = tab.borrow().as_ref() {
            tab.reload();
        }
    });
}

/// The editor's switches as the General tab shows them: inlay hints, the languages ticked in the pull-down, and
/// blame at the line's end, for the self-test.
#[cfg(feature = "selftest")]
pub fn shown() -> Option<(bool, Vec<String>, bool)> {
    with_tab(|tab| {
        let tab = tab.borrow();
        let views = tab.as_ref()?.ivars().views.borrow();
        let views = views.as_ref()?;
        let ticked = views
            .languages
            .itemArray()
            .iter()
            .skip(1)
            .filter(|item| item.state() == NSControlStateValueOn)
            .map(|item| item.title().to_string())
            .collect();
        Some((
            views.hints.state() == NSControlStateValueOn,
            ticked,
            views.blame.state() == NSControlStateValueOn,
        ))
    })
}

//! The part of the Agent tab about commit messages (E88, E101): which of the own agent's providers and models writes
//! them, `claude -p` when none is chosen, which providers outside the Mac the user allowed to get the changes, and
//! taking those permissions back.

use std::cell::{OnceCell, RefCell};

use ide_agent::Endpoint;
use ide_core::settings::Settings;
use objc2::rc::Retained;
use objc2::runtime::{AnyObject, NSObject};
use objc2::{DefinedClass, MainThreadMarker, MainThreadOnly, define_class, msg_send, sel};
use objc2_app_kit::{NSBezelStyle, NSButton, NSComboBox, NSPopUpButton, NSTextField, NSView};
use objc2_foundation::NSObjectProtocol;
use splat_i18n::{t, tf};

use super::form::{self, Row};
use crate::util::{guarded, ns, set_action};
use ide_ui_agent::own_agent::{self, ModelChoice};
use ide_ui_git::message::source::{self, CLAUDE, CONSENT, MODEL};

/// The width of the model field, as in the row for new threads.
const MODEL_WIDTH: f64 = 260.0;

pub struct Ivars {
    endpoints: RefCell<Vec<Endpoint>>,
    views: OnceCell<Views>,
}

struct Views {
    endpoint: Retained<NSPopUpButton>,
    model: Retained<NSComboBox>,
    allowed: Retained<NSTextField>,
    revoke: Retained<NSButton>,
}

define_class!(
    // SAFETY: NSObject has no subclassing requirements and `CommitMessageSettings` does not implement Drop.
    #[unsafe(super(NSObject))]
    #[thread_kind = MainThreadOnly]
    #[name = "IDECommitMessageSettings"]
    #[ivars = Ivars]
    pub struct CommitMessageSettings;

    unsafe impl NSObjectProtocol for CommitMessageSettings {}

    impl CommitMessageSettings {
        #[unsafe(method(ideCommitMessageEndpoint:))]
        fn endpoint_action(&self, _sender: Option<&AnyObject>) {
            guarded((), || self.endpoint_chosen());
        }

        #[unsafe(method(ideCommitMessageModel:))]
        fn model_action(&self, _sender: Option<&AnyObject>) {
            guarded((), || self.store());
        }

        #[unsafe(method(ideCommitMessageRevoke:))]
        fn revoke_action(&self, _sender: Option<&AnyObject>) {
            guarded((), || {
                source::revoke_all();
                self.fill();
            });
        }
    }
);

#[derive(Default)]
pub struct CommitMessageShown {
    /// The part on view, which its controls send their actions to.
    shown: RefCell<Option<Retained<CommitMessageSettings>>>,
}

fn with_shown<R>(f: impl FnOnce(&RefCell<Option<Retained<CommitMessageSettings>>>) -> R) -> R {
    f(&crate::context::app().settings.commit_message_settings.shown)
}

/// The heading, notes and controls of the part, for the Agent tab's column of views.
pub fn views(mtm: MainThreadMarker) -> Vec<Retained<NSView>> {
    let this = CommitMessageSettings::alloc(mtm).set_ivars(Ivars {
        endpoints: RefCell::new(Vec::new()),
        views: OnceCell::new(),
    });
    // SAFETY: NSObject's initializer, called once on the newly allocated object.
    let this: Retained<CommitMessageSettings> = unsafe { msg_send![super(this), init] };
    let endpoint = NSPopUpButton::new(mtm);
    set_action(&endpoint, &this, sel!(ideCommitMessageEndpoint:));
    // The provider's names may be long; its title gives way before the row grows past the tab's column.
    endpoint.setContentCompressionResistancePriority_forOrientation(
        250.0,
        objc2_app_kit::NSLayoutConstraintOrientation::Horizontal,
    );
    endpoint
        .widthAnchor()
        .constraintLessThanOrEqualToConstant(form::CONTROLS)
        .setActive(true);
    let model = NSComboBox::new(mtm);
    model.setCompletes(true);
    model
        .widthAnchor()
        .constraintEqualToConstant(MODEL_WIDTH)
        .setActive(true);
    set_action(&model, &this, sel!(ideCommitMessageModel:));
    let allowed = form::value(mtm, "");
    // SAFETY: Without a target and an action the button sends nothing until `set_action` gives it both.
    let revoke =
        unsafe { NSButton::buttonWithTitle_target_action(&ns(t("Erlaubnisse zurücknehmen")), None, None, mtm) };
    set_action(&revoke, &this, sel!(ideCommitMessageRevoke:));
    revoke.setBezelStyle(NSBezelStyle::Push);
    let consent = form::note_field(
        mtm,
        t(
            "Anbieter außerhalb des Macs fragen einmal um Erlaubnis. Ollama und LM Studio auf diesem Mac brauchen keine: Der Code verlässt den Mac nicht.",
        ),
        form::CONTROLS,
    );
    let grid = form::grid(
        mtm,
        &[
            Row::new(mtm, t("Anbieter:"), &endpoint),
            Row::new(mtm, t("Modell:"), &model),
            Row::new(mtm, t("Erlaubnis:"), &allowed),
            Row::under(&revoke),
            Row::under(&consent),
        ],
    );
    let views = vec![
        form::heading(mtm, t("Commit-Nachrichten")),
        form::note(
            mtm,
            t(
                "Der Knopf im Commit-Feld lässt ein Modell die Nachricht schreiben. Ohne gewähltes Modell schreibt claude mit einem kleinen Modell und ohne Werkzeuge. Vorher prüft gitleaks den Text auf Secrets; bei einem Fund geht nichts hinaus.",
            ),
        ),
        grid.into_super(),
    ];
    let _ = this.ivars().views.set(Views {
        endpoint,
        model,
        allowed,
        revoke,
    });
    this.fill();
    with_shown(|shown| *shown.borrow_mut() = Some(this));
    views
}

/// What the line about consent says: the providers allowed so far, or that there are none.
fn allowed_text(ids: &[String], endpoints: &[Endpoint]) -> String {
    if ids.is_empty() {
        return t("Noch kein Anbieter hat die Erlaubnis, Änderungen zu bekommen.").to_string();
    }
    let names: Vec<String> = ids
        .iter()
        .map(|id| {
            if id == CLAUDE {
                return "Claude (claude -p)".to_string();
            }
            endpoints
                .iter()
                .find(|e| e.id == *id)
                .map_or_else(|| id.clone(), |e| e.name.clone())
        })
        .collect();
    tf("Erlaubt: {}", &[&names.join(", ")])
}

impl CommitMessageSettings {
    /// Shows the settings as they are.
    fn fill(&self) {
        let Some(views) = self.ivars().views.get() else { return };
        let endpoints = own_agent::endpoints();
        let settings = Settings::load_user();
        let choice = settings.get(&MODEL);
        views.endpoint.removeAllItems();
        views
            .endpoint
            .addItemWithTitle(&ns(t("Automatisch: claude mit kleinem Modell")));
        for endpoint in &endpoints {
            views.endpoint.addItemWithTitle(&ns(&endpoint.name));
        }
        let index = choice
            .as_ref()
            .and_then(|c| endpoints.iter().position(|e| e.id == c.endpoint))
            .map_or(0, |n| n + 1);
        views.endpoint.selectItemAtIndex(isize::try_from(index).unwrap_or(0));
        views
            .model
            .setStringValue(&ns(choice.as_ref().map_or("", |c| c.model.as_str())));
        views.model.setEnabled(index > 0);
        // Without a provider, claude writes with this model; the field says so instead of standing empty.
        let automatic = (index == 0).then(|| ns(ide_ui_git::message::CLAUDE_MODEL));
        views.model.setPlaceholderString(automatic.as_deref());
        let consent = settings.get(&CONSENT);
        views.allowed.setStringValue(&ns(&allowed_text(&consent, &endpoints)));
        views.revoke.setEnabled(!consent.is_empty());
        *self.ivars().endpoints.borrow_mut() = endpoints;
        self.fill_models();
    }

    fn chosen(&self) -> Option<Endpoint> {
        let views = self.ivars().views.get()?;
        let index = usize::try_from(views.endpoint.indexOfSelectedItem()).ok()?;
        self.ivars().endpoints.borrow().get(index.checked_sub(1)?).cloned()
    }

    fn fill_models(&self) {
        let Some(views) = self.ivars().views.get() else { return };
        views.model.removeAllItems();
        let Some(endpoint) = self.chosen() else { return };
        for model in own_agent::models(&endpoint.id) {
            // SAFETY: The combo box takes any object as an item; a string is what it shows and completes.
            unsafe { views.model.addItemWithObjectValue(&ns(&model.id)) };
        }
    }

    /// Another provider was chosen: its models are loaded, the first proposed unless one was typed.
    fn endpoint_chosen(&self) {
        let Some(views) = self.ivars().views.get() else { return };
        views.model.setStringValue(&ns(""));
        let Some(endpoint) = self.chosen() else {
            views.model.setEnabled(false);
            self.store();
            return;
        };
        views.model.setEnabled(true);
        if let Some(model) = endpoint.kind.default_model() {
            views.model.setStringValue(&ns(model));
            self.store();
        }
        self.fill_models();
        if !own_agent::usable(&endpoint) {
            return;
        }
        own_agent::load_models(endpoint, |result| {
            let Some(this) = with_shown(|shown| shown.borrow().clone()) else {
                return;
            };
            let Some(views) = this.ivars().views.get() else { return };
            this.fill_models();
            if let Ok(models) = result
                && views.model.stringValue().to_string().trim().is_empty()
                && let Some(first) = models.first()
            {
                views.model.setStringValue(&ns(&first.id));
                this.store();
            }
        });
    }

    fn store(&self) {
        let Some(views) = self.ivars().views.get() else { return };
        let model = views.model.stringValue().to_string().trim().to_string();
        let choice = self.chosen().filter(|_| !model.is_empty()).map(|endpoint| ModelChoice {
            endpoint: endpoint.id,
            model,
        });
        let mut settings = Settings::load_user();
        if let Err(error) = settings.set(&MODEL, &choice) {
            crate::diag::report("saving the model for commit messages", error);
        }
    }
}

/// What the self-test reads from the part.
#[cfg(feature = "selftest")]
pub mod selftest {
    use objc2::DefinedClass;

    /// The line about consent and whether its button can take permissions back.
    pub fn allowed() -> Option<(String, bool)> {
        let shown = super::with_shown(|shown| shown.borrow().clone())?;
        let views = shown.ivars().views.get()?;
        Some((views.allowed.stringValue().to_string(), views.revoke.isEnabled()))
    }

    /// How far right the part's controls reach past the left edge of the provider's menu, and the column's width;
    /// scrolls the part into view first.
    pub fn reach() -> Option<(f64, f64)> {
        let shown = super::with_shown(|shown| shown.borrow().clone())?;
        let views = shown.ivars().views.get()?;
        views.revoke.scrollRectToVisible(views.revoke.bounds());
        let in_window = |view: &objc2_app_kit::NSView| view.convertRect_toView(view.bounds(), None);
        let left = in_window(&views.endpoint).origin.x;
        let right = |view: &objc2_app_kit::NSView| {
            let frame = in_window(view);
            frame.origin.x + frame.size.width - left
        };
        let reach = [
            right(&views.endpoint),
            right(&views.model),
            right(&views.allowed),
            right(&views.revoke),
        ]
        .into_iter()
        .fold(0.0, f64::max);
        Some((reach, super::form::CONTROLS))
    }

    /// Takes every permission back, as the button does.
    pub fn revoke() {
        if let Some(shown) = super::with_shown(|shown| shown.borrow().clone()) {
            ide_ui_git::message::source::revoke_all();
            shown.fill();
        }
    }
}

#[cfg(test)]
mod tests {
    use ide_agent::ProviderKind;

    use super::*;

    #[test]
    fn the_consent_line_names_the_providers_by_their_names() {
        let endpoints = vec![Endpoint::preset(ProviderKind::Anthropic).unwrap()];
        assert!(allowed_text(&[], &endpoints).starts_with("Noch kein"));
        let text = allowed_text(&["anthropic".into(), CLAUDE.into(), "gone".into()], &endpoints);
        assert_eq!(text, "Erlaubt: Anthropic, Claude (claude -p), gone");
    }
}

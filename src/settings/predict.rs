//! The tab of edit prediction (B4, E91, E101): which provider proposes edits while typing, what each needs (Copilot
//! its server and a sign-in through GitHub's device flow, the local ones a server address and a model), the files
//! that never go to a provider, the languages switched off in the status bar, and the providers outside the Mac the
//! user allowed, with taking that back. Local models need no consent: the code does not leave the Mac.

use std::cell::{OnceCell, RefCell};
use std::path::PathBuf;

use ide_core::settings::Settings;
use ide_predict::copilot::Account;
use objc2::rc::Retained;
use objc2::runtime::{AnyObject, NSObject};
use objc2::{DefinedClass, MainThreadMarker, MainThreadOnly, define_class, msg_send, sel};
use objc2_app_kit::{
    NSButton, NSComboBox, NSControlTextEditingDelegate, NSGridView, NSPopUpButton, NSTextField, NSView,
};
use objc2_foundation::NSObjectProtocol;
use splat_i18n::{t, tf};

use super::form::{self, Row};
use crate::editor::predict::model::{self, Choice, Local};
use crate::editor::predict::provider::{self, CopilotState};
use crate::util::{guarded, ns, set_action};

pub struct Ivars {
    views: OnceCell<Views>,
    /// The models the local server listed last.
    models: RefCell<Vec<String>>,
}

struct Views {
    provider: Retained<NSPopUpButton>,
    about: Retained<NSTextField>,
    copilot: Retained<NSTextField>,
    copilot_button: Retained<NSButton>,
    url: Retained<NSTextField>,
    model: Retained<NSComboBox>,
    model_note: Retained<NSTextField>,
    exclude: Retained<NSTextField>,
    off_for: Retained<NSTextField>,
    allowed: Retained<NSTextField>,
    revoke: Retained<NSButton>,
}

define_class!(
    // SAFETY: NSObject has no subclassing requirements and `PredictionSettings` does not implement Drop.
    #[unsafe(super(NSObject))]
    #[thread_kind = MainThreadOnly]
    #[name = "IDEPredictionSettings"]
    #[ivars = Ivars]
    pub struct PredictionSettings;

    unsafe impl NSObjectProtocol for PredictionSettings {}

    unsafe impl NSControlTextEditingDelegate for PredictionSettings {}

    impl PredictionSettings {
        #[unsafe(method(idePredictionProvider:))]
        fn provider_action(&self, _sender: Option<&AnyObject>) {
            guarded((), || self.provider_chosen());
        }

        #[unsafe(method(idePredictionLocal:))]
        fn local_action(&self, _sender: Option<&AnyObject>) {
            guarded((), || {
                self.store_local();
                self.load_models();
            });
        }

        #[unsafe(method(idePredictionModel:))]
        fn model_action(&self, _sender: Option<&AnyObject>) {
            guarded((), || self.store_local());
        }

        #[unsafe(method(idePredictionExclude:))]
        fn exclude_action(&self, _sender: Option<&AnyObject>) {
            guarded((), || self.store_exclusions());
        }

        #[unsafe(method(idePredictionCopilot:))]
        fn copilot_action(&self, _sender: Option<&AnyObject>) {
            guarded((), || self.copilot_pressed());
        }

        #[unsafe(method(idePredictionRevoke:))]
        fn revoke_action(&self, _sender: Option<&AnyObject>) {
            guarded((), || {
                provider::revoke_all();
                self.fill();
            });
        }
    }
);

/// The tab while the settings window exists, which its controls send their actions to.
#[derive(Default)]
pub struct PredictionShown {
    shown: RefCell<Option<Retained<PredictionSettings>>>,
}

fn with_shown<R>(f: impl FnOnce(&RefCell<Option<Retained<PredictionSettings>>>) -> R) -> R {
    f(&crate::context::app().settings.prediction.shown)
}

fn small(mtm: MainThreadMarker, text: &str) -> Retained<NSTextField> {
    form::note_field(mtm, text, form::CONTROLS)
}

fn button(
    mtm: MainThreadMarker,
    title: &str,
    target: &PredictionSettings,
    action: objc2::runtime::Sel,
) -> Retained<NSButton> {
    // SAFETY: Without a target and an action the button sends nothing until `set_action` gives it both.
    let button = unsafe { NSButton::buttonWithTitle_target_action(&ns(title), None, None, mtm) };
    set_action(&button, target, action);
    button
}

/// The tab's view.
pub fn view(mtm: MainThreadMarker) -> Retained<NSView> {
    let this = PredictionSettings::alloc(mtm).set_ivars(Ivars {
        views: OnceCell::new(),
        models: RefCell::new(Vec::new()),
    });
    // SAFETY: NSObject's initializer, called once on the newly allocated object.
    let this: Retained<PredictionSettings> = unsafe { msg_send![super(this), init] };
    let provider_menu = NSPopUpButton::new(mtm);
    for choice in Choice::ALL {
        provider_menu.addItemWithTitle(&ns(provider::name(choice)));
    }
    set_action(&provider_menu, &this, sel!(idePredictionProvider:));
    let url = NSTextField::new(mtm);
    url.widthAnchor().constraintEqualToConstant(260.0).setActive(true);
    set_action(&url, &this, sel!(idePredictionLocal:));
    let model = NSComboBox::new(mtm);
    model.setCompletes(true);
    model.widthAnchor().constraintEqualToConstant(260.0).setActive(true);
    set_action(&model, &this, sel!(idePredictionModel:));
    let exclude = NSTextField::new(mtm);
    exclude
        .widthAnchor()
        .constraintEqualToConstant(form::CONTROLS)
        .setActive(true);
    set_action(&exclude, &this, sel!(idePredictionExclude:));
    let views = Views {
        provider: provider_menu,
        about: small(mtm, ""),
        copilot: form::value(mtm, ""),
        copilot_button: button(mtm, "", &this, sel!(idePredictionCopilot:)),
        url,
        model,
        model_note: small(mtm, ""),
        exclude,
        off_for: form::value(mtm, ""),
        allowed: form::value(mtm, ""),
        revoke: button(mtm, t("Erlaubnisse zurücknehmen"), &this, sel!(idePredictionRevoke:)),
    };
    let grid = grid(mtm, &views);
    let _ = this.ivars().views.set(views);
    this.fill();
    this.load_models();
    with_shown(|shown| *shown.borrow_mut() = Some(this));
    form::page(mtm, &grid)
}

fn grid(mtm: MainThreadMarker, views: &Views) -> Retained<NSGridView> {
    let exclude_note = small(
        mtm,
        t("Muster für Dateinamen, durch Kommas getrennt, wie .env* oder *.pem"),
    );
    form::grid(
        mtm,
        &[
            Row::new(mtm, t("Anbieter:"), &views.provider),
            Row::under(&views.about),
            Row::new(mtm, "Copilot:", &views.copilot),
            Row::under(&views.copilot_button),
            Row::new(mtm, t("Adresse:"), &views.url),
            Row::new(mtm, t("Modell:"), &views.model),
            Row::under(&views.model_note),
            Row::new(mtm, t("Nie senden:"), &views.exclude),
            Row::under(&exclude_note),
            Row::new(mtm, t("Sprachen:"), &views.off_for),
            Row::new(mtm, t("Erlaubnis:"), &views.allowed),
            Row::under(&views.revoke),
        ],
    )
}

/// Shows `text` under the model, and the row only when there is something to say.
fn show_note(note: &NSTextField, text: &str) {
    note.setStringValue(&ns(text));
    form::show(note, !text.is_empty());
}

/// The folder whose Copilot the tab shows: the front window's, if it is trusted, else one Copilot runs for.
fn copilot_root(mtm: MainThreadMarker) -> Option<PathBuf> {
    crate::window::frontmost(mtm)
        .filter(|c| c.is_trusted())
        .and_then(|c| c.project_root())
        .or_else(|| provider::copilot_roots().into_iter().next())
}

impl PredictionSettings {
    fn fill(&self) {
        let Some(views) = self.ivars().views.get() else { return };
        let chosen = provider::chosen();
        let index = Choice::ALL.iter().position(|c| *c == chosen.choice).unwrap_or(0);
        views.provider.selectItemAtIndex(isize::try_from(index).unwrap_or(0));
        let remote = chosen.local().filter(|local| !model::loopback(&local.url));
        views.about.setStringValue(&ns(&match (chosen.choice, remote) {
            (Choice::Off, _) => t("Ohne Anbieter macht der Editor keine Vorschläge beim Tippen.").to_string(),
            (Choice::Copilot, _) => t(
                "Der Code geht an GitHub. Copilot braucht ein Abo; die kostenlose Stufe hat ein Kontingent.",
            )
            .to_string(),
            (_, Some(local)) => tf("Der Code geht an {}.", &[&local.url]),
            (Choice::Ollama | Choice::LmStudio, None) => t(
                "Ein Fill-in-the-Middle-Modell auf diesem Mac: Der Code verlässt den Mac nicht. Tempo und Qualität hängen vom Modell ab.",
            )
            .to_string(),
        }));
        let local = chosen.local().cloned();
        views.url.setEnabled(local.is_some());
        views.model.setEnabled(local.is_some());
        views
            .url
            .setStringValue(&ns(local.as_ref().map_or("", |l| l.url.as_str())));
        views
            .model
            .setStringValue(&ns(local.as_ref().map_or("", |l| l.model.as_str())));
        views.exclude.setStringValue(&ns(&chosen.exclude.join(", ")));
        views.off_for.setStringValue(&ns(&if chosen.off_for.is_empty() {
            t("Alle an; die Statusleiste des Editors schaltet eine Sprache aus.").to_string()
        } else {
            tf("Aus für: {}", &[&chosen.off_for.join(", ")])
        }));
        views.allowed.setStringValue(&ns(&if chosen.consent.is_empty() {
            t("Kein Anbieter außerhalb des Macs hat die Erlaubnis.").to_string()
        } else {
            tf("Erlaubt: {}", &[&chosen.consent.join(", ")])
        }));
        views.revoke.setEnabled(!chosen.consent.is_empty());
        self.fill_copilot();
        self.fill_model_note();
    }

    fn fill_copilot(&self) {
        let Some(views) = self.ivars().views.get() else { return };
        let on = provider::chosen().choice == Choice::Copilot;
        let root = copilot_root(self.mtm());
        let state = root.as_deref().map(provider::copilot_state);
        let (text, action) = match (&root, state) {
            (None, _) => (
                t("Öffne einen vertrauten Ordner, um Copilot zu starten.").to_string(),
                None,
            ),
            (_, None | Some(CopilotState::NotStarted)) => (t("Nicht gestartet.").to_string(), Some(t("Starten"))),
            (_, Some(CopilotState::Starting | CopilotState::Running(None | Some(Account::Starting)))) => {
                (t("Copilot startet …").to_string(), None)
            }
            (_, Some(CopilotState::Missing)) => (
                t("Der Sprachserver von Copilot ist nicht installiert.").to_string(),
                Some(t("Installieren")),
            ),
            (_, Some(CopilotState::Installing(progress))) => (progress, Some(t("Abbrechen"))),
            (_, Some(CopilotState::Failed(message))) => (message, None),
            (_, Some(CopilotState::Running(Some(Account::NeedsSignIn)))) => {
                (t("Nicht angemeldet.").to_string(), Some(t("Anmelden …")))
            }
            (_, Some(CopilotState::Running(Some(Account::SignedIn { user })))) => {
                (tf("Angemeldet als {}.", &[&user]), Some(t("Abmelden")))
            }
            (_, Some(CopilotState::Running(Some(Account::NoSubscription { user })))) => {
                (tf("Copilot: {} hat kein Copilot-Abo", &[&user]), Some(t("Abmelden")))
            }
            (_, Some(CopilotState::Running(Some(Account::Unavailable(message))))) => (message, None),
        };
        views.copilot.setStringValue(&ns(&text));
        views.copilot_button.setTitle(&ns(action.unwrap_or(t("Anmelden …"))));
        views.copilot_button.setEnabled(on && action.is_some());
    }

    fn copilot_pressed(&self) {
        let Some(root) = copilot_root(self.mtm()) else { return };
        match provider::copilot_state(&root) {
            CopilotState::NotStarted => provider::wake(&root),
            CopilotState::Missing => provider::install(&root),
            CopilotState::Installing(_) => provider::cancel_install(&root),
            CopilotState::Running(Some(Account::NeedsSignIn)) => {
                crate::editor::predict::sign_in_sheet(&root);
            }
            CopilotState::Running(Some(Account::SignedIn { .. } | Account::NoSubscription { .. })) => {
                if let Some(copilot) = provider::copilot(&root) {
                    copilot.sign_out();
                }
            }
            _ => {}
        }
        self.fill_copilot();
    }

    fn fill_model_note(&self) {
        let Some(views) = self.ivars().views.get() else { return };
        let chosen = provider::chosen();
        let model = chosen.local().map(|l| l.model.clone()).unwrap_or_default();
        let note = if chosen.local().is_none() {
            String::new()
        } else if model.trim().is_empty() {
            t("Wähle ein Modell, das Fill-in-the-Middle kann.").to_string()
        } else if chosen.choice == Choice::LmStudio && ide_predict::fim::Family::of(&model).is_none() {
            t("Dieses Modell gehört zu keiner bekannten Fill-in-the-Middle-Familie; Vorschläge können ausbleiben.")
                .to_string()
        } else {
            String::new()
        };
        show_note(&views.model_note, &note);
    }

    /// The user picked a provider: one outside the Mac asks for consent first and stays off without it.
    fn provider_chosen(&self) {
        let Some(views) = self.ivars().views.get() else { return };
        let index = usize::try_from(views.provider.indexOfSelectedItem()).unwrap_or(0);
        let choice = Choice::ALL.get(index).copied().unwrap_or_default();
        set_choice(choice);
        let chosen = provider::chosen();
        if !chosen.consented()
            && let Some(window) = super::window()
        {
            // Declining keeps the choice without consent, so nothing goes out and the status bar says why.
            crate::editor::predict::ask_consent(&window, provider::name(choice), move || {
                provider::consent();
                prediction_changed();
            });
        }
        if choice == Choice::Copilot
            && let Some(root) = copilot_root(self.mtm())
        {
            provider::wake(&root);
        }
        self.fill();
        self.load_models();
    }

    fn store_local(&self) {
        let Some(views) = self.ivars().views.get() else { return };
        let local = Local {
            url: views.url.stringValue().to_string().trim().to_string(),
            model: views.model.stringValue().to_string().trim().to_string(),
        };
        let key = match provider::chosen().choice {
            Choice::Ollama => &model::OLLAMA,
            Choice::LmStudio => &model::LM_STUDIO,
            Choice::Off | Choice::Copilot => return,
        };
        let mut settings = Settings::load_user();
        if let Err(error) = settings.set(key, &local) {
            crate::diag::report("saving the model for edit prediction", error);
        }
        provider::settings_changed();
        self.fill_model_note();
    }

    fn store_exclusions(&self) {
        let Some(views) = self.ivars().views.get() else { return };
        let patterns: Vec<String> = views
            .exclude
            .stringValue()
            .to_string()
            .split(',')
            .map(|p| p.trim().to_string())
            .filter(|p| !p.is_empty())
            .collect();
        let mut settings = Settings::load_user();
        if let Err(error) = settings.set(&model::EXCLUDE, &patterns) {
            crate::diag::report("saving the exclusions of edit prediction", error);
        }
        provider::settings_changed();
    }

    /// Asks the local server for its models off the main thread and offers them in the model field.
    fn load_models(&self) {
        let chosen = provider::chosen();
        let Some(local) = chosen.local().cloned() else { return };
        let choice = chosen.choice;
        let _ = std::thread::Builder::new()
            .name("prediction-models".into())
            .spawn(move || {
                let models = match choice {
                    Choice::Ollama => ide_predict::fim::Ollama::models(&local.url),
                    _ => ide_predict::fim::LmStudio::models(&local.url),
                };
                crate::util::main_soon(move || {
                    let Some(this) = with_shown(|shown| shown.borrow().clone()) else {
                        return;
                    };
                    let Some(views) = this.ivars().views.get() else { return };
                    views.model.removeAllItems();
                    match models {
                        Ok(models) => {
                            for model in &models {
                                // SAFETY: The combo box takes any object as an item; a string is what it shows.
                                unsafe { views.model.addItemWithObjectValue(&ns(model)) };
                            }
                            *this.ivars().models.borrow_mut() = models;
                            this.fill_model_note();
                        }
                        Err(error) => show_note(
                            &views.model_note,
                            &match error {
                                ide_predict::Error::Unavailable(text) => text,
                                other => format!("{other:?}"),
                            },
                        ),
                    }
                });
            });
    }
}

fn set_choice(choice: Choice) {
    let mut settings = Settings::load_user();
    if let Err(error) = settings.set(&model::PROVIDER, &choice) {
        crate::diag::report("saving the provider of edit prediction", error);
    }
    provider::settings_changed();
}

/// Shows the tab as the settings and the providers are now.
pub fn prediction_changed() {
    if let Some(this) = with_shown(|shown| shown.borrow().clone()) {
        this.fill();
    }
}

/// What the tab shows, for the self-test: the provider's title, the Copilot line and the consent line.
#[cfg(feature = "selftest")]
pub fn shown() -> Option<(String, String, String, bool)> {
    let this = with_shown(|shown| shown.borrow().clone())?;
    let views = this.ivars().views.get()?;
    Some((
        views.provider.titleOfSelectedItem()?.to_string(),
        views.copilot.stringValue().to_string(),
        views.allowed.stringValue().to_string(),
        views.revoke.isEnabled(),
    ))
}

/// What the tab shows of the local server, for the self-test.
#[cfg(feature = "selftest")]
pub struct LocalShown {
    pub about: String,
    pub url: String,
    pub model: String,
    /// The models the model field offers.
    pub offered: Vec<String>,
    pub note: String,
    pub exclude: String,
}

/// What the tab shows of the local server, for the self-test.
#[cfg(feature = "selftest")]
pub fn local_shown() -> Option<LocalShown> {
    let this = with_shown(|shown| shown.borrow().clone())?;
    let views = this.ivars().views.get()?;
    let offered = (0..views.model.numberOfItems())
        .filter_map(|index| {
            let item = views.model.itemObjectValueAtIndex(index);
            item.downcast::<objc2_foundation::NSString>()
                .ok()
                .map(|s| s.to_string())
        })
        .collect();
    Some(LocalShown {
        about: views.about.stringValue().to_string(),
        url: views.url.stringValue().to_string(),
        model: views.model.stringValue().to_string(),
        offered,
        note: views.model_note.stringValue().to_string(),
        exclude: views.exclude.stringValue().to_string(),
    })
}

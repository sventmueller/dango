//! The dictation tab of the settings: Lowspeak's choices, which only Lowspeak changes, the app's own API keys, and fn.

use std::cell::OnceCell;

use lowspeak::dictation::Choices;
use lowspeak::models::{Language, PostProcessingMode, TranscriptionMode};
use objc2::rc::Retained;
use objc2::runtime::{AnyObject, NSObject};
use objc2::{DefinedClass, MainThreadMarker, MainThreadOnly, Message, define_class, msg_send, sel};
use objc2_app_kit::{
    NSButton, NSColor, NSControlStateValueOff, NSControlStateValueOn, NSGridCellPlacement, NSGridView, NSStackView,
    NSTextField, NSUserInterfaceLayoutOrientation, NSView, NSWorkspace,
};
use objc2_foundation::{NSArray, NSObjectProtocol};
use splat_i18n::{t, tf};

use crate::dictation::{self, FnState, LOWSPEAK_APP, PROVIDERS};
use crate::util::{guarded, label, ns, set_action};

const WRAP: f64 = 340.0;

pub struct DictationIvars {
    views: OnceCell<Views>,
}

struct Views {
    lowspeak: Retained<NSTextField>,
    open: Retained<NSButton>,
    transcription: Retained<NSTextField>,
    language: Retained<NSTextField>,
    post_processing: Retained<NSTextField>,
    microphone: Retained<NSTextField>,
    vocabulary: Retained<NSTextField>,
    keys: Vec<(Retained<NSTextField>, Retained<NSButton>)>,
    with_fn: Retained<NSButton>,
    fn_note: Retained<NSTextField>,
    ask: Retained<NSButton>,
}

define_class!(
    // SAFETY: NSObject has no subclassing requirements and `DictationSettings` does not implement Drop.
    #[unsafe(super(NSObject))]
    #[thread_kind = MainThreadOnly]
    #[name = "IDEDictationSettings"]
    #[ivars = DictationIvars]
    pub struct DictationSettings;

    unsafe impl NSObjectProtocol for DictationSettings {}

    impl DictationSettings {
        #[unsafe(method(ideDictationOpenLowspeak:))]
        fn open_lowspeak(&self, _sender: Option<&AnyObject>) {
            guarded((), || {
                let workspace = NSWorkspace::sharedWorkspace();
                if let Some(url) = workspace.URLForApplicationWithBundleIdentifier(&ns(LOWSPEAK_APP)) {
                    workspace.openURL(&url);
                }
            });
        }

        #[unsafe(method(ideDictationKey:))]
        fn key(&self, sender: Option<&AnyObject>) {
            guarded((), || {
                let index = sender.and_then(|s| s.downcast_ref::<NSButton>()).map(|b| b.tag());
                let Some(&provider) = index.and_then(|i| usize::try_from(i).ok()).and_then(|i| PROVIDERS.get(i)) else {
                    return;
                };
                if dictation::has_key(provider) {
                    if let Err(error) = dictation::remove_key(provider) {
                        crate::diag::report("dictation", error);
                    }
                } else {
                    dictation::enter_key(provider);
                }
                self.reload();
            });
        }

        #[unsafe(method(ideDictationWithFn:))]
        fn with_fn(&self, sender: Option<&AnyObject>) {
            guarded((), || {
                let on = sender
                    .and_then(|s| s.downcast_ref::<NSButton>())
                    .is_some_and(|b| b.state() == NSControlStateValueOn);
                let mut settings = ide_core::settings::Settings::load_user();
                if let Err(error) = settings.set(&dictation::DICTATE_WITH_FN, &on) {
                    crate::diag::report("settings", error);
                }
                dictation::fn_setting_changed(on);
                self.reload();
            });
        }

        #[unsafe(method(ideDictationAskPermission:))]
        fn ask_permission(&self, _sender: Option<&AnyObject>) {
            guarded((), || {
                dictation::ask_fn_permission();
                self.reload();
            });
        }
    }
);

impl DictationSettings {
    pub fn new(mtm: MainThreadMarker) -> Retained<DictationSettings> {
        let this = Self::alloc(mtm).set_ivars(DictationIvars { views: OnceCell::new() });
        // SAFETY: NSObject's initializer, called once on the newly allocated object.
        unsafe { msg_send![super(this), init] }
    }

    /// The tab's view, built once.
    pub fn view(&self) -> Retained<NSView> {
        let mtm = self.mtm();
        let value = || {
            let field = NSTextField::wrappingLabelWithString(&ns(""), mtm);
            field.setPreferredMaxLayoutWidth(WRAP);
            field
        };
        let button = |title: &str, action, tag| {
            // SAFETY: The button sends its action to this object, which answers it, set right below.
            let button = unsafe { NSButton::buttonWithTitle_target_action(&ns(title), None, None, mtm) };
            set_action(&button, self, action);
            button.setTag(tag);
            button
        };
        // SAFETY: As above.
        let with_fn = unsafe {
            NSButton::checkboxWithTitle_target_action(
                &ns(t("Mit fn diktieren, solange Lowspeak nicht läuft")),
                None,
                None,
                mtm,
            )
        };
        set_action(&with_fn, self, sel!(ideDictationWithFn:));
        let fn_note = value();
        fn_note.setTextColor(Some(&NSColor::secondaryLabelColor()));
        let views = Views {
            lowspeak: value(),
            open: button(t("Lowspeak öffnen"), sel!(ideDictationOpenLowspeak:), 0),
            transcription: value(),
            language: value(),
            post_processing: value(),
            microphone: value(),
            vocabulary: value(),
            keys: PROVIDERS
                .iter()
                .enumerate()
                .map(|(n, _)| {
                    (
                        value(),
                        button("", sel!(ideDictationKey:), isize::try_from(n).unwrap_or(-1)),
                    )
                })
                .collect(),
            with_fn,
            fn_note,
            ask: button(t("Freigabe anfragen"), sel!(ideDictationAskPermission:), 0),
        };
        let view = layout(mtm, &views);
        let _ = self.ivars().views.set(views);
        self.reload();
        view
    }

    /// Shows the state of now: Lowspeak's choices, the keys, fn.
    pub fn reload(&self) {
        let Some(views) = self.ivars().views.get() else { return };
        let workspace = NSWorkspace::sharedWorkspace();
        let installed = workspace
            .URLForApplicationWithBundleIdentifier(&ns(LOWSPEAK_APP))
            .is_some();
        views.lowspeak.setStringValue(&ns(if installed {
            t("Installiert. Modus, Modell, Sprache, Nachbearbeitung, Mikrofon und Vokabular wählst du in Lowspeak.")
        } else {
            t("Nicht installiert. Es gelten Lowspeaks Vorgaben: online mit GPT Transcribe, Sprache erkennen.")
        }));
        views.open.setHidden(!installed);
        show_choices(views, &dictation::choices());
        for (provider, (state, button)) in PROVIDERS.iter().zip(&views.keys) {
            let kept = dictation::has_key(*provider);
            state.setStringValue(&ns(&if kept {
                tf("{}: im Schlüsselbund", &[provider.display_name()])
            } else {
                tf("{}: kein Schlüssel", &[provider.display_name()])
            }));
            button.setTitle(&ns(if kept { t("Entfernen") } else { t("Eingeben …") }));
        }
        show_fn(views);
    }
}

fn show_choices(views: &Views, choices: &Choices) {
    let texts = dictation::lowspeak_texts();
    let settings = choices.settings;
    let transcription = match settings.mode {
        TranscriptionMode::Online => tf(
            "Online: {} ({})",
            &[
                settings.online_model.name(),
                settings.online_model.provider().display_name(),
            ],
        ),
        TranscriptionMode::Local => {
            let name = settings.local_model.spec().name;
            let installed = lowspeak::transcription::model_store::installed_models().contains(&settings.local_model);
            let model = if installed {
                name.to_string()
            } else {
                (texts.not_installed)(name)
            };
            tf("{}: {}", &[texts.local, &model])
        }
    };
    views.transcription.setStringValue(&ns(&transcription));
    views.language.setStringValue(&ns(match settings.language {
        Language::Auto => texts.auto_detect,
        language => language.display_name(),
    }));
    views
        .post_processing
        .setStringValue(&ns(match settings.post_processing {
            PostProcessingMode::Off => texts.post_processing_off,
            PostProcessingMode::Cleanup => texts.post_processing_cleanup,
            PostProcessingMode::Format => texts.post_processing_format,
        }));
    views.microphone.setStringValue(&ns(choices
        .microphone
        .as_ref()
        .map_or(texts.system_default, |m| m.name.as_str())));
    views.vocabulary.setStringValue(&ns(&match choices.vocabulary.len() {
        0 => t("Keine Begriffe").to_string(),
        1 => t("1 Begriff").to_string(),
        n => tf("{} Begriffe", &[&n.to_string()]),
    }));
}

fn show_fn(views: &Views) {
    let state = dictation::fn_state();
    views.with_fn.setState(if state == FnState::Off {
        NSControlStateValueOff
    } else {
        NSControlStateValueOn
    });
    let note = match state {
        FnState::Off => String::new(),
        FnState::NotAsked => {
            t("fn braucht die Freigabe für Bedienungshilfen; macOS fragt danach nach dem ersten Diktat.").to_string()
        }
        FnState::Refused => t("fn bleibt aus, bis die Freigabe für Bedienungshilfen erteilt ist.").to_string(),
        FnState::LowspeakHasIt => tf(
            "Lowspeak läuft und diktiert mit fn; in {} dann mit dem Mikrofon oder dem Kurzbefehl.",
            &[ide_product::NAME],
        ),
        FnState::Ready => t("fn halten und sprechen, zweimal tippen diktiert freihändig.").to_string(),
    };
    views.fn_note.setStringValue(&ns(&note));
    views.fn_note.setHidden(note.is_empty());
    views.ask.setHidden(state != FnState::Refused);
}

fn layout(mtm: MainThreadMarker, views: &Views) -> Retained<NSView> {
    let up = |view: &NSView| view.retain();
    let side = |views: &[&NSView]| -> Retained<NSView> {
        let stack = NSStackView::new(mtm);
        stack.setOrientation(NSUserInterfaceLayoutOrientation::Horizontal);
        stack.setSpacing(8.0);
        for view in views {
            stack.addArrangedSubview(view);
        }
        stack.into_super()
    };
    let caption = |text: &str| label(mtm, text).into_super().into_super();
    let mut rows: Vec<Vec<Retained<NSView>>> = vec![
        vec![caption(t("Lowspeak:")), up(&views.lowspeak)],
        vec![caption(""), up(&views.open)],
        vec![caption(t("Transkription:")), up(&views.transcription)],
        vec![caption(t("Sprache:")), up(&views.language)],
        vec![caption(t("Nachbearbeitung:")), up(&views.post_processing)],
        vec![caption(t("Mikrofon:")), up(&views.microphone)],
        vec![caption(t("Vokabular:")), up(&views.vocabulary)],
    ];
    for (n, (state, button)) in views.keys.iter().enumerate() {
        let title = if n == 0 { t("API-Schlüssel:") } else { "" };
        rows.push(vec![caption(title), side(&[state, button])]);
    }
    rows.push(vec![caption(t("fn-Taste:")), up(&views.with_fn)]);
    rows.push(vec![caption(""), side(&[&views.fn_note, &views.ask])]);
    let rows: Vec<Retained<NSArray<NSView>>> = rows.iter().map(|row| NSArray::from_retained_slice(row)).collect();
    let grid = NSGridView::gridViewWithViews(&NSArray::from_retained_slice(&rows), mtm);
    grid.setRowSpacing(10.0);
    grid.setColumnSpacing(8.0);
    if let Some(column) = grid.columnAtIndex(0).into() {
        column.setXPlacement(NSGridCellPlacement::Trailing);
    }
    let view = NSView::new(mtm);
    view.addSubview(&grid);
    grid.setTranslatesAutoresizingMaskIntoConstraints(false);
    for constraint in [
        grid.leadingAnchor()
            .constraintEqualToAnchor_constant(&view.leadingAnchor(), 30.0),
        grid.trailingAnchor()
            .constraintLessThanOrEqualToAnchor_constant(&view.trailingAnchor(), -30.0),
        grid.topAnchor()
            .constraintEqualToAnchor_constant(&view.topAnchor(), 24.0),
        grid.bottomAnchor()
            .constraintEqualToAnchor_constant(&view.bottomAnchor(), -24.0),
        view.widthAnchor().constraintGreaterThanOrEqualToConstant(460.0),
    ] {
        constraint.setActive(true);
    }
    view
}

/// The keys the tab shows, for the self-test: one line per provider.
#[cfg(feature = "selftest")]
pub fn key_lines(settings: &DictationSettings) -> Vec<String> {
    settings
        .ivars()
        .views
        .get()
        .map(|views| {
            views
                .keys
                .iter()
                .map(|(state, _)| state.stringValue().to_string())
                .collect()
        })
        .unwrap_or_default()
}

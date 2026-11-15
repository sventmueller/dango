//! The dictation tab of the settings: the app's own choices for lowspeak's engine (E103), the download of a missing
//! local model, the API keys and fn; Lowspeak's choices are offered for taking over once.

use std::cell::{OnceCell, RefCell};

use lowspeak::dictation::Choices;
use lowspeak::i18n::Texts;
use lowspeak::models::{Language, LocalModel, OnlineModel, PostProcessingMode, TranscriptionMode};
use objc2::rc::{Retained, Weak};
use objc2::runtime::{AnyObject, NSObject};
use objc2::{DefinedClass, MainThreadMarker, MainThreadOnly, define_class, msg_send, sel};
use objc2_app_kit::{
    NSButton, NSControlStateValueOff, NSControlStateValueOn, NSGridView, NSMenuItem, NSPopUpButton,
    NSProgressIndicator, NSProgressIndicatorStyle, NSStackView, NSTextField, NSTokenField,
    NSUserInterfaceLayoutOrientation, NSView,
};
use objc2_foundation::{NSArray, NSNumber, NSNumberFormatter, NSNumberFormatterStyle, NSObjectProtocol, NSString};
use splat_i18n::{t, tf};

use super::form::{self, Row};
use crate::util::{guarded, label, ns, set_action};
use ide_ui_dictation::{self as dictation, FnState, PROVIDERS, choices};

/// Tags of the transcription menu: an online model's index in `OnlineModel::ALL` above the first, a local one's in
/// `LocalModel::ALL` above the second.
pub(crate) const ONLINE: isize = 100;
pub(crate) const LOCAL: isize = 200;
/// The grid's rows that come and go: Lowspeak's offer and the missing model's download.
const OFFER_ROWS: [usize; 2] = [0, 1];
const MODEL_ROW: usize = 3;

pub(crate) struct DictationIvars {
    views: OnceCell<Views>,
    /// The id and name of each microphone the menu lists after the system default, in its order.
    microphones: RefCell<Vec<(String, String)>>,
}

struct Views {
    grid: Retained<NSGridView>,
    offer: Retained<NSTextField>,
    transcription: Retained<NSPopUpButton>,
    model: Retained<NSTextField>,
    progress: Retained<NSProgressIndicator>,
    download: Retained<NSButton>,
    language: Retained<NSPopUpButton>,
    post_processing: Retained<NSPopUpButton>,
    microphone: Retained<NSPopUpButton>,
    vocabulary: Retained<NSTokenField>,
    keys: Vec<KeyRow>,
    with_fn: Retained<NSButton>,
    fn_note: Retained<NSTextField>,
    ask: Retained<NSButton>,
}

struct KeyRow {
    state: Retained<NSTextField>,
    change: Retained<NSButton>,
    remove: Retained<NSButton>,
}

define_class!(
    // SAFETY: NSObject has no subclassing requirements and `DictationSettings` does not implement Drop.
    #[unsafe(super(NSObject))]
    #[thread_kind = MainThreadOnly]
    #[name = "IDEDictationSettings"]
    #[ivars = DictationIvars]
    pub(crate) struct DictationSettings;

    unsafe impl NSObjectProtocol for DictationSettings {}

    impl DictationSettings {
        #[unsafe(method(ideDictationLowspeakChoices:))]
        fn lowspeak_choices(&self, sender: Option<&AnyObject>) {
            guarded((), || {
                let take = tag_of(sender) == Some(1);
                let mut settings = ide_core::settings::Settings::load_user();
                if let Err(error) = choices::answer_offer(&mut settings, &dictation::lowspeak_choices(), take) {
                    crate::diag::report("settings", error);
                }
                self.reload();
            });
        }

        #[unsafe(method(ideDictationTranscription:))]
        fn transcription(&self, sender: Option<&AnyObject>) {
            guarded((), || {
                let Some(tag) = selected_tag(sender) else { return };
                let online = usize::try_from(tag - ONLINE).ok().and_then(|i| OnlineModel::ALL.get(i).copied());
                let local = usize::try_from(tag - LOCAL).ok().and_then(|i| LocalModel::ALL.get(i).copied());
                self.change(|choices| {
                    if let Some(model) = local {
                        choices.settings.mode = TranscriptionMode::Local;
                        choices.settings.local_model = model;
                    } else if let Some(model) = online {
                        choices.settings.mode = TranscriptionMode::Online;
                        choices.settings.online_model = model;
                    }
                });
            });
        }

        #[unsafe(method(ideDictationDownload:))]
        fn download(&self, _sender: Option<&AnyObject>) {
            guarded((), || {
                if dictation::downloading().is_some() {
                    dictation::cancel_download();
                    return;
                }
                let weak = Weak::from(self);
                dictation::download(choices::current().settings.local_model, move || {
                    if let Some(settings) = weak.load() {
                        settings.reload();
                    }
                });
                self.reload();
            });
        }

        #[unsafe(method(ideDictationLanguage:))]
        fn language(&self, sender: Option<&AnyObject>) {
            guarded((), || {
                let chosen = selected_tag(sender).and_then(|tag| usize::try_from(tag).ok());
                if let Some(&language) = chosen.and_then(|i| Language::ALL.get(i)) {
                    self.change(|choices| choices.settings.language = language);
                }
            });
        }

        #[unsafe(method(ideDictationPostProcessing:))]
        fn post_processing(&self, sender: Option<&AnyObject>) {
            guarded((), || {
                let chosen = selected_tag(sender).and_then(|tag| usize::try_from(tag).ok());
                if let Some(&mode) = chosen.and_then(|i| PostProcessingMode::ALL.get(i)) {
                    self.change(|choices| choices.settings.post_processing = mode);
                }
            });
        }

        #[unsafe(method(ideDictationMicrophone:))]
        fn microphone(&self, sender: Option<&AnyObject>) {
            guarded((), || {
                let chosen = selected_tag(sender)
                    .and_then(|tag| usize::try_from(tag).ok())
                    .and_then(|i| self.ivars().microphones.borrow().get(i).cloned());
                self.change(|choices| {
                    choices.microphone = chosen.map(|(id, name)| lowspeak::config::settings::ChosenMicrophone { id, name });
                });
            });
        }

        #[unsafe(method(ideDictationVocabulary:))]
        fn vocabulary(&self, sender: Option<&AnyObject>) {
            guarded((), || {
                let Some(field) = sender.and_then(|s| s.downcast_ref::<NSTokenField>()) else { return };
                let terms = lowspeak::config::vocabulary::parse(&tokens(field).join(","));
                self.change(|choices| choices.vocabulary = terms);
            });
        }

        #[unsafe(method(ideDictationKey:))]
        fn key(&self, sender: Option<&AnyObject>) {
            guarded((), || {
                let Some(provider) = provider_of(sender) else { return };
                if !dictation::has_key(provider) && dictation::lowspeak_has_key(provider) {
                    dictation::take_over_key(provider);
                } else {
                    dictation::enter_key(provider);
                }
                self.reload();
            });
        }

        #[unsafe(method(ideDictationRemoveKey:))]
        fn remove_key(&self, sender: Option<&AnyObject>) {
            guarded((), || {
                let Some(provider) = provider_of(sender) else { return };
                if let Err(error) = dictation::remove_key(provider) {
                    crate::diag::report("dictation", error);
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

fn tag(index: usize) -> isize {
    isize::try_from(index).unwrap_or(-1)
}

fn tag_of(sender: Option<&AnyObject>) -> Option<isize> {
    sender.and_then(|s| s.downcast_ref::<NSButton>()).map(|b| b.tag())
}

fn selected_tag(sender: Option<&AnyObject>) -> Option<isize> {
    sender
        .and_then(|s| s.downcast_ref::<NSPopUpButton>())
        .and_then(|popup| popup.selectedItem())
        .map(|item| item.tag())
}

fn provider_of(sender: Option<&AnyObject>) -> Option<lowspeak::models::ApiProvider> {
    let index = usize::try_from(tag_of(sender)?).ok()?;
    PROVIDERS.get(index).copied()
}

/// The terms of the vocabulary field, as tokens.
fn tokens(field: &NSTokenField) -> Vec<String> {
    let Some(value) = field.objectValue() else {
        return Vec::new();
    };
    match value.downcast::<NSArray>() {
        Ok(array) => array
            .iter()
            .filter_map(|token| token.downcast::<NSString>().ok())
            .map(|token| token.to_string())
            .collect(),
        Err(value) => value
            .downcast::<NSString>()
            .map(|text| vec![text.to_string()])
            .unwrap_or_default(),
    }
}

impl DictationSettings {
    pub(crate) fn new(mtm: MainThreadMarker) -> Retained<DictationSettings> {
        let this = Self::alloc(mtm).set_ivars(DictationIvars {
            views: OnceCell::new(),
            microphones: RefCell::new(Vec::new()),
        });
        // SAFETY: NSObject's initializer, called once on the newly allocated object.
        unsafe { msg_send![super(this), init] }
    }

    /// Changes the app's dictation choices with `change`, writes them and shows them.
    fn change(&self, change: impl FnOnce(&mut Choices)) {
        if let Err(error) = choices::change(change) {
            crate::diag::report("settings", error);
        }
        self.reload();
    }

    /// The tab's view, built once.
    pub(crate) fn view(&self) -> Retained<NSView> {
        let mtm = self.mtm();
        let texts = dictation::lowspeak_texts();
        let value = || form::value(mtm, "");
        let button = |title: &str, action, tag| {
            // SAFETY: The button sends its action to this object, which answers it, set right below.
            let button = unsafe { NSButton::buttonWithTitle_target_action(&ns(title), None, None, mtm) };
            set_action(&button, self, action);
            button.setTag(tag);
            button
        };
        let popup = |action| {
            let popup = NSPopUpButton::new(mtm);
            popup.setAutoenablesItems(false);
            set_action(&popup, self, action);
            popup
        };
        let progress = NSProgressIndicator::new(mtm);
        progress.setStyle(NSProgressIndicatorStyle::Bar);
        progress.setIndeterminate(false);
        progress.setMinValue(0.0);
        progress.setMaxValue(1.0);
        progress.setControlSize(objc2_app_kit::NSControlSize::Small);
        progress.widthAnchor().constraintEqualToConstant(120.0).setActive(true);
        let views = Views {
            grid: NSGridView::new(mtm),
            offer: value(),
            transcription: popup(sel!(ideDictationTranscription:)),
            model: value(),
            progress,
            download: button("", sel!(ideDictationDownload:), 0),
            language: popup(sel!(ideDictationLanguage:)),
            post_processing: popup(sel!(ideDictationPostProcessing:)),
            microphone: popup(sel!(ideDictationMicrophone:)),
            vocabulary: self.vocabulary_field(mtm),
            keys: PROVIDERS
                .iter()
                .enumerate()
                .map(|(n, _)| {
                    let tag = isize::try_from(n).unwrap_or(-1);
                    KeyRow {
                        state: label(mtm, ""),
                        change: button("", sel!(ideDictationKey:), tag),
                        remove: button(t("Entfernen"), sel!(ideDictationRemoveKey:), tag),
                    }
                })
                .collect(),
            with_fn: self.fn_checkbox(mtm),
            fn_note: form::note_field(mtm, "", form::CONTROLS),
            ask: button(t("Freigabe anfragen"), sel!(ideDictationAskPermission:), 0),
        };
        fill_fixed_menus(&views, texts);
        let take = button(t("Einstellungen übernehmen"), sel!(ideDictationLowspeakChoices:), 1);
        let ignore = button(t("Ignorieren"), sel!(ideDictationLowspeakChoices:), 0);
        let view = layout(mtm, &views, &[&take, &ignore], texts);
        let _ = self.ivars().views.set(views);
        self.reload();
        view
    }

    fn vocabulary_field(&self, mtm: MainThreadMarker) -> Retained<NSTokenField> {
        let field = NSTokenField::new(mtm);
        field.setPreferredMaxLayoutWidth(form::CONTROLS);
        if let Some(cell) = field.cell() {
            cell.setWraps(true);
            cell.setSendsActionOnEndEditing(true);
        }
        field
            .widthAnchor()
            .constraintEqualToConstant(form::CONTROLS)
            .setActive(true);
        set_action(&field, self, sel!(ideDictationVocabulary:));
        field
    }

    fn fn_checkbox(&self, mtm: MainThreadMarker) -> Retained<NSButton> {
        // SAFETY: The checkbox sends its action to this object, which answers it, set right below.
        let with_fn = unsafe {
            NSButton::checkboxWithTitle_target_action(
                &ns(t("Mit fn diktieren, solange Lowspeak nicht läuft")),
                None,
                None,
                mtm,
            )
        };
        set_action(&with_fn, self, sel!(ideDictationWithFn:));
        with_fn
    }

    /// Shows the state of now: the offer, the choices, the download, the keys, fn.
    pub(crate) fn reload(&self) {
        let Some(views) = self.ivars().views.get() else { return };
        let texts = dictation::lowspeak_texts();
        let settings = ide_core::settings::Settings::load_user();
        let offer = choices::lowspeak_offer(&settings, &dictation::lowspeak_choices());
        for row in OFFER_ROWS {
            views.grid.rowAtIndex(tag(row)).setHidden(!offer);
        }
        views.offer.setStringValue(&ns(t(
            "Lowspeak hat eigene Einstellungen für das Diktat. Möchtest du sie übernehmen?",
        )));
        let current = choices::in_settings(&settings);
        show_transcription(views, texts, &current);
        show_download(views, texts, &current);
        select_tag(
            &views.language,
            Language::ALL.iter().position(|l| *l == current.settings.language),
        );
        select_tag(
            &views.post_processing,
            PostProcessingMode::ALL
                .iter()
                .position(|m| *m == current.settings.post_processing),
        );
        *self.ivars().microphones.borrow_mut() = show_microphones(views, texts, &current);
        if views.vocabulary.currentEditor().is_none() {
            let terms: Vec<Retained<NSString>> = current.vocabulary.iter().map(|term| ns(term)).collect();
            // SAFETY: A token field takes an array of strings as its value, one per token.
            unsafe {
                views
                    .vocabulary
                    .setObjectValue(Some(&NSArray::from_retained_slice(&terms)))
            };
        }
        show_keys(views);
        show_fn(views);
        super::fit(super::DICTATION_TAB);
    }
}

fn select_tag(popup: &NSPopUpButton, index: Option<usize>) {
    popup.selectItemWithTag(index.and_then(|i| isize::try_from(i).ok()).unwrap_or(0));
}

fn item(mtm: MainThreadMarker, title: &str, tag: isize, indented: bool) -> Retained<NSMenuItem> {
    let item = NSMenuItem::new(mtm);
    item.setTitle(&ns(title));
    item.setTag(tag);
    item.setIndentationLevel(isize::from(indented));
    item
}

fn header(mtm: MainThreadMarker, title: &str) -> Retained<NSMenuItem> {
    let item = item(mtm, title, -1, false);
    item.setEnabled(false);
    item
}

/// The menus whose items never change: languages and post-processing.
fn fill_fixed_menus(views: &Views, texts: &Texts) {
    let mtm = views.grid.mtm();
    for (index, language) in Language::ALL.iter().enumerate() {
        if let Some(menu) = views.language.menu() {
            menu.addItem(&item(mtm, texts.language_name(*language), tag(index), false));
        }
    }
    for (index, mode) in PostProcessingMode::ALL.iter().enumerate() {
        if let Some(menu) = views.post_processing.menu() {
            menu.addItem(&item(mtm, texts.post_processing_mode(*mode), tag(index), false));
        }
    }
}

/// The transcription menu: the online models by provider, then the local models, each marked when not on this Mac.
fn show_transcription(views: &Views, texts: &Texts, current: &Choices) {
    let mtm = views.grid.mtm();
    let Some(menu) = views.transcription.menu() else { return };
    menu.removeAllItems();
    for provider in PROVIDERS {
        menu.addItem(&header(mtm, provider.display_name()));
        for model in OnlineModel::of_provider(provider) {
            let index = OnlineModel::ALL.iter().position(|m| *m == model).unwrap_or(0);
            let tag = ONLINE + tag(index);
            menu.addItem(&item(mtm, &texts.online_model(model), tag, true));
        }
    }
    menu.addItem(&header(mtm, texts.local));
    for (index, model) in LocalModel::ALL.iter().enumerate() {
        let name = texts.local_model(*model);
        let title = if dictation::installed(*model) {
            name
        } else {
            (texts.not_installed)(&name)
        };
        menu.addItem(&item(mtm, &title, LOCAL + tag(index), true));
    }
    let settings = current.settings;
    let tag = match settings.mode {
        TranscriptionMode::Online => {
            ONLINE
                + OnlineModel::ALL
                    .iter()
                    .position(|m| *m == settings.online_model)
                    .map_or(0, tag)
        }
        TranscriptionMode::Local => {
            LOCAL
                + LocalModel::ALL
                    .iter()
                    .position(|m| *m == settings.local_model)
                    .map_or(0, tag)
        }
    };
    views.transcription.selectItemWithTag(tag);
}

/// The row under the transcription menu: a chosen local model that is not on this Mac, with its download.
fn show_download(views: &Views, texts: &Texts, current: &Choices) {
    let chosen = current.settings.local_model;
    let running = dictation::downloading();
    let missing = current.settings.mode == TranscriptionMode::Local && !dictation::installed(chosen);
    views
        .grid
        .rowAtIndex(tag(MODEL_ROW))
        .setHidden(running.is_none() && !missing);
    views.progress.setHidden(running.is_none());
    match running {
        Some((model, fraction)) => {
            views.progress.setDoubleValue(fraction);
            let formatter = NSNumberFormatter::new();
            formatter.setNumberStyle(NSNumberFormatterStyle::PercentStyle);
            let percent = formatter
                .stringFromNumber(&NSNumber::new_f64(fraction))
                .map(|p| p.to_string())
                .unwrap_or_default();
            views
                .model
                .setStringValue(&ns(&tf("{} wird geladen: {}", &[model.spec().name, &percent])));
            views.download.setTitle(&ns(t("Abbrechen")));
        }
        None => {
            let name = chosen.spec().name;
            let line = if dictation::download_failed() == Some(chosen) {
                (texts.download_failed_info)(name)
            } else {
                (texts.not_installed)(name)
            };
            views.model.setStringValue(&ns(&line));
            views.download.setTitle(&ns(&(texts.download_model)(name)));
        }
    }
}

/// The microphone menu: the system default, the devices connected now, and a chosen one that is not; the ids and
/// names after the default, in the menu's order.
fn show_microphones(views: &Views, texts: &Texts, current: &Choices) -> Vec<(String, String)> {
    let mtm = views.grid.mtm();
    let mut listed: Vec<(String, String)> = dictation::microphones().into_iter().map(|m| (m.id, m.name)).collect();
    let connected = listed.len();
    if let Some(chosen) = current
        .microphone
        .as_ref()
        .filter(|c| !listed.iter().any(|(id, _)| *id == c.id))
    {
        listed.push((chosen.id.clone(), chosen.name.clone()));
    }
    let Some(menu) = views.microphone.menu() else {
        return listed;
    };
    menu.removeAllItems();
    menu.addItem(&item(mtm, texts.system_default, -1, false));
    let separator = NSMenuItem::separatorItem(mtm);
    separator.setTag(-2);
    menu.addItem(&separator);
    for (index, (_, name)) in listed.iter().enumerate() {
        let title = if index < connected {
            name.clone()
        } else {
            (texts.not_connected)(name)
        };
        menu.addItem(&item(mtm, &title, tag(index), false));
    }
    let chosen = current
        .microphone
        .as_ref()
        .and_then(|c| listed.iter().position(|(id, _)| *id == c.id));
    views
        .microphone
        .selectItemWithTag(chosen.and_then(|i| isize::try_from(i).ok()).unwrap_or(-1));
    listed
}

fn show_keys(views: &Views) {
    for (provider, row) in PROVIDERS.iter().zip(&views.keys) {
        let name = provider.display_name();
        let has = dictation::has_key(*provider);
        let (line, action) = if has {
            (tf("{}: im Schlüsselbund", &[name]), t("Ändern …"))
        } else if dictation::lowspeak_has_key(*provider) {
            (tf("{}: in Lowspeak", &[name]), t("Schlüssel übernehmen …"))
        } else {
            (tf("{}: kein Schlüssel", &[name]), t("Eingeben …"))
        };
        row.state.setStringValue(&ns(&line));
        row.change.setTitle(&ns(action));
        row.remove.setEnabled(has);
    }
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

/// The missing model's line above its progress and button.
fn download_column(mtm: MainThreadMarker, views: &Views) -> Retained<NSView> {
    let stack = NSStackView::new(mtm);
    stack.setOrientation(NSUserInterfaceLayoutOrientation::Vertical);
    stack.setAlignment(objc2_app_kit::NSLayoutAttribute::Leading);
    stack.setSpacing(ide_ui_common::style::space::S);
    stack.addArrangedSubview(&views.model);
    stack.addArrangedSubview(&form::line(mtm, &[&views.progress, &views.download]));
    stack.into_super()
}

/// The rows of the form; the offer's buttons are `offer`.
fn rows(mtm: MainThreadMarker, views: &Views, offer: &[&NSButton], texts: &Texts) -> Vec<Row> {
    let note = form::note_field(mtm, texts.vocabulary_info, form::CONTROLS);
    let offer: Vec<&NSView> = offer.iter().map(|b| -> &NSView { b }).collect();
    let mut rows = vec![
        Row::new(mtm, t("Lowspeak:"), &views.offer),
        Row::under(&form::line(mtm, &offer)),
        Row::new(mtm, t("Transkription:"), &views.transcription),
        Row::under(&download_column(mtm, views)),
        Row::new(mtm, t("Sprache:"), &views.language),
        Row::new(mtm, t("Nachbearbeitung:"), &views.post_processing),
        Row::new(mtm, t("Mikrofon:"), &views.microphone),
        Row::new(mtm, t("Vokabular:"), &views.vocabulary),
        Row::under(&note),
    ];
    for (n, row) in views.keys.iter().enumerate() {
        rows.push(if n == 0 {
            Row::new(mtm, t("API-Schlüssel:"), &row.state)
        } else {
            Row::under(&row.state)
        });
        rows.push(Row::under(&form::line(mtm, &[&row.change, &row.remove])));
    }
    rows.push(Row::new(mtm, t("fn-Taste:"), &views.with_fn));
    rows.push(Row::under(&form::line(mtm, &[&views.fn_note, &views.ask])));
    rows
}

fn layout(mtm: MainThreadMarker, views: &Views, offer: &[&NSButton], texts: &Texts) -> Retained<NSView> {
    form::fill(&views.grid, &rows(mtm, views, offer, texts));
    form::page(mtm, &views.grid)
}

#[cfg(feature = "selftest")]
mod selftest_probe;
#[cfg(feature = "selftest")]
pub(crate) use selftest_probe::*;

//! The dictation choices in the app's own settings (E103): mode, models, language, post-processing, microphone and
//! vocabulary, which lowspeak's pipeline reads at the start of each dictation. Lowspeak's own choices are taken over
//! once, when the user agrees.

use ide_core::settings::{Key, Settings};
use lowspeak::config::settings::{ChosenMicrophone, Settings as Engine};
use lowspeak::config::vocabulary;
use lowspeak::dictation::Choices;
use lowspeak::models::{Language, LocalModel, OnlineModel, PostProcessingMode, TranscriptionMode};
use serde::{Deserialize, Serialize};

/// The choices as the settings file keeps them: lowspeak's stored names, so a name a later lowspeak drops falls back
/// to its default instead of making the whole entry unreadable.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Stored {
    pub mode: String,
    pub local_model: String,
    pub online_model: String,
    pub language: String,
    pub post_processing: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub microphone: Option<Microphone>,
    pub vocabulary: Vec<String>,
}

/// A chosen input device: CoreAudio's UID and the name shown while it is not connected.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Microphone {
    pub id: String,
    pub name: String,
}

impl Default for Stored {
    fn default() -> Stored {
        Stored::from(&defaults())
    }
}

/// The app's dictation choices.
pub const DICTATION: Key<Stored> = Key::user("dictation", ide_core::settings::Sharing::Shared, Stored::default);

/// Lowspeak's choices were offered for taking over and the user answered; the offer does not come again.
pub const LOWSPEAK_OFFERED: Key<bool> =
    Key::user("dictationLowspeakOffered", ide_core::settings::Sharing::PerMac, || {
        false
    });

/// Lowspeak's defaults, which are the app's until the user chooses.
pub fn defaults() -> Choices {
    Choices {
        settings: Engine {
            mode: TranscriptionMode::Online,
            local_model: LocalModel::LargeV3Turbo,
            online_model: OnlineModel::GptTranscribe,
            language: Language::Auto,
            post_processing: PostProcessingMode::Off,
            preprocessing: true,
            start_sound: false,
        },
        microphone: None,
        vocabulary: Vec::new(),
    }
}

impl From<&Choices> for Stored {
    fn from(choices: &Choices) -> Stored {
        let settings = &choices.settings;
        Stored {
            mode: settings.mode.raw_value().into(),
            local_model: settings.local_model.raw_value().into(),
            online_model: settings.online_model.raw_value().into(),
            language: settings.language.raw_value().into(),
            post_processing: settings.post_processing.raw_value().into(),
            microphone: choices.microphone.as_ref().map(|m| Microphone {
                id: m.id.clone(),
                name: m.name.clone(),
            }),
            vocabulary: choices.vocabulary.clone(),
        }
    }
}

impl From<&Stored> for Choices {
    fn from(stored: &Stored) -> Choices {
        let base = defaults().settings;
        Choices {
            settings: Engine {
                mode: TranscriptionMode::from_raw(&stored.mode).unwrap_or(base.mode),
                local_model: LocalModel::from_raw(&stored.local_model).unwrap_or(base.local_model),
                online_model: OnlineModel::from_raw(&stored.online_model).unwrap_or(base.online_model),
                language: Language::from_raw(&stored.language).unwrap_or(base.language),
                post_processing: PostProcessingMode::from_raw(&stored.post_processing).unwrap_or(base.post_processing),
                ..base
            },
            microphone: stored
                .microphone
                .as_ref()
                .filter(|m| !m.id.is_empty())
                .map(|m| ChosenMicrophone {
                    id: m.id.clone(),
                    name: m.name.clone(),
                }),
            vocabulary: vocabulary::parse(&stored.vocabulary.join(",")),
        }
    }
}

/// The choices in `settings`.
pub fn in_settings(settings: &Settings) -> Choices {
    Choices::from(&settings.get(&DICTATION))
}

/// The app's choices as the settings file has them now.
pub fn current() -> Choices {
    in_settings(&Settings::load_user())
}

/// Changes the app's choices with `change` and writes them.
pub fn change(change: impl FnOnce(&mut Choices)) -> std::io::Result<()> {
    let mut settings = Settings::load_user();
    let mut choices = in_settings(&settings);
    change(&mut choices);
    settings.set(&DICTATION, &Stored::from(&choices))
}

/// Whether Lowspeak's choices wait to be offered: the user never answered, and Lowspeak has some other than its
/// defaults, which are the app's too.
pub fn lowspeak_offer(settings: &Settings, lowspeak: &Choices) -> bool {
    !settings.get(&LOWSPEAK_OFFERED) && *lowspeak != defaults()
}

/// Answers the offer: `take` copies Lowspeak's choices into the app's settings.
pub fn answer_offer(settings: &mut Settings, lowspeak: &Choices, take: bool) -> std::io::Result<()> {
    if take {
        settings.set(&DICTATION, &Stored::from(lowspeak))?;
    }
    settings.set(&LOWSPEAK_OFFERED, &true)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A folder of its own and the settings file in it.
    fn scratch(name: &str) -> (ide_core::testing::Scratch, std::path::PathBuf) {
        let dir = ide_core::testing::scratch(&format!("dictation-choices-{name}"));
        let path = dir.join("settings.json");
        (dir, path)
    }

    fn lowspeaks() -> Choices {
        let mut choices = defaults();
        choices.settings.mode = TranscriptionMode::Local;
        choices.settings.local_model = LocalModel::ParakeetUltra;
        choices.settings.language = Language::De;
        choices.microphone = Some(ChosenMicrophone {
            id: "coreaudio:AirPods".into(),
            name: "AirPods".into(),
        });
        choices.vocabulary = vec!["Postgres".into(), "Traefik".into()];
        choices
    }

    #[test]
    fn nothing_stored_gives_lowspeaks_defaults_and_what_is_stored_comes_back() {
        let (_dir, path) = scratch("roundtrip");
        let mut settings = Settings::load(path.clone());
        assert_eq!(in_settings(&settings), defaults());
        settings.set(&DICTATION, &Stored::from(&lowspeaks())).unwrap();
        assert_eq!(in_settings(&Settings::load(path)), lowspeaks());
    }

    #[test]
    fn an_unknown_name_falls_back_alone_and_the_vocabulary_is_cleaned() {
        let (_dir, path) = scratch("unknown");
        std::fs::write(
            &path,
            r#"{"dictation": {"mode": "local", "localModel": "telepathy", "language": "de",
                "vocabulary": ["Postgres", "postgres", "<x>"]}}"#,
        )
        .unwrap();
        let choices = in_settings(&Settings::load(path));
        assert_eq!(choices.settings.mode, TranscriptionMode::Local);
        assert_eq!(choices.settings.local_model, LocalModel::LargeV3Turbo);
        assert_eq!(choices.settings.language, Language::De);
        assert_eq!(choices.vocabulary, ["Postgres", "x"]);
    }

    #[test]
    fn lowspeaks_choices_are_offered_once_and_only_when_they_differ_from_the_defaults() {
        let (_dir, path) = scratch("offer");
        let mut settings = Settings::load(path.clone());
        assert!(!lowspeak_offer(&settings, &defaults()));
        assert!(lowspeak_offer(&settings, &lowspeaks()));
        answer_offer(&mut settings, &lowspeaks(), false).unwrap();
        assert!(!lowspeak_offer(&settings, &lowspeaks()));
        assert_eq!(in_settings(&settings), defaults());

        let (_dir, path) = scratch("offer-taken");
        let mut settings = Settings::load(path);
        answer_offer(&mut settings, &lowspeaks(), true).unwrap();
        assert!(!lowspeak_offer(&settings, &lowspeaks()));
        assert_eq!(in_settings(&settings), lowspeaks());
    }
}

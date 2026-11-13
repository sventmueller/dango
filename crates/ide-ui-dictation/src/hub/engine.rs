//! Lowspeak's side of dictation: its recorder, pipeline, models and choices with the app's own keys and log folder,
//! or in the self-test a microphone and an engine that play what the scenario prepared.

use std::sync::Arc;

use lowspeak::dictation::{Choices, Dictation, Event, Host, LOWSPEAK_SUITE};
use lowspeak::models::ApiProvider;
use lowspeak::transcription::postprocess::ActiveApp;

use super::keys::{Keys, Shared};

/// Lowspeak's bundle identifier, which also names the defaults suite its choices are in.
pub const LOWSPEAK_APP: &str = LOWSPEAK_SUITE;

/// A dictation whose events go to `events`, on threads of lowspeak's.
pub fn dictation(keys: Arc<Keys>, events: impl Fn(Event) + Send + Sync + 'static) -> Dictation {
    #[cfg(feature = "selftest")]
    if std::env::var_os("IDE_SELFTEST").is_some() {
        return fake::dictation(keys, events);
    }
    let host = Host {
        keys: Box::new(Shared(keys)),
        log_dir: Some(ide_core::paths::logs_dir().join("dictation")),
        app: ActiveApp {
            bundle_id: Some(ide_core::paths::identity().to_string()),
            name: Some(ide_product::NAME.to_string()),
            process_id: i32::try_from(std::process::id()).ok(),
        },
    };
    Dictation::new(host, events)
}

/// Lowspeak's choices as its menu left them.
pub fn choices() -> Choices {
    #[cfg(feature = "selftest")]
    if std::env::var_os("IDE_SELFTEST").is_some() {
        return fake::choices();
    }
    Choices::of_lowspeak()
}

/// Whether Lowspeak keeps a key for `provider`, found without reading it.
pub fn lowspeak_has_key(provider: ApiProvider) -> bool {
    #[cfg(feature = "selftest")]
    if std::env::var_os("IDE_SELFTEST").is_some() {
        return true;
    }
    lowspeak::dictation::lowspeak_has_key(provider)
}

/// Lowspeak's key for `provider`; macOS asks the user first.
pub fn read_lowspeak_key(provider: ApiProvider) -> Result<Option<String>, String> {
    #[cfg(feature = "selftest")]
    if std::env::var_os("IDE_SELFTEST").is_some() {
        return Ok(Some(format!("sk-selftest-{}", provider.raw_value())));
    }
    lowspeak::dictation::read_lowspeak_key(provider).map_err(|error| format!("{error:#}"))
}

/// The microphone and the engine of the self-test: the microphone opens at once and hears sound, the engine waits and
/// answers with what the scenario queued.
#[cfg(feature = "selftest")]
pub mod fake {
    use std::collections::VecDeque;
    use std::sync::{Arc, Mutex, PoisonError};
    use std::time::Duration;

    use lowspeak::audio::recorder::{OnLive, StartError};
    use lowspeak::config::settings::Settings;
    use lowspeak::dictation::{Choices, Dictation, Event, Parts, Recorder, Samples, Transcriber};
    use lowspeak::models::{Language, LocalModel, OnlineModel, PostProcessingMode, TranscriptionMode};
    use lowspeak::transcription::error::TranscriptionError;
    use lowspeak::transcription::postprocess::Status;
    use lowspeak::transcription::service::{Outcome, Transcript};

    use super::{Keys, Shared};

    /// What one transcription of the self-test brings, after how long.
    pub struct Answer {
        pub after: Duration,
        pub result: Result<String, TranscriptionError>,
    }

    static ANSWERS: Mutex<VecDeque<Answer>> = Mutex::new(VecDeque::new());

    /// Queues what the next transcriptions answer.
    pub fn answer(answers: impl IntoIterator<Item = Answer>) {
        ANSWERS.lock().unwrap_or_else(PoisonError::into_inner).extend(answers);
    }

    pub fn choices() -> Choices {
        Choices {
            settings: Settings {
                mode: TranscriptionMode::Online,
                local_model: LocalModel::ParakeetUltra,
                online_model: OnlineModel::GptTranscribe,
                language: Language::Auto,
                post_processing: PostProcessingMode::Off,
                preprocessing: true,
                start_sound: false,
            },
            microphone: None,
            vocabulary: vec![ide_product::NAME.into(), "lowspeak".into()],
        }
    }

    pub fn dictation(keys: Arc<Keys>, events: impl Fn(Event) + Send + Sync + 'static) -> Dictation {
        let parts = Parts {
            recorder: Box::new(Microphone),
            transcriber: Box::new(Engine),
            choices: Box::new(choices),
            keys: Box::new(Shared(keys)),
            limit: Duration::from_secs(300),
        };
        Dictation::with_parts(parts, events)
    }

    struct Microphone;

    impl Recorder for Microphone {
        fn start(
            &self,
            _device: Option<String>,
            on_live: OnLive,
            started: Box<dyn FnOnce(Result<(), StartError>) + Send>,
        ) {
            started(Ok(()));
            on_live();
        }

        fn stop(&self, stopped: Box<dyn FnOnce(Samples) + Send>) {
            stopped(Box::new(|| Ok(vec![0.0; 16_000])));
        }
    }

    struct Engine;

    impl Transcriber for Engine {
        fn transcribe(&self, _samples: Vec<f32>, _choices: &Choices) -> Result<Outcome, TranscriptionError> {
            let answer = ANSWERS.lock().unwrap_or_else(PoisonError::into_inner).pop_front();
            let Some(answer) = answer else {
                return Ok(Outcome::NoSpeech);
            };
            std::thread::sleep(answer.after);
            answer.result.map(|text| {
                Outcome::Text(Transcript {
                    text,
                    local_fallback: None,
                    post_processing: Status::Skipped,
                })
            })
        }
    }
}

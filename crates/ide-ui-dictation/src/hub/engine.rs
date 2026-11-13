//! Lowspeak's side of dictation: its recorder, pipeline and models with the app's own choices, keys and log folder,
//! or in the self-test a microphone and an engine that play what the scenario prepared.

use std::collections::HashSet;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use lowspeak::app::policy::MAX_HANDS_FREE_RECORDING;
use lowspeak::audio::recorder::Microphone as Device;
use lowspeak::dictation::{Choices, Dictation, Event, LOWSPEAK_SUITE, Microphone, Parts, Pipeline};
use lowspeak::models::{ApiProvider, LocalModel};
use lowspeak::transcription::model_store;
use lowspeak::transcription::postprocess::ActiveApp;

use super::choices;
use super::keys::{Keys, Shared};

/// Lowspeak's bundle identifier, which also names the defaults suite its choices are in.
pub const LOWSPEAK_APP: &str = LOWSPEAK_SUITE;

/// A dictation whose events go to `events`, on threads of lowspeak's.
pub fn dictation(keys: Arc<Keys>, events: impl Fn(Event) + Send + Sync + 'static) -> Dictation {
    #[cfg(feature = "selftest")]
    if std::env::var_os("IDE_SELFTEST").is_some() {
        return fake::dictation(keys, events);
    }
    lowspeak::util::paths::use_log_dir(ide_core::paths::logs_dir().join("dictation"));
    let app = ActiveApp {
        bundle_id: Some(ide_core::paths::identity().to_string()),
        name: Some(ide_product::NAME.to_string()),
        process_id: i32::try_from(std::process::id()).ok(),
    };
    let parts = Parts {
        recorder: Box::new(Microphone::new()),
        transcriber: Box::new(Pipeline::new(Box::new(Shared(Arc::clone(&keys))), app)),
        choices: Box::new(choices::current),
        keys: Box::new(Shared(keys)),
        limit: MAX_HANDS_FREE_RECORDING,
    };
    Dictation::with_parts(parts, events)
}

/// Lowspeak's choices as its menu left them, to take over.
pub fn lowspeak_choices() -> Choices {
    #[cfg(feature = "selftest")]
    if std::env::var_os("IDE_SELFTEST").is_some() {
        return fake::lowspeak_choices();
    }
    Choices::of_lowspeak()
}

/// The input devices connected now.
pub fn microphones() -> Vec<Device> {
    #[cfg(feature = "selftest")]
    if std::env::var_os("IDE_SELFTEST").is_some() {
        return fake::microphones();
    }
    lowspeak::audio::recorder::microphones()
}

/// The local models on this Mac.
pub fn installed_models() -> HashSet<LocalModel> {
    #[cfg(feature = "selftest")]
    if std::env::var_os("IDE_SELFTEST").is_some() {
        return fake::installed_models();
    }
    model_store::installed_models()
}

/// Downloads `model` into the folders Lowspeak uses, reporting the fraction done; ends early once `cancel` is set.
pub fn download(model: LocalModel, cancel: &AtomicBool, progress: &mut dyn FnMut(f64)) -> Result<(), String> {
    #[cfg(feature = "selftest")]
    if std::env::var_os("IDE_SELFTEST").is_some() {
        return fake::download(model, cancel, progress);
    }
    model_store::download(model, cancel, progress).map_err(|error| format!("{error:#}"))
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
        return fake::lowspeak_key(provider);
    }
    lowspeak::dictation::read_lowspeak_key(provider).map_err(|error| format!("{error:#}"))
}

/// The microphone and the engine of the self-test: the microphone opens at once and hears sound unless the scenario
/// queued a slow or refused opening, the engine waits and answers with what the scenario queued.
#[cfg(feature = "selftest")]
pub mod fake {
    use std::collections::{HashSet, VecDeque};
    use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
    use std::sync::{Arc, Mutex, PoisonError};
    use std::time::Duration;

    use lowspeak::audio::recorder::{Microphone as Device, OnLive, StartError};
    use lowspeak::config::settings::{ChosenMicrophone, Settings};
    use lowspeak::dictation::{Choices, Dictation, Event, Parts, Recorder, Samples, Transcriber};
    use lowspeak::models::{ApiProvider, Language, LocalModel, OnlineModel, PostProcessingMode, TranscriptionMode};
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
    static OPENINGS: Mutex<VecDeque<Opening>> = Mutex::new(VecDeque::new());
    static TOO_SHORT: AtomicBool = AtomicBool::new(false);
    static KEY_UNREADABLE: AtomicBool = AtomicBool::new(false);
    static LIMIT_SECONDS: AtomicU64 = AtomicU64::new(300);
    static INSTALLED: Mutex<Option<HashSet<LocalModel>>> = Mutex::new(None);

    /// How the microphone opens for one recording.
    pub enum Opening {
        After(Duration),
        Refused(StartError),
    }

    /// Queues what the next transcriptions answer.
    pub fn answer(answers: impl IntoIterator<Item = Answer>) {
        ANSWERS.lock().unwrap_or_else(PoisonError::into_inner).extend(answers);
    }

    /// Queues how the microphone opens for the next recordings.
    pub fn open(openings: impl IntoIterator<Item = Opening>) {
        OPENINGS.lock().unwrap_or_else(PoisonError::into_inner).extend(openings);
    }

    /// The next recording is too short to transcribe.
    pub fn too_short() {
        TOO_SHORT.store(true, Ordering::SeqCst);
    }

    /// Whether macOS hands Lowspeak's key out.
    pub fn lowspeak_key_readable(readable: bool) {
        KEY_UNREADABLE.store(!readable, Ordering::SeqCst);
    }

    /// The longest recording of the dictations made from now on.
    pub fn limit(limit: Duration) {
        LIMIT_SECONDS.store(limit.as_secs(), Ordering::SeqCst);
    }

    pub fn lowspeak_key(provider: ApiProvider) -> Result<Option<String>, String> {
        if KEY_UNREADABLE.load(Ordering::SeqCst) {
            return Err("the user did not allow it".to_string());
        }
        let prefix = match provider {
            ApiProvider::OpenAi => "sk-",
            ApiProvider::Groq => "gsk_",
        };
        Ok(Some(format!("{prefix}selftest-{}", provider.raw_value())))
    }

    /// What Lowspeak's menu left in the self-test.
    pub fn lowspeak_choices() -> Choices {
        Choices {
            settings: Settings {
                mode: TranscriptionMode::Online,
                local_model: LocalModel::ParakeetUltra,
                online_model: OnlineModel::GroqWhisperLargeV3Turbo,
                language: Language::De,
                post_processing: PostProcessingMode::Off,
                preprocessing: true,
                start_sound: false,
            },
            microphone: Some(ChosenMicrophone {
                id: "selftest:headset".into(),
                name: "Headset".into(),
            }),
            vocabulary: vec![ide_product::NAME.into(), "lowspeak".into()],
        }
    }

    pub fn microphones() -> Vec<Device> {
        vec![Device {
            id: "selftest:usb".into(),
            name: "USB Microphone".into(),
        }]
    }

    pub fn installed_models() -> HashSet<LocalModel> {
        INSTALLED
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .get_or_insert_with(|| HashSet::from([LocalModel::ParakeetUltra]))
            .clone()
    }

    /// Five steps a tenth of a second apart, then the model counts as installed.
    pub fn download(model: LocalModel, cancel: &AtomicBool, progress: &mut dyn FnMut(f64)) -> Result<(), String> {
        for step in 1..=5 {
            std::thread::sleep(Duration::from_millis(100));
            if cancel.load(Ordering::SeqCst) {
                return Err("cancelled".into());
            }
            progress(f64::from(step) / 5.0);
        }
        INSTALLED
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .get_or_insert_with(HashSet::new)
            .insert(model);
        Ok(())
    }

    pub fn dictation(keys: Arc<Keys>, events: impl Fn(Event) + Send + Sync + 'static) -> Dictation {
        let parts = Parts {
            recorder: Box::new(Microphone),
            transcriber: Box::new(Engine),
            choices: Box::new(super::choices::current),
            keys: Box::new(Shared(keys)),
            limit: Duration::from_secs(LIMIT_SECONDS.load(Ordering::SeqCst)),
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
            let opening = OPENINGS.lock().unwrap_or_else(PoisonError::into_inner).pop_front();
            match opening {
                None => {
                    started(Ok(()));
                    on_live();
                }
                Some(Opening::Refused(error)) => started(Err(error)),
                Some(Opening::After(delay)) => {
                    std::thread::spawn(move || {
                        std::thread::sleep(delay);
                        started(Ok(()));
                        on_live();
                    });
                }
            }
        }

        fn stop(&self, stopped: Box<dyn FnOnce(Samples) + Send>) {
            stopped(Box::new(|| Ok(vec![0.0; 16_000])));
        }
    }

    struct Engine;

    impl Transcriber for Engine {
        fn transcribe(&self, _samples: Vec<f32>, _choices: &Choices) -> Result<Outcome, TranscriptionError> {
            if TOO_SHORT.swap(false, Ordering::SeqCst) {
                return Ok(Outcome::TooShort);
            }
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

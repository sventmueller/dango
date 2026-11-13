# Dictation through lowspeak

How Dango dictates into the AI composer and the editor with the speech-to-text of `../lowspeak` (E13, E14, R3,
R4, R6). Lowspeak is a shipped menu bar app; the changes it needs are additive, behind a module of its own, on a
`vibeedit/*` branch, and leave `Lowspeak.app` as it is. Its test suite stays green, and the new module brings tests of
its own (R3).

## What lowspeak has, and what a second app needs

| Part | Today | For Dango |
|---|---|---|
| Recording | `audio::recorder::Recorder` (cpal), driven by `app::capture::Capture` on a thread of its own, since opening a Bluetooth microphone takes up to two seconds; `Captured::into_16k` resamples after the stop | the same thread; `capture` is private to `app` today |
| Pipeline | `transcription::service::run(Job, &Services)` on a worker thread: VAD, preprocessing, online (OpenAI, Groq) with local fallback, or local (Whisper, Parakeet), phantom removal, vocabulary spelling, optional LLM post-processing | used as it is |
| Local models | `transcription::local::LocalTranscriber` owns the loaded model on one thread; `model_store` finds models by path in `~/Library/Application Support/{Lowspeak,FluidAudio}/Models` | the same paths, so models Lowspeak downloaded are used; Dango downloads nothing |
| Settings | `config::settings::Settings::load` and `config::vocabulary::load` read `NSUserDefaults.standardUserDefaults` | in Dango that is Dango's own domain: read the suite `com.lowspeak.app` instead (`Settings::load_from` is public, `vocabulary::load_from` is not) |
| API keys | `Api::SYSTEM` reads the keychain items of service `com.lowspeak.app` | another app reading them makes macOS ask each time, and "Always Allow" would change Lowspeak's item (R4): Dango passes a key store of its own |
| Texts | `i18n::texts(Language)`, 35 languages, picked by the dictation language or the macOS order; `StartError` and `TranscriptionError` have `user_facing_description(&Texts)` | errors in Dango's interface language: a lookup by language tag (the mapping `from_locale` is private) |
| Logs | `util::paths::log_dir()`: `LOWSPEAK_LOG_DIR` or Lowspeak's folder | an own directory without setting an environment variable at run time (`set_var` is unsafe in edition 2024) |
| fn key | `hotkey::Hotkey::start(on_key)` / `stop()`; `app::gesture::FnGesture` turns presses into hold and hands-free | used as they are; `stop` is final, so a new `Hotkey` starts after Lowspeak quits |
| Linking | `build.rs` adds `libclang_rt.osx.a` with `cargo:rustc-link-arg` (whisper.cpp's Metal code calls `__isPlatformVersionAtLeast`) | link arguments reach only lowspeak's own binaries and tests; a dependent program fails to link. `rustc-link-search` plus `rustc-link-lib=static=clang_rt.osx` reach it |
| Quitting | `app::shutdown` unloads the model (whisper.cpp asserts in its exit-time destructors while one is loaded) and `transcription::shutdown` ends retries | the host has to do the same |

There is no streaming: Whisper and Parakeet transcribe the whole recording after it stops. While recording, the UI
shows that it listens (from the first real sound on); afterwards "transcribing" until the text arrives (typically
0.3–2 s). A live level meter is left out: `Recorder::level` computes the RMS of the whole recording so far, too much
work ten times a second for a five-minute recording.

## The `lowspeak::dictation` module

One new file, `src/dictation.rs`, plus small visibility changes. It is the only door Dango uses besides `hotkey`,
`app::gesture` and `i18n`.

```rust
/// The defaults suite Lowspeak keeps its choices in, read and never written by a host.
pub const LOWSPEAK_SUITE: &str = "com.lowspeak.app";

/// What a host app provides.
pub struct Host {
    /// Where the API keys come from: the host's own keychain items, never Lowspeak's.
    pub keys: Box<dyn KeyStore + Send + Sync>,
    /// The host's directory for lowspeak's log files.
    pub log_dir: Option<PathBuf>,
    /// The app dictated into, for post-processing's formatting hint.
    pub app: ActiveApp,
}

/// Where a dictation goes; the host's own number for a text field.
pub type Target = u64;

pub enum Event {
    Started(Target),
    /// The first real sound arrived; never before `Started`.
    Live(Target),
    /// The recording reached its limit (300 s) and is transcribed as it is.
    LimitReached(Target),
    Transcribing(Target),
    /// The text to insert, without the trailing space Lowspeak pastes.
    Text(Target, String),
    Nothing(Target, Nothing),
    /// `Failure::describe(&Texts)` says why in the UI language.
    Failed(Target, Failure),
}

pub enum Failure {
    Microphone(StartError),
    Transcription(TranscriptionError),
}

impl Dictation {
    /// Lowspeak's recorder and pipeline; moves lowspeak's logs to `host.log_dir`.
    pub fn new(host: Host, events: impl Fn(Event) + Send + Sync + 'static) -> Dictation;
    /// Other parts behind the same seam (`Recorder`, `Transcriber`, choices, keys, limit), for tests.
    pub fn with_parts(parts: Parts, events: impl Fn(Event) + Send + Sync + 'static) -> Dictation;
    /// Lowspeak's choices as they are now: mode, models, language, post-processing, microphone, vocabulary.
    pub fn choices(&self) -> Choices;
    /// The provider whose key the next dictation needs and the host's store lacks: the online model's, or with
    /// post-processing on and no key for it, the provider post-processing prefers.
    pub fn missing_key(&self) -> Option<ApiProvider>;
    /// Opens the microphone for `target` and preloads the local model of Lowspeak's choice; `Busy` while a recording
    /// runs or its microphone still opens.
    pub fn start(&self, target: Target) -> Result<(), Busy>;
    pub fn stop(&self, target: Target);
    /// Ends the recording of `target` without text and drops its transcriptions still running; no event follows.
    pub fn cancel(&self, target: Target);
    pub fn recording(&self) -> Option<Target>;
    /// Releases the loaded model within `timeout` and ends the retries of online requests; call it on quit.
    pub fn shutdown(&self, timeout: Duration) -> bool;
}

pub fn texts_for_locale(tag: &str) -> &'static Texts;
/// Asked without reading the key (attributes only), so macOS does not ask the user.
pub fn lowspeak_has_key(provider: ApiProvider) -> bool;
/// macOS asks the user for each read; an error is a refusal or a keychain failure.
pub fn read_lowspeak_key(provider: ApiProvider) -> anyhow::Result<Option<String>>;
```

Inside, `Dictation` reuses `app::capture::Capture` (made `pub(crate)`), `service::run` with `Services { local:
LocalTranscriber, models: Disk, api: Api { keys: host.keys, http: UrlSession, cancel, retry_delay }, lexicon:
SpellChecker }`, and reads `Settings::load_from` and `vocabulary::load_from` (made public) on the suite at each start,
so a change in Lowspeak's menu counts from the next dictation. Results are handed out in recording order with their
target, as `policy::Dictations` does for pastes. Recording limits are Lowspeak's: 300 s, then the recording stops and
is transcribed (`LimitReached`). Panics in the worker become `Failed(Engine)` as in the app.

Other changes in lowspeak:

1. `util::paths::use_log_dir(dir)`: sets the log directory before the first log line; afterwards it changes nothing.
2. `i18n::texts_for_locale` over the existing `from_locale`.
3. `config::api_keys`: `lowspeak_has_key` (an attributes-only `SecItemCopyMatching`: the keychain asks only before
   handing out a secret, which the implementation checks with a test item of another program, never with Lowspeak's
   real items) and `read_lowspeak_key` (the existing keychain read).
4. `build.rs`: `cargo:rustc-link-search=native=<resource dir>/lib/darwin` and `cargo:rustc-link-lib=static=clang_rt.osx`
   instead of the link argument, so every program that links lowspeak gets the runtime.
5. Tests: `tests/dictation.rs` drives `Dictation` with substitutes for the recorder and the model thread (seams like
   `Platform`/`ModelLoader` today): target order, cancel during recording and during transcription, a second start
   while busy, a failing microphone, the limit, texts per locale. No microphone, model, key or network.

`Lowspeak.app` does not call the new module; its behaviour and its tests stay as they are.

## Dango

### Build (R6)
- Cargo feature `dictation`, off by default, which turns on the crate `ide-ui-dictation` with its feature `engine`
  (lowspeak): `make lint`/`make check` and the self-test do not build whisper.cpp, cmake or CoreML. The debug app
  (`cargo xtask run`) and the release bundle (`make build`, xtask's `bundle --release`) turn it on.
- `make check-dictation`: clippy and tests of the app and `ide-ui-dictation` with the feature, for changes that touch
  dictation.
- Without the feature the mic buttons, menu items and the fn handling are not there; `ide-ui-dictation` without
  `engine` holds only the `DictationField` trait and the shortcut's spelling.
- Info.plist gets `NSMicrophoneUsageDescription` (Dango's own words, translated with the other texts in the
  release phase) and the hardened runtime the `com.apple.security.device.audio-input` entitlement, both only in
  bundles built with the feature.
- The first dictation asks macOS for the microphone (`crates/ide-ui-dictation/src/hub/microphone.rs`) and starts
  once the user allows it; a dictation by fn ends with the question, since the key is up by then. Lowspeak's recorder
  never asks, it only reports a missing permission, and macOS lists an app under Privacy & Security > Microphone only
  after it asked.

### Where dictation goes
- A dictation is bound to the text field that had the focus when it started: an AI tab's composer or an editor.
  Switching tabs or windows does not move it; the text goes into that field at its cursor (replacing a selection),
  as one undo step, through the field's normal input path, so the editor's model and the composer's mentions see it.
  A field closed meanwhile gets nothing; the text goes to the clipboard with a notice.
- Starting while another field records stops that recording first (it is transcribed and delivered to its own field).
- Esc in the recording field cancels; a second click on the mic button or the shortcut stops.

### Controls
- AI composer: the mic button starts and stops; red while it records, pulsing while it hears sound (not with Reduce
  Motion), a spinner beside it while transcribing. Without the feature there is no mic button.
- Editor: a mic button in the editor's status bar, the same states.
- Menu Bearbeiten ▸ Diktat starten/beenden with the keymap command `dictation.toggle`, ⌃⌥D (E47; ⌥⌘D is the
  system's Dock shortcut and ⌃⌘D the dictionary lookup): the field with the focus starts or ends; without one, a
  recording elsewhere ends.
- fn (E13, E48): the setting "Mit fn diktieren, solange Lowspeak nicht läuft" is on by default. macOS's
  Accessibility permission, which the event tap needs, is asked for once, after the first dictation by microphone,
  menu or shortcut, never at launch; refused, fn stays off and Settings ▸ Diktat says so and asks again on a click.
  Hold fn records, a double tap locks, as in Lowspeak (`FnGesture`). The tap runs only while no application with the
  bundle id `com.lowspeak.app` runs (`NSWorkspace`'s launch and terminate notifications) and only while Dango is
  the active app: an event tap takes fn from every app, and a dictation into a field the user does not see helps
  nobody. `Hotkey::stop` is final, so each of those times starts a new one. While Lowspeak runs, fn belongs to
  Lowspeak and pastes into the front field the usual way.
- macOS adds its own "Diktat starten …" to every Edit menu; with the feature, `NSDisabledDictationMenuItem` leaves it
  out, so the menu offers one dictation.
- The status line of the AI tab and the editor's status bar say "Höre zu …" / "Transkribiere …"; VoiceOver reads the
  state of the button.

### Keys (R4)
- Dango keeps dictation keys in its own keychain items: service `ide_core::paths::identity()`, account
  `dictation.openai` / `dictation.groq`.
- The first dictation that needs a key Dango lacks (Lowspeak in online mode, or post-processing on without any of
  its keys) asks, once per provider and run:
  - when `lowspeak_has_key` finds Lowspeak's key: "Lowspeak hat einen Schlüssel für OpenAI. Für Dango
    übernehmen?" — Übernehmen reads it once (macOS asks; the dialog explains to choose "Erlauben", not "Immer
    erlauben") and stores it as Dango's own item; Lowspeak's item stays untouched and is never read again;
  - otherwise, or on "Selbst eingeben …": a field for the key;
  - "Nicht jetzt": this dictation falls back to an installed local model if there is one (as Lowspeak does), else it
    fails with lowspeak's "no key" text. The question comes again on the next dictation only after a restart.
- Settings ▸ Diktat shows per provider whether Dango keeps a key (remove it), only Lowspeak does (take it over, with
  the same question as above) or neither (enter one).

### Settings and models
- Mode, models, language, post-processing, microphone and vocabulary are Lowspeak's (suite `com.lowspeak.app`,
  read-only). Settings ▸ Diktat shows them and opens Lowspeak to change them; without Lowspeak installed the defaults
  apply (online, gpt-transcribe, auto language) and local models are offered only once Lowspeak downloaded one.
- Dango downloads and deletes no models.

### Errors, logs, quitting
- Failures show in the field's status line in the interface language: `texts_for_locale` with the language of the
  `Localizable.strings` Dango loaded. No alerts during dictation, as in Lowspeak. Four of lowspeak's texts name
  Lowspeak or its menu where the user acts (no microphone access, no key, model not installed, model retired); for
  those Dango has its own.
- lowspeak's logs go to `<Dango's logs>/dictation/` (`use_log_dir`).
- `applicationWillTerminate` calls `Dictation::shutdown(3 s)`.

### Tests
- No live cloud transcription with the owner's keys; no downloads; local transcription only in `#[ignore]` tests that
  use a model already on disk.
- Unit tests for target binding, the start-while-busy rule and the key questions (a fake key store and a fake
  `lowspeak_has_key`).
- Self-test scenario `dictation` (feature on): a fake engine behind the same seam delivers text; checks insertion at
  the cursor in the editor and the composer, cancel with Esc, the states of the buttons, an error text in the UI
  language, the key question. Pictures in light and dark.

## Decisions
- **E46** The cargo feature `dictation` is on in the debug app (`cargo xtask run`), in release bundles and in the
  `dictation` self-test, off in `make lint`, `make check` and the other scenarios; `make check-dictation` runs clippy and the app's tests with it.
- **E47** The shortcut is ⌃⌥D, keymap command `dictation.toggle`.
- **E48** fn is on by default; the Accessibility permission is asked for only after the first dictation, never at
  launch, and if refused fn stays off with a note in Settings ▸ Diktat, where it can be asked for again.

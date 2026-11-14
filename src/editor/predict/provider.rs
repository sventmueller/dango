//! Which provider answers an editor (E91): the one the settings name, Copilot started once per trusted folder through
//! the supervisor with the login shell's environment (E10, E22, E23) and installed only when the user asks, or a
//! fill-in-the-middle model under Ollama or LM Studio. Also the user's consent for a provider outside the Mac (E101).

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use ide_core::settings::Settings;
use ide_predict::Provider;
use ide_predict::copilot::{self, Account, Copilot};
use ide_process::Supervisor;
use splat_i18n::{t, tf};

use super::model::{self, Choice, Local};
use crate::util::main_soon;

/// The settings edit prediction follows, as last read.
#[derive(Debug, Clone, Default)]
pub struct Chosen {
    pub choice: Choice,
    pub off_for: Vec<String>,
    pub exclude: Vec<String>,
    pub ollama: Option<Local>,
    pub lm_studio: Option<Local>,
    pub consent: Vec<String>,
}

impl Chosen {
    fn load() -> Chosen {
        let settings = Settings::load_user();
        Chosen {
            choice: settings.get(&model::PROVIDER),
            off_for: settings.get(&model::OFF_FOR),
            exclude: settings.get(&model::EXCLUDE),
            ollama: Some(settings.get(&model::OLLAMA)),
            lm_studio: Some(settings.get(&model::LM_STUDIO)),
            consent: settings.get(&model::CONSENT),
        }
    }

    /// The local model the choice names, if it is one.
    pub fn local(&self) -> Option<&Local> {
        match self.choice {
            Choice::Ollama => self.ollama.as_ref(),
            Choice::LmStudio => self.lm_studio.as_ref(),
            Choice::Off | Choice::Copilot => None,
        }
    }

    /// The name the consent for the chosen provider is kept under, when it needs one.
    pub fn consent_id(&self) -> Option<String> {
        model::consent_id(self.choice, self.local())
    }

    /// Whether the chosen provider may get text: it needs no consent, or has it.
    pub fn consented(&self) -> bool {
        self.consent_id().is_none_or(|id| self.consent.contains(&id))
    }
}

/// Copilot for one folder.
pub enum Slot {
    Starting,
    Running(Arc<Copilot>),
    /// Its server is not installed.
    Missing,
    /// Being installed, with how to stop it and what it does now.
    Installing(ide_lsp::Cancel, String),
    Failed(String),
}

/// A local provider with the choice and model it was made for.
type Made = (Choice, Local, Arc<dyn Provider>);

/// The providers of the app and what the editors asked them.
#[derive(Default)]
pub struct Predictions {
    chosen: RefCell<Option<Chosen>>,
    /// Counts questions, so that the answer to an older one is dropped.
    next: Cell<u64>,
    /// The provider of the self-test, in place of the settings'.
    forced: RefCell<Option<Arc<dyn Provider>>>,
    /// The local provider made for the settings it was made from.
    local: RefCell<Option<Made>>,
    copilots: RefCell<HashMap<PathBuf, Slot>>,
    accounts: RefCell<HashMap<PathBuf, Account>>,
    /// Whether this run already asked for consent from an editor, so typing does not ask again and again.
    pub(super) asked_consent: Cell<bool>,
    /// Whether the self-test's provider counts as one outside the Mac.
    forced_outside: Cell<bool>,
    /// The last secrets check of each file (R15).
    scans: super::guard::Scans,
    /// Whether this run already said that gitleaks is missing.
    pub(super) told_missing: Cell<bool>,
}

fn with<R>(f: impl FnOnce(&Predictions) -> R) -> R {
    f(&crate::context::app().predictions)
}

/// The settings as last read.
pub fn chosen() -> Chosen {
    with(|p| p.chosen.borrow_mut().get_or_insert_with(Chosen::load).clone())
}

/// Reads the settings again after they changed, and lets the editors follow.
pub fn settings_changed() {
    with(|p| {
        p.chosen.borrow_mut().take();
        p.local.borrow_mut().take();
    });
    super::follow_settings();
}

/// The id of the next question.
pub fn next_question() -> u64 {
    with(|p| {
        p.next.set(p.next.get() + 1);
        p.next.get()
    })
}

/// Whether requests leave the Mac: the chosen provider needs consent, or the self-test's counts as outside.
pub fn outside() -> bool {
    if forced() {
        return with(|p| p.forced_outside.get());
    }
    chosen().consent_id().is_some()
}

/// The last secrets check of each file.
pub fn scans() -> super::guard::Scans {
    with(|p| p.scans.clone())
}

/// The secrets check with gitleaks from the login shell's PATH, through the supervisor; blocks, for worker threads.
pub fn gitleaks() -> super::guard::Scan {
    use super::guard::Verdict;
    Arc::new(|folder: &Path, text: &str| {
        let Some(environment) = crate::environment::wait(std::time::Duration::from_secs(10)) else {
            return Verdict::Failed(t("Die Umgebung der Login-Shell fehlt.").to_string());
        };
        match ide_git::Git::new(Supervisor::global(), environment.to_vec()).scan_text(folder, text) {
            Ok(ide_git::SecretScan::Clean) => Verdict::Clean,
            Ok(ide_git::SecretScan::NotInstalled) => Verdict::NotInstalled,
            Ok(ide_git::SecretScan::Found(_)) => Verdict::Found,
            Err(error) => Verdict::Failed(error.to_string()),
        }
    })
}

/// Whether the self-test put a provider in place.
pub fn forced() -> bool {
    with(|p| p.forced.borrow().is_some())
}

/// What keeps an editor from proposals, for its status bar.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Blocked {
    Off,
    /// Not set up or not reachable, in the user's words.
    Unavailable(String),
    /// Copilot is not installed; the status bar offers to install it.
    Missing,
    SignIn,
    Consent,
    Busy(String),
}

/// The provider for an editor in `root`, or why there is none now.
pub fn provider(root: &Path) -> Result<Arc<dyn Provider>, Blocked> {
    if let Some(forced) = with(|p| p.forced.borrow().clone()) {
        return Ok(forced);
    }
    let chosen = chosen();
    if chosen.choice == Choice::Off {
        return Err(Blocked::Off);
    }
    if !chosen.consented() {
        return Err(Blocked::Consent);
    }
    match chosen.choice {
        Choice::Copilot => copilot_for(root),
        _ => local(&chosen),
    }
}

fn local(chosen: &Chosen) -> Result<Arc<dyn Provider>, Blocked> {
    with(|p| p.local_for(chosen))
}

fn copilot_for(root: &Path) -> Result<Arc<dyn Provider>, Blocked> {
    match with(|p| p.copilot_answer(root)) {
        Some(answer) => answer,
        None => {
            start(root);
            Err(Blocked::Busy(t("Copilot startet …").to_string()))
        }
    }
}

/// Copilot of `root`, once it runs.
pub fn copilot(root: &Path) -> Option<Arc<Copilot>> {
    with(|p| match p.copilots.borrow().get(root) {
        Some(Slot::Running(copilot)) => Some(copilot.clone()),
        _ => None,
    })
}

/// Where Copilot stands for a folder, for the settings.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CopilotState {
    NotStarted,
    Starting,
    Missing,
    Installing(String),
    Failed(String),
    Running(Option<Account>),
}

/// Where Copilot of `root` stands now.
pub fn copilot_state(root: &Path) -> CopilotState {
    with(|p| p.copilot_state(root))
}

impl Predictions {
    /// The local provider `chosen` names, made once for the settings it was made from.
    fn local_for(&self, chosen: &Chosen) -> Result<Arc<dyn Provider>, Blocked> {
        let Some(local) = chosen.local().cloned() else {
            return Err(Blocked::Off);
        };
        if local.model.trim().is_empty() {
            return Err(Blocked::Unavailable(
                t("Kein Modell gewählt – in den Einstellungen unter „Vorhersage“").to_string(),
            ));
        }
        let mut cell = self.local.borrow_mut();
        if let Some((choice, made, provider)) = cell.as_ref()
            && *choice == chosen.choice
            && *made == local
        {
            return Ok(provider.clone());
        }
        let provider: Arc<dyn Provider> = match chosen.choice {
            Choice::Ollama => Arc::new(ide_predict::fim::Ollama::new(&local.url, &local.model)),
            _ => Arc::new(ide_predict::fim::LmStudio::new(&local.url, &local.model)),
        };
        *cell = Some((chosen.choice, local, provider.clone()));
        Ok(provider)
    }

    /// Copilot of `root` for an editor, or why it cannot answer; `None` while nothing runs or starts for `root`.
    fn copilot_answer(&self, root: &Path) -> Option<Result<Arc<dyn Provider>, Blocked>> {
        let running: Arc<dyn Provider> = match self.copilots.borrow().get(root)? {
            Slot::Running(copilot) => copilot.clone(),
            Slot::Starting => return Some(Err(Blocked::Busy(t("Copilot startet …").to_string()))),
            Slot::Missing => return Some(Err(Blocked::Missing)),
            Slot::Installing(_, progress) => return Some(Err(Blocked::Busy(progress.clone()))),
            Slot::Failed(message) => return Some(Err(Blocked::Unavailable(message.clone()))),
        };
        Some(match self.accounts.borrow().get(root).cloned() {
            Some(Account::SignedIn { .. }) => Ok(running),
            Some(Account::NeedsSignIn) => Err(Blocked::SignIn),
            Some(Account::NoSubscription { user }) => {
                Err(Blocked::Unavailable(tf("Copilot: {} hat kein Copilot-Abo", &[&user])))
            }
            Some(Account::Unavailable(message)) => Err(Blocked::Unavailable(message)),
            Some(Account::Starting) | None => Err(Blocked::Busy(t("Copilot startet …").to_string())),
        })
    }

    fn copilot_state(&self, root: &Path) -> CopilotState {
        let account = self.accounts.borrow().get(root).cloned();
        match self.copilots.borrow().get(root) {
            None => CopilotState::NotStarted,
            Some(Slot::Starting) => CopilotState::Starting,
            Some(Slot::Missing) => CopilotState::Missing,
            Some(Slot::Installing(_, progress)) => CopilotState::Installing(progress.clone()),
            Some(Slot::Failed(message)) => CopilotState::Failed(message.clone()),
            Some(Slot::Running(_)) => CopilotState::Running(account),
        }
    }
}

/// The folders Copilot runs or starts for.
pub fn copilot_roots() -> Vec<PathBuf> {
    with(|p| p.copilots.borrow().keys().cloned().collect())
}

/// Starts Copilot's server for the trusted folder `root`, off the main thread, unless it is missing.
fn start(root: &Path) {
    if !crate::project::is_trusted(root) {
        return;
    }
    with(|p| p.copilots.borrow_mut().insert(root.to_path_buf(), Slot::Starting));
    let root = root.to_path_buf();
    let failed = root.clone();
    if let Err(why) = crate::util::spawn("copilot-start", move || {
        let environment = crate::environment::wait(std::time::Duration::from_secs(10))
            .map(<[_]>::to_vec)
            .unwrap_or_default();
        let Some(program) = copilot::find(&ide_lsp::default_tools(), &environment) else {
            main_soon(move || set_slot(&root, Slot::Missing));
            return;
        };
        let account_root = root.clone();
        let started = Copilot::start(Supervisor::global(), &program, &root, environment, move |account| {
            let root = account_root.clone();
            main_soon(move || account_changed(&root, account));
        });
        main_soon(move || match started {
            Ok(copilot) => set_slot(&root, Slot::Running(Arc::new(copilot))),
            Err(error) => set_slot(
                &root,
                Slot::Failed(tf(
                    "Copilot startet nicht: {}",
                    &[&ide_core::explain::Explain::explain(&error)],
                )),
            ),
        });
    }) {
        set_slot(&failed, Slot::Failed(why.0));
    }
}

fn set_slot(root: &Path, slot: Slot) {
    with(|p| p.copilots.borrow_mut().insert(root.to_path_buf(), slot));
    super::follow_settings();
}

fn account_changed(root: &Path, account: Account) {
    let signed_in = matches!(account, Account::SignedIn { .. });
    with(|p| p.accounts.borrow_mut().insert(root.to_path_buf(), account));
    super::follow_settings();
    crate::settings::prediction_changed();
    if signed_in {
        super::ask_again(root);
    }
}

/// Installs Copilot's server into the app's tools folder, after the user asked for it, then starts it for `root`.
pub fn install(root: &Path) {
    let cancel = ide_lsp::Cancel::new();
    set_slot(root, Slot::Installing(cancel.clone(), t("Installiere …").to_string()));
    let root = root.to_path_buf();
    let failed = root.clone();
    if let Err(why) = crate::util::spawn("copilot-install", move || {
        let environment = crate::environment::get().map(<[_]>::to_vec).unwrap_or_default();
        let installer = ide_lsp::Installer::new(
            Supervisor::global(),
            environment,
            ide_lsp::default_tools(),
            Arc::new(ide_net::Http::new("Copilot installer")),
        );
        let progress_root = root.clone();
        let mut report = move |progress: ide_lsp::Progress| {
            if let ide_lsp::Progress::Output(line) = progress {
                let root = progress_root.clone();
                let line: String = line.chars().take(80).collect();
                main_soon(move || {
                    with(|p| {
                        if let Some(Slot::Installing(_, shown)) = p.copilots.borrow_mut().get_mut(&root) {
                            *shown = tf("Installiere … {}", &[&line]);
                        }
                    });
                    super::follow_settings();
                });
            }
        };
        let result = installer.install(copilot::INSTALL, &cancel, &mut report);
        main_soon(move || match result {
            Ok(_) => {
                with(|p| p.copilots.borrow_mut().remove(&root));
                start(&root);
                super::follow_settings();
            }
            Err(error) => {
                crate::diag::report("installing Copilot", error.to_string());
                set_slot(
                    &root,
                    Slot::Failed(tf(
                        "Installation fehlgeschlagen: {}",
                        &[&ide_core::explain::Explain::explain(&error)],
                    )),
                );
            }
        });
    }) {
        set_slot(&failed, Slot::Failed(why.0));
    }
}

/// Stops the installation of Copilot for `root`, if one runs.
pub fn cancel_install(root: &Path) {
    with(|p| {
        if let Some(Slot::Installing(cancel, _)) = p.copilots.borrow().get(root) {
            cancel.cancel();
        }
    });
}

/// Starts Copilot for `root` if the settings choose it and nothing runs there yet, so its state can show.
pub fn wake(root: &Path) {
    if chosen().choice == Choice::Copilot && with(|p| !p.copilots.borrow().contains_key(root)) {
        start(root);
    }
}

/// Keeps the user's consent for the chosen provider.
pub fn consent() {
    let Some(id) = chosen().consent_id() else { return };
    let mut settings = Settings::load_user();
    let mut all = settings.get(&model::CONSENT);
    if !all.contains(&id) {
        all.push(id);
        if let Err(error) = settings.set(&model::CONSENT, &all) {
            crate::diag::report("saving the consent for edit prediction", error);
        }
    }
    settings_changed();
}

/// Takes back every consent given.
pub fn revoke_all() {
    let mut settings = Settings::load_user();
    if let Err(error) = settings.set(&model::CONSENT, &Vec::new()) {
        crate::diag::report("taking back the consent for edit prediction", error);
    }
    with(|p| p.asked_consent.set(false));
    settings_changed();
}

/// The provider's name for the user.
pub fn name(choice: Choice) -> &'static str {
    match choice {
        Choice::Off => t("Aus"),
        Choice::Copilot => "GitHub Copilot",
        Choice::Ollama => "Ollama",
        Choice::LmStudio => "LM Studio",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn chosen(choice: Choice, model: &str) -> Chosen {
        let local = Local {
            url: "http://127.0.0.1:11434".into(),
            model: model.into(),
        };
        Chosen {
            choice,
            ollama: Some(local.clone()),
            lm_studio: Some(local),
            ..Chosen::default()
        }
    }

    #[test]
    fn a_local_model_is_made_once_for_its_settings_and_anew_when_they_change() {
        let predictions = Predictions::default();
        let first = predictions.local_for(&chosen(Choice::Ollama, "qwen")).unwrap();
        let again = predictions.local_for(&chosen(Choice::Ollama, "qwen")).unwrap();
        assert!(Arc::ptr_eq(&first, &again));
        let other = predictions.local_for(&chosen(Choice::Ollama, "llama")).unwrap();
        assert!(!Arc::ptr_eq(&first, &other));
        let studio = predictions.local_for(&chosen(Choice::LmStudio, "llama")).unwrap();
        assert_ne!(studio.name(), other.name());
    }

    #[test]
    fn without_a_local_model_there_is_nothing_to_ask() {
        let predictions = Predictions::default();
        let unnamed = predictions.local_for(&chosen(Choice::Ollama, "  ")).err();
        assert_eq!(
            unnamed,
            Some(Blocked::Unavailable(
                t("Kein Modell gewählt – in den Einstellungen unter „Vorhersage“").to_string()
            ))
        );
        assert_eq!(
            predictions.local_for(&chosen(Choice::Copilot, "x")).err(),
            Some(Blocked::Off)
        );
    }

    #[test]
    fn copilot_says_why_it_cannot_answer_yet() {
        let predictions = Predictions::default();
        let root = Path::new("/project");
        assert!(predictions.copilot_answer(root).is_none());
        assert_eq!(predictions.copilot_state(root), CopilotState::NotStarted);
        let cases = [
            (
                Slot::Starting,
                Blocked::Busy(t("Copilot startet …").to_string()),
                CopilotState::Starting,
            ),
            (Slot::Missing, Blocked::Missing, CopilotState::Missing),
            (
                Slot::Installing(ide_lsp::Cancel::new(), "40 %".into()),
                Blocked::Busy("40 %".into()),
                CopilotState::Installing("40 %".into()),
            ),
            (
                Slot::Failed("no node".into()),
                Blocked::Unavailable("no node".into()),
                CopilotState::Failed("no node".into()),
            ),
        ];
        for (slot, blocked, state) in cases {
            predictions.copilots.borrow_mut().insert(root.to_path_buf(), slot);
            assert_eq!(predictions.copilot_answer(root).map(Result::err), Some(Some(blocked)));
            assert_eq!(predictions.copilot_state(root), state);
        }
    }

    #[test]
    fn consent_is_needed_only_for_a_provider_outside_the_mac() {
        let local = chosen(Choice::Ollama, "qwen");
        assert_eq!(local.consent_id(), None);
        assert!(local.consented());
        let mut copilot = chosen(Choice::Copilot, "");
        assert_eq!(copilot.local(), None);
        assert!(!copilot.consented());
        copilot.consent = vec!["copilot".into()];
        assert!(copilot.consented());
        assert_eq!(name(Choice::LmStudio), "LM Studio");
        assert_eq!(name(Choice::Off), t("Aus"));
    }
}

#[cfg(feature = "selftest")]
mod selftest_probe;
#[cfg(feature = "selftest")]
pub use selftest_probe::*;

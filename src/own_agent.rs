//! The own agent's settings and what a thread of it starts with (E6): the endpoints with their keys in the keychain,
//! the default model, the user's prices, the rules stored per project and the folders its sandbox allows.
//!
//! Rules live in the user's settings, keyed by project, never in the project folder: a repository must not be able
//! to allow its own commands.

mod words;

use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Duration;

use ide_agent::{
    ApiKey, Approval, Config, Endpoint, Http, Keychain, Memory, ModelInfo, Policy, PriceTable, Profile, Provider,
    ProviderKind, Rule, SecretStore, Workspace,
};
use ide_core::settings::{Key, Settings};
use ide_process::Supervisor;
use ide_thread::ThreadId;
use serde::{Deserialize, Serialize};

use crate::util::main_soon;

pub use words::{load_error, provider_error, rule_text};

/// The endpoints the user added, and the addresses they gave the services, by endpoint id.
pub const ENDPOINTS: Key<Vec<Endpoint>> = Key::user("agentEndpoints", Vec::new);
/// The model new threads start with.
pub const MODEL: Key<Option<ModelChoice>> = Key::user("agentModel", || None);
/// How hard the model thinks in new threads; the provider's default when not set.
pub const EFFORT: Key<Option<String>> = Key::user("agentEffort", || None);
/// The prices the user entered.
pub const PRICES: Key<PriceTable> = Key::user("agentPrices", PriceTable::default);
/// The rules the user stored, by project folder.
pub const RULES: Key<BTreeMap<String, Vec<Rule>>> = Key::user("agentRules", BTreeMap::new);
/// Folders besides the project that the sandbox lets the agent reach.
pub const ALLOWED: Key<Vec<PathBuf>> = Key::user("agentAllowedFolders", Vec::new);

/// The effort levels offered, besides the provider's default.
pub const EFFORTS: [&str; 5] = ["low", "medium", "high", "xhigh", "max"];

/// An endpoint and one of its models.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModelChoice {
    /// The endpoint's id.
    pub endpoint: String,
    /// The model's id.
    pub model: String,
}

/// The services with the addresses the user gave them, then the endpoints the user added.
pub fn endpoints() -> Vec<Endpoint> {
    let stored = Settings::load_user().get(&ENDPOINTS);
    let mut out: Vec<Endpoint> = ProviderKind::ALL
        .iter()
        .filter_map(|kind| Endpoint::preset(*kind))
        .map(|preset| stored.iter().find(|e| e.id == preset.id).cloned().unwrap_or(preset))
        .collect();
    out.extend(
        stored
            .into_iter()
            .filter(|e| ProviderKind::from_id(&e.id).is_none_or(|kind| kind.default_base().is_none())),
    );
    out
}

/// The endpoint with `id`.
pub fn endpoint(id: &str) -> Option<Endpoint> {
    endpoints().into_iter().find(|e| e.id == id)
}

/// Keeps `endpoint`: an added one, or a service at another address.
pub fn save_endpoint(endpoint: Endpoint) {
    let mut settings = Settings::load_user();
    let mut stored = settings.get(&ENDPOINTS);
    stored.retain(|e| e.id != endpoint.id);
    stored.push(endpoint);
    if let Err(error) = settings.set(&ENDPOINTS, &stored) {
        crate::diag::report("saving the agent's endpoints", error);
    }
}

/// Forgets the endpoint `id` the user added with its key, or the address the user gave a service, which keeps its key.
pub fn remove_endpoint(id: &str) {
    if ProviderKind::from_id(id).is_none()
        && let Some(endpoint) = endpoint(id)
        && let Err(error) = secrets().delete(&ide_agent::keys::account(&endpoint))
    {
        crate::diag::report("removing an agent key", error);
    }
    let mut settings = Settings::load_user();
    let mut stored = settings.get(&ENDPOINTS);
    stored.retain(|e| e.id != id);
    if let Err(error) = settings.set(&ENDPOINTS, &stored) {
        crate::diag::report("saving the agent's endpoints", error);
    }
}

/// A new id for an endpoint the user adds.
pub fn new_endpoint_id() -> String {
    let taken: Vec<String> = endpoints().into_iter().map(|e| e.id).collect();
    (1..)
        .map(|n| format!("custom-{n}"))
        .find(|id| !taken.contains(id))
        .unwrap_or_default()
}

/// Where the keys are: the keychain, or memory in the self-test.
pub fn secrets() -> Arc<dyn SecretStore> {
    static STORE: OnceLock<Arc<dyn SecretStore>> = OnceLock::new();
    STORE
        .get_or_init(|| {
            if cfg!(feature = "selftest") && std::env::var_os("IDE_SELFTEST").is_some() {
                Arc::new(Memory::default())
            } else {
                let service = objc2_foundation::NSBundle::mainBundle()
                    .bundleIdentifier()
                    .map_or_else(|| ide_core::paths::identity().to_string(), |id| id.to_string());
                Arc::new(Keychain::new(&service))
            }
        })
        .clone()
}

/// The key kept for `endpoint`.
pub fn key(endpoint: &Endpoint) -> Option<ApiKey> {
    ide_agent::keys::key_for(&*secrets(), endpoint).unwrap_or_else(|error| {
        crate::diag::report("reading an agent key", error);
        None
    })
}

/// Keeps `text` as the key of `endpoint`, or forgets its key when `text` is blank.
pub fn set_key(endpoint: &Endpoint, text: &str) -> Result<(), String> {
    let account = ide_agent::keys::account(endpoint);
    let result = match ApiKey::new(text) {
        Some(key) => secrets().set(&account, key.expose()),
        None => secrets().delete(&account),
    };
    forget_models(&endpoint.id);
    result
}

/// Whether `endpoint` can be asked: it has a key, or needs none.
pub fn usable(endpoint: &Endpoint) -> bool {
    !endpoint.kind.takes_key() || key(endpoint).is_some() || !endpoint.is_first_party()
}

fn models_cache() -> &'static Mutex<HashMap<String, Vec<ModelInfo>>> {
    static CACHE: OnceLock<Mutex<HashMap<String, Vec<ModelInfo>>>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

/// The models `endpoint` listed when last asked.
pub fn models(endpoint: &str) -> Vec<ModelInfo> {
    models_cache()
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .get(endpoint)
        .cloned()
        .unwrap_or_default()
}

fn forget_models(endpoint: &str) {
    models_cache()
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .remove(endpoint);
}

/// Asks `endpoint` for its models off the main thread and tells `done` on it.
pub fn load_models(endpoint: Endpoint, done: impl FnOnce(Result<Vec<ModelInfo>, String>) + Send + 'static) {
    let key = key(&endpoint);
    let _ = std::thread::Builder::new().name("agent-models".into()).spawn(move || {
        let result = Provider::new(endpoint.clone(), key, Http::new()).models();
        if let Ok(models) = &result {
            models_cache()
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .insert(endpoint.id.clone(), models.clone());
        }
        main_soon(move || done(result.map_err(|e| provider_error(&e))));
    });
}

/// The model a new thread starts with: the user's choice, else the first usable service's own default.
pub fn default_choice() -> Option<ModelChoice> {
    if let Some(choice) = Settings::load_user().get(&MODEL)
        && endpoint(&choice.endpoint).is_some()
    {
        return Some(choice);
    }
    endpoints()
        .into_iter()
        .filter(|e| e.is_first_party() && e.kind.takes_key() && key(e).is_some())
        .find_map(|e| {
            Some(ModelChoice {
                model: e.kind.default_model()?.to_string(),
                endpoint: e.id,
            })
        })
}

/// The rules stored for the project at `root`.
pub fn rules(root: &Path) -> Vec<Rule> {
    Settings::load_user()
        .get(&RULES)
        .remove(&root.display().to_string())
        .unwrap_or_default()
}

/// Keeps `rule` for the project at `root`.
pub fn add_rule(root: &Path, rule: Rule) {
    change_rules(|all| {
        let rules = all.entry(root.display().to_string()).or_default();
        if !rules.contains(&rule) {
            rules.push(rule);
        }
    });
}

/// Forgets the rule at `index` of the project `project`.
pub fn remove_rule(project: &str, index: usize) {
    change_rules(|all| {
        if let Some(rules) = all.get_mut(project)
            && index < rules.len()
        {
            rules.remove(index);
            if rules.is_empty() {
                all.remove(project);
            }
        }
    });
}

fn change_rules(change: impl FnOnce(&mut BTreeMap<String, Vec<Rule>>)) {
    let mut settings = Settings::load_user();
    let mut all = settings.get(&RULES);
    change(&mut all);
    if let Err(error) = settings.set(&RULES, &all) {
        crate::diag::report("saving the agent's rules", error);
    }
}

/// Where a thread's conversation is kept.
pub fn store_path(thread: &ThreadId) -> PathBuf {
    ide_core::paths::data_dir()
        .join("agent-threads")
        .join(format!("{}.json", thread.as_str()))
}

/// The folders the sandbox lets the agent reach besides the project: the temporary folders and the user's.
fn allowed_folders(environment: &[(String, String)]) -> Vec<PathBuf> {
    let temporary = environment
        .iter()
        .find(|(name, _)| name == "TMPDIR")
        .map_or_else(std::env::temp_dir, |(_, value)| PathBuf::from(value));
    let mut folders = vec![temporary, PathBuf::from("/private/tmp")];
    folders.extend(Settings::load_user().get(&ALLOWED));
    folders
}

/// What a thread of `choice` in `root` starts with.
pub fn config(thread: ThreadId, root: &Path, choice: &ModelChoice, sandbox: bool) -> Result<Config, String> {
    let endpoint = endpoint(&choice.endpoint)
        .ok_or_else(|| splat_i18n::tf("Den Endpunkt „{}“ gibt es nicht mehr.", &[&choice.endpoint]))?;
    let environment = crate::environment::wait(Duration::from_secs(15))
        .map(<[(String, String)]>::to_vec)
        .unwrap_or_default();
    let http = Http::new();
    let workspace = Workspace::new(root, environment.clone(), Supervisor::global().clone(), http.clone())
        .map_err(|e| format!("{}: {e}", root.display()))?
        .allow(&allowed_folders(&environment))
        .with_diagnostics(crate::lsp::agent_diagnostics(root));
    let listed = models(&endpoint.id).into_iter().find(|m| m.id == choice.model);
    let settings = Settings::load_user();
    Ok(Config {
        store: Some(store_path(&thread)),
        thread,
        provider: Provider::new(endpoint.clone(), key(&endpoint), http),
        model: choice.model.clone(),
        effort: settings.get(&EFFORT),
        workspace,
        policy: Policy {
            profile: Profile::Write,
            approval: Approval::Manual,
            sandbox,
            rules: rules(root),
        },
        prices: settings.get(&PRICES),
        listed_price: listed.as_ref().and_then(|m| m.price),
        context_size: listed.as_ref().and_then(|m| m.context),
        max_tokens: None,
        wording: Arc::new(words::Words),
    })
}

/// The name of an endpoint's kind, for the user.
pub fn kind_name(kind: ProviderKind) -> String {
    match kind.brand() {
        Some(brand) => brand.to_string(),
        None if kind == ProviderKind::OpenAiCompatible => splat_i18n::t("OpenAI-kompatibel").to_string(),
        None => splat_i18n::t("Anthropic-kompatibel").to_string(),
    }
}

//! The own agent's settings and what a thread of it starts with (E6): the endpoints with their keys in the keychain,
//! the default model, the user's prices, the rules stored per project and the folders its sandbox allows.
//!
//! Rules live in the user's settings, keyed by project, never in the project folder: a repository must not be able
//! to allow its own commands.

mod words;

use ide_core::LockIgnoringPoison;
use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Duration;

use ide_agent::{
    ApiKey, Approval, Config, Endpoint, Http, Memory, ModelInfo, Policy, PriceTable, Profile, Provider, ProviderKind,
    Rule, SecretStore, Workspace,
};
use ide_core::settings::{Key, Settings};
use ide_process::Supervisor;
use ide_thread::ThreadId;

use ide_ui_common::util::main_soon;

pub use words::{destructive_text, load_error, provider_error, rule_text};

/// The endpoints the user added, and the addresses they gave the services, by endpoint id.
pub const ENDPOINTS: Key<Vec<Endpoint>> = Key::user("agentEndpoints", ide_core::settings::Sharing::PerMac, Vec::new);
/// The model new threads start with.
pub const MODEL: Key<Option<ModelChoice>> = Key::user("agentModel", ide_core::settings::Sharing::Shared, || None);
/// How hard the model thinks in new threads; the provider's default when not set.
pub const EFFORT: Key<Option<String>> = Key::user("agentEffort", ide_core::settings::Sharing::Shared, || None);
/// The prices the user entered.
pub const PRICES: Key<PriceTable> = Key::user("agentPrices", ide_core::settings::Sharing::Shared, PriceTable::default);
/// The rules the user stored, by project folder.
pub const RULES: Key<BTreeMap<String, Vec<Rule>>> =
    Key::user("agentRules", ide_core::settings::Sharing::PerMac, BTreeMap::new);
/// Folders besides the project that the sandbox lets the agent reach.
pub const ALLOWED: Key<Vec<PathBuf>> = Key::user("agentAllowedFolders", ide_core::settings::Sharing::PerMac, Vec::new);

/// The effort levels offered, besides the provider's default.
pub const EFFORTS: [&str; 5] = ["low", "medium", "high", "xhigh", "max"];

pub use ide_agent::ModelChoice;

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
        ide_ui_common::diag::report("saving the agent's endpoints", error);
    }
}

/// Forgets the endpoint `id` the user added with its key, or the address the user gave a service, which keeps its key.
pub fn remove_endpoint(id: &str) {
    if ProviderKind::from_id(id).is_none()
        && let Some(endpoint) = endpoint(id)
        && let Err(error) = secrets().delete(&ide_agent::keys::account(&endpoint))
    {
        ide_ui_common::diag::report("removing an agent key", error);
    }
    let mut settings = Settings::load_user();
    let mut stored = settings.get(&ENDPOINTS);
    stored.retain(|e| e.id != id);
    if let Err(error) = settings.set(&ENDPOINTS, &stored) {
        ide_ui_common::diag::report("saving the agent's endpoints", error);
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

static STORE: OnceLock<Arc<dyn SecretStore>> = OnceLock::new();

/// Keeps the keys in `store`, the app's one store of secrets; once, before the first key is read.
pub fn use_secrets(store: Arc<dyn SecretStore>) {
    let _ = STORE.set(store);
}

/// Where the keys are: the store the app handed over, or memory where it handed none, as in unit tests.
pub fn secrets() -> Arc<dyn SecretStore> {
    STORE.get_or_init(|| Arc::new(Memory::default())).clone()
}

/// The key kept for `endpoint`.
pub fn key(endpoint: &Endpoint) -> Option<ApiKey> {
    ide_agent::keys::key_for(&*secrets(), endpoint).unwrap_or_else(|error| {
        ide_ui_common::diag::report("reading an agent key", error);
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
    result.map_err(|e| e.to_string())
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
        .lock_ignoring_poison()
        .get(endpoint)
        .cloned()
        .unwrap_or_default()
}

fn forget_models(endpoint: &str) {
    models_cache().lock_ignoring_poison().remove(endpoint);
}

/// Asks `endpoint` for its models off the main thread and tells `done` on it.
pub fn load_models(endpoint: Endpoint, done: impl FnOnce(Result<Vec<ModelInfo>, String>) + Send + 'static) {
    let key = key(&endpoint);
    if let Err((done, why)) = ide_ui_common::util::spawn_with("agent-models", done, move |done| {
        let result = Provider::new(endpoint.clone(), key, Http::new()).models();
        if let Ok(models) = &result {
            models_cache()
                .lock_ignoring_poison()
                .insert(endpoint.id.clone(), models.clone());
        }
        main_soon(move || done(result.map_err(|e| provider_error(&e))));
    }) {
        done(Err(why));
    }
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
    change_rules(|all| keep_rule(all, &root.display().to_string(), rule));
}

/// Forgets the rule at `index` of the project `project`.
pub fn remove_rule(project: &str, index: usize) {
    change_rules(|all| drop_rule(all, project, index));
}

fn keep_rule(all: &mut BTreeMap<String, Vec<Rule>>, project: &str, rule: Rule) {
    let rules = all.entry(project.to_string()).or_default();
    if !rules.contains(&rule) {
        rules.push(rule);
    }
}

fn drop_rule(all: &mut BTreeMap<String, Vec<Rule>>, project: &str, index: usize) {
    if let Some(rules) = all.get_mut(project)
        && index < rules.len()
    {
        rules.remove(index);
        if rules.is_empty() {
            all.remove(project);
        }
    }
}

fn change_rules(change: impl FnOnce(&mut BTreeMap<String, Vec<Rule>>)) {
    let mut settings = Settings::load_user();
    let mut all = settings.get(&RULES);
    change(&mut all);
    if let Err(error) = settings.set(&RULES, &all) {
        ide_ui_common::diag::report("saving the agent's rules", error);
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
pub fn config(
    thread: ThreadId,
    root: &Path,
    choice: &ModelChoice,
    sandbox: bool,
    diagnostics: Arc<dyn ide_agent::Diagnostics>,
) -> Result<Config, String> {
    let endpoint = endpoint(&choice.endpoint)
        .ok_or_else(|| splat_i18n::tf("Den Endpunkt „{}“ gibt es nicht mehr.", &[&choice.endpoint]))?;
    let environment = ide_ui_common::window::environment(Duration::from_secs(15));
    let http = Http::new();
    let workspace = Workspace::new(root, environment.clone(), Supervisor::global().clone(), http.clone())
        .map_err(|e| format!("{}: {e}", root.display()))?
        .allow(&allowed_folders(&environment))
        .with_diagnostics(diagnostics);
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

#[cfg(test)]
mod tests {
    use super::*;

    fn command(prefix: &str) -> Rule {
        Rule::Command { prefix: prefix.into() }
    }

    #[test]
    fn a_rule_is_kept_once_per_project() {
        let mut all = BTreeMap::new();
        keep_rule(&mut all, "/p", command("cargo test"));
        keep_rule(&mut all, "/p", command("cargo test"));
        keep_rule(&mut all, "/p", Rule::Edits);
        keep_rule(&mut all, "/q", command("cargo test"));
        assert_eq!(all["/p"], vec![command("cargo test"), Rule::Edits]);
        assert_eq!(all["/q"], vec![command("cargo test")]);
    }

    #[test]
    fn removing_a_project_last_rule_forgets_the_project_and_a_wrong_index_changes_nothing() {
        let mut all = BTreeMap::new();
        keep_rule(&mut all, "/p", command("cargo test"));
        keep_rule(&mut all, "/p", Rule::Edits);
        drop_rule(&mut all, "/p", 5);
        drop_rule(&mut all, "/other", 0);
        assert_eq!(all["/p"].len(), 2);
        drop_rule(&mut all, "/p", 0);
        assert_eq!(all["/p"], vec![Rule::Edits]);
        drop_rule(&mut all, "/p", 0);
        assert!(all.is_empty());
    }

    #[test]
    fn a_kind_is_named_by_its_brand_else_by_the_protocol_it_speaks() {
        assert_eq!(kind_name(ProviderKind::Gemini), "Google Gemini");
        assert_eq!(kind_name(ProviderKind::OpenAiCompatible), "OpenAI-kompatibel");
        assert_eq!(kind_name(ProviderKind::AnthropicCompatible), "Anthropic-kompatibel");
    }
}

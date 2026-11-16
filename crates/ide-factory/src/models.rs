//! The models and efforts a ranked list's entry can name, and the model an agent's CLI takes when the entry names none,
//! read from what the CLIs keep on the Mac; the app reads the files and hands their texts in.
//!
//! ```
//! use ide_factory::models::{Sources, catalog};
//! use ide_factory::routing::Agent;
//!
//! let sources = Sources {
//!     claude_settings: vec![r#"{"model": "opus"}"#.into()],
//!     ..Sources::default()
//! };
//! let claude = catalog(&Agent::Claude, &sources);
//! assert_eq!(claude.default_model.as_deref(), Some("opus"));
//! assert!(claude.models.iter().any(|m| m.value == "sonnet"));
//! ```

use serde_json::Value;

use crate::routing::Agent;

/// The aliases Claude Code takes for the newest model of each family, offered until a session told its models.
const CLAUDE_MODELS: [&str; 3] = ["opus", "sonnet", "haiku"];

/// The efforts Claude Code's `--effort` takes.
const CLAUDE_EFFORTS: [&str; 5] = ["low", "medium", "high", "xhigh", "max"];

/// What the CLIs keep that names their models, as text.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Sources {
    /// The models Claude Code offered in its last session, each with its efforts.
    pub claude_models: Vec<(String, Vec<String>)>,
    /// `ANTHROPIC_MODEL` of the login shell.
    pub claude_env_model: Option<String>,
    /// Claude Code's settings files, the one that wins first: the project's local and shared ones, then the user's.
    pub claude_settings: Vec<String>,
    /// Codex's `config.toml`.
    pub codex_config: Option<String>,
    /// Codex's `models_cache.json`, the models its account offers.
    pub codex_models: Option<String>,
}

/// A model an entry can name.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModelOption {
    /// What the entry stores and the CLI takes.
    pub value: String,
    /// The efforts the model offers; empty where the CLI does not say.
    pub efforts: Vec<String>,
}

/// What an agent offers for an entry.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Catalog {
    /// The models, in the CLI's order.
    pub models: Vec<ModelOption>,
    /// The efforts of every model, for an entry without a model.
    pub efforts: Vec<String>,
    /// The model the CLI takes without one, where its settings name it.
    pub default_model: Option<String>,
    /// The effort the CLI takes without one, where its settings name it.
    pub default_effort: Option<String>,
}

impl Catalog {
    /// The efforts that go with `model`; every effort the agent knows for an unknown or no model.
    pub fn efforts_of(&self, model: Option<&str>) -> &[String] {
        let model = model.or(self.default_model.as_deref());
        self.models
            .iter()
            .find(|m| Some(m.value.as_str()) == model && !m.efforts.is_empty())
            .map_or(&self.efforts, |m| &m.efforts)
    }
}

/// What `agent` offers, from `sources`.
pub fn catalog(agent: &Agent, sources: &Sources) -> Catalog {
    match agent {
        Agent::Claude => claude(sources),
        Agent::Codex => codex(sources),
        _ => Catalog::default(),
    }
}

fn claude(sources: &Sources) -> Catalog {
    let efforts: Vec<String> = CLAUDE_EFFORTS.map(String::from).to_vec();
    let settings: Vec<Value> = sources
        .claude_settings
        .iter()
        .filter_map(|text| serde_json::from_str(text).ok())
        .collect();
    let named = |value: Option<&Value>| Some(value?.as_str()?.trim().to_string()).filter(|s| !s.is_empty());
    // The CLI takes ANTHROPIC_MODEL before the setting "model", and the settings' "env" sets it too.
    let model = named(
        sources
            .claude_env_model
            .as_ref()
            .map(|m| Value::from(m.as_str()))
            .as_ref(),
    )
    .or_else(|| {
        settings
            .iter()
            .find_map(|s| named(s.get("env").and_then(|e| e.get("ANTHROPIC_MODEL"))))
    })
    .or_else(|| settings.iter().find_map(|s| named(s.get("model"))));
    // The CLI lists "default" for whatever the account defaults to, which an entry without a model already means.
    let offered: Vec<ModelOption> = sources
        .claude_models
        .iter()
        .filter(|(value, _)| value != "default")
        .map(|(value, offered)| ModelOption {
            value: value.clone(),
            efforts: offered.clone(),
        })
        .collect();
    let models = if offered.is_empty() {
        CLAUDE_MODELS
            .iter()
            .map(|value| ModelOption {
                value: (*value).to_string(),
                efforts: efforts.clone(),
            })
            .collect()
    } else {
        offered
    };
    Catalog {
        models,
        default_model: model,
        default_effort: None,
        efforts,
    }
}

fn codex(sources: &Sources) -> Catalog {
    let mut catalog = Catalog::default();
    let cache: Option<Value> = sources
        .codex_models
        .as_deref()
        .and_then(|t| serde_json::from_str(t).ok());
    let listed = cache
        .as_ref()
        .and_then(|c| c.get("models")?.as_array().cloned())
        .unwrap_or_default();
    for model in listed {
        let Some(value) = model.get("slug").and_then(Value::as_str) else {
            continue;
        };
        if model.get("visibility").and_then(Value::as_str) == Some("hide") {
            continue;
        }
        let efforts: Vec<String> = model
            .get("supported_reasoning_levels")
            .and_then(Value::as_array)
            .map(|levels| {
                levels
                    .iter()
                    .filter_map(|l| l.get("effort").and_then(Value::as_str).map(String::from))
                    .collect()
            })
            .unwrap_or_default();
        for effort in &efforts {
            if !catalog.efforts.contains(effort) {
                catalog.efforts.push(effort.clone());
            }
        }
        catalog.models.push(ModelOption {
            value: value.to_string(),
            efforts,
        });
    }
    let config = sources
        .codex_config
        .as_deref()
        .and_then(|t| t.parse::<toml_edit::DocumentMut>().ok());
    let read = |key: &str| {
        config
            .as_ref()
            .and_then(|doc| doc.get(key)?.as_str().map(String::from))
            .filter(|s| !s.trim().is_empty())
    };
    catalog.default_model = read("model");
    catalog.default_effort = read("model_reasoning_effort");
    catalog
}

#[cfg(test)]
mod tests {
    use super::*;

    const CACHE: &str = r#"{"models": [
        {"slug": "gpt-a", "visibility": "list",
         "supported_reasoning_levels": [{"effort": "low"}, {"effort": "high"}]},
        {"slug": "gpt-hidden", "visibility": "hide"},
        {"slug": "gpt-b", "visibility": "list", "supported_reasoning_levels": [{"effort": "max"}]}
    ]}"#;

    #[test]
    fn codex_offers_the_listed_models_of_its_cache_and_its_configured_default() {
        let sources = Sources {
            codex_config: Some("model = \"gpt-b\"\nmodel_reasoning_effort = \"high\"\n[profiles.x]\n".into()),
            codex_models: Some(CACHE.into()),
            ..Sources::default()
        };
        let catalog = catalog(&Agent::Codex, &sources);
        let values: Vec<&str> = catalog.models.iter().map(|m| m.value.as_str()).collect();
        assert_eq!(values, ["gpt-a", "gpt-b"]);
        assert_eq!(catalog.default_model.as_deref(), Some("gpt-b"));
        assert_eq!(catalog.default_effort.as_deref(), Some("high"));
        assert_eq!(catalog.efforts_of(Some("gpt-a")), ["low", "high"]);
        assert_eq!(catalog.efforts_of(None), ["max"]);
        assert_eq!(catalog.efforts_of(Some("unknown")), ["low", "high", "max"]);
    }

    #[test]
    fn claude_takes_the_login_shells_model_then_the_settings_env_then_their_model() {
        let mut sources = Sources {
            claude_settings: vec!["{}".into(), "not json".into(), r#"{"model": "sonnet"}"#.into()],
            ..Sources::default()
        };
        assert_eq!(
            catalog(&Agent::Claude, &sources).default_model.as_deref(),
            Some("sonnet")
        );
        sources
            .claude_settings
            .push(r#"{"env": {"ANTHROPIC_MODEL": "haiku"}}"#.into());
        assert_eq!(
            catalog(&Agent::Claude, &sources).default_model.as_deref(),
            Some("haiku")
        );
        sources.claude_env_model = Some("opus[1m]".into());
        assert_eq!(
            catalog(&Agent::Claude, &sources).default_model.as_deref(),
            Some("opus[1m]")
        );
    }

    #[test]
    fn claude_offers_the_models_its_last_session_told_and_else_the_aliases() {
        let aliases = catalog(&Agent::Claude, &Sources::default());
        assert_eq!(aliases.models.len(), CLAUDE_MODELS.len());
        let sources = Sources {
            claude_models: vec![
                ("default".into(), Vec::new()),
                ("claude-fable-5-2[1m]".into(), vec!["high".into(), "max".into()]),
            ],
            ..Sources::default()
        };
        let told = catalog(&Agent::Claude, &sources);
        let values: Vec<&str> = told.models.iter().map(|m| m.value.as_str()).collect();
        assert_eq!(values, ["claude-fable-5-2[1m]"]);
        assert_eq!(told.efforts_of(Some("claude-fable-5-2[1m]")), ["high", "max"]);
    }

    #[test]
    fn agents_without_sources_offer_nothing_and_name_no_default() {
        assert_eq!(catalog(&Agent::Own, &Sources::default()), Catalog::default());
        let codex = catalog(&Agent::Codex, &Sources::default());
        assert!(codex.models.is_empty() && codex.default_model.is_none());
    }
}

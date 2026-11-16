//! The files of the agents' CLIs that name their models, read for the role lists (`ide_factory::models`): Claude
//! Code's settings of the project and the user and `ANTHROPIC_MODEL` of the login shell, Codex's `config.toml` and
//! the model list it caches.

use std::path::{Path, PathBuf};

use ide_factory::models::{Catalog, Sources, catalog};
use ide_factory::routing::Agent;

/// What `agent` offers for a role's entry, with the project at `root` where its settings count.
pub(crate) fn of(agent: &Agent, root: Option<&Path>) -> Catalog {
    catalog(agent, &sources(agent, root))
}

fn sources(agent: &Agent, root: Option<&Path>) -> Sources {
    let env = |name: &str| {
        crate::environment::get()
            .and_then(|vars| vars.iter().find(|(key, _)| key == name).map(|(_, value)| value.clone()))
    };
    let read = |path: PathBuf| std::fs::read_to_string(path).ok();
    let home = ide_core::paths::home();
    match agent {
        Agent::Claude => {
            let user = env("CLAUDE_CONFIG_DIR").map_or_else(|| home.join(".claude"), PathBuf::from);
            let mut files = Vec::new();
            if let Some(root) = root {
                files.push(root.join(".claude/settings.local.json"));
                files.push(root.join(".claude/settings.json"));
            }
            files.push(user.join("settings.json"));
            Sources {
                claude_env_model: env("ANTHROPIC_MODEL"),
                claude_settings: files.into_iter().filter_map(read).collect(),
                ..Sources::default()
            }
        }
        Agent::Codex => {
            let codex = env("CODEX_HOME").map_or_else(|| home.join(".codex"), PathBuf::from);
            Sources {
                codex_config: read(codex.join("config.toml")),
                codex_models: read(codex.join("models_cache.json")),
                ..Sources::default()
            }
        }
        _ => Sources::default(),
    }
}

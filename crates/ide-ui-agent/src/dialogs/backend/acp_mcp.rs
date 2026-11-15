//! The MCP servers an ACP agent gets when its session starts (E146): the project's `.mcp.json`, switched per
//! project, and the user's own list, whose secret values go to the app's store of secrets and never to the state
//! file (R42).

use std::path::Path;

use ide_core::explain::Explained;
use ide_core::mcp_servers::{
    self, McpOrigin, McpServerSpec, McpTransport, SWITCHED_OFF, StoredMcpServer, USER_SERVERS, secret_account,
};
use ide_core::settings::State;
use serde_json::{Value, json};

use crate::dialogs::model::{McpDraft, Scope, Transport};

/// Every server the agent gets in `project`, also those switched off, with the secret values filled in; the
/// project's only where the folder is trusted.
pub(crate) fn servers(project: &Path, env: &[(String, String)], trusted: bool) -> Vec<McpServerSpec> {
    let state = State::load_user();
    let off = state.get(&SWITCHED_OFF);
    let mut servers: Vec<McpServerSpec> = if trusted {
        mcp_servers::project_servers(project, env)
            .into_iter()
            .map(|mut server| {
                server.enabled = !mcp_servers::switched_off(&off, project, &server.name);
                server
            })
            .collect()
    } else {
        Vec::new()
    };
    let store = crate::own_agent::secrets();
    for stored in state.get(&USER_SERVERS) {
        let secrets = store
            .get(&secret_account(&stored.name))
            .ok()
            .flatten()
            .and_then(|text| serde_json::from_str::<Value>(&text).ok())
            .unwrap_or(Value::Null);
        servers.push(mcp_servers::user_server(&stored, &secrets));
    }
    servers
}

/// Switches the server `name` on or off: in the user's list, or for the project.
pub(crate) fn toggle(project: &Path, name: &str, enabled: bool) -> Result<(), Explained> {
    let mut state = State::load_user();
    let mut users = state.get(&USER_SERVERS);
    if let Some(user) = users.iter_mut().find(|s| s.name == name) {
        user.disabled = !enabled;
        return state.set(&USER_SERVERS, &users).map_err(|e| Explained::of(&e));
    }
    let mut off = state.get(&SWITCHED_OFF);
    mcp_servers::switch(&mut off, project, name, enabled);
    state.set(&SWITCHED_OFF, &off).map_err(|e| Explained::of(&e))
}

/// Adds `draft` to the user's list, its values to the store of secrets.
pub(crate) fn add(draft: &McpDraft) -> Result<(), Explained> {
    let mut state = State::load_user();
    let mut users = state.get(&USER_SERVERS);
    if users.iter().any(|s| s.name == draft.name) {
        return Err(Explained(
            splat_i18n::t("Einen Server mit diesem Namen gibt es schon.").to_string(),
        ));
    }
    let (command, args) = draft
        .command
        .split_first()
        .map_or((String::new(), Vec::new()), |(c, a)| (c.clone(), a.to_vec()));
    let object = |pairs: &[(String, String)]| -> serde_json::Map<String, Value> {
        pairs.iter().map(|(k, v)| (k.clone(), Value::from(v.clone()))).collect()
    };
    let secrets = json!({ "env": object(&draft.env), "headers": object(&draft.headers) });

    crate::own_agent::secrets()
        .set(&secret_account(&draft.name), &secrets.to_string())
        .map_err(|e| Explained::of(&e))?;
    users.push(StoredMcpServer {
        name: draft.name.clone(),
        transport: Some(match draft.transport {
            Transport::Stdio => McpTransport::Stdio,
            Transport::Http => McpTransport::Http,
            Transport::Sse => McpTransport::Sse,
        }),
        command,
        args,
        url: draft.url.clone(),
        env: draft.env.iter().map(|(k, _)| k.clone()).collect(),
        headers: draft.headers.iter().map(|(k, _)| k.clone()).collect(),
        disabled: false,
    });
    state.set(&USER_SERVERS, &users).map_err(|e| Explained::of(&e))
}

/// Removes `name` from the user's list and its values from the store of secrets.
pub(crate) fn remove(name: &str) -> Result<(), Explained> {
    let mut state = State::load_user();
    let mut users = state.get(&USER_SERVERS);
    users.retain(|s| s.name != name);
    state.set(&USER_SERVERS, &users).map_err(|e| Explained::of(&e))?;
    crate::own_agent::secrets()
        .delete(&secret_account(name))
        .map_err(|e| Explained::of(&e))
}

/// The scope `server` shows under.
pub(crate) fn scope(server: &McpServerSpec) -> Scope {
    match server.origin {
        McpOrigin::Project => Scope::Project,
        McpOrigin::User => Scope::User,
    }
}

//! The MCP servers the app hands an ACP agent when its session starts (E146): the project's `.mcp.json`, which the
//! caller reads only in a trusted folder (E22), and the user's own list in the app's state, whose secret values live
//! in the keychain under [`secret_account`] and never in the state file (R42).

use std::path::Path;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::settings::StateKey;

/// How the agent reaches a server.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum McpTransport {
    /// A program the agent starts.
    Stdio,
    /// Streamable HTTP.
    Http,
    /// Server-sent events.
    Sse,
}

/// Where a server is defined.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum McpOrigin {
    /// The project's `.mcp.json`.
    Project,
    /// The user's own list in the app.
    User,
}

/// A server as it is handed over: names and values resolved.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct McpServerSpec {
    /// Its name.
    pub name: String,
    /// How the agent reaches it.
    pub transport: McpTransport,
    /// The program, for `Stdio`.
    pub command: String,
    /// The program's arguments.
    pub args: Vec<String>,
    /// The address, for `Http` and `Sse`.
    pub url: String,
    /// The program's environment.
    pub env: Vec<(String, String)>,
    /// The HTTP headers.
    pub headers: Vec<(String, String)>,
    /// Where it is defined.
    pub origin: McpOrigin,
    /// Whether it is handed over.
    pub enabled: bool,
}

/// A server of the user's list as the state file keeps it: the names of its variables and headers, whose values are
/// in the keychain.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct StoredMcpServer {
    /// Its name.
    pub name: String,
    /// How the agent reaches it.
    pub transport: Option<McpTransport>,
    /// The program.
    pub command: String,
    /// The program's arguments.
    pub args: Vec<String>,
    /// The address.
    pub url: String,
    /// The names of the program's variables.
    pub env: Vec<String>,
    /// The names of the headers.
    pub headers: Vec<String>,
    /// Whether it is switched off.
    pub disabled: bool,
}

/// The user's own servers, for every ACP agent.
pub const USER_SERVERS: StateKey<Vec<StoredMcpServer>> = StateKey::new("acpMcpServers", Vec::new);

/// The projects' servers the user switched off, as `<project>\n<name>`.
pub const SWITCHED_OFF: StateKey<Vec<String>> = StateKey::new("acpMcpServersOff", Vec::new);

/// The keychain account holding the secret values of the user's server `name`, as a JSON object
/// `{"env": {…}, "headers": {…}}`.
pub fn secret_account(name: &str) -> String {
    format!("mcp:{name}")
}

/// The servers of `project`'s `.mcp.json`, with `${VAR}` and `${VAR:-default}` filled from `env`; none when it is
/// missing or unreadable.
pub fn project_servers(project: &Path, env: &[(String, String)]) -> Vec<McpServerSpec> {
    let Ok(text) = std::fs::read_to_string(project.join(".mcp.json")) else {
        return Vec::new();
    };
    let Ok(json) = serde_json::from_str::<Value>(&text) else {
        return Vec::new();
    };
    json.get("mcpServers")
        .and_then(Value::as_object)
        .into_iter()
        .flatten()
        .map(|(name, server)| project_server(name, server, env))
        .collect()
}

fn project_server(name: &str, server: &Value, env: &[(String, String)]) -> McpServerSpec {
    let text = |key: &str| expand(server.get(key).and_then(Value::as_str).unwrap_or_default(), env);
    let pairs = |key: &str| -> Vec<(String, String)> {
        let mut pairs: Vec<(String, String)> = server
            .get(key)
            .and_then(Value::as_object)
            .into_iter()
            .flatten()
            .map(|(k, v)| (k.clone(), expand(v.as_str().unwrap_or_default(), env)))
            .collect();
        pairs.sort();
        pairs
    };
    let transport = match server.get("type").and_then(Value::as_str) {
        Some("http") => McpTransport::Http,
        Some("sse") => McpTransport::Sse,
        _ if server.get("url").is_some() && server.get("command").is_none() => McpTransport::Http,
        _ => McpTransport::Stdio,
    };
    McpServerSpec {
        name: name.to_string(),
        transport,
        command: text("command"),
        args: server
            .get("args")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(Value::as_str)
            .map(|arg| expand(arg, env))
            .collect(),
        url: text("url"),
        env: pairs("env"),
        headers: pairs("headers"),
        origin: McpOrigin::Project,
        enabled: true,
    }
}

/// `text` with `${VAR}` and `${VAR:-default}` filled from `env`, as Claude Code reads `.mcp.json`.
fn expand(text: &str, env: &[(String, String)]) -> String {
    let mut out = String::new();
    let mut rest = text;
    while let Some(start) = rest.find("${") {
        out.push_str(&rest[..start]);
        let after = &rest[start + 2..];
        let Some(end) = after.find('}') else {
            out.push_str(&rest[start..]);
            return out;
        };
        let inner = &after[..end];
        let (name, default) = inner.split_once(":-").map_or((inner, None), |(n, d)| (n, Some(d)));
        match env.iter().find(|(k, _)| k == name) {
            Some((_, value)) => out.push_str(value),
            None => out.push_str(default.unwrap_or_default()),
        }
        rest = &after[end + 1..];
    }
    out.push_str(rest);
    out
}

/// The user's server `stored` with the secret values `secrets` (`{"env": {…}, "headers": {…}}`) filled in.
pub fn user_server(stored: &StoredMcpServer, secrets: &Value) -> McpServerSpec {
    let filled = |names: &[String], key: &str| -> Vec<(String, String)> {
        names
            .iter()
            .map(|name| {
                let value = secrets[key][name].as_str().unwrap_or_default();
                (name.clone(), value.to_string())
            })
            .collect()
    };
    McpServerSpec {
        name: stored.name.clone(),
        transport: stored.transport.unwrap_or(McpTransport::Stdio),
        command: stored.command.clone(),
        args: stored.args.clone(),
        url: stored.url.clone(),
        env: filled(&stored.env, "env"),
        headers: filled(&stored.headers, "headers"),
        origin: McpOrigin::User,
        enabled: !stored.disabled,
    }
}

/// Whether the user switched off the project server `name` of `project`.
pub fn switched_off(off: &[String], project: &Path, name: &str) -> bool {
    let entry = format!("{}\n{name}", project.display());
    off.contains(&entry)
}

/// `off` with the project server `name` of `project` switched on or off.
pub fn switch(off: &mut Vec<String>, project: &Path, name: &str, on: bool) {
    let entry = format!("{}\n{name}", project.display());
    off.retain(|o| *o != entry);
    if !on {
        off.push(entry);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn a_project_file_reads_with_its_variables_filled() {
        let dir = std::env::temp_dir().join(format!("ide-core-mcp-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let file = json!({"mcpServers": {
            "docs": {"command": "npx", "args": ["-y", "docs-${KIND}"], "env": {"TOKEN": "${TOKEN:-none}"}},
            "web": {"type": "http", "url": "https://x.example/${KIND}", "headers": {"A": "b"}},
            "plain": {"url": "https://y.example"}
        }});
        std::fs::write(dir.join(".mcp.json"), file.to_string()).unwrap();
        let env = vec![("KIND".to_string(), "mcp".to_string())];
        let servers = project_servers(&dir, &env);
        let docs = servers.iter().find(|s| s.name == "docs").unwrap();
        assert_eq!(
            (docs.transport, docs.args.clone()),
            (McpTransport::Stdio, vec!["-y".into(), "docs-mcp".into()])
        );
        assert_eq!(docs.env, vec![("TOKEN".into(), "none".into())]);
        let web = servers.iter().find(|s| s.name == "web").unwrap();
        assert_eq!(
            (web.transport, web.url.as_str()),
            (McpTransport::Http, "https://x.example/mcp")
        );
        assert_eq!(
            servers.iter().find(|s| s.name == "plain").unwrap().transport,
            McpTransport::Http
        );
        assert!(project_servers(&dir.join("missing"), &env).is_empty());
    }

    #[test]
    fn a_user_server_takes_its_secrets_from_outside_the_state() {
        let stored = StoredMcpServer {
            name: "api".into(),
            transport: Some(McpTransport::Http),
            url: "https://api.example".into(),
            headers: vec!["Authorization".into()],
            ..StoredMcpServer::default()
        };
        let spec = user_server(&stored, &json!({"headers": {"Authorization": "Bearer t"}}));
        assert_eq!(spec.headers, vec![("Authorization".into(), "Bearer t".into())]);
        assert!(!serde_json::to_string(&stored).unwrap().contains("Bearer"));
    }

    #[test]
    fn a_project_server_switches_off_per_project() {
        let mut off = Vec::new();
        switch(&mut off, Path::new("/p"), "docs", false);
        assert!(switched_off(&off, Path::new("/p"), "docs"));
        assert!(!switched_off(&off, Path::new("/q"), "docs"));
        switch(&mut off, Path::new("/p"), "docs", true);
        assert!(off.is_empty());
    }
}

//! Launch configurations: what `.vscode/launch.json` describes, with its variables filled in, and the ones the app
//! makes for attaching to a process.

use std::fmt;
use std::path::{Path, PathBuf};

use serde_json::{Map, Value, json};

use crate::adapter::AdapterKind;
use crate::jsonc;
use crate::types::Request;

/// How one program is debugged: which adapter, launch or attach, and the arguments the adapter gets.
#[derive(Debug, Clone, PartialEq)]
pub struct LaunchConfig {
    /// What the user reads, such as in the menu of configurations.
    pub name: String,
    /// The adapter.
    pub kind: AdapterKind,
    /// Launch or attach, which decides who owns the program (Review C7).
    pub request: Request,
    /// The arguments of `launch` or `attach`, in the adapter's terms, with every variable filled in.
    pub arguments: Map<String, Value>,
    /// The configuration asks for a process to attach to (`${command:pickProcess}`); the app lets the user pick one
    /// and passes it to `with_process` before starting.
    pub process_pick: bool,
}

impl LaunchConfig {
    /// Attaching `kind`'s adapter to the running process `pid`.
    pub fn attach(kind: AdapterKind, pid: u32, name: &str) -> LaunchConfig {
        LaunchConfig {
            name: name.to_string(),
            kind,
            request: Request::Attach,
            arguments: Map::new(),
            process_pick: false,
        }
        .with_process(pid)
    }

    /// The configuration attached to `pid`, in the field the adapter reads it from.
    #[must_use]
    pub fn with_process(mut self, pid: u32) -> LaunchConfig {
        match self.kind {
            AdapterKind::Lldb | AdapterKind::Fake => {
                self.arguments.insert("pid".into(), json!(pid));
            }
            AdapterKind::Go => {
                self.arguments.insert("processId".into(), json!(pid));
                self.arguments.entry("mode").or_insert(json!("local"));
            }
            AdapterKind::JsDebug | AdapterKind::Python => {
                self.arguments.insert("processId".into(), json!(pid));
            }
        }
        self.process_pick = false;
        self
    }

    /// The `type` the adapter is told, as its own name for this kind of configuration.
    pub(crate) fn adapter_type(&self) -> String {
        let written = self.arguments.get("type").and_then(Value::as_str);
        match (self.kind, written) {
            (AdapterKind::JsDebug, Some("node" | "pwa-node") | None) => "pwa-node".into(),
            (AdapterKind::JsDebug, Some(other)) => other.into(),
            (kind, _) => kind.adapter_id().into(),
        }
    }
}

/// What the variables of a launch configuration stand for.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Variables {
    /// `${workspaceFolder}`: the project root.
    pub workspace_folder: PathBuf,
    /// `${file}`: the file in the editor, if one is open.
    pub file: Option<PathBuf>,
    /// `${lineNumber}`: the caret's line in it, from 1.
    pub line: Option<u32>,
    /// `${selectedText}`.
    pub selected_text: Option<String>,
    /// `${env:NAME}`: the login shell's environment.
    pub env: Vec<(String, String)>,
}

/// One configuration of `launch.json`: ready, or why it cannot be used.
#[derive(Debug, Clone, PartialEq)]
pub struct Entry {
    /// Its name, also when it cannot be used.
    pub name: String,
    /// The configuration, or why it cannot be used, such as an unknown `type` or a variable without a value.
    pub config: Result<LaunchConfig, String>,
}

/// Why `launch.json` could not be read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LaunchError {
    /// The folder is not trusted, so nothing it configures is read (E22).
    Untrusted,
    /// There is no `.vscode/launch.json`.
    Missing,
    /// It could not be read.
    Io(String),
    /// It is not valid JSON with comments, or has no `configurations` list.
    Syntax(String),
}

impl fmt::Display for LaunchError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LaunchError::Untrusted => f.write_str("the folder is not trusted"),
            LaunchError::Missing => f.write_str("there is no .vscode/launch.json"),
            LaunchError::Io(why) => write!(f, ".vscode/launch.json could not be read: {why}"),
            LaunchError::Syntax(why) => write!(f, ".vscode/launch.json is not valid: {why}"),
        }
    }
}

impl std::error::Error for LaunchError {}

/// The configurations of `<root>/.vscode/launch.json`, only for a trusted folder.
pub fn launch_configs(root: &Path, trusted: bool, variables: &Variables) -> Result<Vec<Entry>, LaunchError> {
    if !trusted {
        return Err(LaunchError::Untrusted);
    }
    let path = root.join(".vscode/launch.json");
    let text = match std::fs::read_to_string(&path) {
        Ok(text) => text,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Err(LaunchError::Missing),
        Err(error) => return Err(LaunchError::Io(error.to_string())),
    };
    parse_launch_json(&text, variables)
}

/// The configurations in `text`, the content of a `launch.json`.
pub fn parse_launch_json(text: &str, variables: &Variables) -> Result<Vec<Entry>, LaunchError> {
    let value: Value = serde_json::from_str(&jsonc::strip(text)).map_err(|e| LaunchError::Syntax(e.to_string()))?;
    let list = value
        .get("configurations")
        .and_then(Value::as_array)
        .ok_or_else(|| LaunchError::Syntax("no list of configurations".into()))?;
    Ok(list
        .iter()
        .filter_map(Value::as_object)
        .enumerate()
        .map(|(n, object)| {
            let name = object
                .get("name")
                .and_then(Value::as_str)
                .map_or_else(|| format!("#{}", n + 1), str::to_string);
            Entry {
                config: config(object, &name, variables),
                name,
            }
        })
        .collect())
}

fn config(object: &Map<String, Value>, name: &str, variables: &Variables) -> Result<LaunchConfig, String> {
    let mut arguments = object.clone();
    // Settings for macOS take precedence over the common ones; those for other systems do not apply.
    if let Some(Value::Object(mac)) = arguments.remove("osx") {
        arguments.extend(mac);
    }
    arguments.remove("windows");
    arguments.remove("linux");
    let kind_name = arguments.get("type").and_then(Value::as_str).unwrap_or_default();
    let kind = AdapterKind::from_type(kind_name).ok_or_else(|| format!("unknown debugger type {kind_name:?}"))?;
    let request = match arguments.get("request").and_then(Value::as_str) {
        Some("launch") => Request::Launch,
        Some("attach") => Request::Attach,
        other => return Err(format!("unknown request {other:?}")),
    };
    let mut process_pick = false;
    let mut value = Value::Object(arguments);
    substitute(&mut value, variables, &mut process_pick)?;
    let Value::Object(mut arguments) = value else {
        return Err("not an object".into());
    };
    if process_pick {
        for key in ["pid", "processId"] {
            arguments.remove(key);
        }
    }
    if kind == AdapterKind::Lldb {
        environment_as_list(&mut arguments);
    }
    Ok(LaunchConfig {
        name: name.to_string(),
        kind,
        request,
        arguments,
        process_pick,
    })
}

/// lldb-dap reads the environment as `NAME=value` strings; other editors' configurations often give an object.
fn environment_as_list(arguments: &mut Map<String, Value>) {
    if let Some(Value::Object(env)) = arguments.get("env") {
        let list: Vec<Value> = env
            .iter()
            .map(|(key, value)| json!(format!("{key}={}", value.as_str().unwrap_or_default())))
            .collect();
        arguments.insert("env".into(), Value::Array(list));
    }
}

/// Fills in every `${…}` in the strings of `value`.
fn substitute(value: &mut Value, variables: &Variables, process_pick: &mut bool) -> Result<(), String> {
    match value {
        Value::String(text) => {
            if text.contains("${command:pickProcess}") || text.contains("${command:PickProcess}") {
                *process_pick = true;
                *value = Value::Null;
                return Ok(());
            }
            *text = expand(text, variables)?;
        }
        Value::Array(items) => {
            for item in items {
                substitute(item, variables, process_pick)?;
            }
        }
        Value::Object(fields) => {
            for item in fields.values_mut() {
                substitute(item, variables, process_pick)?;
            }
        }
        _ => {}
    }
    Ok(())
}

/// `text` with its variables filled in.
pub(crate) fn expand(text: &str, variables: &Variables) -> Result<String, String> {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(start) = rest.find("${") {
        out.push_str(&rest[..start]);
        let after = &rest[start + 2..];
        let end = after
            .find('}')
            .ok_or_else(|| format!("an unclosed variable in {text:?}"))?;
        out.push_str(&value_of(&after[..end], variables)?);
        rest = &after[end + 1..];
    }
    out.push_str(rest);
    Ok(out)
}

fn value_of(name: &str, variables: &Variables) -> Result<String, String> {
    let root = &variables.workspace_folder;
    let file = || {
        variables
            .file
            .as_deref()
            .ok_or_else(|| format!("${{{name}}} needs an open file"))
    };
    let text = |path: &Path| path.display().to_string();
    let base = |path: &Path| {
        path.file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default()
    };
    Ok(match name {
        "workspaceFolder" | "workspaceRoot" => text(root),
        "workspaceFolderBasename" => base(root),
        "file" => text(file()?),
        "fileBasename" => base(file()?),
        "fileBasenameNoExtension" => file()?
            .file_stem()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default(),
        "fileExtname" => file()?
            .extension()
            .map(|e| format!(".{}", e.to_string_lossy()))
            .unwrap_or_default(),
        "fileDirname" => file()?.parent().map(text).unwrap_or_default(),
        "fileDirnameBasename" => file()?.parent().map(base).unwrap_or_default(),
        "relativeFile" => text(file()?.strip_prefix(root).unwrap_or(file()?)),
        "relativeFileDirname" => {
            let dir = file()?.parent().unwrap_or(root);
            text(dir.strip_prefix(root).unwrap_or(dir))
        }
        "lineNumber" => variables
            .line
            .map(|l| l.to_string())
            .ok_or("${lineNumber} needs an open file")?,
        "selectedText" => variables.selected_text.clone().unwrap_or_default(),
        "cwd" => text(root),
        "pathSeparator" | "/" => "/".into(),
        "userHome" => variables
            .env
            .iter()
            .find(|(k, _)| k == "HOME")
            .map_or_else(|| text(&ide_core::paths::home()), |(_, v)| v.clone()),
        _ => {
            if let Some(key) = name.strip_prefix("env:") {
                return Ok(variables
                    .env
                    .iter()
                    .find(|(k, _)| k == key)
                    .map(|(_, v)| v.clone())
                    .unwrap_or_default());
            }
            return Err(format!("${{{name}}} is not supported"));
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn variables() -> Variables {
        Variables {
            workspace_folder: PathBuf::from("/work/app"),
            file: Some(PathBuf::from("/work/app/src/bin/tool.rs")),
            line: Some(12),
            selected_text: None,
            env: vec![("HOME".into(), "/Users/me".into()), ("PORT".into(), "8080".into())],
        }
    }

    const LAUNCH_JSON: &str = r#"{
        // Written by hand, as such files are.
        "version": "0.2.0",
        "configurations": [
            {
                "type": "lldb",
                "request": "launch",
                "name": "Debug tool",
                "program": "${workspaceFolder}/target/debug/tool",
                "args": ["--port", "${env:PORT}", "${relativeFile}:${lineNumber}"],
                "cwd": "${fileDirname}",
                "env": {"RUST_LOG": "debug"},
            },
            {"type": "node", "request": "attach", "name": "Attach", "processId": "${command:pickProcess}"},
            {"type": "go", "request": "launch", "name": "Go", "program": "${fileDirname}",
             "osx": {"env": {"CGO": "0"}}, "linux": {"env": {"X": "1"}}},
            {"type": "cppvsdbg", "request": "launch", "name": "Windows only"},
            {"type": "python", "request": "launch", "name": "Input", "program": "${input:script}"},
        ]
    }"#;

    #[test]
    fn launch_json_is_read_with_comments_and_its_variables_filled_in() {
        let entries = parse_launch_json(LAUNCH_JSON, &variables()).unwrap();
        let names: Vec<&str> = entries.iter().map(|e| e.name.as_str()).collect();
        assert_eq!(names, ["Debug tool", "Attach", "Go", "Windows only", "Input"]);
        let lldb = entries[0].config.as_ref().unwrap();
        assert_eq!((lldb.kind, lldb.request), (AdapterKind::Lldb, Request::Launch));
        assert_eq!(lldb.arguments["program"], "/work/app/target/debug/tool");
        assert_eq!(lldb.arguments["args"], json!(["--port", "8080", "src/bin/tool.rs:12"]));
        assert_eq!(lldb.arguments["cwd"], "/work/app/src/bin");
        assert_eq!(lldb.arguments["env"], json!(["RUST_LOG=debug"]));
    }

    #[test]
    fn a_process_pick_is_left_to_the_app_and_filled_in_where_the_adapter_reads_it() {
        let entries = parse_launch_json(LAUNCH_JSON, &variables()).unwrap();
        let attach = entries[1].config.clone().unwrap();
        assert!(attach.process_pick);
        assert!(!attach.arguments.contains_key("processId"));
        assert_eq!(attach.adapter_type(), "pwa-node");
        let picked = attach.with_process(4242);
        assert_eq!(
            (picked.process_pick, &picked.arguments["processId"]),
            (false, &json!(4242))
        );
        let lldb = LaunchConfig::attach(AdapterKind::Lldb, 7, "x");
        assert_eq!(lldb.arguments["pid"], 7);
    }

    #[test]
    fn mac_settings_win_and_unusable_entries_say_why() {
        let entries = parse_launch_json(LAUNCH_JSON, &variables()).unwrap();
        let go = entries[2].config.as_ref().unwrap();
        assert_eq!(go.arguments["env"], json!({"CGO": "0"}));
        assert!(!go.arguments.contains_key("linux"));
        assert!(entries[3].config.as_ref().unwrap_err().contains("cppvsdbg"));
        assert!(entries[4].config.as_ref().unwrap_err().contains("input:script"));
    }

    #[test]
    fn variables_about_the_file_need_one() {
        let without = Variables {
            file: None,
            ..variables()
        };
        assert!(expand("${file}", &without).is_err());
        assert_eq!(
            expand("${workspaceFolderBasename}-${userHome}", &without).unwrap(),
            "app-/Users/me"
        );
        assert!(expand("${unclosed", &without).is_err());
    }

    #[test]
    fn each_adapter_reads_the_process_from_its_own_field() {
        let go = LaunchConfig::attach(AdapterKind::Go, 7, "go");
        assert_eq!(
            (&go.arguments["processId"], &go.arguments["mode"]),
            (&json!(7), &json!("local"))
        );
        assert_eq!(go.adapter_type(), "go");
        let python = LaunchConfig::attach(AdapterKind::Python, 8, "py");
        assert_eq!(python.arguments["processId"], 8);
        let mut chrome = LaunchConfig::attach(AdapterKind::JsDebug, 9, "web");
        chrome.arguments.insert("type".into(), json!("pwa-chrome"));
        assert_eq!(chrome.adapter_type(), "pwa-chrome");
    }

    #[test]
    fn the_file_variables_name_its_parts_relative_to_the_folder() {
        let expanded = expand(
            "${fileBasename} ${fileBasenameNoExtension} ${fileExtname} ${fileDirnameBasename} ${relativeFileDirname} \
             ${workspaceRoot} ${selectedText}|",
            &variables(),
        )
        .unwrap();
        assert_eq!(expanded, "tool.rs tool .rs bin src/bin /work/app |");
        let without = Variables {
            line: None,
            ..variables()
        };
        assert!(expand("${lineNumber}", &without).unwrap_err().contains("lineNumber"));
    }

    #[test]
    fn a_broken_file_or_entry_says_what_is_wrong() {
        assert!(matches!(
            parse_launch_json("{", &variables()),
            Err(LaunchError::Syntax(_))
        ));
        let entries = parse_launch_json(
            r#"{"configurations": [{"type": "lldb", "request": "debug", "name": "Odd"}]}"#,
            &variables(),
        )
        .unwrap();
        assert!(entries[0].config.as_ref().unwrap_err().contains("unknown request"));
        let root = std::env::temp_dir().join(format!("ide-dap-launch-{}", std::process::id()));
        std::fs::create_dir_all(root.join(".vscode/launch.json")).unwrap();
        assert!(matches!(
            launch_configs(&root, true, &variables()),
            Err(LaunchError::Io(_))
        ));
        let _ = std::fs::remove_dir_all(&root);
        let words = [
            (LaunchError::Untrusted, "the folder is not trusted"),
            (LaunchError::Missing, "there is no .vscode/launch.json"),
            (
                LaunchError::Io("denied".into()),
                ".vscode/launch.json could not be read: denied",
            ),
            (
                LaunchError::Syntax("no list".into()),
                ".vscode/launch.json is not valid: no list",
            ),
        ];
        for (error, text) in words {
            assert_eq!(error.to_string(), text);
        }
    }

    #[test]
    fn an_untrusted_folder_is_not_read() {
        assert_eq!(
            launch_configs(Path::new("/nonexistent"), false, &variables()),
            Err(LaunchError::Untrusted)
        );
        assert_eq!(
            launch_configs(Path::new("/nonexistent"), true, &variables()),
            Err(LaunchError::Missing)
        );
    }
}

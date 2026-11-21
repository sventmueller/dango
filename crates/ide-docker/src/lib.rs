//! Managing containers, images, volumes and Compose projects through the `docker` command line (E136), without
//! AppKit and without starting processes: the arguments of each command, read with `--format '{{json .}}'` as one JSON
//! object per line, and the readers of what they print. Every command names its context with `--context`, chosen per
//! window; `docker context use`, which changes it for all of the user's shells, is never called. Compose projects and
//! their services come from the labels `com.docker.compose.project` and `com.docker.compose.service`, since
//! `docker compose ls` lists running projects only and `compose ps` changed its format in v2.21.0.
//!
//! ```
//! let line = r#"{"ID":"a1","Names":"app-db-1","Image":"postgres:16","State":"running","Status":"Up 2 hours","Ports":"0.0.0.0:5432->5432/tcp","Labels":"com.docker.compose.project=app,com.docker.compose.service=db"}"#;
//! let containers = ide_docker::containers(line);
//! assert_eq!(containers[0].project.as_deref(), Some("app"));
//! assert_eq!(containers[0].ports[0].host_port, 5432);
//! ```

#![forbid(unsafe_code)]

pub mod board;

use std::path::{Path, PathBuf};

use serde::Deserialize;

/// The oldest Docker whose `docker events` speaks JSON (A10).
pub(crate) const MIN_VERSION: (u32, u32, u32) = (24, 0, 6);

const PROJECT_LABEL: &str = "com.docker.compose.project";
const SERVICE_LABEL: &str = "com.docker.compose.service";

/// A port a container publishes on the Mac.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Port {
    /// The address it listens on, such as `0.0.0.0`.
    pub host_ip: String,
    /// The Mac's port.
    pub host_port: u16,
    /// The container's port.
    pub container_port: u16,
    /// Its transport.
    pub protocol: Protocol,
}

/// The transport of a published port, as Docker writes it after the slash.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Protocol {
    /// `tcp`, the one a browser tab can open.
    Tcp,
    /// `udp`.
    Udp,
    /// `sctp`.
    Sctp,
    /// One a later Docker adds, in its words.
    Other(String),
}

impl Protocol {
    fn read(word: &str) -> Protocol {
        match word {
            "tcp" => Protocol::Tcp,
            "udp" => Protocol::Udp,
            "sctp" => Protocol::Sctp,
            other => Protocol::Other(other.to_string()),
        }
    }
}

/// Where a container is in its life, by Docker's `State`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ContainerState {
    /// Made and never started.
    Created,
    /// Running.
    Running,
    /// Frozen by `docker pause`.
    Paused,
    /// Being started again by its restart policy.
    Restarting,
    /// Being deleted.
    Removing,
    /// Ended.
    Exited,
    /// Could not be deleted completely.
    Dead,
    /// A state a later Docker adds, in its words.
    Other(String),
}

impl ContainerState {
    fn read(word: &str) -> ContainerState {
        match word {
            "created" => ContainerState::Created,
            "running" => ContainerState::Running,
            "paused" => ContainerState::Paused,
            "restarting" => ContainerState::Restarting,
            "removing" => ContainerState::Removing,
            "exited" => ContainerState::Exited,
            "dead" => ContainerState::Dead,
            other => ContainerState::Other(other.to_string()),
        }
    }
}

/// A container as `docker ps -a` lists it.
#[derive(Debug, Clone, PartialEq, Eq)]
#[expect(missing_docs, reason = "the names say it")]
pub struct Container {
    pub id: String,
    pub name: String,
    pub image: String,
    /// Where it is in its life.
    pub state: ContainerState,
    /// What Docker says of it, such as `Up 2 hours`.
    pub status: String,
    /// The ports it publishes.
    pub ports: Vec<Port>,
    /// The Compose project it belongs to.
    pub project: Option<String>,
    /// Its Compose service.
    pub service: Option<String>,
}

impl Container {
    /// Whether it runs.
    pub fn running(&self) -> bool {
        matches!(self.state, ContainerState::Running | ContainerState::Restarting)
    }

    /// The address of its first published TCP port on the Mac, for a browser tab.
    pub fn url(&self) -> Option<String> {
        self.ports
            .iter()
            .find(|p| p.protocol == Protocol::Tcp)
            .map(|p| format!("http://localhost:{}", p.host_port))
    }
}

/// An image as `docker images` lists it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Image {
    pub(crate) id: String,
    /// Its repository, `<none>` for a dangling image.
    pub(crate) repository: String,
    pub(crate) tag: String,
    /// Its size as Docker writes it.
    pub(crate) size: String,
}

impl Image {
    /// `repository:tag`, or the id for a dangling image.
    pub(crate) fn reference(&self) -> String {
        if self.repository == "<none>" {
            self.id.clone()
        } else {
            format!("{}:{}", self.repository, self.tag)
        }
    }
}

/// A volume as `docker volume ls` lists it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Volume {
    pub(crate) name: String,
    pub(crate) driver: String,
    /// The Compose project it belongs to.
    pub(crate) project: Option<String>,
}

/// A Docker context as `docker context ls` lists it.
#[derive(Debug, Clone, PartialEq, Eq)]
#[expect(missing_docs, reason = "the names say it")]
pub struct Context {
    pub name: String,
    /// Whether it is the user's current one.
    pub current: bool,
    /// What it says of itself.
    pub(crate) description: String,
}

/// A change `docker events` reports.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Event {
    /// What changed.
    pub(crate) kind: ObjectKind,
    pub(crate) action: EventAction,
    /// The id or name of what changed.
    pub(crate) id: String,
}

/// What a change of `docker events` is about, by its `Type`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ObjectKind {
    /// A container.
    Container,
    /// An image.
    Image,
    /// A volume.
    Volume,
    /// A network.
    Network,
    /// Anything else, such as `daemon` or `plugin`, in Docker's words.
    Other(String),
}

impl ObjectKind {
    fn read(word: &str) -> ObjectKind {
        match word {
            "container" => ObjectKind::Container,
            "image" => ObjectKind::Image,
            "volume" => ObjectKind::Volume,
            "network" => ObjectKind::Network,
            other => ObjectKind::Other(other.to_string()),
        }
    }
}

/// How something changed, by the `Action` of `docker events`, in Docker's words such as `start` or
/// `health_status: healthy`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum EventAction {
    /// Leaves what the panel lists as it was: a container's health check alone reports `exec_*` and `health_status`
    /// every few seconds.
    Quiet(String),
    /// May change what the panel lists, such as `start`, `die` or `destroy`.
    Listed(String),
}

impl EventAction {
    fn read(word: &str) -> EventAction {
        const QUIET: &[&str] = &[
            "exec_",
            "health_status",
            "top",
            "resize",
            "attach",
            "detach",
            "archive-path",
            "extract-to-dir",
            "export",
        ];
        if QUIET.iter().any(|q| word.starts_with(q)) {
            EventAction::Quiet(word.to_string())
        } else {
            EventAction::Listed(word.to_string())
        }
    }

    /// Docker's words for it.
    #[cfg(test)]
    pub(crate) fn word(&self) -> &str {
        match self {
            EventAction::Quiet(word) | EventAction::Listed(word) => word,
        }
    }
}

impl Event {
    /// Whether it can change what the panel lists.
    pub fn changes_listing(&self) -> bool {
        matches!(self.action, EventAction::Listed(_))
    }
}

/// The object of one line, all values strings as Docker writes them.
#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
struct Fields {
    #[serde(default, rename = "ID")]
    id: String,
    #[serde(default)]
    names: String,
    #[serde(default)]
    image: String,
    #[serde(default)]
    state: String,
    #[serde(default)]
    status: String,
    #[serde(default)]
    ports: String,
    #[serde(default)]
    labels: String,
    #[serde(default)]
    repository: String,
    #[serde(default)]
    tag: String,
    #[serde(default)]
    size: String,
    #[serde(default)]
    name: String,
    #[serde(default)]
    driver: String,
    #[serde(default)]
    current: serde_json::Value,
    #[serde(default)]
    description: String,
}

fn lines(output: &str) -> impl Iterator<Item = Fields> + '_ {
    output
        .lines()
        .filter_map(|line| serde_json::from_str::<Fields>(line.trim()).ok())
}

/// The value of label `key` in Docker's comma-joined `labels`.
fn label(labels: &str, key: &str) -> Option<String> {
    labels
        .split(',')
        .filter_map(|pair| pair.split_once('='))
        .find(|(k, _)| k.trim() == key)
        .map(|(_, v)| v.to_string())
}

/// The ports of Docker's `Ports` text, such as `0.0.0.0:8080->80/tcp, :::8080->80/tcp`; one per port, IPv6 twins left
/// out.
pub(crate) fn ports(text: &str) -> Vec<Port> {
    let mut out: Vec<Port> = Vec::new();
    for part in text.split(", ") {
        let Some((host, inner)) = part.split_once("->") else {
            continue;
        };
        let Some((container, protocol)) = inner.split_once('/') else {
            continue;
        };
        let Some((ip, port)) = host.rsplit_once(':') else {
            continue;
        };
        let (Ok(host_port), Ok(container_port)) = (port.parse::<u16>(), container.parse::<u16>()) else {
            continue;
        };
        let protocol = Protocol::read(protocol);
        if out.iter().any(|p| p.host_port == host_port && p.protocol == protocol) {
            continue;
        }
        out.push(Port {
            host_ip: ip.to_string(),
            host_port,
            container_port,
            protocol,
        });
    }
    out
}

/// The containers `docker ps -a --format '{{json .}}'` printed.
pub fn containers(output: &str) -> Vec<Container> {
    lines(output)
        .map(|f| Container {
            id: f.id,
            name: f.names,
            image: f.image,
            state: ContainerState::read(&f.state),
            status: f.status,
            ports: ports(&f.ports),
            project: label(&f.labels, PROJECT_LABEL),
            service: label(&f.labels, SERVICE_LABEL),
        })
        .collect()
}

/// The images `docker images --format '{{json .}}'` printed.
pub fn images(output: &str) -> Vec<Image> {
    lines(output)
        .map(|f| Image {
            id: f.id,
            repository: f.repository,
            tag: f.tag,
            size: f.size,
        })
        .collect()
}

/// The volumes `docker volume ls --format '{{json .}}'` printed.
pub fn volumes(output: &str) -> Vec<Volume> {
    lines(output)
        .map(|f| Volume {
            project: label(&f.labels, PROJECT_LABEL),
            name: f.name,
            driver: f.driver,
        })
        .collect()
}

/// The contexts `docker context ls --format '{{json .}}'` printed.
pub fn contexts(output: &str) -> Vec<Context> {
    lines(output)
        .map(|f| Context {
            current: f.current.as_bool().unwrap_or(false),
            name: f.name,
            description: f.description,
        })
        .collect()
}

/// The change one line of `docker events --format '{{json .}}'` reports.
pub fn event(line: &str) -> Option<Event> {
    let value: serde_json::Value = serde_json::from_str(line.trim()).ok()?;
    let kind = ObjectKind::read(value.get("Type")?.as_str()?);
    let action = EventAction::read(value.get("Action")?.as_str()?);
    let id = value
        .get("Actor")
        .and_then(|a| a.get("ID"))
        .and_then(|i| i.as_str())
        .unwrap_or_default()
        .to_string();
    Some(Event { kind, action, id })
}

/// Whether `version`, such as `24.0.6` or `29.9.3`, is at least [`MIN_VERSION`].
pub fn supported(version: &str) -> bool {
    let mut parts = version
        .trim()
        .split(['.', '-', '+'])
        .map(|p| p.parse::<u32>().unwrap_or(0));
    let found = (
        parts.next().unwrap_or(0),
        parts.next().unwrap_or(0),
        parts.next().unwrap_or(0),
    );
    found >= MIN_VERSION
}

/// What to run: `docker` with these arguments.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Command {
    /// The arguments after `docker`, the context first.
    pub args: Vec<String>,
    /// Where it runs, for Compose; else anywhere.
    pub cwd: Option<PathBuf>,
}

fn docker(context: &str, args: &[&str]) -> Command {
    let mut all = vec!["--context".to_string(), context.to_string()];
    all.extend(args.iter().map(|a| (*a).to_string()));
    Command { args: all, cwd: None }
}

const JSON: &str = "{{json .}}";

/// Lists every container, running or not.
pub fn list_containers(context: &str) -> Command {
    docker(context, &["ps", "-a", "--no-trunc", "--format", JSON])
}

/// Lists the images.
pub fn list_images(context: &str) -> Command {
    docker(context, &["images", "--format", JSON])
}

/// Lists the volumes.
pub fn list_volumes(context: &str) -> Command {
    docker(context, &["volume", "ls", "--format", JSON])
}

/// Lists the contexts; this one needs no context of its own.
pub fn list_contexts() -> Command {
    Command {
        args: vec!["context".into(), "ls".into(), "--format".into(), JSON.into()],
        cwd: None,
    }
}

/// The server's version, which says whether `docker events` speaks JSON.
pub fn server_version(context: &str) -> Command {
    docker(context, &["version", "--format", "{{.Server.Version}}"])
}

/// Reports every change as it happens, until stopped.
pub fn watch_events(context: &str) -> Command {
    docker(context, &["events", "--format", JSON])
}

/// What can be done to a container.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    /// Start it.
    Start,
    /// Stop it.
    Stop,
    /// Restart it.
    Restart,
    /// Delete it, destructive.
    Remove,
}

/// Does `action` to container `id`.
pub fn container(context: &str, action: Action, id: &str) -> Command {
    let verb = match action {
        Action::Start => "start",
        Action::Stop => "stop",
        Action::Restart => "restart",
        Action::Remove => "rm",
    };
    let mut command = docker(context, &[verb]);
    if action == Action::Remove {
        command.args.push("--force".into());
    }
    command.args.push(id.to_string());
    command
}

/// Deletes image `reference`, destructive.
pub fn remove_image(context: &str, reference: &str) -> Command {
    docker(context, &["rmi", reference])
}

/// Deletes volume `name`, destructive.
pub fn remove_volume(context: &str, name: &str) -> Command {
    docker(context, &["volume", "rm", name])
}

/// Deletes stopped containers, dangling images and unused networks, destructive.
pub fn prune(context: &str) -> Command {
    docker(context, &["system", "prune", "--force"])
}

/// What can be done to a Compose project.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Compose {
    /// Start it in the background.
    Up,
    /// Stop and remove its containers.
    Down,
    /// Pull its images.
    Pull,
}

/// Does `action` to the Compose project of `file`, in the file's folder; only in trusted folders, since Compose builds
/// Dockerfiles and runs commands of the project (E22).
pub fn compose(context: &str, action: Compose, file: &Path) -> Command {
    let verb: &[&str] = match action {
        Compose::Up => &["up", "--detach"],
        Compose::Down => &["down"],
        Compose::Pull => &["pull"],
    };
    let mut command = docker(context, &["compose", "--file"]);
    command.args.push(file.display().to_string());
    command.args.extend(verb.iter().map(|v| (*v).to_string()));
    command.cwd = file.parent().map(Path::to_path_buf);
    command
}

/// The line typed into a terminal tab to follow container `id`'s log.
pub fn logs_line(context: &str, id: &str) -> String {
    format!("docker --context {} logs --follow {}", quote(context), quote(id))
}

/// The line typed into a terminal tab for a shell in container `id`.
pub fn shell_line(context: &str, id: &str) -> String {
    format!(
        "docker --context {} exec --interactive --tty {} sh -c 'command -v bash >/dev/null && exec bash || exec sh'",
        quote(context),
        quote(id)
    )
}

fn quote(text: &str) -> String {
    if text.chars().all(|c| c.is_alphanumeric() || "-_.:/".contains(c)) {
        text.to_string()
    } else {
        format!("'{}'", text.replace('\'', "'\\''"))
    }
}

/// The Compose files of the project at `root`: `compose.yaml` and its kin in the folder itself and one level down.
pub fn compose_files(root: &Path) -> Vec<PathBuf> {
    const NAMES: &[&str] = &[
        "compose.yaml",
        "compose.yml",
        "docker-compose.yaml",
        "docker-compose.yml",
    ];
    let mut dirs = vec![root.to_path_buf()];
    if let Ok(entries) = std::fs::read_dir(root) {
        let mut subs: Vec<PathBuf> = entries
            .flatten()
            .map(|e| e.path())
            .filter(|p| p.is_dir() && !p.file_name().is_some_and(|n| n.to_string_lossy().starts_with('.')))
            .collect();
        subs.sort();
        dirs.extend(subs);
    }
    dirs.iter()
        .flat_map(|dir| NAMES.iter().map(move |name| dir.join(name)))
        .filter(|p| p.is_file())
        .collect()
}

/// The project name Compose gives `file` by default: its folder's name, lower case, with only letters, digits, `-`
/// and `_`.
pub fn project_of(file: &Path) -> String {
    file.parent()
        .and_then(|d| d.file_name())
        .map(|n| {
            n.to_string_lossy()
                .to_lowercase()
                .chars()
                .filter(|c| c.is_ascii_alphanumeric() || *c == '-' || *c == '_')
                .collect()
        })
        .unwrap_or_default()
}

/// How long to wait before watching events again after the `attempt`-th failure: 1, 2, 4 … up to 60 seconds.
pub fn backoff(attempt: u32) -> std::time::Duration {
    std::time::Duration::from_secs(2u64.saturating_pow(attempt.min(6)).min(60))
}

#[cfg(test)]
mod tests;

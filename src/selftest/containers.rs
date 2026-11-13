//! The `containers-panel` scenario (B33): with a stand-in `docker` on the panel's PATH, the Containers panel shows an
//! empty state without `docker`, "Docker läuft nicht" while the daemon does not answer and tries again by itself, then
//! the tree of the folder's Compose project, single containers, images and volumes; its commands stop a container, open
//! its port in a browser tab and its logs in a terminal tab that does not come back after a restart; Compose commands
//! only in a trusted folder; deleting a volume asks with a sheet whose destructive button is not the default.

use std::path::PathBuf;
use std::time::Duration;

use ide_docker::board::Look;
use objc2::rc::Retained;
use objc2_app_kit::{NSAlertFirstButtonReturn, NSButton};
use splat_i18n::{t, tf};

use super::git::{answer_sheet, sheet_default_button, sheet_views};
use super::helpers::{self, wait_within};
use super::{check, finish, snapshot};
use crate::containers_panel::{self as panel, ContainersPanel};
use crate::window::{self, Panel, WindowController};

const SCENARIO: &str = "containers-panel";
const PROJECT: &str = "containers-project";
const TOOLS: &str = "containers-tools";
const EMPTY: &str = "containers-empty";
/// Answers as Docker 29 would from the files beside it, or as a daemon that does not run while `state` says `down`;
/// writes every call to `calls`.
const STAND_IN: &str = "#!/bin/sh\n\
    dir=\"$(dirname \"$0\")\"\n\
    echo \"$*\" >> \"$dir/calls\"\n\
    if [ \"$1\" = --context ]; then shift 2; fi\n\
    state=\"$(cat \"$dir/state\")\"\n\
    down='Cannot connect to the Docker daemon at unix:///var/run/docker.sock. Is the docker daemon running?'\n\
    case \"$1\" in\n\
      context) cat \"$dir/contexts.jsonl\";;\n\
      version) if [ \"$state\" = down ]; then echo \"$down\" >&2; exit 1; fi; echo 29.0.1;;\n\
      ps) cat \"$dir/ps.jsonl\";;\n\
      images) cat \"$dir/images.jsonl\";;\n\
      volume) if [ \"$2\" = ls ]; then cat \"$dir/volumes.jsonl\"; fi;;\n\
      events) if [ \"$state\" = down ]; then echo \"$down\" >&2; exit 1; fi; exec sleep 600;;\n\
    esac\n\
    exit 0\n";
const CONTEXTS: &str = "{\"Current\":false,\"Description\":\"Current DOCKER_HOST based configuration\",\"Name\":\"default\"}\n\
    {\"Current\":true,\"Description\":\"Docker Desktop\",\"Name\":\"desktop-linux\"}\n";
const IMAGES: &str = "{\"ID\":\"sha256:11aa\",\"Repository\":\"postgres\",\"Size\":\"438MB\",\"Tag\":\"16\"}\n";
const VOLUME: &str = "containers-project_data";

fn root() -> PathBuf {
    helpers::root(PROJECT)
}

fn tools() -> PathBuf {
    helpers::root(TOOLS)
}

fn controller() -> Option<Retained<WindowController>> {
    window::for_project(&root())
}

fn the_panel() -> Option<Retained<ContainersPanel>> {
    controller().and_then(|c| panel::panel_of(&c))
}

fn shown() -> (Vec<(usize, String, String, Look)>, String) {
    the_panel().map(|p| p.shown()).unwrap_or_default()
}

fn row(label: &str) -> Option<(usize, String, String, Look)> {
    shown().0.into_iter().find(|(_, l, _, _)| l == label)
}

fn calls() -> String {
    std::fs::read_to_string(tools().join("calls")).unwrap_or_default()
}

fn enabled(title: &str) -> bool {
    controller().is_some_and(|c| panel::enabled(&c, title))
}

fn perform(title: &str) -> bool {
    controller().is_some_and(|c| panel::perform_titled(&c, title))
}

fn select(label: &str) -> bool {
    the_panel().is_some_and(|p| p.select(label))
}

/// `docker ps -a` of a Compose project of the folder with a running and an exited service, and a container alone.
fn containers(project: &str) -> String {
    let compose = |id: &str, service: &str, state: &str, status: &str| {
        format!(
            "{{\"ID\":\"{id}\",\"Image\":\"img\",\"Labels\":\"com.docker.compose.project={project},com.docker.compose.service={service}\",\"Names\":\"{project}-{service}-1\",\"Ports\":\"\",\"State\":\"{state}\",\"Status\":\"{status}\"}}\n"
        )
    };
    compose("3f2a1c", "db", "running", "Up 2 hours")
        + &compose("9b8c7d", "web", "exited", "Exited (1) 5 minutes ago")
        + "{\"ID\":\"77aa11\",\"Image\":\"nginx:latest\",\"Labels\":\"maintainer=NGINX\",\"Names\":\"web-proxy\",\"Ports\":\"0.0.0.0:8080->80/tcp, [::]:8080->80/tcp\",\"State\":\"running\",\"Status\":\"Up 30 hours\"}\n"
}

fn stand_in() -> Result<(), String> {
    let project = ide_docker::project_of(&root().join("compose.yaml"));
    let volumes =
        format!("{{\"Driver\":\"local\",\"Labels\":\"com.docker.compose.project={project}\",\"Name\":\"{VOLUME}\"}}\n");
    helpers::make_project(
        TOOLS,
        &[
            ("docker", STAND_IN),
            ("state", "down\n"),
            ("calls", ""),
            ("contexts.jsonl", CONTEXTS),
            ("ps.jsonl", &containers(&project)),
            ("images.jsonl", IMAGES),
            ("volumes.jsonl", &volumes),
        ],
    )
    .map_err(|e| e.to_string())?;
    helpers::make_project(EMPTY, &[("README", "")]).map_err(|e| e.to_string())?;
    std::fs::set_permissions(
        tools().join("docker"),
        std::os::unix::fs::PermissionsExt::from_mode(0o755),
    )
    .map_err(|e| e.to_string())
}

pub fn run() {
    helpers::open_project(
        SCENARIO,
        PROJECT,
        &[("compose.yaml", "services:\n  db:\n    image: postgres:16\n")],
        |controller| {
            if let Err(error) = stand_in() {
                check("the stand-in docker is made", false, error);
                return finish(SCENARIO);
            }
            let idle = panel::panel_of(&controller).is_some_and(|p| !p.started());
            controller.activate_panel(Panel::Containers);
            check(
                "a window runs no docker before it shows its Containers panel, and reads once it does",
                idle && panel::panel_of(&controller).is_some_and(|p| p.started()),
                String::new(),
            );
            missing();
        },
    );
}

fn missing() {
    let Some(panel) = the_panel() else {
        check("the window has a Containers panel", false, String::new());
        return finish(SCENARIO);
    };
    panel.set_path(&helpers::root(EMPTY));
    wait_within(
        Duration::from_secs(10),
        || the_panel().is_some_and(|p| p.reach() == panel::Reach::Missing),
        |_| {
            let (rows, note) = shown();
            check(
                "without docker the panel shows its empty state and the Containers menu is greyed",
                rows.is_empty()
                    && note
                        == t(
                            "Docker ist nicht installiert. Installiere Docker Desktop oder die docker-Befehlszeile, dann zeigt dieses Panel Container, Images und Volumes.",
                        )
                    && !enabled(t("Aktualisieren"))
                    && !enabled(t("Ungenutztes aufräumen …")),
                format!("{rows:?} / {note:?}"),
            );
            not_running();
        },
    );
}

fn not_running() {
    if let Some(panel) = the_panel() {
        panel.set_path(&tools());
    }
    wait_within(
        Duration::from_secs(10),
        || matches!(the_panel().map(|p| p.reach()), Some(panel::Reach::NotRunning(_))),
        |found| {
            let note = shown().1;
            check(
                "while the daemon does not answer the panel says Docker does not run",
                found
                    && note == t("Docker läuft nicht. Starte Docker; das Panel versucht es von selbst wieder.")
                    && !enabled(t("Stoppen")),
                format!("{:?}", the_panel().map(|p| p.reach())),
            );
            if let Some(window) = controller().and_then(|c| c.window()) {
                snapshot(&window, "containers-not-running");
            }
            if let Err(error) = std::fs::write(tools().join("state"), "up\n") {
                check("the stand-in daemon starts", false, error.to_string());
                return finish(SCENARIO);
            }
            tree();
        },
    );
}

fn tree() {
    let project = ide_docker::project_of(&root().join("compose.yaml"));
    wait_within(
        Duration::from_secs(20),
        || the_panel().is_some_and(|p| p.watching()) && row("web-proxy").is_some(),
        move |found| {
            let (rows, note) = shown();
            let project_row = row(&project);
            check(
                "once Docker answers the panel tries again by itself and shows the folder's Compose project with its services, the container alone, images and volumes",
                found
                    && note.is_empty()
                    && project_row.as_ref().is_some_and(|(level, _, detail, look)| {
                        *level == 1 && detail == "1/2" && *look == Look::Partly
                    })
                    && row("db").is_some_and(|r| r.0 == 2 && r.3 == Look::Running)
                    && row("web").is_some_and(|r| r.0 == 2 && r.3 == Look::Stopped)
                    && row("postgres:16").is_some_and(|r| r.2 == "438MB")
                    && row(VOLUME).is_some()
                    && the_panel().is_some_and(|p| p.context() == "desktop-linux"),
                format!("{rows:?} / {:?}", the_panel().map(|p| p.reach())),
            );
            if let Some(window) = controller().and_then(|c| c.window()) {
                snapshot(&window, "containers-panel");
                let findings = super::a11y::audit::findings(&super::a11y::audit::elements(&window));
                check(
                    "the accessibility audit finds nothing in the window with the Containers panel",
                    findings.is_empty(),
                    findings
                        .iter()
                        .map(|f| format!("{}: {} ({})", f.problem, f.what, f.path))
                        .collect::<Vec<_>>()
                        .join("; "),
                );
            }
            container_commands();
        },
    );
}

fn container_commands() {
    let chosen = select("web-proxy");
    check(
        "a running container can stop, restart and open its port, not start",
        chosen
            && enabled(t("Stoppen"))
            && enabled(t("Neu starten"))
            && enabled(t("Port im Browser öffnen"))
            && !enabled(t("Starten"))
            && !enabled(t("Compose: Starten (up)")),
        format!("{:?}", shown().0),
    );
    perform(t("Stoppen"));
    wait_within(
        Duration::from_secs(10),
        || calls().contains("--context desktop-linux stop 77aa11"),
        |stopped| {
            check("Stoppen runs docker stop in the window's context", stopped, calls());
            port();
        },
    );
}

fn browser_at(url: &str) -> bool {
    controller().is_some_and(|c| {
        c.tab_ids()
            .into_iter()
            .any(|tab| crate::browser::url_of_tab(&c, tab).is_some_and(|u| u.starts_with(url)))
    })
}

fn port() {
    perform(t("Port im Browser öffnen"));
    wait_within(
        Duration::from_secs(10),
        || browser_at("http://localhost:8080"),
        |opened| {
            check("the published port opens in a browser tab", opened, String::new());
            logs();
        },
    );
}

fn logs() {
    let title = tf("Logs: {}", &["web-proxy"]);
    perform(t("Logs anzeigen"));
    let wanted = title.clone();
    wait_within(
        Duration::from_secs(10),
        move || controller().is_some_and(|c| helpers::tab(&c, &wanted).is_some()),
        move |opened| {
            let kept = controller().and_then(|c| {
                let tab = helpers::tab(&c, &title)?;
                Some(crate::terminal::folder_of_tab(&c, tab).is_some())
            });
            check(
                "the logs open in a terminal tab that does not open again after a restart",
                opened && kept == Some(false),
                format!("{kept:?} {:?}", controller().map(|c| c.tab_titles())),
            );
            compose();
        },
    );
}

fn compose() {
    let project = ide_docker::project_of(&root().join("compose.yaml"));
    let chosen = select(&project);
    let trusted = enabled(t("Compose: Starten (up)")) && enabled(t("Compose: Images holen (pull)"));
    crate::project::distrust(&root());
    let distrusted = !enabled(t("Compose: Starten (up)"));
    crate::project::trust(&root());
    check(
        "Compose commands apply to the folder's project only while the folder is trusted",
        chosen && trusted && distrusted,
        format!("{chosen} {trusted} {distrusted}"),
    );
    perform(t("Compose: Starten (up)"));
    wait_within(
        Duration::from_secs(10),
        || calls().contains("compose") && calls().contains("up --detach"),
        |ran| {
            check(
                "Compose up runs with the folder's file",
                ran && calls().contains("compose.yaml"),
                calls(),
            );
            delete();
        },
    );
}

fn delete() {
    let chosen = select(VOLUME);
    perform(t("Löschen …"));
    wait_within(
        Duration::from_secs(10),
        || controller().is_some_and(|c| c.has_sheet()),
        move |asked| {
            let default = sheet_default_button(&root());
            let destructive = sheet_views(&root())
                .into_iter()
                .filter_map(|view| view.downcast::<NSButton>().ok())
                .find(|button| button.title().to_string() == t("Löschen"))
                .is_some_and(|button| button.hasDestructiveAction());
            check(
                "deleting a volume asks with a sheet; Löschen is destructive and not the default",
                chosen && asked && destructive && default.as_deref() == Some(t("Abbrechen")),
                format!("{default:?}"),
            );
            super::git::sheet_pictures(root(), "containers-delete", |root| {
                if !answer_sheet(&root, NSAlertFirstButtonReturn, None) {
                    return finish(SCENARIO);
                }
                deleted();
            });
        },
    );
}

fn deleted() {
    let wanted = format!("volume rm {VOLUME}");
    wait_within(
        Duration::from_secs(10),
        move || calls().contains(&wanted),
        |removed| {
            check("agreeing deletes the volume", removed, calls());
            finish(SCENARIO);
        },
    );
}

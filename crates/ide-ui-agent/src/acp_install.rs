//! Installing an ACP agent on the user's word (E5): what the question before it says, what an installation that
//! fails or is refused says, and the progress the tab shows meanwhile.

use std::sync::Arc;
use std::time::Duration;

use ide_acp::install::{InstallError, Progress, Refusal, Source as InstallSource};
use ide_acp::{Installer, Installs, Plan};
use ide_net::Http;
use ide_process::Supervisor;
use objc2_app_kit::{NSAlert, NSAlertFirstButtonReturn};
use splat_i18n::{t, tf};

use super::source::Problem;
use super::tab::AgentTab;
use ide_ui_common::util::{main_soon, ns};

/// Why the register's entry is not installed.
pub(super) fn refusal_text(refusal: &Refusal) -> String {
    match refusal {
        Refusal::Unversioned(package) => tf(
            "{} nennt keine feste Version und wird deshalb nicht installiert",
            &[package],
        ),
        Refusal::Nothing { unknown } if unknown.is_empty() => t("Für diesen Mac gibt es keine Ausgabe").to_string(),
        Refusal::Nothing { unknown } => tf(
            "Nur als {} erhältlich, was hier nicht unterstützt wird",
            &[&unknown.join(", ")],
        ),
    }
}

/// What installing `plan` would download, for the question before it.
fn plan_text(plan: &Plan) -> String {
    let source = match &plan.source {
        InstallSource::Archive { url, .. } => tf("Archiv von {}", &[url]),
        InstallSource::Npm { package, version, .. } => tf("npm-Paket {}", &[&format!("{package}@{version}")]),
        InstallSource::Uv { package, version, .. } => {
            tf("Python-Paket {} über uv", &[&format!("{package}=={version}")])
        }
    };
    let mut lines = vec![
        tf("Version {}", &[&plan.version]),
        source,
        tf(
            "Ziel: {}",
            &[&Installs::default_root().join(&plan.id).display().to_string()],
        ),
    ];
    if !plan.verified {
        lines.push(t("Diese Version wurde mit dieser App nicht erprobt.").to_string());
    }
    if !plan.checksum {
        lines.push(t("Das Register nennt keine Prüfsumme; der Download wird nicht geprüft.").to_string());
    }
    lines.join("\n")
}

fn install_error_text(error: &InstallError) -> String {
    ide_core::explain::Explain::explain(error)
}

fn progress_text(name: &str, progress: Progress) -> String {
    match progress {
        Progress::Checking => tf("{} wird vorbereitet …", &[name]),
        Progress::Downloading(bytes) => tf("{} wird geladen: {}", &[name, &super::format::bytes(bytes)]),
        Progress::Unpacking => tf("{} wird entpackt …", &[name]),
        Progress::Installing => tf("{} wird installiert …", &[name]),
    }
}

/// Asks before installing `plan` of `agent`; whether the user agreed.
pub fn confirm_install(mtm: objc2::MainThreadMarker, agent: &str, plan: &Plan) -> bool {
    let alert = NSAlert::new(mtm);
    alert.setMessageText(&ns(&tf("{} installieren?", &[agent])));
    alert.setInformativeText(&ns(&plan_text(plan)));
    alert.addButtonWithTitle(&ns(t("Installieren")));
    alert.addButtonWithTitle(&ns(t("Abbrechen")));
    alert.runModal() == NSAlertFirstButtonReturn
}

impl AgentTab {
    /// Installs the agent after the user agreed, showing the progress, then starts it.
    pub fn install_agent(&self) {
        let (Some(agent), Some(Problem::Missing(Some(plan)))) =
            (self.acp_agent(), self.state.borrow().problem().cloned())
        else {
            return;
        };
        if !confirm_install(self.mtm, &agent.name, &plan) {
            return;
        }
        let key = self.key;
        let name = agent.name.clone();
        self.set_problem(Some(Problem::Installing(progress_text(&name, Progress::Checking))));
        if let Err(why) = ide_ui_common::util::spawn("acp-install", move || {
            let env = ide_ui_common::window::environment(Duration::from_secs(15));
            let installer = Installer::new(
                Supervisor::global(),
                env,
                Installs::new(Installs::default_root()),
                Arc::new(Http::new("agent register")),
            );
            let shown = name.clone();
            let progress = move |step: Progress| {
                let text = progress_text(&shown, step);
                main_soon(move || {
                    if let Some(tab) = super::get(key)
                        && matches!(tab.state.borrow().problem(), Some(Problem::Installing(_)))
                    {
                        tab.set_problem(Some(Problem::Installing(text)));
                    }
                });
            };
            let result = installer.install(&plan, true, &progress);
            main_soon(move || {
                let Some(tab) = super::get(key) else { return };
                match result {
                    Ok(_) => tab.start(),
                    Err(error) => tab.set_problem(Some(Problem::Start(install_error_text(&error)))),
                }
            });
        }) {
            self.set_problem(Some(Problem::Start(why.0)));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn plan(source: InstallSource, verified: bool, checksum: bool) -> Plan {
        Plan {
            id: "test-agent".into(),
            name: "Test".into(),
            version: "1.2.3".into(),
            source,
            verified,
            checksum,
            runtime: ide_acp::install::Runtime::None,
        }
    }

    #[test]
    fn the_question_before_installing_says_what_is_downloaded_and_what_is_not_checked() {
        let npm = InstallSource::Npm {
            package: "@acme/agent".into(),
            version: "1.2.3".into(),
            integrity: None,
            bin: None,
            args: Vec::new(),
            env: Vec::new(),
        };
        let text = plan_text(&plan(npm, false, false));
        let lines: Vec<&str> = text.lines().collect();
        assert_eq!(lines[0], "Version 1.2.3");
        assert_eq!(lines[1], "npm-Paket @acme/agent@1.2.3");
        assert!(
            lines[2].starts_with("Ziel: ") && lines[2].ends_with("test-agent"),
            "{}",
            lines[2]
        );
        assert_eq!(lines[3], "Diese Version wurde mit dieser App nicht erprobt.");
        assert_eq!(
            lines[4],
            "Das Register nennt keine Prüfsumme; der Download wird nicht geprüft."
        );
        let uv = InstallSource::Uv {
            package: "acme".into(),
            version: "0.9".into(),
            args: Vec::new(),
            env: Vec::new(),
        };
        let text = plan_text(&plan(uv, true, true));
        assert_eq!(
            text.lines().count(),
            3,
            "a verified, checked plan has no warnings: {text}"
        );
        assert!(text.contains("Python-Paket acme==0.9 über uv"));
        let archive = InstallSource::Archive {
            url: "https://example.org/a.tar.gz".into(),
            sha256: Some("ab".into()),
            cmd: "agent".into(),
            args: Vec::new(),
            env: Vec::new(),
        };
        assert!(plan_text(&plan(archive, true, true)).contains("Archiv von https://example.org/a.tar.gz"));
    }

    #[test]
    fn refusals_failures_and_progress_of_an_installation_are_worded() {
        assert_eq!(
            refusal_text(&Refusal::Unversioned("acme".into())),
            "acme nennt keine feste Version und wird deshalb nicht installiert"
        );
        assert_eq!(
            refusal_text(&Refusal::Nothing { unknown: Vec::new() }),
            "Für diesen Mac gibt es keine Ausgabe"
        );
        assert_eq!(
            refusal_text(&Refusal::Nothing {
                unknown: vec!["docker".into(), "brew".into()]
            }),
            "Nur als docker, brew erhältlich, was hier nicht unterstützt wird"
        );
        assert_eq!(
            install_error_text(&InstallError::Runtime("Node", Some(20))),
            "Node 20 oder neuer wird gebraucht"
        );
        assert_eq!(
            install_error_text(&InstallError::Runtime("uv", None)),
            "uv wird gebraucht"
        );
        assert_eq!(
            install_error_text(&InstallError::Checksum {
                expected: "a".into(),
                actual: "b".into()
            }),
            "Der Download stimmt nicht mit seiner Prüfsumme überein und wurde verworfen"
        );
        let tool = InstallError::Tool("npm ERR! 404".into());
        assert_eq!(install_error_text(&tool), tool.to_string());
        assert_eq!(progress_text("Codex", Progress::Checking), "Codex wird vorbereitet …");
        assert_eq!(progress_text("Codex", Progress::Unpacking), "Codex wird entpackt …");
        assert_eq!(progress_text("Codex", Progress::Installing), "Codex wird installiert …");
        assert!(progress_text("Codex", Progress::Downloading(2_000_000)).starts_with("Codex wird geladen: "));
    }
}

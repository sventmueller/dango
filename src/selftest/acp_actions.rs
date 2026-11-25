//! The `acp-actions` scenario: the composer's menus of an agent over ACP, filled from what the test agent of
//! `ide_acp` offers, answering its permission with the rule it offers, and an agent that is missing: its installation
//! is declined, so nothing is downloaded, and it starts once it is there. No agent installed on the Mac starts.

use std::path::PathBuf;
use std::time::Duration;

use ide_thread::Status;
use objc2_app_kit::{NSAlertSecondButtonReturn, NSControlStateValueOn};

use super::agent::{controller_of, tab_of};
use super::agent_actions::{index_of, titles};
use super::helpers::{answer_when_shown, answered};
use super::{check, finish, give_up, give_up_saying, helpers};
use ide_ui_agent::{AgentKey, TabMenu};

const NAME: &str = "acp-actions";
/// The overlays' commands the test agent's capabilities allow (E146), after its own.
pub(crate) const ACP_LOCAL: [&str; 8] = [
    "/status", "/export", "/mcp", "/memory", "/config", "/logout", "/btw", "/resume",
];
const PROJECT: &str = "acp-actions-project";
const REGISTRY: &str = include_str!("../../crates/ide-acp/tests/fixtures/registry.json");
/// The places of the agents in the "+" menu.
const GEMINI: usize = 1;
const OPENCODE: usize = 3;

fn test_launch(root: &std::path::Path) -> Option<ide_acp::Launch> {
    let program = std::env::current_exe().ok()?;
    Some(ide_acp::Launch::new(
        "test",
        "Test",
        program,
        Vec::new(),
        vec![
            ("IDE_ACP_TEST_AGENT".into(), "basic".into()),
            ("PATH".into(), "/usr/bin:/bin".into()),
        ],
        root,
    ))
}

/// A moment for the timeline to lay out the cards it scrolled to, whose buttons a press needs; it shows no state to
/// wait for.
const SCROLL_SETTLE: Duration = Duration::from_millis(300);

fn soon(what: &'static str, ready: impl Fn() -> bool + 'static, then: impl FnOnce() + 'static) {
    helpers::wait_within(Duration::from_secs(15), ready, move |ok| {
        if ok {
            then();
        } else {
            give_up_saying(NAME, what, "waited in vain");
        }
    });
}

pub(crate) fn run() {
    if let Ok(register) = ide_acp::Register::parse(REGISTRY) {
        ide_ui_agent::set_test_register(register);
    }
    let made = helpers::make_project(PROJECT, &[("README.md", "# ACP actions\n")]);
    ide_ui_agent::set_test_launch(made.ok().and_then(|root| test_launch(&root)));
    helpers::open_project(NAME, PROJECT, &[], |controller| {
        let root = helpers::root(PROJECT);
        let Some(tab) = ide_ui_agent::open_preset(&controller, GEMINI) else {
            return give_up(NAME, "the AI tab opens");
        };
        let key = tab.key;
        soon(
            "the ACP session opens with its commands",
            move || {
                tab_of(key).is_some_and(|t| {
                    t.is_live()
                        && t.acp_capabilities()
                            .is_some_and(|c| !c.modes.is_empty() && !c.commands.is_empty())
                })
            },
            move || menus(root, key),
        );
    });
}

/// The slash, model and mode menus list what the agent offers; picking from them switches the agent.
fn menus(root: PathBuf, key: AgentKey) {
    let Some(tab) = tab_of(key) else {
        return give_up(NAME, "the AI tab is open");
    };
    let slash = tab.menu(TabMenu::Slash);
    let tip = slash.itemAtIndex(0).and_then(|i| i.toolTip()).map(|t| t.to_string());
    slash.performActionForItemAtIndex(0);
    check(
        "the slash menu lists the agent's commands, and picking one puts it into the composer",
        titles(&slash)
            == ACP_LOCAL.iter().fold(vec!["/review".to_string()], |mut all, c| {
                all.push((*c).to_string());
                all
            })
            && tip.as_deref() == Some("Review the changes")
            && tab.composer_text() == "/review ",
        format!("{:?}, {tip:?}, {:?}", titles(&slash), tab.composer_text()),
    );
    let model = tab.menu(TabMenu::Model);
    let shown = titles(&model);
    let ticked = index_of(&model, "Fast")
        .and_then(|n| model.itemAtIndex(n))
        .is_some_and(|i| i.state() == NSControlStateValueOn);
    check(
        "the model menu lists the agent's models under its heading, the current one ticked",
        shown.len() == 3
            && shown[1..] == ["Fast", "Deep"]
            && ticked
            && model.itemAtIndex(0).is_some_and(|i| !i.isEnabled()),
        format!("{shown:?}, ticked {ticked}"),
    );
    if let Some(deep) = index_of(&model, "Deep") {
        model.performActionForItemAtIndex(deep);
    }
    let mode = tab.menu(TabMenu::Mode);
    let modes = titles(&mode);
    mode.performActionForItemAtIndex(1);
    soon(
        "the agent takes the model and mode picked from the menus",
        move || tab_of(key).is_some_and(|t| t.pills().0 == "Deep" && Some(&t.pills().1) != modes.first()),
        move || {
            check("picking from the menus switches the agent's model and mode", true, "");
            permission(root, key);
        },
    );
}

/// The agent's own rule allows the command for good.
fn permission(root: PathBuf, key: AgentKey) {
    let Some(tab) = tab_of(key) else {
        return give_up(NAME, "the AI tab is open");
    };
    tab.type_and_send("list the files");
    soon(
        "the agent asks for permission",
        move || tab_of(key).is_some_and(|t| t.snapshot().pending_requests().len() == 1),
        move || {
            let Some(tab) = tab_of(key) else {
                return give_up(NAME, "the AI tab is open");
            };
            tab.timeline().scroll_to_end();
            helpers::after(SCROLL_SETTLE, move || {
                let Some(tab) = tab_of(key) else {
                    return give_up(NAME, "the AI tab is open");
                };
                let pressed = tab.timeline().press_last("Immer erlauben", 0);
                soon(
                    "the turn ends after the rule",
                    move || {
                        tab_of(key).is_some_and(|t| {
                            t.snapshot().pending_requests().is_empty() && t.snapshot().status() == Status::Idle
                        })
                    },
                    move || {
                        check("allowing with the agent's rule answers its request", pressed, "");
                        missing(root);
                    },
                );
            });
        },
    );
}

/// A missing agent offers its installation; declining downloads nothing, and once it is there it starts again.
fn missing(root: PathBuf) {
    let Some(controller) = controller_of(&root) else {
        return give_up(NAME, "the folder's window is open");
    };
    ide_ui_agent::set_test_launch(None);
    register_agents(&root);
    let Some(tab) = ide_ui_agent::open_preset(&controller, OPENCODE) else {
        return give_up(NAME, "the AI tab opens");
    };
    let key = tab.key;
    soon(
        "the missing agent says so",
        move || tab_of(key).is_some_and(|t| t.problem_name() == Some("missing")),
        move || {
            let Some(tab) = tab_of(key) else {
                return give_up(NAME, "the AI tab is open");
            };
            answer_when_shown(NSAlertSecondButtonReturn, None);
            tab.install_agent();
            check(
                "installing asks first, and declining leaves the agent missing",
                answered().last().map(String::as_str) == Some("OpenCode installieren?")
                    && tab.problem_name() == Some("missing"),
                format!("{:?}", answered()),
            );
            ide_ui_agent::set_test_launch(test_launch(&root));
            tab.retry();
            soon(
                "the agent starts once it is there",
                move || tab_of(key).is_some_and(|t| t.is_live()),
                move || {
                    check("trying again starts an agent that is there now", true, "");
                    crashed(root, key);
                },
            );
        },
    );
}

/// The agent crashes mid-turn: the tab says it was ended with its last words, a prompt typed meanwhile waits, and
/// starting it again sends that prompt.
fn crashed(root: PathBuf, key: AgentKey) {
    let Some(tab) = tab_of(key) else {
        return give_up(NAME, "the AI tab is open");
    };
    tab.type_and_send("please crash");
    soon(
        "the crashed agent shows as ended",
        move || tab_of(key).is_some_and(|t| t.problem_name() == Some("exited")),
        move || {
            let Some(tab) = tab_of(key) else {
                return give_up(NAME, "the AI tab is open");
            };
            let shown = tab.problem_texts();
            check(
                "an agent that crashes says it was ended, with what it wrote last, and offers to start again",
                !tab.is_live()
                    && shown
                        .iter()
                        .any(|t| t == "Test wurde beendet" || t.ends_with(" wurde beendet"))
                    && shown
                        .iter()
                        .any(|t| *t == format!("Letzte Meldung: {}", ide_acp::test_agent::CRASH))
                    && shown.iter().any(|t| t == "Erneut versuchen"),
                format!("{shown:?}"),
            );
            tab.type_and_send("list the files again");
            let queued = tab.queued();
            tab.retry();
            soon(
                "the prompt typed after the crash goes out once the agent runs again",
                move || {
                    tab_of(key).is_some_and(|t| {
                        t.is_live() && t.queued().is_empty() && t.snapshot().pending_requests().len() == 1
                    })
                },
                move || {
                    check(
                        "a prompt typed after the crash waits and goes out when the agent is started again",
                        queued == ["list the files again"],
                        format!("{queued:?}"),
                    );
                    quit(root, key);
                },
            );
        },
    );
}

/// The agent ends on its own after a turn, without a word: the tab says so instead of looking busy.
fn quit(root: PathBuf, key: AgentKey) {
    let Some(tab) = tab_of(key) else {
        return give_up(NAME, "the AI tab is open");
    };
    tab.timeline().press_last("Ablehnen", 0);
    tab.type_and_send("quit now");
    soon(
        "the agent that quit shows as ended",
        move || tab_of(key).is_some_and(|t| t.problem_name() == Some("exited")),
        move || {
            let shown = tab_of(key).map(|t| t.problem_texts()).unwrap_or_default();
            check(
                "an agent that ends after a turn without a word says so",
                shown.iter().any(|t| t == "Das Programm hat sich ohne Meldung beendet."),
                format!("{shown:?}"),
            );
            dead_start(root);
        },
    );
}

/// An agent that dies while it starts says why in the interface language, with its last line.
fn dead_start(root: PathBuf) {
    let Some(controller) = controller_of(&root) else {
        return give_up(NAME, "the folder's window is open");
    };
    ide_ui_agent::set_test_launch(Some(ide_acp::Launch::new(
        "test",
        "Test",
        PathBuf::from("/bin/sh"),
        vec!["-c".into(), "echo 'no API key set' >&2; exit 1".into()],
        vec![("PATH".into(), "/usr/bin:/bin".into())],
        &root,
    )));
    let Some(tab) = ide_ui_agent::open_preset(&controller, GEMINI) else {
        return give_up(NAME, "the AI tab opens");
    };
    let key = tab.key;
    soon(
        "the agent's death at start shows with its reason",
        move || {
            tab_of(key).is_some_and(|t| {
                t.problem_texts()
                    .iter()
                    .any(|s| s.starts_with("Fehler: Das Programm hat sich beim Start beendet"))
            })
        },
        move || {
            let shown = tab_of(key).map(|t| t.problem_texts()).unwrap_or_default();
            check(
                "an agent that dies while starting says so in German with the line it wrote",
                shown
                    .iter()
                    .any(|t| t == "Fehler: Das Programm hat sich beim Start beendet: no API key set"),
                format!("{shown:?}"),
            );
            if let Some(controller) = controller_of(&root) {
                ide_ui_agent::window_closed(&controller);
            }
            helpers::wait_until(
                || {
                    !ide_process::Supervisor::global()
                        .running()
                        .iter()
                        .any(|p| p.kind == ide_process::Kind::Agent)
                },
                |_| finish(NAME),
            );
        },
    );
}

/// Agents only the registry knows: named as it names them, installable where it pins a version this Mac can use,
/// else unavailable with the reason.
fn register_agents(root: &std::path::Path) {
    use ide_ui_agent::{AcpAgent, Availability, agent_name, availability};
    let of = |id: &str| AcpAgent::find(id).map(|agent| availability(&agent, &[], root));
    let names = ["fast-agent", "unpinned-agent"].map(agent_name);
    let uv = of("fast-agent");
    let unpinned = of("unpinned-agent");
    let docker = of("container-agent");
    check(
        "an agent of the registry is named as the registry names it and offered as it can be had",
        names == ["fast-agent", "Unpinned"]
            && matches!(&uv, Some(Availability::Installable(plan)) if matches!(plan.source, ide_acp::install::Source::Uv { .. }))
            && matches!(&unpinned, Some(Availability::Unavailable(why)) if why.ends_with("nennt keine feste Version und wird deshalb nicht installiert"))
            && matches!(&docker, Some(Availability::Unavailable(why)) if why == "Nur als docker erhältlich, was hier nicht unterstützt wird"),
        format!("{names:?}, {uv:?}, {unpinned:?}, {docker:?}"),
    );
}

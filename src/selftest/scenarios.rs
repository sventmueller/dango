//! Every scenario the app carries, with what the runner in `xtask/src/selftest.rs` needs to know to run it; the
//! runner asks for this table with `--list-scenarios` instead of keeping lists of its own.

use std::time::Duration;

use super::*;

/// How a scenario's launches end.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Launches {
    /// One launch that ends by itself.
    Once,
    /// A first launch that quits as the user does and a second one in the same home that checks what came back.
    Quit,
    /// A first launch that kills itself with `SIGKILL` once it has reported and a second one in the same home that
    /// checks what survived.
    Crash,
}

/// One scenario and how to run it.
pub(crate) struct Scenario {
    pub(crate) name: &'static str,
    pub(crate) run: fn(),
    /// How long it may run before the runner kills it as hung.
    pub(crate) limit: Duration,
    /// Runs without network, account, real agent CLI or person, so coverage runs it.
    pub(crate) offline: bool,
    pub(crate) launches: Launches,
    /// Arguments for the launch, which go to the app's defaults before anything reads them.
    pub(crate) arguments: &'static [&'static str],
    /// Needs the app built with dictation.
    pub(crate) dictation: bool,
    /// Measures time against a budget, or waits on a limit that the load of other scenarios breaks, so the runner
    /// starts it only while no other scenario runs.
    pub(crate) alone: bool,
    /// Uses the Mac's pasteboard, which a real program outside the app reads too, instead of one of the run's own.
    pub(crate) system_pasteboard: bool,
}

const SHORT: Duration = Duration::from_secs(120);
/// Dozens of git commands, commits and pushes against local remotes, which take minutes under the load of parallel
/// builds.
const GIT: Duration = Duration::from_secs(360);
/// A real language server indexing, which a busy Mac slows down by minutes; these scenarios end themselves sooner.
const LANGUAGE_SERVER: Duration = Duration::from_secs(600);
const A11Y: Duration = Duration::from_secs(300);

const fn plain(name: &'static str, run: fn()) -> Scenario {
    Scenario {
        name,
        run,
        limit: SHORT,
        offline: true,
        launches: Launches::Once,
        arguments: &[],
        dictation: false,
        alone: false,
        system_pasteboard: false,
    }
}

const fn timed(name: &'static str, run: fn()) -> Scenario {
    Scenario {
        alone: true,
        ..plain(name, run)
    }
}

const fn limited(name: &'static str, run: fn(), limit: Duration) -> Scenario {
    Scenario {
        limit,
        ..plain(name, run)
    }
}

const fn outside(name: &'static str, run: fn()) -> Scenario {
    Scenario {
        offline: false,
        ..plain(name, run)
    }
}

const fn launched(name: &'static str, run: fn(), launches: Launches) -> Scenario {
    Scenario {
        launches,
        ..plain(name, run)
    }
}

const fn with_arguments(
    name: &'static str,
    run: fn(),
    limit: Duration,
    arguments: &'static [&'static str],
) -> Scenario {
    Scenario {
        limit,
        arguments,
        ..plain(name, run)
    }
}

#[cfg(feature = "dictation")]
fn dictation() {
    super::dictation::run();
}

#[cfg(not(feature = "dictation"))]
fn dictation() {
    give_up("dictation", "the app is built with dictation");
}

pub(crate) const SCENARIOS: &[Scenario] = &[
    plain("probe", probe),
    plain("app-context", app_context::run),
    plain("app-menus", app_menus::run),
    plain("activity-actions", activity_actions::run),
    plain("settings-general", settings_general::run),
    plain("settings-pages", settings_pages::run),
    plain("settings-toolbar", settings_toolbar::run),
    // Greek has the longest tab labels.
    with_arguments(
        "settings-toolbar-long",
        settings_toolbar::run,
        SHORT,
        &["-AppleLanguages", "(el)"],
    ),
    plain("window-commands", window_commands::run),
    plain("markdown-commands", markdown_commands::run),
    limited("a11y", a11y::run, A11Y),
    // macOS turns an app right to left only with `AppleTextDirection` on, which it sets with a right-to-left language
    // of the Mac or of the app; the language alone keeps it left to right.
    Scenario {
        alone: true,
        ..with_arguments(
            "a11y-rtl",
            a11y::run,
            A11Y,
            &["-AppleLanguages", "(ar)", "-AppleTextDirection", "YES"],
        )
    },
    plain("acp", acp::run),
    plain("acp-actions", acp_actions::run),
    plain("acp-dialogs", acp_dialogs::run),
    plain("codex", codex::run),
    plain("opencode", opencode::run),
    plain("own-agent", own_agent::run),
    // The decoding service that fills a pasted picture's tile gives up after 10 s, which it reaches under that load.
    Scenario {
        alone: true,
        ..limited("agent", agent::run, GIT)
    },
    plain("agent-actions", agent_actions::run),
    plain("agent-dialogs", agent_dialogs::run),
    timed("agent-perf", agent_perf::run),
    plain("agent-background", agent_background::run),
    plain("agent-sleep", agent_sleep::run),
    plain("file-icons", file_icons::run),
    outside("agent-live", agent_live::live),
    plain("browser", browser::run),
    timed("browser-dev", browser_dev::run),
    plain("browser-view", browser_view::run),
    plain("editor-extras", extras::run),
    plain("export", export::run),
    plain("tabs", tabs::run),
    plain("replace-folder", replace::run),
    plain("quit", quit::run),
    plain("stored-rules", stored_rules::run),
    plain("preview", preview::run),
    plain("markdown", markdown::run),
    plain("panels", panels::run),
    plain("panes", panes::run),
    limited("git", git::run, GIT),
    limited("git-stash", git::stash::run, GIT),
    limited("git-message", git::message::run, GIT),
    limited("ci", ci::run, GIT),
    outside("ci-live", ci::live),
    plain("ci-login", ci::login),
    plain("ci-settings", ci::hosts),
    limited("worktree", worktree::run, GIT),
    plain("worktree-git", worktree_git::run),
    limited("lsp", lsp::run, LANGUAGE_SERVER),
    limited("lsp-navigation", lsp_navigation::run, LANGUAGE_SERVER),
    limited("lsp-actions", lsp_actions::run_rust, LANGUAGE_SERVER),
    limited("lsp-actions-go", lsp_actions::run_go, LANGUAGE_SERVER),
    Scenario {
        offline: false,
        ..limited("lsp-diagnostics", lsp_diagnostics::run, Duration::from_secs(1200))
    },
    plain("language-servers", servers::run),
    plain("editor-multi", editor_multi::run),
    timed("editor-scroll", editor_scroll::run),
    plain("editor-undo", editor_undo::run),
    plain("editor-commands", editor_commands::run),
    timed("edit-prediction", predict::run),
    timed("editor-insets", insets::run),
    Scenario {
        alone: true,
        ..limited("inlay-hints", inlay::run, Duration::from_secs(600))
    },
    limited("inlay-repo", inlay_repo::run, LANGUAGE_SERVER),
    plain("editor-blame", blame::run),
    timed("editor-minimap", minimap::run),
    plain("editor-sticky", sticky::run),
    plain("editor-vim", vim::run),
    timed("editor-line-diagnostics", line_diagnostics::run),
    plain("page-preview", page_preview::run),
    plain("prettier-save", prettier::run),
    plain("settings-sync", settings_sync::run),
    Scenario {
        alone: true,
        ..limited("tests-panel", tests_panel::run, Duration::from_secs(400))
    },
    plain("containers-panel", containers::run),
    plain("leitstand", leitstand::run),
    plain("leitstand-attach", leitstand::attach::run),
    limited("leitstand-order", leitstand::order::run, GIT),
    limited("leitstand-queue", leitstand::queue::run, Duration::from_secs(900)),
    limited("leitstand-metrics", leitstand::metrics::run, GIT),
    limited("leitstand-gaps", leitstand::gaps::run, GIT),
    limited("leitstand-hunks", leitstand::hunks::run, GIT),
    limited("leitstand-intake", leitstand::intake::run, GIT),
    limited("leitstand-setup", leitstand::setup::run, GIT),
    Scenario {
        limit: GIT,
        ..launched("leitstand-pause", leitstand::pause::run, Launches::Quit)
    },
    Scenario {
        offline: false,
        ..limited("companions", companions::run, Duration::from_secs(1200))
    },
    Scenario {
        offline: false,
        ..limited(
            "companions-eslint-latest",
            companions::run_latest,
            Duration::from_secs(1200),
        )
    },
    plain("search", search::run),
    timed("snappy-tree", snappy::tree),
    timed("snappy-resize", snappy_input::resize),
    timed("snappy-scroll", snappy_scroll::run),
    timed("snappy-tabs", snappy_input::tabs),
    timed("snappy-restore", snappy_restore::run),
    timed("snappy-launch", snappy_launch::run),
    timed("snappy-recolor", snappy_recolor::run),
    timed("snappy-recolor-markdown", snappy_recolor_markdown::run),
    timed("snappy-preview-markdown", snappy_preview::run),
    Scenario {
        alone: true,
        ..limited("snappy-tests", snappy_input::tests, Duration::from_secs(400))
    },
    timed("terminal", terminal::run),
    plain("terminal-view", terminal_view::run),
    Scenario {
        system_pasteboard: true,
        ..outside("paste-cli", paste_cli::run)
    },
    launched("recovery", recovery::run, Launches::Crash),
    Scenario {
        alone: true,
        ..launched("restore", restore::run, Launches::Quit)
    },
    plain("conflict", conflict::run),
    plain("debug", debug::run),
    launched("debug-restore", debug_restore::run, Launches::Quit),
    plain("debug-menus", debug_menus::run),
    Scenario {
        dictation: true,
        ..plain("dictation", dictation)
    },
    outside("show", show),
];

/// The scenario named `name`.
pub(crate) fn named(name: &str) -> Option<&'static Scenario> {
    SCENARIOS.iter().find(|scenario| scenario.name == name)
}

/// The table as JSON lines, one scenario each, for the runner.
pub(crate) fn listing() -> String {
    SCENARIOS
        .iter()
        .map(|s| {
            let launches = match s.launches {
                Launches::Once => "once",
                Launches::Quit => "quit",
                Launches::Crash => "crash",
            };
            let arguments: Vec<String> = s.arguments.iter().map(|a| quote(a)).collect();
            format!(
                r#"{{"name": {}, "limit_secs": {}, "offline": {}, "launches": "{launches}", "arguments": [{}], "dictation": {}, "alone": {}}}"#,
                quote(s.name),
                s.limit.as_secs(),
                s.offline,
                arguments.join(", "),
                s.dictation,
                s.alone,
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
}

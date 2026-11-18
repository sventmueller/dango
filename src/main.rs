mod about;
mod activity;
mod app;
mod browser;
mod ci_panel;
mod command_line;
mod containers_panel;
mod context;
mod debug;
mod diag;
mod editor;
mod environment;
mod feedback;
mod host;
mod leitstand;
mod leitstand_ui;
mod markdown;
mod menu;
mod navigation;
mod preview;
mod project;
mod quit;
#[cfg(feature = "selftest")]
mod selftest;
mod session;
mod settings;
mod settings_sync;
mod site_preview;
mod terminal;
mod tests_panel;
mod updates;
mod util;
mod window;
mod worktrees;

use objc2::MainThreadMarker;
use objc2::runtime::ProtocolObject;
use objc2_app_kit::{NSApplication, NSApplicationActivationPolicy};
use objc2_foundation::NSBundle;
use splat_i18n as i18n;

pub(crate) use ide_ui_icons as file_icon;

fn main() {
    // The self-test's ACP agent is this program, started by the self-test with this variable instead of a window.
    #[cfg(feature = "selftest")]
    if let Ok(scenario) = std::env::var("IDE_ACP_TEST_AGENT") {
        ide_acp::test_agent::run(std::io::stdin().lock(), std::io::stdout().lock(), &scenario);
        return;
    }
    // The self-test's Codex app-server is this program too, started with this variable (E147).
    #[cfg(feature = "selftest")]
    if std::env::var_os("IDE_CODEX_TEST_SERVER").is_some() {
        let args: Vec<String> = std::env::args().skip(1).collect();
        std::process::exit(ide_codex::test_server::main(&args));
    }
    // The self-test's OpenCode server is this program too, started with this variable (E147).
    #[cfg(feature = "selftest")]
    if std::env::var_os("IDE_OPENCODE_TEST_SERVER").is_some() {
        let scenario = std::env::var(ide_opencode::test_server::SCENARIO_VARIABLE).unwrap_or_else(|_| "basic".into());
        if std::env::args().any(|a| a == "--version") {
            println!("{}", ide_opencode::test_server::version(&scenario));
            return;
        }
        let log = std::env::var_os(ide_opencode::test_server::LOG_VARIABLE).map(std::path::PathBuf::from);
        ide_opencode::test_server::run(std::io::stdin(), std::io::stdout().lock(), &scenario, log);
        return;
    }
    i18n::install(&app::localized_strings());
    ide_ui_common::window::install(&host::AppHost);
    ide_ui_debug::install(&host::AppHost);
    ide_ui_export::install(&host::AppHost);
    ide_ui_git::install(&host::AppHost);
    ide_ui_agent::install(&host::AppHost);
    ide_ui_browser::install(&host::AppHost);
    ide_ui_terminal::install(&host::AppHost);
    ide_ui_lsp::install(&host::AppHost);
    #[cfg(feature = "dictation")]
    ide_ui_dictation::install(&host::AppHost);
    if let Some(id) = NSBundle::mainBundle().bundleIdentifier() {
        ide_core::paths::set_identity(&id.to_string());
    }
    ide_core::diag::install(Box::new(|what, detail| diag::report(what, detail)));
    #[cfg(feature = "selftest")]
    selftest::prepare();
    let report = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        diag::report("panic", info.location().map_or_else(String::new, ToString::to_string));
        report(info);
    }));
    let mtm = MainThreadMarker::new().expect("the app starts on the main thread");
    ide_ui_common::util::stall::install();
    preview::start(mtm);
    #[cfg(feature = "selftest")]
    selftest::application::install(mtm);
    let app = NSApplication::sharedApplication(mtm);
    app.setActivationPolicy(NSApplicationActivationPolicy::Regular);
    let delegate = app::AppDelegate::new(mtm);
    app.setDelegate(Some(ProtocolObject::from_ref(&*delegate)));
    #[cfg(feature = "dictation")]
    ide_ui_dictation::prepare();
    updates::start(mtm);
    menu::install(&app, &delegate, mtm);
    environment::start();
    app.run();
}

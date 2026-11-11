mod activity;
mod agent;
mod agent_panel;
mod app;
mod browser;
mod ci_panel;
mod command_line;
mod diag;
#[cfg(feature = "dictation")]
mod dictation;
mod editor;
mod environment;
mod file_icon;
mod git_panel;
mod lsp;
mod markdown;
mod menu;
mod navigation;
mod own_agent;
mod preview;
mod project;
#[cfg(feature = "selftest")]
mod selftest;
mod session;
mod settings;
mod terminal;
mod updates;
mod util;
mod window;
mod worktrees;

use objc2::MainThreadMarker;
use objc2::runtime::ProtocolObject;
use objc2_app_kit::{NSApplication, NSApplicationActivationPolicy};
use objc2_foundation::NSBundle;
use splat_i18n as i18n;

fn main() {
    // The self-test's ACP agent is this program, started by the self-test with this variable instead of a window.
    #[cfg(feature = "selftest")]
    if let Ok(scenario) = std::env::var("IDE_ACP_TEST_AGENT") {
        ide_acp::test_agent::run(std::io::stdin().lock(), std::io::stdout().lock(), &scenario);
        return;
    }
    i18n::install(&app::localized_strings());
    if let Some(id) = NSBundle::mainBundle().bundleIdentifier() {
        ide_core::paths::set_identity(&id.to_string());
    }
    #[cfg(feature = "selftest")]
    selftest::prepare();
    let report = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        diag::report("panic", info.location().map_or_else(String::new, ToString::to_string));
        report(info);
    }));
    let mtm = MainThreadMarker::new().expect("the app starts on the main thread");
    preview::start(mtm);
    #[cfg(feature = "selftest")]
    selftest::application::install(mtm);
    let app = NSApplication::sharedApplication(mtm);
    app.setActivationPolicy(NSApplicationActivationPolicy::Regular);
    let delegate = app::AppDelegate::new(mtm);
    app.setDelegate(Some(ProtocolObject::from_ref(&*delegate)));
    #[cfg(feature = "dictation")]
    dictation::prepare();
    updates::start(mtm);
    menu::install(&app, &delegate, mtm);
    environment::start();
    app.run();
}

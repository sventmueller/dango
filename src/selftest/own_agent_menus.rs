//! The end of the `own-agent` scenario: the composer's menus of the own agent's tab, the endpoint's models and the
//! effort levels, the profile and the approval modes, as picking from them changes the pills.

use std::path::PathBuf;
use std::time::{Duration, Instant};

use super::agent::tab_of;
use super::agent_actions::{index_of, titles};
use super::{check, finish};
use crate::agent::TabMenu;
use crate::util::main_after;

const NAME: &str = "own-agent";

fn soon(what: &'static str, ready: impl Fn() -> bool + Send + 'static, then: impl FnOnce() + Send + 'static) {
    let started = Instant::now();
    poll(what, started, ready, then);
}

fn poll(
    what: &'static str,
    started: Instant,
    ready: impl Fn() -> bool + Send + 'static,
    then: impl FnOnce() + Send + 'static,
) {
    if ready() {
        return then();
    }
    if started.elapsed() > Duration::from_secs(10) {
        check(what, false, "waited in vain");
        return finish(NAME);
    }
    main_after(Duration::from_millis(100), move || poll(what, started, ready, then));
}

/// Picks from the menus of the tab `key`, whose model is `model`, then ends the scenario.
pub fn run(_root: PathBuf, key: u64, model: &'static str) {
    let Some(tab) = tab_of(key) else { return finish(NAME) };
    let slash = tab.menu(TabMenu::Slash);
    check(
        "the own agent's slash menu offers compacting",
        titles(&slash) == ["/compact"],
        format!("{:?}", titles(&slash)),
    );
    let menu = tab.menu(TabMenu::Model);
    let shown = titles(&menu);
    let at = |title: &str| index_of(&menu, title).and_then(|n| menu.itemAtIndex(n));
    check(
        "the model menu lists the endpoint's models, the one in use ticked, then the effort levels",
        at(model).is_some_and(|i| i.state() == objc2_app_kit::NSControlStateValueOn)
            && shown.iter().any(|t| t == "Denktiefe")
            && shown.iter().any(|t| t == "Wie der Anbieter")
            && shown.last().map(String::as_str) == Some("max"),
        format!("{shown:?}"),
    );
    if let Some(high) = index_of(&menu, "high") {
        menu.performActionForItemAtIndex(high);
    }
    let menu = tab.menu(TabMenu::Model);
    if let Some(same) = index_of(&menu, model) {
        menu.performActionForItemAtIndex(same);
    }
    soon(
        "the effort reaches the pill",
        move || tab_of(key).is_some_and(|t| t.pills().0 == format!("{model} · high")),
        move || {
            check("a picked effort shows in the model pill", true, "");
            modes(key);
        },
    );
}

fn modes(key: u64) {
    let Some(tab) = tab_of(key) else { return finish(NAME) };
    let menu = tab.menu(TabMenu::Mode);
    check(
        "the mode menu offers the profiles, then the approval modes",
        titles(&menu)
            == [
                "Nur fragen",
                "Schreiben",
                "",
                "Manuell",
                "Änderungen erlauben",
                "Alles erlauben",
            ],
        format!("{:?}", titles(&menu)),
    );
    menu.performActionForItemAtIndex(0);
    let asking = tab.pills().1;
    let menu = tab.menu(TabMenu::Mode);
    let locked = (3..6).all(|n| menu.itemAtIndex(n).is_some_and(|i| !i.isEnabled()));
    menu.performActionForItemAtIndex(1);
    let menu = tab.menu(TabMenu::Mode);
    menu.performActionForItemAtIndex(5);
    check(
        "asking only shows in the pill and locks the approval modes; writing again allows everything",
        asking == "Nur fragen" && locked && tab.pills().1 == "Alles erlauben",
        format!("{asking}, locked {locked}, {}", tab.pills().1),
    );
    finish(NAME);
}

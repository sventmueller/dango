//! Formulas and diagrams in agent answers, drawn by the hidden renderer the Markdown preview uses (E1): an answer shows
//! them as code until they are drawn, then its card is built again.

use std::cell::{Cell, RefCell};
use std::collections::HashSet;
use std::path::PathBuf;
use std::rc::Rc;

use objc2::MainThreadMarker;
use objc2_app_kit::NSApplication;
use splat_file_state::Stamp;
use splat_markdown::palette::Appearance;
use splat_render::Viewer;
use splat_render::images::ImageFile;
use splat_render::js::{self, Key, Owner};

use ide_ui_common::util::main_soon;

/// The most figures the answers keep asked for; the oldest give way.
const KEPT: usize = 400;

struct Asked {
    viewer: Rc<dyn Viewer>,
    owner: Option<Owner>,
    keys: Vec<Key>,
    ready: HashSet<Key>,
}

/// The figures the AI tabs asked the renderer for.
#[derive(Default)]
pub(crate) struct Figures {
    asked: RefCell<Option<Asked>>,
    asking: Cell<bool>,
    collect: RefCell<Option<Vec<Key>>>,
}

/// Records the figures a card uses, including those it finds in the renderer's cache.
pub(crate) fn collect<R>(build: impl FnOnce() -> R) -> (R, Vec<Key>) {
    let state = crate::host::state();
    struct Capture<'a> {
        collect: &'a RefCell<Option<Vec<Key>>>,
        previous: Option<Vec<Key>>,
    }
    impl Drop for Capture<'_> {
        fn drop(&mut self) {
            *self.collect.borrow_mut() = self.previous.take();
        }
    }
    let capture = Capture {
        collect: &state.figures.collect,
        previous: state.figures.collect.replace(Some(Vec::new())),
    };
    let result = build();
    let keys = state.figures.collect.borrow_mut().take().unwrap_or_default();
    drop(capture);
    (result, keys)
}

/// Whether the app shows itself dark, which the pictures of formulas and diagrams follow.
#[cfg_attr(
    not(feature = "selftest"),
    expect(unreachable_pub, reason = "the self-test re-exports it")
)]
pub fn is_dark(mtm: MainThreadMarker) -> bool {
    let appearance = NSApplication::sharedApplication(mtm).effectiveAppearance();
    matches!(
        splat_render::style::appearance_of(&appearance),
        Appearance::Dark | Appearance::DarkHighContrast
    )
}

/// Records `all` as what a card uses and asks for `new` on a later turn, as a card is built while the table lays out.
pub(crate) fn want_among(all: &[Key], new: Vec<Key>) {
    let state = crate::host::state();
    if let Some(collected) = state.figures.collect.borrow_mut().as_mut() {
        collected.extend(all.iter().cloned());
    }
    ask_for(&state.figures, new);
}

/// Asks for `keys` on a later turn among the figures the answers asked for in `figures`.
fn ask_for(figures: &Figures, keys: Vec<Key>) {
    if keys.is_empty() {
        return;
    }
    let added = {
        let mut asked = figures.asked.borrow_mut();
        let asked = asked.get_or_insert_with(|| Asked {
            viewer: Rc::new(Answers),
            owner: None,
            keys: Vec::new(),
            ready: HashSet::new(),
        });
        let before = asked.keys.len();
        for key in keys {
            if !asked.keys.contains(&key) {
                asked.keys.push(key);
            }
        }
        let excess = asked.keys.len().saturating_sub(KEPT);
        asked.keys.drain(..excess);
        let kept: HashSet<_> = asked.keys.iter().collect();
        asked.ready.retain(|key| kept.contains(key));
        asked.keys.len() != before || excess > 0
    };
    if added && !figures.asking.replace(true) {
        main_soon(move || {
            let state = crate::host::state();
            state.figures.asking.set(false);
            if let Some(mtm) = MainThreadMarker::new() {
                ask(&state.figures, mtm);
            }
        });
    }
}

/// Has the renderer draw the figures `figures` holds as asked for.
fn ask(figures: &Figures, mtm: MainThreadMarker) {
    let Some((viewer, owner, keys)) = ({
        let asked = figures.asked.borrow();
        asked
            .as_ref()
            .map(|asked| (asked.viewer.clone(), asked.owner, asked.keys.clone()))
    }) else {
        return;
    };
    let renderer = js::renderer(mtm);
    let owner = owner.unwrap_or_else(|| renderer.listen(&viewer));
    if let Some(asked) = figures.asked.borrow_mut().as_mut() {
        asked.owner = Some(owner);
    }
    renderer.request(owner, keys);
}

/// Builds the cards anew when more of the answers' figures are drawn than when they were last built; the renderer
/// tells every document when it has run empty, also when it drew only for a preview.
fn arrived() {
    let state = crate::host::state();
    let changed: Vec<Key> = {
        let mut asked = state.figures.asked.borrow_mut();
        match asked.as_mut() {
            Some(asked) => {
                let ready: HashSet<_> = asked.keys.iter().filter(|key| js::ready(key)).cloned().collect();
                let changed = ready.symmetric_difference(&asked.ready).cloned().collect();
                asked.ready = ready;
                changed
            }
            None => Vec::new(),
        }
    };
    if !changed.is_empty() {
        for tab in state.tabs() {
            tab.timeline().figures_arrived(&changed);
        }
    }
}

/// The answers of all AI tabs as one document of the renderer.
struct Answers;

impl Viewer for Answers {
    fn renders_arrived(&self) {
        main_soon(arrived);
    }

    fn image_arrived(&self, _url: String) {}

    fn image_loaded(&self, _path: PathBuf, _file: ImageFile) {}

    fn images_changed(&self, _changed: Vec<(PathBuf, Stamp)>) {}

    fn web_images_shown(&self) -> Vec<String> {
        Vec::new()
    }

    fn has_closed(&self) -> bool {
        false
    }
}

#[cfg(feature = "selftest")]
mod selftest_probe;
#[cfg(feature = "selftest")]
pub use selftest_probe::*;

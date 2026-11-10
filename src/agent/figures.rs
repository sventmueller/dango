//! Formulas and diagrams in agent answers, drawn by the hidden renderer the Markdown preview uses (E1): an answer shows
//! them as code until they are drawn, then its card is built again.

use std::cell::{Cell, RefCell};
use std::path::PathBuf;
use std::rc::Rc;

use objc2::MainThreadMarker;
use objc2_app_kit::NSApplication;
use splat_file_state::Stamp;
use splat_markdown::palette::Appearance;
use splat_render::Viewer;
use splat_render::images::ImageFile;
use splat_render::js::{self, Key, Owner};

use crate::util::main_soon;

/// The most figures the answers keep asked for; the oldest give way.
const KEPT: usize = 400;

struct Asked {
    viewer: Rc<dyn Viewer>,
    owner: Option<Owner>,
    keys: Vec<Key>,
    /// How many of `keys` were drawn when the cards were last built for them.
    drawn: usize,
}

thread_local! {
    static ASKED: RefCell<Option<Asked>> = const { RefCell::new(None) };
    static ASKING: Cell<bool> = const { Cell::new(false) };
}

/// Whether the app shows itself dark, which the pictures of formulas and diagrams follow.
pub fn is_dark(mtm: MainThreadMarker) -> bool {
    let appearance = NSApplication::sharedApplication(mtm).effectiveAppearance();
    matches!(
        splat_render::style::appearance_of(&appearance),
        Appearance::Dark | Appearance::DarkHighContrast
    )
}

/// Asks for `keys` beside what earlier answers asked for, on a later turn: a card is built while the table lays out.
pub fn want(keys: Vec<Key>) {
    if keys.is_empty() {
        return;
    }
    let added = ASKED.with(|asked| {
        let mut asked = asked.borrow_mut();
        let asked = asked.get_or_insert_with(|| Asked {
            viewer: Rc::new(Answers),
            owner: None,
            keys: Vec::new(),
            drawn: 0,
        });
        let before = asked.keys.len();
        for key in keys {
            if !asked.keys.contains(&key) {
                asked.keys.push(key);
            }
        }
        let excess = asked.keys.len().saturating_sub(KEPT);
        asked.keys.drain(..excess);
        asked.keys.len() != before || excess > 0
    });
    if added && !ASKING.replace(true) {
        main_soon(move || {
            ASKING.set(false);
            if let Some(mtm) = MainThreadMarker::new() {
                ask(mtm);
            }
        });
    }
}

fn ask(mtm: MainThreadMarker) {
    let Some((viewer, owner, keys)) = ASKED.with(|asked| {
        let asked = asked.borrow();
        let asked = asked.as_ref()?;
        Some((asked.viewer.clone(), asked.owner, asked.keys.clone()))
    }) else {
        return;
    };
    let renderer = js::renderer(mtm);
    let owner = owner.unwrap_or_else(|| renderer.listen(&viewer));
    ASKED.with(|asked| {
        if let Some(asked) = asked.borrow_mut().as_mut() {
            asked.owner = Some(owner);
        }
    });
    renderer.request(owner, keys);
}

/// Builds the cards anew when more of the answers' figures are drawn than when they were last built; the renderer
/// tells every document when it has run empty, also when it drew only for a preview.
fn arrived() {
    let more = ASKED.with(|asked| {
        let mut asked = asked.borrow_mut();
        let Some(asked) = asked.as_mut() else { return false };
        let drawn = asked.keys.iter().filter(|key| js::ready(key)).count();
        drawn != std::mem::replace(&mut asked.drawn, drawn)
    });
    if more {
        for tab in super::all() {
            tab.timeline().figures_arrived();
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

/// Whether the answers asked for `key`, for the self-test.
#[cfg(feature = "selftest")]
pub fn asked_for(key: &Key) -> bool {
    ASKED.with(|asked| asked.borrow().as_ref().is_some_and(|asked| asked.keys.contains(key)))
}

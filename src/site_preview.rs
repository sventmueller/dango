//! The live preview of static pages (E131): a page of a trusted folder opens in a browser tab from a GET-only server
//! of the app, which serves the project folder at its root; the tab reloads 200 ms after the last change of a file
//! below it, and the server ends once no tab shows its pages.

use std::cell::{Cell, RefCell};
use std::path::{Path, PathBuf};
use std::time::Duration;

use ide_project::site::{COOKIE, Site};
use objc2::MainThreadMarker;
use splat_i18n::t;

use crate::context::AppContext;
use crate::window::WindowController;

/// How long the files must rest before the tabs reload.
const QUIET: Duration = Duration::from_millis(200);
/// How often a running server looks whether a tab still shows its pages.
const SWEEP: Duration = Duration::from_secs(10);

/// The servers of the open previews, one per folder.
#[derive(Default)]
pub(crate) struct SitePreviews {
    sites: RefCell<Vec<Site>>,
    /// Counts the changes, so that only the last of a burst reloads.
    changes: Cell<u64>,
    sweeping: Cell<bool>,
}

fn previews(mtm: MainThreadMarker) -> std::rc::Rc<AppContext> {
    AppContext::get(mtm)
}

/// A live preview as the layout keeps it, which opens again with a server on a new port.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub(crate) struct KeptSite {
    /// The page's file.
    pub(crate) path: PathBuf,
}

impl ide_core::layout::Kind for KeptSite {
    const NAME: &'static str = "sitepreview";
}

/// Whether `path` is a page the preview opens.
pub(crate) fn is_page(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| e.eq_ignore_ascii_case("html") || e.eq_ignore_ascii_case("htm"))
}

/// Opens `page` in a browser tab of `controller`, served from its project folder; only in a trusted folder, since the
/// page's scripts can read every file the server serves.
pub(crate) fn open(controller: &WindowController, page: &Path) {
    let mtm = objc2::MainThreadOnly::mtm(controller);
    let root = controller
        .project()
        .unwrap_or_else(|| page.parent().map(Path::to_path_buf).unwrap_or_default());
    if !crate::project::is_trusted(&root) {
        ide_ui_common::alert::warn(
            mtm,
            t("Vorschau nur in vertrauten Ordnern"),
            t("Die Seite könnte über die Vorschau jede Datei des Ordners lesen. Vertraue dem Ordner zuerst."),
        );
        return;
    }
    let cx = previews(mtm);
    let started = {
        let mut sites = cx.site_previews().sites.borrow_mut();
        match sites
            .iter()
            .position(|site| root.canonicalize().is_ok_and(|r| site.root() == r))
        {
            Some(index) => Ok(index),
            None => secret()
                .ok_or_else(|| std::io::Error::other("the system's random source gave nothing"))
                .and_then(|secret| Site::start(&root, secret))
                .map(|site| {
                    sites.push(site);
                    sites.len() - 1
                }),
        }
    };
    let index = match started {
        Ok(index) => index,
        Err(error) => return crate::diag::report("starting the page preview's server", error),
    };
    let (url, secret) = {
        let sites = cx.site_previews().sites.borrow();
        let site = &sites[index];
        (site.url_of(page), site.secret().to_string())
    };
    let Some(url) = url else { return };
    let window = controller.id();
    let set = ide_ui_browser::set_local_cookie(mtm, COOKIE, &secret, move || {
        ide_ui_browser::open_tab(mtm, window, Some(&url));
    });
    if set.is_none() {
        crate::diag::report("the page preview's cookie", "WebKit made no cookie");
    }
    sweep_later(mtm);
}

/// The page file a preview tab at `url` shows, for opening it again after a restart.
pub(crate) fn page_of(mtm: MainThreadMarker, url: &str) -> Option<PathBuf> {
    previews(mtm)
        .site_previews()
        .sites
        .borrow()
        .iter()
        .find_map(|site| site.file_of(url))
}

/// Files below `root` changed on disk: the previews of that folder reload once they rest.
pub(crate) fn files_changed(mtm: MainThreadMarker, root: &Path, files: &[PathBuf]) {
    let cx = previews(mtm);
    let origins: Vec<String> = {
        let sites = cx.site_previews().sites.borrow();
        if sites.is_empty() {
            return;
        }
        let canonical = root.canonicalize().ok();
        sites
            .iter()
            .filter(|site| {
                let same_root = canonical.as_deref() == Some(site.root());
                files.iter().any(|file| {
                    file.starts_with(site.root())
                        || same_root
                            && !file
                                .components()
                                .any(|c| c.as_os_str().to_string_lossy().starts_with('.'))
                })
            })
            .map(Site::origin)
            .collect()
    };
    if origins.is_empty() {
        return;
    }
    let generation = cx.site_previews().changes.get() + 1;
    cx.site_previews().changes.set(generation);
    crate::util::main_after(QUIET, move || {
        let Some(mtm) = MainThreadMarker::new() else { return };
        if previews(mtm).site_previews().changes.get() != generation {
            return;
        }
        for controller in previews(mtm).windows().all() {
            for origin in &origins {
                ide_ui_browser::reload_matching(controller.id(), origin);
            }
        }
    });
}

/// Ends the servers whose pages no tab shows, and looks again later while any runs.
fn sweep_later(mtm: MainThreadMarker) {
    let cx = previews(mtm);
    if cx.site_previews().sweeping.replace(true) {
        return;
    }
    crate::util::main_after(SWEEP, || {
        let Some(mtm) = MainThreadMarker::new() else { return };
        let cx = previews(mtm);
        cx.site_previews().sweeping.set(false);
        let shown: Vec<String> = previews(mtm)
            .windows()
            .all()
            .iter()
            .flat_map(|controller| ide_ui_browser::urls(controller.id()))
            .collect();
        cx.site_previews()
            .sites
            .borrow_mut()
            .retain(|site| shown.iter().any(|url| url.starts_with(&site.origin())));
        if !cx.site_previews().sites.borrow().is_empty() {
            sweep_later(mtm);
        }
    });
}

/// A value no other process of the Mac can guess, from the system's random source; `None` when it gave nothing.
fn secret() -> Option<String> {
    let mut bytes = [0u8; 16];
    let mut random = std::fs::File::open("/dev/urandom").ok()?;
    std::io::Read::read_exact(&mut random, &mut bytes).ok()?;
    Some(bytes.iter().map(|b| format!("{b:02x}")).collect())
}

#[cfg(feature = "selftest")]
mod selftest_probe;
#[cfg(feature = "selftest")]
pub(crate) use selftest_probe::*;

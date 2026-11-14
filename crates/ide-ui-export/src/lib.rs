//! A Markdown document as PDF, a self-contained HTML page or a Word file, through splat's crates: the text as the
//! preview sets it for paper, `splat_markdown::html` and `splat_markdown::docx`. Formulas and diagrams go in as the
//! hidden renderer drew them, so an export waits for them first; pictures come from the document's folder, and from
//! the web only in a trusted folder (E45).
//!
//! An export never holds up the window: it builds its file on a thread of its own and comes to the main thread only
//! for AppKit's pictures and text, a short step at a time, while a sheet says what runs and offers to cancel it.

mod pdf;
mod pictures;
mod sheet;

use ide_core::explain::Explained;
use std::cell::{Cell, OnceCell, RefCell};
use std::path::PathBuf;
use std::rc::{Rc, Weak};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, OnceLock};
use std::time::Duration;

use objc2::MainThreadMarker;
use objc2_app_kit::{NSModalResponseOK, NSSavePanel, NSWindow};
use objc2_foundation::NSURL;
use splat_file_state::Stamp;
use splat_i18n::{t, tf};
use splat_markdown::doc::Figure;
use splat_markdown::docx::{WordExport, to_docx};
use splat_markdown::html::{Export, to_html};
use splat_render::Viewer;
use splat_render::images::ImageFile;
use splat_render::js::{self, Key};
use splat_render::remote;

use ide_ui_common::util::{main_after, main_soon, ns};
use sheet::Sheet;

/// How long an export waits for formulas and diagrams; what is not drawn by then goes in as its source.
const FIGURE_WAIT: Duration = Duration::from_secs(20);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Pdf,
    Html,
    Word,
}

impl Kind {
    /// The extension of the files of this kind.
    ///
    /// ```
    /// use ide_ui_export::Kind;
    ///
    /// assert_eq!(Kind::Word.extension(), "docx");
    /// ```
    pub fn extension(self) -> &'static str {
        match self {
            Kind::Pdf => "pdf",
            Kind::Html => "html",
            Kind::Word => "docx",
        }
    }
}

/// How an export ended.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Outcome {
    Written,
    Cancelled,
    /// Why it failed, in words for the user.
    Failed(String),
}

/// What an export makes a file of.
pub struct Source {
    pub text: String,
    /// Where its relative links and pictures start.
    pub doc_dir: Option<PathBuf>,
    /// The name of the page or document, the file's name without its extension.
    pub title: String,
    /// Whether its folder is trusted, the only case in which pictures from the web go along (E45).
    pub trusted: bool,
}

/// Asks where `source` goes as `kind`, then writes it there, with a sheet on `window` while it runs.
pub fn ask(mtm: MainThreadMarker, window: Option<&NSWindow>, source: Source, kind: Kind) {
    let panel = NSSavePanel::savePanel(mtm);
    panel.setNameFieldStringValue(&ns(&format!("{}.{}", source.title, kind.extension())));
    panel.setCanCreateDirectories(true);
    if let Some(dir) = &source.doc_dir {
        panel.setDirectoryURL(Some(&NSURL::fileURLWithPath_isDirectory(
            &ns(&dir.to_string_lossy()),
            true,
        )));
    }
    if panel.runModal() != NSModalResponseOK {
        return;
    }
    let Some(path) = panel
        .URL()
        .and_then(|url| url.path())
        .map(|p| PathBuf::from(p.to_string()))
    else {
        return;
    };
    start(source, kind, path, window, |_| {});
}

/// Writes `source` as `kind` to `path`, with a sheet on `window` while it runs; `done` hears how it ended, an error
/// also stands in the sheet. The number names the export, for `cancel`.
pub fn start(
    source: Source,
    kind: Kind,
    path: PathBuf,
    window: Option<&NSWindow>,
    done: impl FnOnce(Outcome) + 'static,
) -> u64 {
    let number = state().next.replace(state().next.get() + 1);
    let what = match kind {
        Kind::Pdf => tf("„{}“ wird als PDF exportiert …", &[&source.title]),
        Kind::Html => tf("„{}“ wird als HTML exportiert …", &[&source.title]),
        Kind::Word => tf("„{}“ wird als Word-Dokument exportiert …", &[&source.title]),
    };
    let keys = figures(&source.text);
    let run = Rc::new(Run {
        number,
        cancelled: Arc::new(AtomicBool::new(false)),
        kind,
        path,
        source: RefCell::new(Some(source)),
        keys: keys.clone(),
        viewer: OnceCell::new(),
        started: Cell::new(false),
        pdf: RefCell::new(None),
        sheet: RefCell::new(window.map(|window| Sheet::show(window, &what, number))),
        done: RefCell::new(Some(Box::new(done))),
    });
    let viewer: Rc<dyn Viewer> = Rc::new(Drawing(Rc::downgrade(&run)));
    let _ = run.viewer.set(viewer.clone());
    with_runs(|runs| runs.borrow_mut().push(run));
    if keys.iter().all(drawn) {
        main_soon(move || go(number));
        return number;
    }
    if let Some(mtm) = objc2::MainThreadMarker::new() {
        let renderer = js::renderer(mtm);
        let owner = renderer.listen(&viewer);
        renderer.request(owner, keys);
    }
    main_after(FIGURE_WAIT, move || go(number));
    number
}

/// Stops the export `number`: what it built so far is dropped and no file is written.
pub fn cancel(number: u64) {
    let Some(run) = take(number) else { return };
    run.cancelled.store(true, Ordering::SeqCst);
    if let Some(sheet) = run.sheet.borrow_mut().take() {
        sheet.close();
    }
    run.end(Outcome::Cancelled);
}

/// The button of the sheet of the export `number`: it cancels a running export and closes a failed one's sheet.
fn sheet_button(number: u64, failed: bool) {
    if failed {
        with_failed(|failed| {
            let mut failed = failed.borrow_mut();
            if let Some(at) = failed.iter().position(|(n, _)| *n == number) {
                failed.remove(at).1.close();
            }
        });
    } else {
        cancel(number);
    }
}

/// The app the exports run in, as far as they need it.
pub trait Host {
    /// The exports of the running app.
    fn state(&self) -> Rc<State>;
}

static HOST: OnceLock<&'static (dyn Host + Sync)> = OnceLock::new();

/// Makes `host` the app the exports belong to; once, before the first export.
pub fn install(host: &'static (dyn Host + Sync)) {
    let _ = HOST.set(host);
}

fn state() -> Rc<State> {
    HOST.get()
        .expect("the app installs its host before the first export")
        .state()
}

/// The exports that run and the sheets of those that failed.
pub struct State {
    runs: RefCell<Vec<Rc<Run>>>,
    /// The sheets of exports that failed, which stay until the user closes them.
    failed: RefCell<Vec<(u64, Sheet)>>,
    next: Cell<u64>,
}

impl Default for State {
    fn default() -> Self {
        State {
            runs: RefCell::new(Vec::new()),
            failed: RefCell::new(Vec::new()),
            next: Cell::new(1),
        }
    }
}

fn with_runs<R>(f: impl FnOnce(&RefCell<Vec<Rc<Run>>>) -> R) -> R {
    f(&state().runs)
}

fn with_failed<R>(f: impl FnOnce(&RefCell<Vec<(u64, Sheet)>>) -> R) -> R {
    f(&state().failed)
}

/// Who hears how an export ended.
type Done = Box<dyn FnOnce(Outcome)>;

/// A running export.
struct Run {
    number: u64,
    /// Read by the export's thread, which stops at the next picture or page once it is set.
    cancelled: Arc<AtomicBool>,
    kind: Kind,
    path: PathBuf,
    source: RefCell<Option<Source>>,
    keys: Vec<Key>,
    viewer: OnceCell<Rc<dyn Viewer>>,
    /// Past waiting for formulas and diagrams.
    started: Cell<bool>,
    pdf: RefCell<Option<pdf::Work>>,
    sheet: RefCell<Option<Sheet>>,
    done: RefCell<Option<Done>>,
}

impl Run {
    fn end(&self, outcome: Outcome) {
        if let Some(viewer) = self.viewer.get() {
            splat_render::stops_showing(viewer);
        }
        let done = self.done.borrow_mut().take();
        if let Some(done) = done {
            done(outcome);
        }
    }
}

fn find(number: u64) -> Option<Rc<Run>> {
    with_runs(|runs| runs.borrow().iter().find(|run| run.number == number).cloned())
}

fn take(number: u64) -> Option<Rc<Run>> {
    with_runs(|runs| {
        let mut runs = runs.borrow_mut();
        let at = runs.iter().position(|run| run.number == number)?;
        Some(runs.remove(at))
    })
}

/// The export `number` ended as `outcome`, if it still runs.
fn finish(number: u64, outcome: Outcome) {
    let Some(run) = take(number) else { return };
    let sheet = run.sheet.borrow_mut().take();
    if let Some(sheet) = sheet {
        match &outcome {
            Outcome::Failed(reason) => {
                sheet.failed(reason);
                with_failed(|failed| failed.borrow_mut().push((number, sheet)));
            }
            _ => sheet.close(),
        }
    }
    run.end(outcome);
}

/// Starts building the file of the export `number`, once.
fn go(number: u64) {
    let Some(run) = find(number) else { return };
    if run.started.replace(true) {
        return;
    }
    let Some(source) = run.source.borrow_mut().take() else {
        return;
    };
    let job = Job {
        source,
        kind: run.kind,
        path: run.path.clone(),
        cancelled: run.cancelled.clone(),
    };
    if run.kind == Kind::Pdf {
        pdf::start(number, job);
        return;
    }
    let spawned = std::thread::Builder::new().name("export".into()).spawn(move || {
        let outcome = job.write();
        main_soon(move || finish(number, outcome));
    });
    if spawned.is_err() {
        finish(
            number,
            Outcome::Failed(t("Der Export ließ sich nicht starten.").to_string()),
        );
    }
}

/// What the export's thread needs.
struct Job {
    source: Source,
    kind: Kind,
    path: PathBuf,
    cancelled: Arc<AtomicBool>,
}

impl Job {
    fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::SeqCst)
    }

    /// Builds and writes an HTML or Word file; on the export's thread.
    fn write(&self) -> Outcome {
        let data = match self.kind {
            Kind::Html => Ok(html(self).into_bytes()),
            Kind::Word => word(self),
            Kind::Pdf => return Outcome::Failed(t("Das PDF ließ sich nicht schreiben.").to_string()),
        };
        if self.is_cancelled() {
            return Outcome::Cancelled;
        }
        let written = data.and_then(|data| std::fs::write(&self.path, data).map_err(|e| Explained::of(&e)));
        match written {
            Ok(()) => Outcome::Written,
            Err(Explained(reason)) => Outcome::Failed(reason),
        }
    }

    /// Runs `step` on the main thread and waits for it; `None` once the export is cancelled.
    fn on_main<T: Send + 'static>(&self, step: impl FnOnce() -> Option<T> + Send + 'static) -> Option<T> {
        if self.is_cancelled() {
            return None;
        }
        let (answer, answered) = std::sync::mpsc::channel();
        main_soon(move || {
            let _ = answer.send(step());
        });
        loop {
            match answered.recv_timeout(Duration::from_millis(100)) {
                Ok(value) => return value,
                Err(std::sync::mpsc::RecvTimeoutError::Timeout) if !self.is_cancelled() => {}
                Err(_) => return None,
            }
        }
    }
}

/// The formulas and diagrams of `text` as they look on paper, in the light appearance.
fn figures(text: &str) -> Vec<Key> {
    let doc = splat_markdown::reader::parse(text);
    js::wanted(&doc, false)
        .keys
        .into_iter()
        .filter(|key| matches!(key, Key::Math { dark: false, .. } | Key::Diagram { dark: false, .. }))
        .collect()
}

fn drawn(key: &Key) -> bool {
    js::ready(key) || js::outcome(key).is_some()
}

/// An export as the render path knows it: it hears when the renderer has run empty and when a picture of the PDF
/// was read.
struct Drawing(Weak<Run>);

impl Viewer for Drawing {
    fn renders_arrived(&self) {
        let Some(run) = self.0.upgrade() else { return };
        if !run.started.get() && run.keys.iter().all(drawn) {
            let number = run.number;
            main_soon(move || go(number));
        }
    }

    fn image_arrived(&self, _url: String) {}

    fn image_loaded(&self, path: PathBuf, file: ImageFile) {
        if let Some(run) = self.0.upgrade()
            && let Some(work) = run.pdf.borrow().as_ref()
        {
            work.picture_read(path, file);
        }
    }

    fn images_changed(&self, _changed: Vec<(PathBuf, Stamp)>) {}

    fn web_images_shown(&self) -> Vec<String> {
        Vec::new()
    }

    fn has_closed(&self) -> bool {
        self.0.upgrade().is_none_or(|run| run.cancelled.load(Ordering::SeqCst))
    }
}

/// The page that goes to the export's path, with its pictures inside it; on the export's thread.
fn html(job: &Job) -> String {
    let source = &job.source;
    let doc = splat_markdown::reader::parse(&source.text);
    let macros = js::wanted(&doc, false).macros;
    let dir = source.doc_dir.as_deref();
    let figure = |f: &Figure| {
        let (key, alt) = (Key::of(f, false, &macros), pictures::alt_of(f));
        job.on_main(move || pictures::figure_html(&key, &alt))
    };
    let embed = |url: &str| (!job.is_cancelled()).then(|| pictures::embedded(url, dir)).flatten();
    let rebase = |link: &str| pictures::rebased(link, dir, job.path.parent());
    let none = |_: &str| None;
    to_html(
        &doc,
        &Export {
            title: &source.title,
            figure: &figure,
            embed: &embed,
            rebase: &rebase,
            quotes: None,
            reference: &none,
            citation: &none,
        },
    )
}

/// The Word file of the export; on the export's thread.
fn word(job: &Job) -> Result<Vec<u8>, Explained> {
    let source = &job.source;
    let doc = splat_markdown::reader::parse(&source.text);
    let macros = js::wanted(&doc, false).macros;
    let dir = source.doc_dir.as_deref();
    let figure = |f: &Figure| {
        let key = Key::of(f, false, &macros);
        job.on_main(move || pictures::word_figure(&key))
    };
    let image = |url: &str| {
        if job.is_cancelled() {
            return None;
        }
        let picture = if remote::is_remote(url) {
            let url = url.to_string();
            source
                .trusted
                .then(|| job.on_main(move || pictures::remote_picture(&url)))??
        } else {
            pictures::local_picture(url, dir)?
        };
        Some(pictures::on_paper(picture))
    };
    let none = |_: &str| None;
    to_docx(
        &doc,
        &WordExport {
            figure: &figure,
            image: &image,
            quotes: None,
            reference: &none,
            citation: &none,
            title: None,
        },
    )
    .map_err(|_| Explained(t("Das Dokument ist zu groß für eine Word-Datei.").to_string()))
}

/// What the self-test reads of a running or failed export.
#[cfg(feature = "selftest")]
pub mod seen {
    /// Whether the export `number` still runs.
    pub fn running(number: u64) -> bool {
        super::find(number).is_some()
    }

    /// What the sheet of the export `number` says, while it runs or after it failed.
    pub fn sheet_text(number: u64) -> Option<String> {
        if let Some(text) = super::find(number).and_then(|run| run.sheet.borrow().as_ref().map(super::Sheet::text)) {
            return Some(text);
        }
        super::with_failed(|failed| {
            failed
                .borrow()
                .iter()
                .find(|(n, _)| *n == number)
                .map(|(_, sheet)| sheet.text())
        })
    }

    /// Presses the button of the sheet of the export `number`.
    pub fn press_sheet_button(number: u64) {
        let failed = super::with_failed(|failed| failed.borrow().iter().any(|(n, _)| *n == number));
        super::sheet_button(number, failed);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_kind_has_the_extension_of_its_files() {
        assert_eq!(
            [Kind::Pdf, Kind::Html, Kind::Word].map(Kind::extension),
            ["pdf", "html", "docx"]
        );
    }

    #[test]
    fn an_export_waits_for_the_light_formulas_and_diagrams_of_its_text() {
        let keys = figures("# T\n\n$$x^2$$\n\n```mermaid\ngraph TD; A-->B\n```\n\nNo figure here.\n");
        assert_eq!(keys.len(), 2, "{keys:?}");
        assert!(keys.iter().any(|key| matches!(key, Key::Math { dark: false, .. })));
        assert!(keys.iter().any(|key| matches!(key, Key::Diagram { dark: false, .. })));
        assert!(figures("Plain text.").is_empty());
    }
}

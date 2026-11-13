//! The `export` scenario: formulas and diagrams go into HTML, Word and PDF as the hidden renderer drew them; pictures
//! of the document's folder go along, converted where a page or Word cannot show them, while a web picture that was
//! never loaded and a picture from outside the folder stay out; an export cancelled while it waits for its formulas
//! writes nothing; the save panel names the file after the document.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, PoisonError};
use std::time::Duration;

use ide_ui_export::{Kind, Outcome, Source};
use objc2::MainThreadMarker;
use objc2_app_kit::{NSApplication, NSModalResponseOK, NSSavePanel};
use splat_i18n::t;

use super::helpers::{answer_alert, in_any_mode_after, wait_until};
use super::{check, finish};
use crate::util::main_after;

const SCENARIO: &str = "export";

const TEXT: &str = "# Formeln\n\n$$x^2 + y^2$$\n\nInline $a_1$ im Satz.\n\n```mermaid\ngraph TD\n  A --> B\n```\n\n\
![punkt](punkt.gif)\n\n![raster](raster.bmp)\n\n![kreis](kreis.svg)\n\n![netz](https://example.invalid/bild.png)\n\n\
![aussen](../aussen.gif)\n";

/// The pictures of `TEXT` that Word takes: two formulas, the diagram and the three files of the folder.
const WORD_PICTURES: usize = 6;

/// A picture of one pixel.
const GIF: &[u8] = b"GIF89a\x01\x00\x01\x00\x80\x00\x00\xff\xff\xff\x00\x00\x00!\xf9\x04\x01\x00\x00\x00\x00,\x00\x00\x00\x00\x01\x00\x01\x00\x00\x02\x02D\x01\x00;";

const SVG: &str =
    "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"8\" height=\"8\"><circle cx=\"4\" cy=\"4\" r=\"3\"/></svg>";

type Outcomes = Arc<Mutex<Vec<(Kind, Outcome)>>>;

/// A bitmap of 2 × 2 red pixels, a format neither a page nor Word shows.
fn bmp() -> Vec<u8> {
    let mut file = Vec::new();
    file.extend_from_slice(b"BM");
    file.extend_from_slice(&70u32.to_le_bytes());
    file.extend_from_slice(&0u32.to_le_bytes());
    file.extend_from_slice(&54u32.to_le_bytes());
    file.extend_from_slice(&40u32.to_le_bytes());
    file.extend_from_slice(&2i32.to_le_bytes());
    file.extend_from_slice(&2i32.to_le_bytes());
    file.extend_from_slice(&1u16.to_le_bytes());
    file.extend_from_slice(&24u16.to_le_bytes());
    file.extend_from_slice(&0u32.to_le_bytes());
    file.extend_from_slice(&16u32.to_le_bytes());
    for value in [2835u32, 2835, 0, 0] {
        file.extend_from_slice(&value.to_le_bytes());
    }
    for _ in 0..2 {
        file.extend_from_slice(&[0, 0, 255, 0, 0, 255, 0, 0]);
    }
    file
}

fn root() -> PathBuf {
    let home = ide_core::paths::home();
    let folder = home.join("export-project");
    let _ = std::fs::create_dir_all(&folder);
    let _ = std::fs::write(folder.join("punkt.gif"), GIF);
    let _ = std::fs::write(folder.join("raster.bmp"), bmp());
    let _ = std::fs::write(folder.join("kreis.svg"), SVG);
    let _ = std::fs::write(home.join("aussen.gif"), GIF);
    folder.canonicalize().unwrap_or(folder)
}

fn source(root: &Path, title: &str, text: &str) -> Source {
    Source {
        text: text.to_string(),
        doc_dir: Some(root.to_path_buf()),
        title: title.to_string(),
        trusted: true,
    }
}

pub fn run() {
    let root = root();
    let out = root.join("out");
    let _ = std::fs::create_dir_all(&out);
    let outcomes: Outcomes = Arc::default();
    in_turn(
        root.clone(),
        out.clone(),
        Arc::clone(&outcomes),
        vec![
            (Kind::Html, "formeln.html"),
            (Kind::Word, "formeln.docx"),
            (Kind::Pdf, "formeln.pdf"),
        ],
    );
    let all = move |outcomes: &Outcomes| outcomes.lock().unwrap_or_else(PoisonError::into_inner).len() == 3;
    let waiting = Arc::clone(&outcomes);
    // The renderer may take a while to load the first time, so the wait is twice the usual one.
    wait_until(
        move || all(&waiting),
        move |ok| {
            if ok {
                return exported(&root, &out, &outcomes);
            }
            let waiting = Arc::clone(&outcomes);
            wait_until(move || all(&waiting), move |_| exported(&root, &out, &outcomes));
        },
    );
}

/// Exports `TEXT` as each of `kinds` in turn, one after the other ended.
fn in_turn(root: PathBuf, out: PathBuf, outcomes: Outcomes, mut kinds: Vec<(Kind, &'static str)>) {
    if kinds.is_empty() {
        return;
    }
    let (kind, name) = kinds.remove(0);
    let path = out.join(name);
    ide_ui_export::start(source(&root, "formeln", TEXT), kind, path, None, move |outcome| {
        outcomes
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push((kind, outcome));
        in_turn(root, out, outcomes, kinds);
    });
}

/// The `<img>` tags of `page` as the start of their `src` and their `alt`.
fn pictures(page: &str) -> Vec<(String, String)> {
    let attribute = |tag: &str, name: &str| {
        let start = tag.find(&format!("{name}=\""))? + name.len() + 2;
        let end = tag[start..].find('"')? + start;
        Some(tag[start..end].to_string())
    };
    page.split("<img ")
        .skip(1)
        .filter_map(|rest| rest.split('>').next())
        .map(|tag| {
            let src = attribute(tag, "src").unwrap_or_default();
            let kind = src.split(';').next().unwrap_or_default().to_string();
            (kind, attribute(tag, "alt").unwrap_or_default())
        })
        .collect()
}

/// The pictures a Word file carries.
fn word_pictures(docx: &[u8]) -> Vec<String> {
    let needle = b"word/media/";
    let mut names: Vec<String> = Vec::new();
    for at in 0..docx.len().saturating_sub(needle.len()) {
        if &docx[at..at + needle.len()] != needle {
            continue;
        }
        let rest: String = docx[at..]
            .iter()
            .take(40)
            .take_while(|b| b.is_ascii_graphic())
            .map(|b| char::from(*b))
            .collect();
        let Some(end) = [".png", ".jpeg"]
            .iter()
            .filter_map(|ext| rest.find(ext).map(|at| at + ext.len()))
            .min()
        else {
            continue;
        };
        let name = rest[..end].to_string();
        if !names.contains(&name) {
            names.push(name);
        }
    }
    names
}

fn exported(root: &Path, out: &Path, outcomes: &Outcomes) {
    let outcomes = outcomes.lock().unwrap_or_else(PoisonError::into_inner).clone();
    check(
        "a document with formulas and a diagram exports to HTML, Word and PDF",
        outcomes.len() == 3 && outcomes.iter().all(|(_, outcome)| *outcome == Outcome::Written),
        format!("{outcomes:?}"),
    );
    let page = std::fs::read_to_string(out.join("formeln.html")).unwrap_or_default();
    let shown = pictures(&page);
    let has = |kind: &str, alt: &str| shown.iter().any(|(k, a)| k == kind && a == alt);
    check(
        "the page carries the formulas and the diagram as the renderer drew them",
        has("data:image/png", "x^2 + y^2")
            && has("data:image/png", "a_1")
            && has("data:image/png", t("Diagramm"))
            && page.contains("class=\"math\""),
        format!("{shown:?}"),
    );
    check(
        "the page carries the folder's GIF and SVG as they are and the bitmap converted to PNG",
        has("data:image/gif", "punkt") && has("data:image/svg+xml", "kreis") && has("data:image/png", "raster"),
        format!("{shown:?}"),
    );
    check(
        "a web picture stays a link and a picture from outside the folder stays out of the page",
        has("https://example.invalid/bild.png", "netz")
            && !shown.iter().any(|(k, a)| a == "aussen" && k.starts_with("data:")),
        format!("{shown:?}"),
    );
    let docx = std::fs::read(out.join("formeln.docx")).unwrap_or_default();
    let media = word_pictures(&docx);
    check(
        "the Word file carries the formulas, the diagram and the folder's pictures, not the web or foreign ones",
        docx.starts_with(b"PK") && media.len() == WORD_PICTURES && media.iter().all(|m| m.ends_with(".png")),
        format!("{media:?}"),
    );
    let pdf = std::fs::read(out.join("formeln.pdf")).unwrap_or_default();
    check(
        "the PDF has its formulas and diagram drawn in",
        pdf.starts_with(b"%PDF") && pdf.len() > 4_000,
        format!("{} bytes", pdf.len()),
    );
    cancelled(root.to_path_buf(), out.to_path_buf());
}

/// An export cancelled while its formula is still being drawn hears so and writes nothing.
fn cancelled(root: PathBuf, out: PathBuf) {
    let path = out.join("abgebrochen.html");
    let outcome: Arc<Mutex<Option<Outcome>>> = Arc::default();
    let seen = Arc::clone(&outcome);
    let number = ide_ui_export::start(
        source(&root, "abgebrochen", "$$\\int_0^1 z^3\\,dz$$\n"),
        Kind::Html,
        path.clone(),
        None,
        move |ended| *seen.lock().unwrap_or_else(PoisonError::into_inner) = Some(ended),
    );
    ide_ui_export::cancel(number);
    // Long enough for the renderer to deliver the formula to an export that is gone.
    main_after(Duration::from_secs(3), move || {
        let ended = outcome.lock().unwrap_or_else(PoisonError::into_inner).clone();
        check(
            "an export cancelled while its formula is drawn ends as cancelled and writes no file",
            ended == Some(Outcome::Cancelled) && !path.exists(),
            format!("{ended:?}, file {}", path.exists()),
        );
        save_panel(root);
    });
}

/// "Exportieren …" asks where the file goes, named after the document in its folder, and writes it there.
fn save_panel(root: PathBuf) {
    let Some(mtm) = MainThreadMarker::new() else { return };
    let offered = Arc::new(Mutex::new(String::new()));
    answer_save_panel(Arc::clone(&offered), 40);
    ide_ui_export::ask(mtm, None, source(&root, "gespeichert", "# Gespeichert\n"), Kind::Html);
    let target = root.join("gespeichert.html");
    let expected = target.to_string_lossy().into_owned();
    let written = target.clone();
    wait_until(
        move || written.exists(),
        move |ok| {
            let offered = offered.lock().unwrap_or_else(PoisonError::into_inner).clone();
            let page = std::fs::read_to_string(&target).unwrap_or_default();
            check(
                "the save panel offers the document's name in its folder, and the export goes where it says",
                ok && offered == expected && page.contains("<h1"),
                format!("offered {offered:?}, expected {expected:?}"),
            );
            finish(SCENARIO);
        },
    );
}

/// Notes where the save panel in front offers to save and answers it with OK, looking again every half second until
/// one is in front, `tries` times.
fn answer_save_panel(offered: Arc<Mutex<String>>, tries: u32) {
    in_any_mode_after(Duration::from_millis(500), move || {
        let Some(mtm) = MainThreadMarker::new() else { return };
        let panel = NSApplication::sharedApplication(mtm)
            .modalWindow()
            .and_then(|window| window.downcast::<NSSavePanel>().ok());
        let Some(panel) = panel else {
            if tries > 0 {
                answer_save_panel(offered, tries - 1);
            }
            return;
        };
        let name = panel.nameFieldStringValue().to_string();
        let dir = panel
            .directoryURL()
            .and_then(|url| url.path())
            .map(|p| p.to_string())
            .unwrap_or_default();
        *offered.lock().unwrap_or_else(PoisonError::into_inner) = format!("{dir}/{name}");
        answer_alert(NSModalResponseOK);
    });
}

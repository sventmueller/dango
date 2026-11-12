//! The `preview` scenario: a Markdown file opens its preview in a panel to the right, with formulas, a diagram and a
//! picture of its folder drawn, follows typing and scrolling; an AI answer shows a drawn formula; a PDF file opens in
//! a tab of its own, a picture and on request an SVG drawing in a Quick Look tab, an HTML file in the browser beside
//! its editor. In an untrusted folder pictures from the web wait for the user to ask for them (E22).

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use objc2::MainThreadMarker;
use objc2_foundation::NSRange;

use super::helpers::{editor, type_at, utf16_len, window_of};
use super::{check, finish, snapshot};
use crate::preview::seen;
use crate::util::main_after;

const SCENARIO: &str = "preview";

/// How long the first formulas and diagrams may take: the renderer's page loads its libraries with the first one.
const FIGURES: Duration = Duration::from_secs(45);

/// A picture of 4 × 4 pixels.
const PNG: [u8; 73] = [
    0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a, 0x00, 0x00, 0x00, 0x0d, 0x49, 0x48, 0x44, 0x52, 0x00, 0x00, 0x00,
    0x04, 0x00, 0x00, 0x00, 0x04, 0x08, 0x02, 0x00, 0x00, 0x00, 0x26, 0x93, 0x09, 0x29, 0x00, 0x00, 0x00, 0x10, 0x49,
    0x44, 0x41, 0x54, 0x78, 0x9c, 0x63, 0xb8, 0xe0, 0xa0, 0x00, 0x47, 0x0c, 0xc4, 0x71, 0x00, 0xf9, 0x43, 0x13, 0x01,
    0xd7, 0x3a, 0xcd, 0xee, 0x00, 0x00, 0x00, 0x00, 0x49, 0x45, 0x4e, 0x44, 0xae, 0x42, 0x60, 0x82,
];

/// A picture from the web that never loads: nothing listens on that port of this Mac.
const WEB_PICTURE: &str = "https://127.0.0.1:9/bild.png";

fn document() -> String {
    let mut text = String::from(
        "# Vorschau\n\nFormel $E = mc^2$ im Text.\n\n$$\\int_0^1 x\\,dx$$\n\n```mermaid\ngraph TD; A-->B\n```\n\n\
         ![lokal](bild.png)\n\n| a | b |\n|---|---|\n| 1 | 2 |\n\n",
    );
    for n in 1..=80 {
        text.push_str(&format!("Absatz {n} mit etwas Text, damit die Vorschau scrollt.\n\n"));
    }
    text
}

/// A PDF of one empty page, with the offsets its cross-reference table needs.
fn pdf() -> Vec<u8> {
    let objects = [
        "<< /Type /Catalog /Pages 2 0 R >>",
        "<< /Type /Pages /Kids [3 0 R] /Count 1 >>",
        "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 200 200] >>",
    ];
    let mut out = String::from("%PDF-1.4\n");
    let mut offsets = Vec::new();
    for (n, object) in objects.iter().enumerate() {
        offsets.push(out.len());
        out.push_str(&format!("{} 0 obj {object} endobj\n", n + 1));
    }
    let xref = out.len();
    out.push_str(&format!("xref\n0 {}\n0000000000 65535 f \n", objects.len() + 1));
    for offset in offsets {
        out.push_str(&format!("{offset:010} 00000 n \n"));
    }
    out.push_str(&format!(
        "trailer << /Size {} /Root 1 0 R >>\nstartxref\n{xref}\n%%EOF\n",
        objects.len() + 1
    ));
    out.into_bytes()
}

fn folder(name: &str) -> PathBuf {
    let folder = ide_core::paths::home().join(name);
    let _ = std::fs::create_dir_all(&folder);
    folder.canonicalize().unwrap_or(folder)
}

pub(super) fn run() {
    let Some(mtm) = MainThreadMarker::new() else { return };
    let root = folder("preview-project");
    let _ = std::fs::write(root.join("doc.md"), document());
    let _ = std::fs::write(root.join("bild.png"), PNG);
    let _ = std::fs::write(root.join("heft.pdf"), pdf());
    let _ = image::save_buffer(
        root.join("bildschirmfoto.png"),
        &vec![200; 2880 * 1800 * 3],
        2880,
        1800,
        image::ExtendedColorType::Rgb8,
    );
    let _ = std::fs::write(
        root.join("logo.svg"),
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"40\" height=\"40\"><circle cx=\"20\" cy=\"20\" r=\"16\" fill=\"teal\"/></svg>\n",
    );
    let _ = std::fs::write(root.join("seite.html"), "<html><body><h1>Seite</h1></body></html>\n");
    crate::project::trust(&root);
    crate::app::open_project(mtm, root.clone());
    main_after(Duration::from_millis(500), move || open_preview(root));
}

fn open_preview(root: PathBuf) {
    let Some(controller) = window_of(&root) else {
        check("the folder gets its window", false, root.display().to_string());
        return finish(SCENARIO);
    };
    controller.open_file(&root.join("doc.md"));
    let Some(view) = editor(&controller, "doc.md") else {
        check("the file opens in an editor", false, controller.tab_titles().join(", "));
        return finish(SCENARIO);
    };
    let tabs = controller.window_tabs();
    check(
        "a Markdown file opens with its preview right of the editor in its tab, and the editor stays the focused one",
        tabs.iter().any(|docs| docs == &["doc.md", "Vorschau: doc.md"])
            && controller.selected_tab() == Some(crate::editor::tab_of(&view)),
        format!("{tabs:?}, focused {:?}", controller.selected_tab()),
    );
    let path = root.join("doc.md");
    let shown = path.clone();
    super::helpers::wait_until(
        move || seen::text(&shown).is_some_and(|(text, _)| text.contains("Absatz 80")),
        move |ok| {
            let text = seen::text(&path).map(|(text, _)| text).unwrap_or_default();
            check(
                "the preview shows the document as text, its table among it",
                ok && text.starts_with("Vorschau") && text.contains("Formel") && text.contains("1\n2"),
                text.chars().take(80).collect::<String>(),
            );
            wait_for_figures(root, Instant::now());
        },
    );
}

/// Waits until the two formulas, the diagram and the picture of the folder are pictures in the preview.
fn wait_for_figures(root: PathBuf, since: Instant) {
    let path = root.join("doc.md");
    let pictures = seen::text(&path).map_or(0, |(_, attachments)| attachments);
    if pictures < 4 && since.elapsed() < FIGURES {
        main_after(Duration::from_millis(250), move || wait_for_figures(root, since));
        return;
    }
    check(
        "formulas, the diagram and the picture of the folder show as pictures",
        pictures >= 4,
        format!("{pictures} pictures after {:.1} s", since.elapsed().as_secs_f64()),
    );
    if let Some(window) = window_of(&root).and_then(|c| c.window()) {
        snapshot(&window, "preview");
    }
    typing(root);
}

fn typing(root: PathBuf) {
    let Some(controller) = window_of(&root) else {
        return finish(SCENARIO);
    };
    let Some(view) = editor(&controller, "doc.md") else {
        return finish(SCENARIO);
    };
    let end_of_heading = utf16_len("# Vorschau");
    type_at(&view, NSRange::new(end_of_heading, 0), " neu");
    let path = root.join("doc.md");
    let shown = path.clone();
    super::helpers::wait_until(
        move || seen::text(&shown).is_some_and(|(text, _)| text.starts_with("Vorschau neu")),
        move |ok| {
            let text = seen::text(&path).map(|(text, _)| text).unwrap_or_default();
            check(
                "typing in the editor shows in the preview",
                ok,
                text.lines().next().unwrap_or_default().to_string(),
            );
            // Saved, so quitting asks nothing.
            if let Some(view) = window_of(&root).and_then(|c| editor(&c, "doc.md")) {
                crate::editor::save_view(&view);
            }
            scrolling(root);
        },
    );
}

fn scrolling(root: PathBuf) {
    let Some(controller) = window_of(&root) else {
        return finish(SCENARIO);
    };
    let Some(view) = editor(&controller, "doc.md") else {
        return finish(SCENARIO);
    };
    let text = view.document().text().to_string();
    if let Some(byte) = text.find("Absatz 60 ") {
        crate::editor::scroll_to_byte(&view, byte);
    }
    let path = root.join("doc.md");
    let shown = path.clone();
    super::helpers::wait_until(
        move || seen::top_line(&shown).is_some_and(|line| line.starts_with("Absatz 6")),
        move |ok| {
            check(
                "the preview scrolls with the editor",
                ok,
                format!("top: {:?}, at {:?}", seen::top_line(&path), seen::scrolled(&path)),
            );
            if let Some(window) = window_of(&root).and_then(|c| c.window()) {
                snapshot(&window, "preview-scrolled");
            }
            smooth_scrolling(&root, &path);
            answers(root);
        },
    );
}

/// Each small step of one side moves the other a little, rather than in jumps from paragraph to paragraph; upwards,
/// as the editor stands near the end of the text.
fn smooth_scrolling(root: &Path, path: &Path) {
    let Some(view) = window_of(root).and_then(|c| editor(&c, "doc.md")) else {
        return;
    };
    let editor_top = || view.visibleRect().origin.y;
    let preview_top = || seen::scrolled(path).unwrap_or_default();
    let step = |lead: &dyn Fn(f64), top: &dyn Fn() -> f64, other: &dyn Fn() -> f64| {
        let start = top();
        let mut seen = vec![other()];
        for k in 1..=12 {
            lead(start - 4.0 * f64::from(k));
            seen.push(other());
        }
        seen
    };
    let follows = |seen: &[f64]| seen.windows(2).all(|w| w[1] < w[0]);
    let by_editor = step(
        &|y| view.scrollPoint(objc2_foundation::NSPoint::new(0.0, y)),
        &editor_top,
        &preview_top,
    );
    check(
        "scrolling the editor in small steps moves the preview at every step",
        follows(&by_editor),
        format!("{by_editor:?}"),
    );
    let by_preview = step(&|y| seen::scroll_to(path, y), &preview_top, &editor_top);
    check(
        "scrolling the preview in small steps moves the editor at every step",
        follows(&by_preview),
        format!("{by_preview:?}"),
    );
}

/// An AI answer shows the formula the preview had drawn as a picture, and asks for one it has not seen.
fn answers(root: PathBuf) {
    let Some(mtm) = MainThreadMarker::new() else { return };
    let drawn = crate::agent::answer_text(mtm, "Wie gesagt: $E = mc^2$.");
    let text = drawn.string().to_string();
    check(
        "an AI answer shows a formula the renderer drew as a picture",
        text.contains('\u{FFFC}') && !text.contains("mc^2"),
        text,
    );
    let fresh = crate::agent::answer_text(mtm, "Neu: $a^2 + b^2$.").string().to_string();
    let key = splat_render::js::Key::math(
        "a^2 + b^2",
        false,
        crate::agent::is_dark(mtm),
        &std::sync::Arc::from(""),
    );
    main_after(Duration::from_millis(100), move || {
        check(
            "an AI answer shows a formula as code until it is drawn, and asks for it",
            fresh.contains("a^2 + b^2") && crate::agent::asked_for_figure(&key),
            fresh,
        );
        pdf_and_html(root);
    });
}

fn pdf_and_html(root: PathBuf) {
    let Some(controller) = window_of(&root) else {
        return finish(SCENARIO);
    };
    controller.open_file(&root.join("heft.pdf"));
    let pages = controller.selected_view().map(|view| seen::page_count(&view));
    check(
        "a PDF file opens in a tab that shows its page",
        controller.tab_titles().contains(&"heft.pdf".to_string()) && pages == Some(1),
        format!("{:?}, {pages:?} pages", controller.tab_titles()),
    );
    controller.open_file(&root.join("bild.png"));
    let quick_look = controller
        .selected_view()
        .is_some_and(|view| seen::is_quick_look(&view));
    check(
        "a picture opens in a tab that shows it through Quick Look",
        controller.tab_titles().contains(&"bild.png".to_string()) && quick_look,
        format!("{:?}, Quick Look {quick_look}", controller.tab_titles()),
    );
    main_after(Duration::from_millis(1500), move || {
        if let Some(window) = window_of(&root).and_then(|c| c.window()) {
            snapshot(&window, "preview-picture");
        }
        screenshot(root);
    });
}

/// A picture wider than the window keeps the sidebar at its width, as a screenshot of a whole screen is.
fn screenshot(root: PathBuf) {
    let Some(controller) = window_of(&root) else {
        return finish(SCENARIO);
    };
    let before = controller.inspect().sidebar_width;
    controller.open_file(&root.join("bildschirmfoto.png"));
    main_after(Duration::from_millis(1500), move || {
        let Some(controller) = window_of(&root) else {
            return finish(SCENARIO);
        };
        let after = controller.inspect().sidebar_width;
        check(
            "a picture wider than the window keeps the sidebar's width",
            (before - after).abs() < 1.0,
            format!("{before} pt before, {after} pt after"),
        );
        if let Some(window) = controller.window() {
            snapshot(&window, "preview-screenshot");
        }
        svg(root);
    });
}

/// An SVG file opens as text, and its drawing in a tab of its own on request.
fn svg(root: PathBuf) {
    let Some(controller) = window_of(&root) else {
        return finish(SCENARIO);
    };
    let path = root.join("logo.svg");
    controller.open_file(&path);
    crate::preview::open_drawing(&controller, &path);
    let quick_look = controller
        .selected_view()
        .is_some_and(|view| seen::is_quick_look(&view));
    let titles = controller.tab_titles();
    check(
        "an SVG file opens in an editor, and as a preview in a Quick Look tab of its own",
        editor(&controller, "logo.svg").is_some() && titles.contains(&"Vorschau: logo.svg".to_string()) && quick_look,
        format!("{titles:?}, Quick Look {quick_look}"),
    );
    main_after(Duration::from_millis(1500), move || {
        if let Some(window) = window_of(&root).and_then(|c| c.window()) {
            snapshot(&window, "preview-svg");
        }
        html(root);
    });
}

fn html(root: PathBuf) {
    let Some(controller) = window_of(&root) else {
        return finish(SCENARIO);
    };
    controller.open_file(&root.join("seite.html"));
    let Some(view) = editor(&controller, "seite.html") else {
        check(
            "an HTML file opens in an editor",
            false,
            controller.tab_titles().join(", "),
        );
        return finish(SCENARIO);
    };
    let before = crate::browser::inspect(&controller).len();
    crate::preview::toggle_beside(&view);
    let seen = crate::browser::inspect(&controller);
    check(
        "the preview of an HTML file is the browser, beside its editor",
        seen.len() == before + 1
            && seen
                .iter()
                .any(|tab| tab.url.as_deref().is_some_and(|url| url.ends_with("seite.html")))
            && controller
                .window_tabs()
                .iter()
                .any(|docs| docs.len() == 2 && docs[0] == "seite.html"),
        format!("{:?}", controller.window_tabs()),
    );
    untrusted();
}

/// In a folder the user has not trusted, the pictures from the web wait for the user to ask for them.
fn untrusted() {
    let Some(mtm) = MainThreadMarker::new() else { return };
    let root = folder("preview-untrusted");
    let path = root.join("fremd.md");
    let _ = std::fs::write(&path, format!("# Fremd\n\n![netz]({WEB_PICTURE})\n"));
    crate::app::open_project(mtm, root.clone());
    let Some(controller) = window_of(&root) else {
        check("the untrusted folder gets its window", false, "");
        return finish(SCENARIO);
    };
    controller.open_file(&path);
    if editor(&controller, "fremd.md").is_none() {
        return finish(SCENARIO);
    }
    let shown = path.clone();
    super::helpers::wait_until(
        move || seen::text(&shown).is_some_and(|(text, _)| text.starts_with("Fremd")),
        move |ok| web_pictures(&path, ok),
    );
}

fn web_pictures(path: &Path, laid_out: bool) {
    let placeholder = seen::text(path).is_some_and(|(_, attachments)| attachments == 1);
    let offered = seen::offers_web_pictures(path);
    let reported = seen::web_pictures_shown(path).unwrap_or_default();
    check(
        "in an untrusted folder a picture from the web stays a placeholder and is offered",
        laid_out && placeholder && offered == Some(true) && reported.is_empty(),
        format!("placeholder {placeholder}, offered {offered:?}, asked for {reported:?}"),
    );
    seen::load_web_pictures(path);
    let reported = seen::web_pictures_shown(path).unwrap_or_default();
    check(
        "the offer loads the pictures from the web of this document",
        seen::offers_web_pictures(path) == Some(false) && reported == [WEB_PICTURE],
        format!("{reported:?}"),
    );
    finish(SCENARIO);
}

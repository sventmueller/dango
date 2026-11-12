//! The browser scenario: a page served on 127.0.0.1 opens in a browser tab, reloads by the toolbar and by ⌘R, fills
//! a form, shows a failed load and a file of the project.

use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::atomic::{AtomicU16, AtomicUsize, Ordering};
use std::time::{Duration, Instant};

use objc2::MainThreadMarker;
use objc2::rc::Retained;
use splat_i18n::t;

use super::{check, finish, first_window, poll, snapshot};
use crate::window;

/// The port of the page server of the browser scenario.
pub(super) static PAGE_PORT: AtomicU16 = AtomicU16::new(0);
/// How many times the page server sent its page.
static PAGES_SENT: AtomicUsize = AtomicUsize::new(0);
pub(super) const PAGE_TITLE: &str = "Browser Selftest";

/// Serves one page on 127.0.0.1 from a thread, for as long as the app runs.
pub(super) fn serve_page() -> std::io::Result<u16> {
    let listener = TcpListener::bind("127.0.0.1:0")?;
    let port = listener.local_addr()?.port();
    std::thread::spawn(move || {
        for stream in listener.incoming().flatten() {
            let _ = answer(stream);
        }
    });
    Ok(port)
}

fn answer(mut stream: TcpStream) -> std::io::Result<()> {
    let mut request = Vec::new();
    let mut buffer = [0u8; 1024];
    while !request.windows(4).any(|w| w == b"\r\n\r\n") && request.len() < 16 * 1024 {
        let read = stream.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        request.extend_from_slice(&buffer[..read]);
    }
    let line = String::from_utf8_lossy(&request);
    let path = line.split_whitespace().nth(1).unwrap_or("/");
    let (status, body) = if path == "/favicon.ico" {
        ("404 Not Found", String::new())
    } else {
        PAGES_SENT.fetch_add(1, Ordering::SeqCst);
        let body = format!(
            "<!doctype html><html><head><meta charset=\"utf-8\"><title>{PAGE_TITLE}</title>\
             <style>body{{font:15px -apple-system;margin:40px;color:#1d1d1f}}\
             h1{{color:#0a66d8}}</style></head><body><h1>{PAGE_TITLE}</h1>\
             <p>Served from 127.0.0.1 for the browser tab.</p>\
             <form><input name=user autocomplete=username> <input name=secret type=password></form>\
             <script>addEventListener('input', () => {{ const f = document.forms[0]; \
             document.title = `{FILLED} ${{f.user.value}} ${{f.secret.value.length}}`; }})</script></body></html>"
        );
        ("200 OK", body)
    };
    write!(
        stream,
        "HTTP/1.1 {status}\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\n\
         Connection: close\r\n\r\n{body}",
        body.len()
    )
}

pub(super) fn page_url(path: &str) -> String {
    format!("http://127.0.0.1:{}{path}", PAGE_PORT.load(Ordering::SeqCst))
}

/// A browser tab at a page of a local server: its title, one tab per server, the reload, the native error page.
pub(super) fn run() {
    let Some(controller) = first_window() else {
        check("a window opens", false, "");
        finish("browser");
        return;
    };
    match serve_page() {
        Ok(port) => PAGE_PORT.store(port, Ordering::SeqCst),
        Err(error) => {
            check("the page server starts", false, error.to_string());
            finish("browser");
            return;
        }
    }
    let before = controller.tab_count();
    crate::browser::open_tab(&controller, Some(&page_url("/")));
    check(
        "a browser tab opens",
        controller.tab_count() == before + 1,
        format!("{before} then {} tabs", controller.tab_count()),
    );
    let deadline = Instant::now() + Duration::from_secs(20);
    poll(
        deadline,
        || first_window().is_some_and(|c| c.tab_titles().iter().any(|t| t == PAGE_TITLE)),
        browser_loaded,
    );
}

fn browser_loaded(ok: bool) {
    let Some(controller) = first_window() else { return };
    check("the tab takes the page's title", ok, controller.tab_titles().join(", "));
    let seen = crate::browser::inspect(&controller);
    check(
        "the Web Inspector is on",
        !seen.is_empty() && seen.iter().all(|seen| seen.inspector),
        format!("{} tabs", seen.len()),
    );
    let tabs = controller.tab_count();
    let sent = PAGES_SENT.load(Ordering::SeqCst);
    crate::browser::open_tab(&controller, Some(&page_url("")));
    check(
        "the same local server opens no second tab",
        controller.tab_count() == tabs,
        format!("{tabs} then {} tabs", controller.tab_count()),
    );
    BEFORE_RELOAD.store(sent, Ordering::SeqCst);
    poll(
        Instant::now() + Duration::from_secs(10),
        || PAGES_SENT.load(Ordering::SeqCst) > BEFORE_RELOAD.load(Ordering::SeqCst),
        browser_reloaded,
    );
}

static BEFORE_RELOAD: AtomicUsize = AtomicUsize::new(0);

fn browser_reloaded(ok: bool) {
    let Some(controller) = first_window() else { return };
    check(
        "it reloads the open tab instead",
        ok,
        format!("{} pages sent", PAGES_SENT.load(Ordering::SeqCst)),
    );
    let Some(window) = controller.window() else { return };
    snapshot(&window, "browser-page");
    press(&window, "l", 37);
    let page = || {
        crate::browser::inspect(&controller)
            .into_iter()
            .find(|seen| seen.url.as_deref() == Some(page_url("/").as_str()))
    };
    check(
        "⌘L focuses the address field",
        page().is_some_and(|seen| seen.editing),
        "",
    );
    if let Some(editor) = window
        .firstResponder()
        .and_then(|responder| responder.downcast::<objc2_app_kit::NSTextView>().ok())
    {
        editor.setString(&crate::util::ns("typed over"));
        // SAFETY: The field editor handles the command as Esc does, through the field's delegate.
        unsafe {
            objc2_app_kit::NSStandardKeyBindingResponding::doCommandBySelector(&*editor, objc2::sel!(cancelOperation:))
        };
    }
    let address = page().map(|seen| seen.address).unwrap_or_default();
    check(
        "Esc in the field restores the address",
        address == page_url("/"),
        address,
    );
    BEFORE_RELOAD.store(PAGES_SENT.load(Ordering::SeqCst), Ordering::SeqCst);
    press(&window, "r", 15);
    poll(
        Instant::now() + Duration::from_secs(10),
        || PAGES_SENT.load(Ordering::SeqCst) > BEFORE_RELOAD.load(Ordering::SeqCst),
        browser_shortcut_reloaded,
    );
}

/// Sends ⌘ and `key`, whose virtual key code is `code`, to `window` as a key equivalent.
fn press(window: &objc2_app_kit::NSWindow, key: &str, code: u16) {
    let event = objc2_app_kit::NSEvent::keyEventWithType_location_modifierFlags_timestamp_windowNumber_context_characters_charactersIgnoringModifiers_isARepeat_keyCode(
        objc2_app_kit::NSEventType::KeyDown,
        objc2_foundation::NSPoint::new(0.0, 0.0),
        objc2_app_kit::NSEventModifierFlags::Command,
        0.0,
        window.windowNumber(),
        None,
        &crate::util::ns(key),
        &crate::util::ns(key),
        false,
        code,
    );
    if let Some(event) = event {
        window.performKeyEquivalent(&event);
    }
}

fn browser_shortcut_reloaded(ok: bool) {
    let Some(controller) = first_window() else { return };
    check(
        "⌘R reloads the page",
        ok,
        format!("{} pages sent", PAGES_SENT.load(Ordering::SeqCst)),
    );
    check(
        "the menu bar offers reloading for the browser tab in front",
        crate::browser::validate(&controller, objc2::sel!(ideBrowserReload:)),
        "",
    );
    crate::browser::fill_login(&controller);
    check(
        "the key button asks for a login",
        crate::browser::answer_asking(&["selftest", "secret"]),
        "",
    );
    poll(
        Instant::now() + Duration::from_secs(10),
        || first_window().is_some_and(|c| c.tab_titles().iter().any(|t| t == &format!("{FILLED} selftest 6"))),
        browser_filled,
    );
}

/// The title the self-test page takes once its login form was filled in.
const FILLED: &str = "Filled";

fn browser_filled(ok: bool) {
    let Some(controller) = first_window() else { return };
    check(
        "the login lands in the page's form as typing would",
        ok,
        controller.tab_titles().join(", "),
    );
    let closed = TcpListener::bind("127.0.0.1:0")
        .and_then(|listener| listener.local_addr())
        .map(|address| address.port())
        .unwrap_or(9);
    crate::browser::open_tab(&controller, Some(&format!("http://127.0.0.1:{closed}/")));
    poll(
        Instant::now() + Duration::from_secs(20),
        || first_window().is_some_and(|c| crate::browser::inspect(&c).iter().any(|seen| seen.failure.is_some())),
        browser_failed,
    );
}

fn browser_failed(ok: bool) {
    let Some(controller) = first_window() else { return };
    let seen = crate::browser::inspect(&controller);
    let failure = seen.iter().find_map(|seen| seen.failure.clone()).unwrap_or_default();
    check(
        "a server that is not running shows the native error page",
        ok && failure.starts_with(&splat_i18n::tf("Keine Verbindung zu {}", &["127.0.0.1"])),
        failure,
    );
    if let Some(window) = controller.window() {
        snapshot(&window, "browser-error");
    }
    crate::browser::open_tab(&controller, None);
    let seen = crate::browser::inspect(&controller);
    let empty = seen.last();
    check(
        "an empty tab invites an address in its focused field",
        empty.is_some_and(|seen| seen.failure.as_deref() == Some(t("Browser")) && seen.url.is_none() && seen.editing),
        empty.and_then(|seen| seen.failure.clone()).unwrap_or_default(),
    );
    if let Some(window) = controller.window() {
        snapshot(&window, "browser-empty");
    }
    crate::browser::open_tab(&controller, Some("file:///etc/hosts"));
    let refused = crate::browser::inspect(&controller)
        .last()
        .and_then(|seen| seen.failure.clone())
        .unwrap_or_default();
    check(
        "a local file outside the window's folder stays closed",
        refused == t("Datei außerhalb des Projekts"),
        refused,
    );
    browser_project_file();
}

const FILE_TITLE: &str = "Project File";

/// A page of the window's folder opens from its path, with the stylesheet beside it.
fn browser_project_file() {
    let Some(mtm) = MainThreadMarker::new() else { return };
    let folder = ide_core::paths::home().join("browser-project");
    let page = folder.join("site/index.html");
    let _ = std::fs::create_dir_all(folder.join("site"));
    let _ = std::fs::write(
        &page,
        format!("<!doctype html><title>{FILE_TITLE}</title><link rel=stylesheet href=style.css><h1>{FILE_TITLE}</h1>"),
    );
    let _ = std::fs::write(
        folder.join("site/style.css"),
        "h1 { color: #b0451f; font: 600 28px -apple-system }",
    );
    crate::app::open_project(mtm, folder.clone());
    let root = folder.canonicalize().unwrap_or(folder);
    let Some(controller) = window::open_windows()
        .into_iter()
        .find(|w| w.project().as_ref() == Some(&root))
    else {
        check("the folder gets its window", false, "");
        finish("browser");
        return;
    };
    crate::browser::open_tab(&controller, Some(&root.join("site/index.html").to_string_lossy()));
    poll(
        Instant::now() + Duration::from_secs(20),
        || project_window().is_some_and(|c| c.tab_titles().iter().any(|t| t == FILE_TITLE)),
        browser_project_file_loaded,
    );
}

fn project_window() -> Option<Retained<window::WindowController>> {
    window::open_windows().into_iter().find(|w| w.project().is_some())
}

fn browser_project_file_loaded(ok: bool) {
    let Some(controller) = project_window() else { return };
    check(
        "a page of the window's folder opens",
        ok,
        controller.tab_titles().join(", "),
    );
    if let Some(window) = controller.window() {
        snapshot(&window, "browser-file");
    }
    finish("browser");
}

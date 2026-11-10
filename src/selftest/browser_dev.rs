//! The `browser-dev` scenario: what a dev server asks of a browser tab. A self-signed certificate on this Mac, served by
//! `openssl s_server`; an HTTP login, a JavaScript `prompt` and a download, served by a thread.

use std::cell::RefCell;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::PathBuf;
use std::process::{ChildStdin, Command, Stdio};
use std::sync::atomic::{AtomicU16, Ordering};
use std::time::{Duration, Instant};

use ide_process::{Kind, ProcessId, Supervisor};
use splat_i18n::{t, tf};

use super::{check, finish, first_window, poll, snapshot};

const SCENARIO: &str = "browser-dev";
const TLS_TITLE: &str = "TLS Page";
const PRIVATE_TITLE: &str = "Private Page";
/// `dev:secret` in Base64, as HTTP Basic sends it.
const LOGIN: &str = "ZGV2OnNlY3JldA==";
const ANSWER: &str = "Grace";
const DOWNLOAD: &str = "report.txt";

static PORT: AtomicU16 = AtomicU16::new(0);
static TLS_PORT: AtomicU16 = AtomicU16::new(0);

thread_local! {
    /// The TLS server and its input, which it reads commands from and must keep open.
    static TLS_SERVER: RefCell<Option<(ProcessId, Option<ChildStdin>)>> = const { RefCell::new(None) };
}

fn url(path: &str) -> String {
    format!("http://127.0.0.1:{}{path}", PORT.load(Ordering::SeqCst))
}

fn tls_url() -> String {
    format!("https://127.0.0.1:{}/index.html", TLS_PORT.load(Ordering::SeqCst))
}

fn tls_folder() -> PathBuf {
    ide_core::paths::home().join("tls")
}

fn free_port() -> u16 {
    TcpListener::bind("127.0.0.1:0")
        .and_then(|listener| listener.local_addr())
        .map_or(0, |address| address.port())
}

pub(super) fn run() {
    let server = TcpListener::bind("127.0.0.1:0").and_then(|listener| {
        let port = listener.local_addr()?.port();
        std::thread::spawn(move || {
            for stream in listener.incoming().flatten() {
                let _ = answer(stream);
            }
        });
        Ok(port)
    });
    match server {
        Ok(port) => PORT.store(port, Ordering::SeqCst),
        Err(error) => {
            check("the page server starts", false, error.to_string());
            finish(SCENARIO);
            return;
        }
    }
    let folder = tls_folder();
    let _ = std::fs::create_dir_all(folder.join("site"));
    let _ = std::fs::write(
        folder.join("site/index.html"),
        format!("<!doctype html><title>{TLS_TITLE}</title><h1>{TLS_TITLE}</h1>"),
    );
    let mut command = Command::new("/usr/bin/openssl");
    command
        .args([
            "req",
            "-x509",
            "-newkey",
            "rsa:2048",
            "-nodes",
            "-days",
            "1",
            "-subj",
            "/CN=localhost",
        ])
        .args(["-addext", "subjectAltName=DNS:localhost,IP:127.0.0.1"])
        .args(["-keyout", "key.pem", "-out", "cert.pem"])
        .current_dir(&folder)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    if let Err(error) = Supervisor::global().spawn(command, Kind::Tool, "openssl req") {
        check("a self-signed certificate is made", false, error.to_string());
        finish(SCENARIO);
        return;
    }
    poll(
        Instant::now() + Duration::from_secs(20),
        || std::fs::metadata(tls_folder().join("cert.pem")).is_ok_and(|m| m.len() > 0),
        start_tls_server,
    );
}

/// Serves the page that needs a login, the page that prompts and the file to download.
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
    let request = String::from_utf8_lossy(&request);
    let path = request.split_whitespace().nth(1).unwrap_or("/");
    let logged_in = request.contains(&format!("Authorization: Basic {LOGIN}"));
    let (status, headers, body) = match path {
        "/private" if logged_in => (
            "200 OK",
            "Content-Type: text/html; charset=utf-8".to_string(),
            format!("<!doctype html><title>{PRIVATE_TITLE}</title><h1>{PRIVATE_TITLE}</h1>"),
        ),
        "/private" => (
            "401 Unauthorized",
            "Content-Type: text/html; charset=utf-8\r\nWWW-Authenticate: Basic realm=\"Selftest\"".to_string(),
            "<!doctype html><title>Locked</title>".to_string(),
        ),
        "/prompt" => (
            "200 OK",
            "Content-Type: text/html; charset=utf-8".to_string(),
            "<!doctype html><title>Prompt</title><script>\
             document.title = prompt(\"Wie heißt du?\", \"Ada\") || \"cancelled\";</script>"
                .to_string(),
        ),
        "/report.txt" => (
            "200 OK",
            format!("Content-Type: text/plain\r\nContent-Disposition: attachment; filename=\"{DOWNLOAD}\""),
            "downloaded by the self-test\n".to_string(),
        ),
        _ => ("404 Not Found", "Content-Type: text/plain".to_string(), String::new()),
    };
    write!(
        stream,
        "HTTP/1.1 {status}\r\n{headers}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    )
}

fn start_tls_server(made: bool) {
    check(
        "a self-signed certificate is made",
        made,
        tls_folder().display().to_string(),
    );
    let port = free_port();
    TLS_PORT.store(port, Ordering::SeqCst);
    let mut command = Command::new("/usr/bin/openssl");
    command
        .args(["s_server", "-quiet", "-WWW", "-accept", &port.to_string()])
        .args(["-cert", "../cert.pem", "-key", "../key.pem"])
        .current_dir(tls_folder().join("site"))
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    match Supervisor::global().spawn(command, Kind::Tool, "openssl s_server") {
        Ok(spawned) => TLS_SERVER.with(|server| *server.borrow_mut() = Some((spawned.id, spawned.stdin))),
        Err(error) => {
            check("the TLS server starts", false, error.to_string());
            finish(SCENARIO);
            return;
        }
    }
    poll(
        Instant::now() + Duration::from_secs(10),
        || TcpStream::connect(("127.0.0.1", TLS_PORT.load(Ordering::SeqCst))).is_ok(),
        open_tls_page,
    );
}

fn open_tls_page(listening: bool) {
    check(
        "the TLS server listens",
        listening,
        format!("port {}", TLS_PORT.load(Ordering::SeqCst)),
    );
    let Some(controller) = first_window() else { return };
    crate::browser::open_tab(&controller, Some(&tls_url()));
    poll(
        Instant::now() + Duration::from_secs(20),
        || first_window().is_some_and(|c| crate::browser::inspect(&c).iter().any(|seen| seen.proceed)),
        tls_refused,
    );
}

fn tls_refused(offered: bool) {
    let Some(controller) = first_window() else { return };
    let failure = crate::browser::inspect(&controller)
        .iter()
        .find_map(|seen| seen.failure.clone())
        .unwrap_or_default();
    check(
        "a self-signed certificate on this Mac shows the error page with “Trotzdem öffnen”",
        offered && failure == t("Keine sichere Verbindung"),
        failure,
    );
    if let Some(window) = controller.window() {
        snapshot(&window, "browser-tls");
    }
    let pressed = crate::browser::proceed(&controller);
    check("“Trotzdem öffnen” is pressed", pressed, "");
    poll(
        Instant::now() + Duration::from_secs(20),
        || first_window().is_some_and(|c| c.tab_titles().iter().any(|t| t == TLS_TITLE)),
        tls_accepted,
    );
}

fn tls_accepted(loaded: bool) {
    let Some(controller) = first_window() else { return };
    check(
        "the accepted certificate opens the page",
        loaded,
        controller.tab_titles().join(", "),
    );
    if let Some((id, _)) = TLS_SERVER.with(|server| server.borrow_mut().take()) {
        Supervisor::global().stop(id, Duration::from_millis(500));
    }
    crate::browser::open_tab(&controller, Some(&url("/private")));
    poll(
        Instant::now() + Duration::from_secs(20),
        || first_window().is_some_and(|c| c.has_sheet()),
        login_asked,
    );
}

fn login_asked(asked: bool) {
    let Some(controller) = first_window() else { return };
    check("a page behind an HTTP login asks for it in a sheet", asked, "");
    if let Some(window) = controller.window() {
        snapshot(&window, "browser-login");
    }
    let answered = crate::browser::answer_asking(&["dev", "secret"]);
    check("the login sheet takes a user name and password", answered, "");
    poll(
        Instant::now() + Duration::from_secs(20),
        || first_window().is_some_and(|c| c.tab_titles().iter().any(|t| t == PRIVATE_TITLE)),
        logged_in,
    );
}

fn logged_in(ok: bool) {
    let Some(controller) = first_window() else { return };
    check("the login opens the page", ok, controller.tab_titles().join(", "));
    crate::browser::open_tab(&controller, Some(&url("/prompt")));
    poll(
        Instant::now() + Duration::from_secs(20),
        || first_window().is_some_and(|c| c.has_sheet()),
        prompt_asked,
    );
}

fn prompt_asked(asked: bool) {
    let Some(controller) = first_window() else { return };
    check("a JavaScript prompt shows as a sheet", asked, "");
    if let Some(window) = controller.window() {
        snapshot(&window, "browser-prompt");
    }
    crate::browser::answer_asking(&[ANSWER]);
    poll(
        Instant::now() + Duration::from_secs(20),
        || first_window().is_some_and(|c| c.tab_titles().iter().any(|t| t == ANSWER)),
        prompt_answered,
    );
}

fn prompt_answered(ok: bool) {
    let Some(controller) = first_window() else { return };
    check("the page gets the typed answer", ok, controller.tab_titles().join(", "));
    crate::browser::open_tab(&controller, Some(&url("/report.txt")));
    poll(
        Instant::now() + Duration::from_secs(20),
        || first_window().is_some_and(|c| crate::browser::inspect(&c).iter().any(|seen| seen.notice.is_some())),
        downloaded,
    );
}

fn downloaded(noticed: bool) {
    let Some(controller) = first_window() else { return };
    let notice = crate::browser::inspect(&controller)
        .iter()
        .find_map(|seen| seen.notice.clone())
        .unwrap_or_default();
    check(
        "a finished download is told in the tab",
        noticed && notice == tf("„{}“ geladen", &[DOWNLOAD]),
        notice,
    );
    let file = ide_core::paths::home().join("Downloads").join(DOWNLOAD);
    check(
        "the downloaded file is saved",
        std::fs::read_to_string(&file).is_ok_and(|text| text.contains("self-test")),
        file.display().to_string(),
    );
    check("the downloaded file is quarantined", is_quarantined(&file), "");
    if let Some(window) = controller.window() {
        snapshot(&window, "browser-download");
    }
    finish(SCENARIO);
}

/// Whether macOS marked `file` as coming from elsewhere, so Gatekeeper checks it before it first opens.
fn is_quarantined(file: &std::path::Path) -> bool {
    let Ok(path) = std::ffi::CString::new(file.as_os_str().as_encoded_bytes()) else {
        return false;
    };
    // SAFETY: Both strings end in NUL; a null buffer of size 0 asks only for the attribute's size.
    let size = unsafe {
        libc::getxattr(
            path.as_ptr(),
            c"com.apple.quarantine".as_ptr(),
            std::ptr::null_mut(),
            0,
            0,
            0,
        )
    };
    size > 0
}

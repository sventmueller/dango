//! The `browser-view` scenario: a browser tab driven by its buttons, the menu bar and its address field. Typing an
//! address, Back and Forward without a history, the reloads and stopping a slow page, an address it does not open, a
//! page's alert and confirm, a popup that closes itself, HTTP logins, a download by a link and one that breaks off,
//! and a login asked for on a page without a form.

use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::atomic::{AtomicU16, AtomicUsize, Ordering};
use std::time::{Duration, Instant};

use objc2::runtime::{AnyObject, Sel};
use objc2::{MainThreadOnly, sel};
use objc2_app_kit::{
    NSAlertFirstButtonReturn, NSAlertSecondButtonReturn, NSApplication, NSStandardKeyBindingResponding, NSTextField,
    NSTextView, NSView,
};
use objc2_foundation::NSString;
use splat_i18n::t;

use super::{check, finish, first_window};
use crate::util::main_after;

const NAME: &str = "browser-view";
static PORT: AtomicU16 = AtomicU16::new(0);
/// How many times the server sent `/b`.
static B_SENT: AtomicUsize = AtomicUsize::new(0);
const FILE_BODY: &str = "tv-file-body";

/// Serves the scenario's pages on 127.0.0.1, each request on a thread of its own so a slow page holds up no other.
fn serve() -> std::io::Result<u16> {
    let listener = TcpListener::bind("127.0.0.1:0")?;
    let port = listener.local_addr()?.port();
    std::thread::spawn(move || {
        for stream in listener.incoming().flatten() {
            std::thread::spawn(move || answer(stream));
        }
    });
    Ok(port)
}

fn page(title: &str, body: &str) -> String {
    format!("<!doctype html><html><head><meta charset=utf-8><title>{title}</title></head><body>{body}</body></html>")
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
    let path = line.split_whitespace().nth(1).unwrap_or("/").to_string();
    let link = "<a href=/b style='display:block;height:100vh'>to b</a>";
    let (kind, extra, body) = match path.as_str() {
        "/a" => ("text/html", "", page("tv-a", link)),
        "/b" => {
            B_SENT.fetch_add(1, Ordering::SeqCst);
            ("text/html", "", page("tv-b", "<p>No form here.</p>"))
        }
        "/slow" => {
            std::thread::sleep(Duration::from_secs(6));
            ("text/html", "", page("tv-slow", ""))
        }
        "/alert" => (
            "text/html",
            "",
            page(
                "tv-alert",
                "<script>onload = () => { alert('tv-alert-text'); \
                 document.title = confirm('tv-confirm-text') ? 'tv-yes' : 'tv-no'; }</script>",
            ),
        ),
        "/popup" => (
            "text/html",
            "",
            page("tv-popup", "<script>onload = () => window.open('/child')</script>"),
        ),
        "/child" => (
            "text/html",
            "",
            page("tv-child", "<script>setTimeout(() => window.close(), 500)</script>"),
        ),
        "/dl-link" => (
            "text/html",
            "",
            page(
                "tv-dl",
                "<a id=d href=/file.txt download=tv-file.txt>file</a><script>onload = () => d.click()</script>",
            ),
        ),
        "/file.txt" => ("text/plain", "", FILE_BODY.to_string()),
        "/dl-broken" => return broken_download(stream),
        "/basic" | "/digest" | "/ntlm" => return login_page(stream, &path, &line),
        _ => ("text/plain", "", String::new()),
    };
    respond(stream, "200 OK", kind, extra, &body)
}

fn respond(mut stream: TcpStream, status: &str, kind: &str, extra: &str, body: &str) -> std::io::Result<()> {
    write!(
        stream,
        "HTTP/1.1 {status}\r\nContent-Type: {kind}; charset=utf-8\r\nContent-Length: {}\r\n{extra}Connection: close\r\n\r\n{body}",
        body.len()
    )
}

/// Pages behind an HTTP login: Basic, which lets in "tv" with the password "right", and Digest and NTLM, which let in
/// nobody.
fn login_page(stream: TcpStream, path: &str, request: &str) -> std::io::Result<()> {
    let authorized = request
        .lines()
        .any(|line| line.eq_ignore_ascii_case("Authorization: Basic dHY6cmlnaHQ="));
    if path == "/basic" && authorized {
        return respond(stream, "200 OK", "text/html", "", &page("tv-in", "Logged in."));
    }
    let scheme = match path {
        "/basic" => "Basic realm=\"tv\"",
        "/digest" => "Digest realm=\"tv-digest\", nonce=\"tv-nonce\", qop=\"auth\"",
        _ => "NTLM",
    };
    let header = format!("WWW-Authenticate: {scheme}\r\n");
    respond(
        stream,
        "401 Unauthorized",
        "text/html",
        &header,
        &page("tv-denied", "Denied."),
    )
}

/// An attachment that promises far more than it sends before the connection ends.
fn broken_download(mut stream: TcpStream) -> std::io::Result<()> {
    write!(
        stream,
        "HTTP/1.1 200 OK\r\nContent-Type: application/octet-stream\r\nContent-Disposition: attachment; \
         filename=tv-broken.bin\r\nContent-Length: 100000\r\nConnection: close\r\n\r\nbroken"
    )?;
    stream.flush()?;
    std::thread::sleep(Duration::from_millis(300));
    stream.shutdown(std::net::Shutdown::Both)
}

fn url(path: &str) -> String {
    format!("http://127.0.0.1:{}{path}", PORT.load(Ordering::SeqCst))
}

fn poll(deadline: Instant, done: impl Fn() -> bool + Send + 'static, then: impl FnOnce(bool) + Send + 'static) {
    let held = done();
    if held || Instant::now() >= deadline {
        return then(held);
    }
    main_after(Duration::from_millis(100), move || poll(deadline, done, then));
}

fn wait(seconds: u64, done: impl Fn() -> bool + Send + 'static, then: impl FnOnce(bool) + Send + 'static) {
    poll(Instant::now() + Duration::from_secs(seconds), done, then);
}

fn window_id() -> u64 {
    first_window().map_or(0, |controller| controller.id())
}

/// The address of the browser tab in front.
fn front_url() -> String {
    front_seen().and_then(|seen| seen.url).unwrap_or_default()
}

/// What the browser tab in front shows, told apart from the others by its address field.
fn front_seen() -> Option<ide_ui_browser::Seen> {
    let window = window_id();
    let field = ide_ui_browser::address_field(window)?.stringValue().to_string();
    ide_ui_browser::inspect(window)
        .into_iter()
        .find(|seen| seen.address == field)
}

fn shows(path: &'static str) -> impl Fn() -> bool + Send + 'static {
    move || front_url() == url(path)
}

/// Sends `action` to the browser tab in front, as its button does.
fn press(action: Sel) {
    let Some(view) = ide_ui_browser::front_view(window_id()) else {
        return;
    };
    let app = NSApplication::sharedApplication(view.mtm());
    let target: &AnyObject = &view;
    // SAFETY: Every action of the tab takes any sender, here none.
    unsafe { app.sendAction_to_from(action, Some(target), None) };
}

/// Types `text` into the address field of the tab in front and presses Return, through the field editor.
fn enter(text: &str) {
    let window = window_id();
    ide_ui_browser::perform(window, sel!(ideBrowserFocusAddress:));
    let Some(field) = ide_ui_browser::address_field(window) else {
        return;
    };
    let Some(editor) = field
        .currentEditor()
        .and_then(|editor| editor.downcast::<NSTextView>().ok())
    else {
        return;
    };
    editor.setString(&NSString::from_str(text));
    // SAFETY: The field editor hands the command to the field's delegate, the tab.
    unsafe { editor.doCommandBySelector(sel!(insertNewline:)) };
}

pub fn run() {
    let Some(controller) = first_window() else {
        check("a window opens", false, "");
        return finish(NAME);
    };
    match serve() {
        Ok(port) => PORT.store(port, Ordering::SeqCst),
        Err(error) => {
            check("the page server starts", false, error.to_string());
            return finish(NAME);
        }
    }
    ide_ui_browser::open_popups_unasked();
    crate::browser::open_tab(&controller, Some(&url("/a")));
    wait(20, shows("/a"), |opened| {
        check("the first page opens in a browser tab", opened, front_url());
        let window = window_id();
        ide_ui_browser::perform(window, sel!(ideBrowserFocusAddress:));
        let editing = front_seen().is_some_and(|seen| seen.editing);
        enter(&url("/b"));
        wait(10, shows("/b"), move |went| {
            check(
                "the menu's command puts the focus into the address field, where Return opens what was typed",
                editing && went,
                front_url(),
            );
            arrow_in_field();
        });
    });
}

/// An arrow key in the address field moves in the field and leaves the address as it is.
fn arrow_in_field() {
    let window = window_id();
    ide_ui_browser::perform(window, sel!(ideBrowserFocusAddress:));
    let field = ide_ui_browser::address_field(window);
    let editor = field
        .as_ref()
        .and_then(|field| field.currentEditor())
        .and_then(|editor| editor.downcast::<NSTextView>().ok());
    if let Some(editor) = &editor {
        // SAFETY: The field editor hands the command to the field's delegate, the tab, which leaves it to the editor.
        unsafe { editor.doCommandBySelector(sel!(moveUp:)) };
    }
    let kept = field.is_some_and(|field| field.stringValue().to_string() == url("/b"));
    check(
        "an arrow key in the address field stays in the field",
        editor.is_some() && kept && front_seen().is_some_and(|seen| seen.editing),
        front_url(),
    );
    history();
}

/// A tab whose page nobody interacted with has no history for WebKit to go back to: Back and Forward are greyed out
/// in the menu bar, and the buttons and menu items leave the page as it is.
fn history() {
    let window = window_id();
    let offered =
        [sel!(ideBrowserBack:), sel!(ideBrowserForward:)].map(|action| ide_ui_browser::validate(window, action));
    press(sel!(ideBrowserBack:));
    press(sel!(ideBrowserForward:));
    ide_ui_browser::perform(window, sel!(ideBrowserBack:));
    ide_ui_browser::perform(window, sel!(ideBrowserForward:));
    main_after(Duration::from_secs(1), move || {
        check(
            "without a history Back and Forward are greyed out and leave the page",
            offered == [false, false] && front_url() == url("/b"),
            format!("{offered:?}, {}", front_url()),
        );
        reloads();
    });
}

/// Calls `then` once the server sent `/b` again after `ask` asked for it.
fn reloaded(ask: impl FnOnce(), then: impl FnOnce(bool) + Send + 'static) {
    let before = B_SENT.load(Ordering::SeqCst);
    ask();
    wait(10, move || B_SENT.load(Ordering::SeqCst) > before, then);
}

fn reloads() {
    reloaded(
        || press(sel!(ideBrowserReloadOrStop:)),
        |button| {
            reloaded(
                || ide_ui_browser::perform(window_id(), sel!(ideBrowserReload:)),
                move |menu| {
                    reloaded(
                        || ide_ui_browser::reload_matching(window_id(), &url("")),
                        move |origin| {
                            reloaded(
                                || press(sel!(ideBrowserRetry:)),
                                move |retry| {
                                    check(
                                        "the reload button, the menu, a reload of the server's pages and Try Again \
                                         each load the page again",
                                        button && menu && origin && retry,
                                        format!("{button} {menu} {origin} {retry}"),
                                    );
                                    stop_slow();
                                },
                            );
                        },
                    );
                },
            );
        },
    );
}

/// The reload button stops a page that is still loading, and the page before stays.
fn stop_slow() {
    enter(&url("/slow"));
    main_after(Duration::from_millis(600), || {
        press(sel!(ideBrowserReloadOrStop:));
        main_after(Duration::from_secs(7), || {
            let titles = first_window().map(|c| c.tab_titles()).unwrap_or_default();
            check(
                "the reload button stops a page while it loads, and the page before stays",
                front_url() == url("/b") && !titles.iter().any(|title| title == "tv-slow"),
                format!("{} | {}", front_url(), titles.join(", ")),
            );
            unsupported();
        });
    });
}

fn unsupported() {
    enter("ftp://127.0.0.1/tv-file");
    wait(
        5,
        || front_seen().and_then(|seen| seen.failure).as_deref() == Some(t("Adresse nicht unterstützt")),
        |refused| {
            check(
                "an address of a scheme the browser does not open shows why instead",
                refused,
                format!("{:?}", front_seen().and_then(|seen| seen.failure)),
            );
            enter(&url("/alert"));
            dialogs();
        },
    );
}

/// The texts of the sheet on the window, if one shows.
fn sheet_texts() -> Option<Vec<String>> {
    fn collect(view: &NSView, out: &mut Vec<String>) {
        if let Some(field) = view.downcast_ref::<NSTextField>() {
            out.push(field.stringValue().to_string());
        }
        for child in view.subviews().iter() {
            collect(&child, out);
        }
    }
    let sheet = first_window()?.window()?.attachedSheet()?;
    let mut texts = Vec::new();
    let content = sheet.contentView()?;
    collect(&content, &mut texts);
    Some(texts)
}

fn sheet_says(text: &'static str) -> impl Fn() -> bool + Send + 'static {
    move || sheet_texts().is_some_and(|texts| texts.iter().any(|shown| shown.contains(text)))
}

/// Ends the sheet on the window with its first button.
fn answer_sheet() {
    let Some(window) = first_window().and_then(|controller| controller.window()) else {
        return;
    };
    if let Some(sheet) = window.attachedSheet() {
        window.endSheet_returnCode(&sheet, NSAlertFirstButtonReturn);
    }
}

/// A page's `alert` and `confirm` ask in a sheet that names the page, and the page gets the answer.
fn dialogs() {
    wait(10, sheet_says("tv-alert-text"), |alerted| {
        let named = sheet_texts().is_some_and(|texts| texts.iter().any(|text| text.contains("127.0.0.1")));
        answer_sheet();
        wait(10, sheet_says("tv-confirm-text"), move |asked| {
            answer_sheet();
            let titled = || first_window().is_some_and(|c| c.tab_titles().iter().any(|title| title == "tv-yes"));
            wait(10, titled, move |answered| {
                check(
                    "a page's alert and confirm ask in a sheet that names the page, and OK reaches the page",
                    alerted && named && asked && answered,
                    format!("{alerted} {named} {asked} {answered}"),
                );
                popup();
            });
        });
    });
}

fn tab_ids() -> Vec<isize> {
    first_window()
        .map(|controller| controller.tab_ids())
        .unwrap_or_default()
}

fn popup_open() -> bool {
    ide_ui_browser::inspect(window_id())
        .iter()
        .any(|seen| seen.url.as_deref() == Some(url("/child").as_str()))
}

/// A page's popup opens a tab of its own, which closes when the popup closes itself.
fn popup() {
    let before = tab_ids().len();
    enter(&url("/popup"));
    wait(15, popup_open, move |opened| {
        let during = tab_ids().len();
        let closed = move || tab_ids().len() == before && !popup_open();
        wait(20, closed, move |closed| {
            check(
                "a page's popup opens in a tab of its own, which closes when the popup closes itself",
                opened && during == before + 1 && closed,
                format!("{before}, {during}, then {}", tab_ids().len()),
            );
            basic_login();
        });
    });
}

fn titled(title: &'static str) -> impl Fn() -> bool + Send + 'static {
    move || first_window().is_some_and(|c| c.tab_titles().iter().any(|shown| shown == title))
}

/// A wrong password asks again and says so; the right one lets the page in.
fn basic_login() {
    enter(&url("/basic"));
    let wrong = || crate::browser::answer_asking(&["tv", "wrong"]);
    wait(10, wrong, |asked| {
        wait(
            10,
            sheet_says(t("Benutzername oder Passwort waren falsch.")),
            move |again| {
                let right = || crate::browser::answer_asking(&["tv", "right"]);
                wait(10, right, move |_| {
                    wait(10, titled("tv-in"), move |admitted| {
                        check(
                            "a page behind a login asks for it, asks again after a wrong password and opens with the \
                         right one",
                            asked && again && admitted,
                            format!("{asked} {again} {admitted}"),
                        );
                        digest_cancelled();
                    });
                });
            },
        );
    });
}

/// Abbrechen in the login of a Digest page shows the server's refusal; an NTLM page is refused without asking.
fn digest_cancelled() {
    enter(&url("/digest"));
    wait(
        10,
        sheet_says(t("Die Seite verlangt einen Benutzernamen und ein Passwort.")),
        |asked| {
            if let Some(window) = first_window().and_then(|controller| controller.window())
                && let Some(sheet) = window.attachedSheet()
            {
                window.endSheet_returnCode(&sheet, NSAlertSecondButtonReturn);
            }
            wait(10, shows("/digest"), move |refused| {
                enter(&url("/ntlm"));
                wait(10, shows("/ntlm"), move |ntlm| {
                    let unasked = sheet_texts().is_none();
                    check(
                        "Abbrechen in a page's login shows the server's refusal, and a login the browser cannot give is \
                     refused without asking",
                        asked && refused && ntlm && unasked,
                        format!("{asked} {refused} {ntlm} {unasked}"),
                    );
                    download_link();
                });
            });
        },
    );
}

/// A link with `download` saves its file and says so in the notice bar, which closes.
fn download_link() {
    let file = ide_core::paths::home().join("Downloads").join("tv-file.txt");
    enter(&url("/dl-link"));
    let saved = file.clone();
    let done = move || std::fs::read_to_string(&saved).is_ok_and(|text| text == FILE_BODY);
    wait(10, done, move |downloaded| {
        let told = front_seen().and_then(|seen| seen.notice).is_some();
        press(sel!(ideBrowserCloseNotice:));
        let closed = front_seen().is_some_and(|seen| seen.notice.is_none());
        check(
            "a link with download saves the file into Downloads, the notice says so and closes",
            downloaded && told && closed,
            format!("{downloaded} {told} {closed} {}", file.display()),
        );
        enter(&url("/dl-broken"));
        wait(10, sheet_says(t("Download fehlgeschlagen")), |failed| {
            check(
                "a download that breaks off says so in a sheet",
                failed,
                format!("{:?}", sheet_texts()),
            );
            answer_sheet();
            login_without_form();
        });
    });
}

/// A login asked for on a page without a form ends in a notice that says so.
fn login_without_form() {
    enter(&url("/b"));
    wait(10, shows("/b"), |_| {
        press(sel!(ideBrowserFillLogin:));
        let answered = || crate::browser::answer_asking(&["tv-user", "tv-secret"]);
        wait(10, answered, |asked| {
            let told = || {
                front_seen().and_then(|seen| seen.notice).is_some_and(|notice| {
                    notice == t("Auf dieser Seite gibt es kein Anmeldeformular, oder sie hat inzwischen gewechselt.")
                })
            };
            wait(10, told, move |told| {
                press(sel!(ideBrowserCloseNotice:));
                let closed = front_seen().is_some_and(|seen| seen.notice.is_none());
                check(
                    "a login for a page without a form says so in the notice bar, which closes",
                    asked && told && closed,
                    format!("{asked} {told} {closed}"),
                );
                finish(NAME);
            });
        });
    });
}

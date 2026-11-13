//! What the user can hand Claude besides files: the output of the terminal in view, the selection of an editor and
//! the page of a browser tab. Each becomes an attachment of the message; the terminal's and the page's are outside
//! text (E22).

use std::cell::RefCell;
use std::collections::HashMap;
use std::ptr::NonNull;
use std::time::Duration;

use block2::RcBlock;
use ide_thread::Attachment;
use objc2::Message;
use objc2::rc::Retained;
use objc2::runtime::AnyObject;
use objc2_app_kit::NSView;
use objc2_foundation::{NSError, NSString};
use objc2_web_kit::WKWebView;

use ide_ui_common::util::{main_after, ns};
use ide_ui_common::window::AsWindow;

/// How many lines of terminal output go along at most, the latest.
const TERMINAL_LINES: usize = 200;
/// How long a page may take to say what is selected on it.
const BROWSER_WAIT: Duration = Duration::from_millis(1500);

/// The output of the terminal in view, else of the one opened last.
pub fn terminal(controller: &(impl AsWindow + ?Sized)) -> Option<Attachment> {
    let controller = &controller.as_window();
    let (label, text) = crate::host::host().recent_output(controller.id())?;
    let lines: Vec<&str> = text.lines().collect();
    let shown = &lines[lines.len().saturating_sub(TERMINAL_LINES)..];
    Some(super::outside::terminal(&label, shown.join("\n")))
}

/// The selection of the editor in view in `controller`'s window, else of any of its editors that has one.
pub fn editor_selection(controller: &(impl AsWindow + ?Sized)) -> Option<Attachment> {
    let controller = &controller.as_window();
    let (path, lines, text) = crate::host::host().editor_selection(controller.id())?;
    Some(Attachment::Selection { path, lines, text })
}

/// The web views of `view` and below, the visible ones first.
fn web_views(view: &NSView, found: &mut Vec<Retained<WKWebView>>) {
    if let Some(web) = view.downcast_ref::<WKWebView>() {
        found.push(web.retain());
        return;
    }
    for child in view.subviews().iter() {
        web_views(&child, found);
    }
}

/// What runs once a page said what is selected on it.
type Waiting = Box<dyn FnOnce(Option<String>)>;

/// The answers awaited from pages about what is selected on them, by id.
pub struct PageSelections {
    waiting: RefCell<HashMap<u64, Waiting>>,
    next: std::cell::Cell<u64>,
}

impl Default for PageSelections {
    fn default() -> Self {
        PageSelections {
            waiting: RefCell::new(HashMap::new()),
            next: std::cell::Cell::new(1),
        }
    }
}

fn with_waiting<R>(f: impl FnOnce(&RefCell<HashMap<u64, Waiting>>) -> R) -> R {
    f(&crate::host::state().page_selections.waiting)
}

fn with_next<R>(f: impl FnOnce(&std::cell::Cell<u64>) -> R) -> R {
    f(&crate::host::state().page_selections.next)
}

fn settle(key: u64, selection: Option<String>) {
    if let Some(done) = with_waiting(|w| w.borrow_mut().remove(&key)) {
        done(selection);
    }
}

/// The page of the browser tab in view in `controller`'s window, else of any: its address, title and selected text,
/// handed to `done` once the page answered or did not in time.
pub fn browser(controller: &(impl AsWindow + ?Sized), done: impl FnOnce(Option<Attachment>) + 'static) {
    let controller = &controller.as_window();
    let mut found = Vec::new();
    for window in controller.tab_windows() {
        if let Some(content) = window.contentView() {
            web_views(&content, &mut found);
        }
    }
    found.sort_by_key(|w| {
        w.isHiddenOrHasHiddenAncestor() || !w.window().is_some_and(|w| ide_ui_common::panel::in_front(&w))
    });
    let Some(web) = found.into_iter().next() else {
        done(None);
        return;
    };
    // SAFETY: The web view lives on the main thread, where this runs, and only its properties are read.
    let (url, title) = unsafe { (web.URL(), web.title()) };
    let url = url
        .and_then(|u| u.absoluteString())
        .map(|s| s.to_string())
        .unwrap_or_default();
    let title = title.map(|t| t.to_string()).unwrap_or_default();
    let key = with_next(|n| {
        let key = n.get();
        n.set(key + 1);
        key
    });
    let page = (url, title);
    with_waiting(|w| {
        w.borrow_mut().insert(
            key,
            Box::new(move |selection: Option<String>| done(Some(page_attachment(&page.0, &page.1, selection)))),
        );
    });
    let answered = RcBlock::new(move |result: *mut AnyObject, _error: *mut NSError| {
        ide_ui_common::util::guarded((), || {
            // SAFETY: WebKit hands over the script's result, a string for this script, or null; it stays alive for the
            // call.
            let text = NonNull::new(result)
                .and_then(|r| unsafe { r.as_ref() }.downcast_ref::<NSString>().map(|s| s.to_string()));
            settle(key, text.filter(|t| !t.trim().is_empty()));
        })
    });
    // SAFETY: The script only reads the selection; WebKit calls the block on the main thread.
    unsafe {
        web.evaluateJavaScript_completionHandler(&ns("window.getSelection().toString()"), Some(&answered));
    }
    main_after(BROWSER_WAIT, move || settle(key, None));
}

fn page_attachment(url: &str, title: &str, selection: Option<String>) -> Attachment {
    let mut text = format!("URL: {url}\nTitle: {title}");
    if let Some(selection) = &selection {
        text.push_str(&format!("\nSelected text:\n{selection}"));
    }
    super::outside::page(url, title, text)
}

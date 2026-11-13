//! The panels a page may ask for: `alert`, `confirm`, `prompt`, a login and choosing files to upload, each as a
//! sheet on the tab's window and naming the site that asks, as WebKit asks of a browser; and the panel that takes a
//! login from macOS AutoFill for the page's form.

#[cfg(feature = "selftest")]
use std::cell::RefCell;

use block2::{DynBlock, RcBlock};
use objc2::MainThreadOnly;
use objc2::rc::Retained;
use objc2::runtime::Bool;
use objc2_app_kit::{
    NSAlert, NSAlertFirstButtonReturn, NSModalResponse, NSModalResponseOK, NSOpenPanel, NSSecureTextField,
    NSTextContent, NSTextField, NSView, NSWindow,
};
use objc2_foundation::{
    NSArray, NSPoint, NSRect, NSSize, NSString, NSURL, NSURLAuthenticationChallenge, NSURLCredential,
    NSURLCredentialPersistence, NSURLSessionAuthChallengeDisposition,
};
use objc2_web_kit::{WKFrameInfo, WKOpenPanelParameters};
use splat_i18n::{t, tf};

use ide_ui_common::constant;
use ide_ui_common::util::{guarded, ns};

/// How WebKit takes the answer to a login.
pub type LoginAnswer = RcBlock<dyn Fn(NSURLSessionAuthChallengeDisposition, *mut NSURLCredential)>;

const FIELD_WIDTH: f64 = 260.0;
const FIELD_HEIGHT: f64 = 24.0;

/// A panel with its text fields.
#[cfg(feature = "selftest")]
type Asking = (Retained<NSAlert>, Vec<Retained<NSTextField>>);

#[cfg(feature = "selftest")]
/// The panel with text fields that shows, for the self-test to fill in.
#[derive(Default)]
pub struct Dialogs {
    asking: RefCell<Option<Asking>>,
}

#[cfg(feature = "selftest")]
fn with_asking<R>(f: impl FnOnce(&RefCell<Option<Asking>>) -> R) -> R {
    f(&super::host::state().dialogs.asking)
}

/// The host of the frame that asks, which the panel names so a page cannot pass itself off as the app.
fn asker(frame: &WKFrameInfo) -> String {
    // SAFETY: The frame and its request are valid while WebKit asks.
    let url = unsafe { frame.request() }.URL();
    url.and_then(|u| u.host()).map_or_else(String::new, |h| h.to_string())
}

/// A panel naming the site whose page shows `message`; `question` for one the page wants an answer to.
fn page_alert(window: &NSWindow, message: &NSString, frame: &WKFrameInfo, question: bool) -> Retained<NSAlert> {
    let alert = NSAlert::new(window.mtm());
    let host = asker(frame);
    let heading = match (host.is_empty(), question) {
        (true, false) => t("Die Seite meldet:").to_string(),
        (true, true) => t("Die Seite fragt:").to_string(),
        (false, false) => tf("Die Seite „{}“ meldet:", &[&host]),
        (false, true) => tf("Die Seite „{}“ fragt:", &[&host]),
    };
    alert.setMessageText(&ns(&heading));
    alert.setInformativeText(message);
    alert
}

/// A text field of a panel, holding `value`, or a password field.
fn text_field(window: &NSWindow, value: &str, placeholder: &str, secret: bool) -> Retained<NSTextField> {
    let frame = NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(FIELD_WIDTH, FIELD_HEIGHT));
    let field: Retained<NSTextField> = if secret {
        NSSecureTextField::initWithFrame(NSSecureTextField::alloc(window.mtm()), frame).into_super()
    } else {
        NSTextField::initWithFrame(NSTextField::alloc(window.mtm()), frame)
    };
    field.setStringValue(&ns(value));
    field.setPlaceholderString(Some(&ns(placeholder)));
    ide_ui_common::util::set_label(&field, Some(placeholder));
    field
}

/// A user name field holding `user` and a password field, marked so macOS AutoFill offers the Passwords app's logins.
fn credential_fields(window: &NSWindow, user: &str) -> Vec<Retained<NSTextField>> {
    let name = text_field(window, user, t("Benutzername"), false);
    name.setContentType(Some(constant!(objc2_app_kit::NSTextContentTypeUsername)));
    let password = text_field(window, "", t("Passwort"), true);
    password.setContentType(Some(constant!(objc2_app_kit::NSTextContentTypePassword)));
    vec![name, password]
}

/// Puts `fields` below each other into `alert` and focuses the first.
fn add_fields(alert: &NSAlert, fields: &[Retained<NSTextField>]) {
    let Some(first) = fields.first() else { return };
    let gap = 8.0;
    let count = fields.len() as f64;
    let height = count * FIELD_HEIGHT + (count - 1.0) * gap;
    let stack = NSView::initWithFrame(
        NSView::alloc(first.mtm()),
        NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(FIELD_WIDTH, height)),
    );
    for (index, field) in fields.iter().enumerate() {
        let y = height - (index as f64 + 1.0) * FIELD_HEIGHT - index as f64 * gap;
        field.setFrameOrigin(NSPoint::new(0.0, y));
        stack.addSubview(field);
    }
    for pair in fields.windows(2) {
        // SAFETY: Both fields live in the same accessory view, which keeps them for as long as the panel shows.
        unsafe { pair[0].setNextKeyView(Some(&pair[1])) };
    }
    alert.setAccessoryView(Some(&stack));
    alert.layout();
    alert.window().setInitialFirstResponder(Some(first));
}

/// Shows `alert` as a sheet of `window`; `done` gets whether its first button closed it and the fields' texts.
fn ask(
    window: &NSWindow,
    alert: Retained<NSAlert>,
    fields: Vec<Retained<NSTextField>>,
    done: impl Fn(bool, Vec<String>) + 'static,
) {
    #[cfg(feature = "selftest")]
    with_asking(|asking| *asking.borrow_mut() = Some((alert.clone(), fields.clone())));
    let read = fields.clone();
    let handler = RcBlock::new(move |response: NSModalResponse| {
        #[cfg(feature = "selftest")]
        with_asking(|asking| asking.borrow_mut().take());
        let texts = read.iter().map(|field| field.stringValue().to_string()).collect();
        done(response == NSAlertFirstButtonReturn, texts);
    });
    alert.beginSheetModalForWindow_completionHandler(window, Some(&handler));
}

/// Fills the fields of the panel that asks with `texts` and closes it with its first button, as the self-test's
/// user; whether a panel was asking.
#[cfg(feature = "selftest")]
pub fn answer_asking(texts: &[&str]) -> bool {
    let Some((alert, fields)) = with_asking(|asking| asking.borrow().clone()) else {
        return false;
    };
    for (field, text) in fields.iter().zip(texts) {
        field.setStringValue(&ns(text));
    }
    let Some(first) = alert.buttons().firstObject() else {
        return false;
    };
    // SAFETY: Clicking the alert's own button ends its sheet as a person would; a nil sender is allowed.
    unsafe { first.performClick(None) };
    true
}

/// A JavaScript `alert`; without a window WebKit gets its answer right away.
pub fn alert(window: Option<&NSWindow>, message: &NSString, frame: &WKFrameInfo, done: &DynBlock<dyn Fn()>) {
    let Some(window) = window else {
        done.call(());
        return;
    };
    let alert = page_alert(window, message, frame, false);
    alert.addButtonWithTitle(&ns(t("OK")));
    let done = done.copy();
    let handler = RcBlock::new(move |_: NSModalResponse| done.call(()));
    alert.beginSheetModalForWindow_completionHandler(window, Some(&handler));
}

/// A JavaScript `confirm`; without a window it counts as cancelled.
pub fn confirm(window: Option<&NSWindow>, message: &NSString, frame: &WKFrameInfo, done: &DynBlock<dyn Fn(Bool)>) {
    let Some(window) = window else {
        done.call((Bool::NO,));
        return;
    };
    let alert = page_alert(window, message, frame, true);
    alert.addButtonWithTitle(&ns(t("OK")));
    alert.addButtonWithTitle(&ns(t("Abbrechen")));
    let done = done.copy();
    let handler = RcBlock::new(move |response: NSModalResponse| {
        done.call((Bool::new(response == NSAlertFirstButtonReturn),));
    });
    alert.beginSheetModalForWindow_completionHandler(window, Some(&handler));
}

/// A JavaScript `prompt`, answered with the typed text, or nothing when cancelled or without a window.
pub fn prompt(
    window: Option<&NSWindow>,
    question: &NSString,
    default: Option<&NSString>,
    frame: &WKFrameInfo,
    done: &DynBlock<dyn Fn(*mut NSString)>,
) {
    let Some(window) = window else {
        done.call((std::ptr::null_mut(),));
        return;
    };
    let alert = page_alert(window, question, frame, true);
    alert.addButtonWithTitle(&ns(t("OK")));
    alert.addButtonWithTitle(&ns(t("Abbrechen")));
    let default = default.map(|d| d.to_string()).unwrap_or_default();
    let field = text_field(window, &default, "", false);
    add_fields(&alert, std::slice::from_ref(&field));
    let done = done.copy();
    ask(window, alert, vec![field], move |ok, texts| {
        let text = ok.then(|| ns(texts.first().map_or("", String::as_str)));
        let pointer = text
            .as_ref()
            .map_or(std::ptr::null_mut(), |t| Retained::as_ptr(t).cast_mut());
        done.call((pointer,));
    });
}

/// Asks for the user name and password a site's HTTP login wants. They live in memory until the app quits, so the
/// page's other requests need no second asking, and are never written anywhere; "Abbrechen" lets the page show
/// the server's answer to a missing login.
pub fn login(window: Option<&NSWindow>, challenge: &NSURLAuthenticationChallenge, done: LoginAnswer) {
    let refuse = NSURLSessionAuthChallengeDisposition::RejectProtectionSpace;
    let Some(window) = window else {
        done.call((refuse, std::ptr::null_mut()));
        return;
    };
    let space = challenge.protectionSpace();
    let alert = NSAlert::new(window.mtm());
    alert.setMessageText(&ns(&tf("Anmelden bei „{}“", &[&space.host().to_string()])));
    let mut detail = vec![];
    if challenge.previousFailureCount() > 0 {
        detail.push(t("Benutzername oder Passwort waren falsch."));
    }
    detail.push(t("Die Seite verlangt einen Benutzernamen und ein Passwort."));
    if !space.receivesCredentialSecurely() {
        detail.push(t(
            "Die Verbindung ist nicht verschlüsselt, das Passwort wird lesbar übertragen.",
        ));
    }
    alert.setInformativeText(&ns(&detail.join(" ")));
    alert.addButtonWithTitle(&ns(t("Anmelden")));
    alert.addButtonWithTitle(&ns(t("Abbrechen")));
    let proposed = challenge
        .proposedCredential()
        .and_then(|credential| credential.user())
        .map(|user| user.to_string())
        .unwrap_or_default();
    let fields = credential_fields(window, &proposed);
    add_fields(&alert, &fields);
    ask(window, alert, fields, move |ok, texts| {
        let [user, password] = texts.as_slice() else {
            done.call((refuse, std::ptr::null_mut()));
            return;
        };
        if !ok {
            done.call((refuse, std::ptr::null_mut()));
            return;
        }
        let credential = NSURLCredential::credentialWithUser_password_persistence(
            &ns(user),
            &ns(password),
            NSURLCredentialPersistence::ForSession,
        );
        let pointer = Retained::as_ptr(&credential).cast_mut();
        done.call((NSURLSessionAuthChallengeDisposition::UseCredential, pointer));
    });
}

/// Asks for the login to fill into the form of the page at `host`, offering what macOS AutoFill has; `done` gets
/// the user name and password unless the user cancelled. `plain` for a page that sends them unencrypted.
pub fn fill_login(window: &NSWindow, host: &str, plain: bool, done: impl Fn(String, String) + 'static) {
    let alert = NSAlert::new(window.mtm());
    alert.setMessageText(&ns(&tf("Anmeldung für „{}“ ausfüllen", &[host])));
    let mut detail = vec![t(
        "Wähle im Feld eine Anmeldung aus Passwörter; die App trägt sie in das Formular der Seite ein und behält sie nicht.",
    )];
    if plain {
        detail.push(t(
            "Die Verbindung ist nicht verschlüsselt, das Passwort wird lesbar übertragen.",
        ));
    }
    alert.setInformativeText(&ns(&detail.join(" ")));
    alert.addButtonWithTitle(&ns(t("Ausfüllen")));
    alert.addButtonWithTitle(&ns(t("Abbrechen")));
    let fields = credential_fields(window, "");
    add_fields(&alert, &fields);
    // macOS AutoFill offers the Passwords app's logins for other sites only in the password field, and fills the
    // user name field beside it from there.
    if let Some(password) = fields.last() {
        alert.window().setInitialFirstResponder(Some(password));
    }
    ask(window, alert, fields, move |ok, texts| {
        if let (true, [user, password]) = (ok, texts.as_slice())
            && !(user.is_empty() && password.is_empty())
        {
            done(user.clone(), password.clone());
        }
    });
}

/// The files a page's upload field gets: only those the user picks in the open panel.
pub fn choose_files(
    window: Option<&NSWindow>,
    parameters: &WKOpenPanelParameters,
    done: &DynBlock<dyn Fn(*mut NSArray<NSURL>)>,
) {
    let Some(window) = window else {
        done.call((std::ptr::null_mut(),));
        return;
    };
    let panel = NSOpenPanel::openPanel(window.mtm());
    // SAFETY: Plain properties of the parameters WebKit passes for this request.
    let (multiple, folders) = unsafe { (parameters.allowsMultipleSelection(), parameters.allowsDirectories()) };
    panel.setAllowsMultipleSelection(multiple);
    panel.setCanChooseDirectories(folders);
    panel.setCanChooseFiles(true);
    let done = done.copy();
    let chosen = panel.clone();
    let handler = RcBlock::new(move |response: NSModalResponse| {
        let urls = guarded(None, || (response == NSModalResponseOK).then(|| chosen.URLs()));
        let pointer = urls
            .as_ref()
            .map_or(std::ptr::null_mut(), |urls| Retained::as_ptr(urls).cast_mut());
        done.call((pointer,));
    });
    panel.beginSheetModalForWindow_completionHandler(window, &handler);
}

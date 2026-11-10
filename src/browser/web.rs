//! Calls into WebKit: the configuration every tab shares and the rules for what a page may open. objc2-web-kit marks
//! every method unsafe because the framework has not been reviewed for it yet, so most blocks here state only that a
//! method asks for nothing.

use std::cell::OnceCell;
use std::path::Path;

use objc2::rc::Retained;
use objc2::runtime::{AnyObject, ProtocolObject};
use objc2::{MainThreadMarker, MainThreadOnly};
use objc2_app_kit::NSEventModifierFlags;
use objc2_foundation::{NSNumber, NSURL, NSURLRequest};
use objc2_web_kit::{
    WKDownload, WKDownloadDelegate, WKNavigationAction, WKNavigationActionPolicy, WKNavigationResponse,
    WKNavigationResponsePolicy, WKNavigationType, WKWebView, WKWebViewConfiguration, WKWebsiteDataStore,
};

use super::address;
use crate::util::{guarded, ns};

thread_local! {
    static STORE: OnceCell<Retained<WKWebsiteDataStore>> = const { OnceCell::new() };
}

/// Where the pages of every tab keep cookies and storage (E17): the app's one persistent store, so a login to a dev
/// server survives a restart; a self-test keeps nothing.
fn store(mtm: MainThreadMarker) -> Retained<WKWebsiteDataStore> {
    STORE.with(|store| {
        store
            .get_or_init(|| {
                #[cfg(feature = "selftest")]
                if std::env::var_os("IDE_SELFTEST").is_some() {
                    // SAFETY: A store that lives in memory only, created on the main thread.
                    return unsafe { WKWebsiteDataStore::nonPersistentDataStore(mtm) };
                }
                // SAFETY: The app's default store, created on the main thread.
                unsafe { WKWebsiteDataStore::defaultDataStore(mtm) }
            })
            .clone()
    })
}

/// The configuration of a new tab. It adds no script message handler, so no page can call into the app.
pub fn configuration(mtm: MainThreadMarker) -> Retained<WKWebViewConfiguration> {
    // SAFETY: A plain configuration object, created on the main thread.
    let config = unsafe { WKWebViewConfiguration::new(mtm) };
    // SAFETY: The shared store and plain flags, set before any view uses the configuration.
    unsafe {
        config.setWebsiteDataStore(&store(mtm));
        let preferences = config.preferences();
        preferences.setJavaScriptCanOpenWindowsAutomatically(false);
        preferences.setFraudulentWebsiteWarningEnabled(true);
        preferences.setElementFullscreenEnabled(true);
    }
    enable_inspect_element(&config);
    config
}

/// Adds "Inspect Element" to a page's context menu. `isInspectable` alone only lets Safari's Develop menu attach;
/// the menu item needs this preference, which WebKit offers through key-value coding only.
fn enable_inspect_element(config: &WKWebViewConfiguration) {
    guarded((), || {
        // SAFETY: The preferences object answers this key with a BOOL property; an unknown key raises an exception,
        // which objc2 turns into a panic that `guarded` stops.
        unsafe {
            let preferences = config.preferences();
            let on = NSNumber::new_bool(true);
            let _: () = objc2::msg_send![&*preferences, setValue: &*on, forKey: &*ns("developerExtrasEnabled")];
        }
    });
}

/// Whether `web` offers the Web Inspector, from Safari's Develop menu and its own context menu.
#[cfg(feature = "selftest")]
pub fn has_inspector(web: &WKWebView) -> bool {
    // SAFETY: Plain properties; reading the key `enable_inspect_element` set raises no exception once setting it did
    // not.
    unsafe {
        let preferences = web.configuration().preferences();
        let extras: Option<Retained<AnyObject>> =
            objc2::msg_send![&*preferences, valueForKey: &*ns("developerExtrasEnabled")];
        let extras = extras.is_some_and(|value| value.downcast_ref::<NSNumber>().is_some_and(|n| n.boolValue()));
        web.isInspectable() && extras
    }
}

pub fn new_view(mtm: MainThreadMarker, config: &WKWebViewConfiguration) -> Retained<WKWebView> {
    // SAFETY: The configuration is complete and the frame empty, which constraints replace.
    let web = unsafe {
        WKWebView::initWithFrame_configuration(WKWebView::alloc(mtm), objc2_foundation::NSRect::ZERO, config)
    };
    // SAFETY: Plain flags of a new view; `setInspectable` exists from macOS 13.3, below the app's minimum.
    unsafe {
        web.setInspectable(true);
        web.setAllowsBackForwardNavigationGestures(true);
        web.setAllowsMagnification(true);
    }
    web
}

/// Loads `url` in `web`; `false` if it is no URL.
pub fn load(web: &WKWebView, url: &str) -> bool {
    let Some(url) = NSURL::URLWithString(&ns(url)) else {
        return false;
    };
    // SAFETY: A request for a URL that parsed.
    unsafe { web.loadRequest(&NSURLRequest::requestWithURL(&url)) };
    true
}

/// Loads the file at `path`, letting the page read the files of `root` and nothing else.
pub fn load_file(web: &WKWebView, path: &Path, root: &Path) {
    let file = NSURL::fileURLWithPath(&ns(&path.to_string_lossy()));
    let folder = NSURL::fileURLWithPath_isDirectory(&ns(&root.to_string_lossy()), true);
    // SAFETY: Both are file URLs, the file inside the folder, checked by the caller.
    unsafe { web.loadFileURL_allowingReadAccessToURL(&file, &folder) };
}

/// What the view shows: its URL, title, whether it loads and where it can go.
pub struct State {
    pub url: Option<String>,
    pub title: String,
    pub loading: bool,
    pub back: bool,
    pub forward: bool,
}

pub fn state(web: &WKWebView) -> State {
    // SAFETY: Plain properties of a view on the main thread.
    unsafe {
        State {
            url: web.URL().and_then(|u| u.absoluteString()).map(|s| s.to_string()),
            title: web.title().map(|s| s.to_string()).unwrap_or_default(),
            loading: web.isLoading(),
            back: web.canGoBack(),
            forward: web.canGoForward(),
        }
    }
}

pub fn go_back(web: &WKWebView) {
    // SAFETY: Asks for nothing; without a page to go back to it does nothing.
    unsafe { web.goBack() };
}

pub fn go_forward(web: &WKWebView) {
    // SAFETY: Asks for nothing; without a page to go forward to it does nothing.
    unsafe { web.goForward() };
}

pub fn reload(web: &WKWebView) {
    // SAFETY: Asks for nothing.
    unsafe { web.reload() };
}

pub fn stop(web: &WKWebView) {
    // SAFETY: Asks for nothing.
    unsafe { web.stopLoading() };
}

/// What to do with a navigation: let it load, cancel it, cancel it and open its URL in a new tab, or download it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Decision {
    Allow,
    Cancel,
    NewTab(String),
    Download,
}

/// Decides a navigation of a page in a window whose folder is `root`. Pages may lead to web pages, to local files
/// only inside the folder (a web page reaches none, WebKit sees to that), and to other apps' schemes never.
pub fn decide_action(action: &WKNavigationAction, root: Option<&Path>) -> Decision {
    // SAFETY: The action and its request are valid while WebKit asks about them.
    let (url, download, link, command, main_frame) = unsafe {
        let url = action
            .request()
            .URL()
            .and_then(|u| u.absoluteString())
            .map(|s| s.to_string());
        let main_frame = action.targetFrame().is_some_and(|frame| frame.isMainFrame());
        (
            url,
            action.shouldPerformDownload(),
            action.navigationType() == WKNavigationType::LinkActivated,
            action.modifierFlags().contains(NSEventModifierFlags::Command),
            main_frame,
        )
    };
    let Some(url) = url else { return Decision::Cancel };
    let scheme = address::scheme_of(&url).unwrap_or_default();
    let allowed = match scheme.as_str() {
        "file" => address::file_path(&url)
            .zip(root)
            .is_some_and(|(path, root)| address::is_inside(&path, root)),
        scheme => address::is_web_scheme(scheme),
    };
    if !allowed {
        return Decision::Cancel;
    }
    if download {
        return Decision::Download;
    }
    if link && command && main_frame && matches!(scheme.as_str(), "http" | "https") {
        return Decision::NewTab(url);
    }
    Decision::Allow
}

/// Downloads what a page cannot show, and what its server sends as an attachment.
pub fn decide_response(response: &WKNavigationResponse) -> WKNavigationResponsePolicy {
    // SAFETY: The response is valid while WebKit asks about it.
    let (main_frame, showable, response) = unsafe {
        (
            response.isForMainFrame(),
            response.canShowMIMEType(),
            response.response(),
        )
    };
    let attachment = response
        .downcast_ref::<objc2_foundation::NSHTTPURLResponse>()
        .and_then(|http| http.valueForHTTPHeaderField(&ns("Content-Disposition")))
        .is_some_and(|value| {
            value
                .to_string()
                .trim_start()
                .to_ascii_lowercase()
                .starts_with("attachment")
        });
    if main_frame && (!showable || attachment) {
        WKNavigationResponsePolicy::Download
    } else {
        WKNavigationResponsePolicy::Allow
    }
}

pub fn action_policy(decision: &Decision) -> WKNavigationActionPolicy {
    match decision {
        Decision::Allow => WKNavigationActionPolicy::Allow,
        Decision::Download => WKNavigationActionPolicy::Download,
        Decision::Cancel | Decision::NewTab(_) => WKNavigationActionPolicy::Cancel,
    }
}

/// Hands `download` to `delegate`, which asks where to save it.
pub fn adopt_download(download: &WKDownload, delegate: &ProtocolObject<dyn WKDownloadDelegate>) {
    // SAFETY: The download keeps its delegate weakly; the delegate lives as long as the app.
    unsafe { download.setDelegate(Some(delegate)) };
}

/// The value of `key` in an error's details, as text.
pub fn error_detail(error: &objc2_foundation::NSError, key: &objc2_foundation::NSString) -> Option<String> {
    let value: Retained<AnyObject> = error.userInfo().objectForKey(key)?;
    if let Some(text) = value.downcast_ref::<objc2_foundation::NSString>() {
        return Some(text.to_string());
    }
    let url = value.downcast_ref::<NSURL>()?;
    url.absoluteString().map(|s| s.to_string())
}

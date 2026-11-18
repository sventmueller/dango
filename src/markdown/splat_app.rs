//! Opening a Markdown document in Splat, the Markdown editor this app takes its preview from, where it is installed.

use std::path::{Path, PathBuf};

use objc2::rc::Retained;
use objc2::runtime::{AnyObject, NSObject};
use objc2::{MainThreadMarker, MainThreadOnly, define_class, msg_send, sel};
use objc2_app_kit::{NSMenuItem, NSWorkspace, NSWorkspaceOpenConfiguration};
use objc2_foundation::{NSArray, NSObjectProtocol, NSString, NSURL};
use splat_i18n::t;

use crate::util::{guarded, item, ns, set_target};

/// Splat's bundle identifier, which Launch Services finds it by wherever it is installed.
const SPLAT: &str = "local.splat";

/// Where Splat is installed, if it is.
pub(crate) fn location() -> Option<Retained<NSURL>> {
    NSWorkspace::sharedWorkspace().URLForApplicationWithBundleIdentifier(&ns(SPLAT))
}

/// Opens the file at `path` in Splat, after an editor that has it open saved what it holds; a document that does
/// not save stays here.
pub(crate) fn open(_mtm: MainThreadMarker, path: &Path) {
    let Some(app) = location() else { return };
    if !saved(path) {
        return;
    }
    let url = NSURL::fileURLWithPath(&ns(&path.to_string_lossy()));
    let configuration = NSWorkspaceOpenConfiguration::configuration();
    NSWorkspace::sharedWorkspace().openURLs_withApplicationAtURL_configuration_completionHandler(
        &NSArray::from_slice(&[&*url]),
        &app,
        &configuration,
        None,
    );
}

/// Saves every editor of `path` with unsaved changes; whether none is left.
pub(crate) fn saved(path: &Path) -> bool {
    let unsaved = || {
        crate::editor::all_editors()
            .into_iter()
            .filter(|view| view.document().path() == Some(path) && view.document().is_dirty())
            .collect::<Vec<_>>()
    };
    for view in unsaved() {
        crate::editor::save(&view, false);
    }
    unsaved().is_empty()
}

/// "In Splat öffnen" for a context menu, when `path` is a Markdown file and Splat is installed.
pub(crate) fn open_in_splat_item(mtm: MainThreadMarker, path: &Path) -> Option<Retained<NSMenuItem>> {
    if !crate::preview::is_markdown(path) || location().is_none() {
        return None;
    }
    let entry = item(mtm, t("In Splat öffnen"), Some(sel!(ideOpenInSplat:)), "");
    // SAFETY: An NSString, which the target reads back as the path.
    unsafe { entry.setRepresentedObject(Some(&ns(&path.to_string_lossy()))) };
    set_target(&entry, &target(mtm));
    Some(entry)
}

define_class!(
    // SAFETY: NSObject has no subclassing requirements and `OpenTarget` does not implement Drop.
    #[unsafe(super(NSObject))]
    #[thread_kind = MainThreadOnly]
    #[name = "IDEOpenInSplatTarget"]
    #[ivars = ()]
    struct OpenTarget;

    unsafe impl NSObjectProtocol for OpenTarget {}

    impl OpenTarget {
        #[unsafe(method(ideOpenInSplat:))]
        fn open_in_splat(&self, sender: Option<&AnyObject>) {
            guarded((), || {
                let path = sender
                    .and_then(|s| s.downcast_ref::<NSMenuItem>())
                    .and_then(|item| item.representedObject())
                    .and_then(|object| object.downcast::<NSString>().ok())
                    .map(|path| PathBuf::from(path.to_string()));
                if let Some(path) = path {
                    open(self.mtm(), &path);
                }
            });
        }
    }
);

thread_local! {
    /// Menu items hold their target weakly, so one target lives as long as the app.
    static TARGET: std::cell::OnceCell<Retained<OpenTarget>> = const { std::cell::OnceCell::new() };
}

fn target(mtm: MainThreadMarker) -> Retained<OpenTarget> {
    TARGET.with(|cell| {
        cell.get_or_init(|| {
            let this = OpenTarget::alloc(mtm).set_ivars(());
            // SAFETY: NSObject's initializer, called once on the newly allocated object.
            unsafe { msg_send![super(this), init] }
        })
        .clone()
    })
}

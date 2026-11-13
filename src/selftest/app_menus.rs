//! The `app-menus` scenario: the commands the app delegate and the navigator answer. Back and forward from the Go menu
//! and the toolbar skip a place whose file is gone and say which way there is to go; Open Recent lists the recent
//! folders, opens one and clears them; folders handed to the app open, a file does not; a click on the Dock icon
//! without a window opens one; installing the terminal command writes it, never over a file of the user's, and says
//! how it went.

use std::path::{Path, PathBuf};
use std::time::Duration;

use ide_core::settings::{State, remembered};
use objc2::rc::Retained;
use objc2::runtime::{AnyObject, Bool, Sel};
use objc2::{MainThreadMarker, MainThreadOnly, Message, msg_send, sel};
use objc2_app_kit::{NSAlertFirstButtonReturn, NSApplication, NSMenu, NSToolbarItemGroup};
use objc2_foundation::{NSArray, NSURL};
use splat_i18n::{t, tf};

use super::{check, finish, helpers};
use crate::util::ns;
use crate::window::WindowController;

const NAME: &str = "app-menus";

fn folders() -> (PathBuf, PathBuf) {
    let home = ide_core::paths::home();
    (home.join("menus-a"), home.join("menus-b"))
}

pub fn run() {
    let Some(mtm) = MainThreadMarker::new() else { return };
    let (a, b) = folders();
    for (folder, files) in [(&a, &["one.txt", "gone.txt", "two.txt"][..]), (&b, &["b.txt"][..])] {
        std::fs::create_dir_all(folder).expect("scratch project");
        for file in files {
            std::fs::write(folder.join(file), format!("{file}\n")).expect("scratch file");
        }
        crate::project::trust(folder);
    }
    crate::app::open_project(mtm, a.clone());
    helpers::wait_for_window(&a.clone(), move |controller| {
        let Some(controller) = controller else {
            check("the project's window opens", false, "");
            return finish(NAME);
        };
        navigation(&controller, &a);
        recent(controller.mtm());
    });
}

/// Where the editor in front is, by file name.
fn front_file(controller: &WindowController) -> Option<String> {
    let view = crate::editor::editor_of_tab(controller, controller.selected_tab()?)?;
    Some(view.document().path()?.file_name()?.to_string_lossy().into_owned())
}

fn navigation(controller: &WindowController, root: &Path) {
    let mtm = controller.mtm();
    for name in ["one.txt", "gone.txt", "two.txt"] {
        let path = root.join(name);
        controller.open_file(&path);
        if let Some(view) = crate::editor::editor_of(controller, &path)
            && let Some(window) = view.window()
        {
            window.makeFirstResponder(Some(&view));
        }
    }
    let _ = std::fs::remove_file(root.join("gone.txt"));
    let navigator = crate::navigation::navigator(mtm);
    let front = crate::window::frontmost(mtm).is_some_and(|c| std::ptr::eq(&*c, controller));
    send(&navigator, sel!(ideGoBack:), None);
    let back = front_file(controller);
    let validate = |action: Sel| {
        let item = crate::util::item(mtm, "", Some(action), "");
        // SAFETY: The navigator answers `validateMenuItem:` with a BOOL for any menu item.
        let enabled: Bool = unsafe { msg_send![&*navigator, validateMenuItem: &*item] };
        enabled.as_bool()
    };
    let can = (validate(sel!(ideGoBack:)), validate(sel!(ideGoForward:)));
    check(
        "Go Back skips a place whose file is gone, and then only forward is left",
        front && back.as_deref() == Some("one.txt") && can == (false, true),
        format!("front window {front}, back to {back:?}, back/forward enabled {can:?}"),
    );
    let group = controller.window().and_then(|window| {
        window
            .toolbar()?
            .items()
            .iter()
            .find(|item| item.itemIdentifier().to_string() == crate::navigation::TOOLBAR_ID)
            .and_then(|item| item.downcast::<NSToolbarItemGroup>().ok())
    });
    if let Some(group) = &group {
        group.setSelectedIndex(1);
        send(&navigator, sel!(ideGoBackOrForward:), Some(group));
    }
    let forward = front_file(controller);
    check(
        "the toolbar's second arrow goes forward, past the gone file",
        group.is_some() && forward.as_deref() == Some("two.txt"),
        format!("{forward:?}"),
    );
    send(&navigator, sel!(ideGoBack:), None);
    send(&navigator, sel!(ideGoForward:), None);
    check(
        "Go Forward from the menu comes back to where Go Back left",
        front_file(controller).as_deref() == Some("two.txt"),
        format!("{:?}", front_file(controller)),
    );
}

fn send(target: &AnyObject, action: Sel, sender: Option<&AnyObject>) {
    let Some(mtm) = MainThreadMarker::new() else { return };
    // SAFETY: Each action sent takes the sender, which may be none, as a menu item or a toolbar item sends it.
    unsafe { NSApplication::sharedApplication(mtm).sendAction_to_from(action, Some(target), sender) };
}

fn delegate(mtm: MainThreadMarker) -> Option<Retained<AnyObject>> {
    let delegate = NSApplication::sharedApplication(mtm).delegate()?;
    let object: &AnyObject = delegate.as_ref();
    Some(object.retain())
}

fn titles(menu: &NSMenu) -> Vec<(String, bool)> {
    menu.itemArray()
        .iter()
        .filter(|item| !item.isSeparatorItem())
        .map(|item| (item.title().to_string(), item.isEnabled()))
        .collect()
}

fn recent(mtm: MainThreadMarker) {
    let (a, b) = folders();
    let (a, b) = (a.canonicalize().unwrap_or(a), b.canonicalize().unwrap_or(b));
    let Some(delegate) = delegate(mtm) else {
        check("the app has its delegate", false, "");
        return finish(NAME);
    };
    let _ = State::load_user().note_recent(&b);
    let menu = NSMenu::new(mtm);
    // SAFETY: The delegate answers `menuNeedsUpdate:` for any menu.
    unsafe {
        let _: () = msg_send![&*delegate, menuNeedsUpdate: &*menu];
    }
    let listed = titles(&menu);
    let clear = t("Liste löschen");
    check(
        "Open Recent lists the recent folders, latest first, then an enabled item that clears them",
        listed.first().map(|(title, _)| title.clone()) == Some(crate::app::display_path(&b))
            && listed.iter().any(|(title, _)| *title == crate::app::display_path(&a))
            && listed.last() == Some(&(clear.to_string(), true)),
        format!("{listed:?}"),
    );
    menu.performActionForItemAtIndex(0);
    check(
        "a recent folder opens in place of the one in front",
        helpers::window_of(&b).is_some() && helpers::window_of(&a).is_none(),
        format!("{:?}", crate::window::open_projects()),
    );
    menu.performActionForItemAtIndex(menu.numberOfItems() - 1);
    // SAFETY: As above.
    unsafe {
        let _: () = msg_send![&*delegate, menuNeedsUpdate: &*menu];
    }
    check(
        "clearing forgets the recent folders and leaves the clear item greyed out",
        State::load_user().get(&remembered::RECENT_PROJECTS).is_empty()
            && titles(&menu) == [(clear.to_string(), false)],
        format!("{:?}", titles(&menu)),
    );
    handed(mtm, &delegate, &a, &b);
    helpers::wait_until(
        move || NSApplication::sharedApplication(mtm).modalWindow().is_none(),
        |_| install(),
    );
}

/// Folders and files handed to the app, as the Finder or the terminal command hand them, and a click on the Dock icon.
fn handed(mtm: MainThreadMarker, delegate: &AnyObject, a: &Path, b: &Path) {
    let windows = crate::window::open_windows().len();
    let urls = NSArray::from_retained_slice(&[
        NSURL::fileURLWithPath(&ns(&a.to_string_lossy())),
        NSURL::fileURLWithPath(&ns(&a.join("one.txt").to_string_lossy())),
    ]);
    let app = NSApplication::sharedApplication(mtm);
    // SAFETY: The delegate's `application:openURLs:` takes the application and an array of file URLs.
    unsafe {
        let _: () = msg_send![delegate, application: &*app, openURLs: &*urls];
    }
    check(
        "a folder handed to the app takes the place of the one in front, a file handed with it opens no window",
        helpers::window_of(a).is_some()
            && helpers::window_of(b).is_none()
            && crate::window::open_windows().len() == windows,
        format!("{:?}", crate::window::open_projects()),
    );
    // SAFETY: The delegate answers `applicationShouldHandleReopen:hasVisibleWindows:` with a BOOL.
    let handled: Bool = unsafe { msg_send![delegate, applicationShouldHandleReopen: &*app, hasVisibleWindows: false] };
    check(
        "a click on the Dock icon without a visible window opens one",
        handled.as_bool() && crate::window::open_windows().len() == windows + 1,
        format!("{} → {}", windows, crate::window::open_windows().len()),
    );
}

/// Installs the terminal command three times: into an empty folder, over a file of the user's, and where a folder is
/// in the way; each time an alert says how it went.
fn install() {
    let Some(mtm) = MainThreadMarker::new() else { return };
    let Some(delegate) = delegate(mtm) else {
        return finish(NAME);
    };
    let command = ide_product::COMMAND;
    let path = ide_core::paths::home().join(".local/bin").join(command);
    let installed = tf("Der Befehl „{}“ ist installiert.", &[command]);
    let refused = tf("Der Befehl „{}“ wurde nicht installiert.", &[command]);
    let run = |message: &str| {
        let answered = helpers::answer_alert_saying(
            Duration::from_millis(400),
            message.to_string(),
            NSAlertFirstButtonReturn,
        );
        send(&delegate, sel!(ideInstallCommand:), None);
        answered.get()
    };
    let said = run(&installed);
    let script = std::fs::read_to_string(&path).unwrap_or_default();
    let mode = std::fs::metadata(&path).map_or(0, |m| std::os::unix::fs::PermissionsExt::mode(&m.permissions()));
    check(
        "the terminal command is installed, executable, and the alert says so",
        said && script.contains("# Installed by") && mode & 0o111 != 0,
        format!("alert {said}, mode {mode:o}"),
    );
    let mine = "#!/bin/sh\necho mine\n";
    let _ = std::fs::write(&path, mine);
    let said = run(&refused);
    check(
        "a command of the user's at the place stays as it is, and the alert says the install was refused",
        said && std::fs::read_to_string(&path).is_ok_and(|text| text == mine),
        format!("alert {said}"),
    );
    let _ = std::fs::remove_file(&path);
    let _ = std::fs::create_dir_all(&path);
    let said = run(&refused);
    check(
        "a folder in the command's place fails the install, and the alert says so",
        said && path.is_dir(),
        format!("alert {said}"),
    );
    send(&delegate, sel!(ideShowSettings:), None);
    check(
        "Settings… opens the settings window",
        crate::settings::window().is_some_and(|w| w.isVisible()),
        "",
    );
    finish(NAME);
}

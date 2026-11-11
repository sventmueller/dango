//! The CI tab of the settings: the host rules that say which service runs on a host whose name does not tell, and
//! the OAuth apps the browser login uses on hosts other than github.com and gitlab.com. Only the user sets either.

use std::cell::{OnceCell, RefCell};

use ide_ci::{HostRule, Kind};
use ide_core::settings::Settings;
use objc2::rc::Retained;
use objc2::runtime::{AnyObject, NSObject, ProtocolObject};
use objc2::{DefinedClass, MainThreadMarker, MainThreadOnly, Message, define_class, msg_send, sel};
use objc2_app_kit::{
    NSAccessibility, NSAlert, NSAlertFirstButtonReturn, NSButton, NSColor, NSControlTextEditingDelegate, NSFont,
    NSLayoutAttribute, NSLineBreakMode, NSPopUpButton, NSScrollView, NSStackView, NSTableColumn, NSTableView,
    NSTableViewDataSource, NSTableViewDelegate, NSTextField, NSUserInterfaceLayoutOrientation, NSView,
};
use objc2_foundation::{NSNotification, NSObjectProtocol, NSPoint, NSRect, NSSize};
use splat_i18n::{t, tf};

use super::list_bar;
use crate::ci_panel::{CI_HOSTS, CI_OAUTH_CLIENTS};
use crate::util::{guarded, label, ns};

const TABLE_WIDTH: f64 = 420.0;
const TABLE_HEIGHT: f64 = 110.0;

/// The two lists of the tab.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum List {
    Hosts,
    Apps,
}

pub struct CiIvars {
    hosts: RefCell<Vec<String>>,
    apps: RefCell<Vec<String>>,
    views: OnceCell<Views>,
}

struct Views {
    hosts: Retained<NSTableView>,
    apps: Retained<NSTableView>,
    remove_host: Retained<NSButton>,
    remove_app: Retained<NSButton>,
    problem: Retained<NSTextField>,
}

define_class!(
    // SAFETY: NSObject has no subclassing requirements and `CiSettings` does not implement Drop.
    #[unsafe(super(NSObject))]
    #[thread_kind = MainThreadOnly]
    #[name = "IDECiSettings"]
    #[ivars = CiIvars]
    pub struct CiSettings;

    unsafe impl NSObjectProtocol for CiSettings {}

    unsafe impl NSControlTextEditingDelegate for CiSettings {}

    unsafe impl NSTableViewDataSource for CiSettings {
        #[unsafe(method(numberOfRowsInTableView:))]
        fn row_count(&self, table: &NSTableView) -> isize {
            guarded(0, || isize::try_from(self.rows(self.list_of(table)).len()).unwrap_or(0))
        }
    }

    unsafe impl NSTableViewDelegate for CiSettings {
        #[unsafe(method_id(tableView:viewForTableColumn:row:))]
        fn view_for(&self, table: &NSTableView, column: Option<&NSTableColumn>, row: isize) -> Option<Retained<NSView>> {
            guarded(None, || {
                let first = column.is_some_and(|c| c.identifier().to_string() == "first");
                let row = self.rows(self.list_of(table)).into_iter().nth(usize::try_from(row).ok()?)?;
                let cell = label(self.mtm(), if first { &row[0] } else { &row[1] });
                cell.setLineBreakMode(NSLineBreakMode::ByTruncatingMiddle);
                Some(cell.into_super().into_super())
            })
        }

        #[unsafe(method(tableViewSelectionDidChange:))]
        fn selection_changed(&self, _notification: &NSNotification) {
            guarded((), || self.update_buttons());
        }
    }

    impl CiSettings {
        #[unsafe(method(ideAddCiHost:))]
        fn add_host_action(&self, _sender: Option<&AnyObject>) {
            guarded((), || self.ask_host());
        }

        #[unsafe(method(ideRemoveCiHost:))]
        fn remove_host_action(&self, _sender: Option<&AnyObject>) {
            guarded((), || self.remove_selected(List::Hosts));
        }

        #[unsafe(method(ideRemoveCiApp:))]
        fn remove_app_action(&self, _sender: Option<&AnyObject>) {
            guarded((), || self.remove_selected(List::Apps));
        }
    }
);

impl CiSettings {
    pub fn new(mtm: MainThreadMarker) -> Retained<CiSettings> {
        let this = Self::alloc(mtm).set_ivars(CiIvars {
            hosts: RefCell::new(Vec::new()),
            apps: RefCell::new(Vec::new()),
            views: OnceCell::new(),
        });
        // SAFETY: NSObject's initializer, called once on the newly allocated object.
        let this: Retained<CiSettings> = unsafe { msg_send![super(this), init] };
        this.read();
        this
    }

    /// Reads both lists from the user's settings again and shows them.
    pub fn reload(&self) {
        self.read();
        if let Some(views) = self.ivars().views.get() {
            views.hosts.reloadData();
            views.apps.reloadData();
        }
        self.update_buttons();
    }

    fn read(&self) {
        let settings = Settings::load_user();
        *self.ivars().hosts.borrow_mut() = settings.get(&CI_HOSTS);
        *self.ivars().apps.borrow_mut() = settings.get(&CI_OAUTH_CLIENTS);
    }

    fn list_of(&self, table: &NSTableView) -> List {
        match self.ivars().views.get() {
            Some(views) if std::ptr::eq(table, &*views.apps) => List::Apps,
            _ => List::Hosts,
        }
    }

    /// The rows of `list` as the table shows them: the place, then the service or the app id.
    pub fn rows(&self, list: List) -> Vec<[String; 2]> {
        match list {
            List::Hosts => self
                .ivars()
                .hosts
                .borrow()
                .iter()
                .map(|entry| match HostRule::parse(entry) {
                    Some(rule) => [format!("{}{}", rule.host, rule.prefix), service_name(rule.kind).into()],
                    None => [entry.clone(), "?".into()],
                })
                .collect(),
            List::Apps => self
                .ivars()
                .apps
                .borrow()
                .iter()
                .map(|entry| {
                    let (host, id) = entry.split_once('=').unwrap_or((entry, ""));
                    [host.to_string(), id.to_string()]
                })
                .collect(),
        }
    }

    fn update_buttons(&self) {
        let Some(views) = self.ivars().views.get() else { return };
        views.remove_host.setEnabled(views.hosts.selectedRow() >= 0);
        views.remove_app.setEnabled(views.apps.selectedRow() >= 0);
    }

    fn set_problem(&self, problem: Option<&str>) {
        let Some(views) = self.ivars().views.get() else { return };
        views.problem.setStringValue(&ns(problem.unwrap_or("")));
        views.problem.setHidden(problem.is_none());
    }

    /// Writes `entries` as the setting of `list` and tells the CI panels.
    fn store(&self, list: List, entries: &[String]) {
        let mut settings = Settings::load_user();
        let written = match list {
            List::Hosts => settings.set(&CI_HOSTS, &entries.to_vec()),
            List::Apps => settings.set(&CI_OAUTH_CLIENTS, &entries.to_vec()),
        };
        if let Err(error) = written {
            crate::diag::report("saving the CI settings", error);
        }
        self.reload();
        crate::ci_panel::settings_changed();
    }

    fn remove_selected(&self, list: List) {
        let Some(views) = self.ivars().views.get() else { return };
        let table = match list {
            List::Hosts => &views.hosts,
            List::Apps => &views.apps,
        };
        let Ok(row) = usize::try_from(table.selectedRow()) else {
            return;
        };
        let mut entries = match list {
            List::Hosts => self.ivars().hosts.borrow().clone(),
            List::Apps => self.ivars().apps.borrow().clone(),
        };
        if row < entries.len() {
            entries.remove(row);
            self.store(list, &entries);
        }
    }

    /// Keeps the rule `place` is run by `kind`, in place of one for the same place; else says why not.
    fn add_rule(&self, place: &str, kind: Kind) {
        let place = place.trim();
        let bare = ["https://", "http://"]
            .iter()
            .find_map(|scheme| place.strip_prefix(scheme))
            .unwrap_or(place);
        let Some(rule) = HostRule::parse(&format!("{bare}={}", kind.word())) else {
            self.set_problem(Some(&tf("„{}“ ist kein Host, den die App versteht.", &[place])));
            return;
        };
        self.set_problem(None);
        let mut entries: Vec<String> = self
            .ivars()
            .hosts
            .borrow()
            .iter()
            .filter(|entry| {
                HostRule::parse(entry).is_none_or(|r| (r.host, r.prefix) != (rule.host.clone(), rule.prefix.clone()))
            })
            .cloned()
            .collect();
        entries.push(rule.entry());
        self.store(List::Hosts, &entries);
    }

    /// The sheet that asks for a host and its service.
    fn ask_host(&self) {
        let Some(window) = self.ivars().views.get().and_then(|v| v.hosts.window()) else {
            return;
        };
        let mtm = self.mtm();
        let alert = NSAlert::new(mtm);
        alert.setMessageText(&ns(t("CI-Host hinzufügen")));
        alert.setInformativeText(&ns(t(
            "Der Host, mit Port und Pfad, wo der Dienst sie braucht, etwa „code.example.com:8443/gitlab“.",
        )));
        alert.addButtonWithTitle(&ns(t("Hinzufügen")));
        alert.addButtonWithTitle(&ns(t("Abbrechen")));
        let field = NSTextField::textFieldWithString(&ns(""), mtm);
        field.setPlaceholderString(Some(&ns("git.example.com")));
        let service = NSPopUpButton::new(mtm);
        for kind in [Kind::GitHub, Kind::GitLab] {
            service.addItemWithTitle(&ns(service_name(kind)));
        }
        service.selectItemAtIndex(1);
        let accessory = NSStackView::new(mtm);
        accessory.setOrientation(NSUserInterfaceLayoutOrientation::Vertical);
        accessory.setAlignment(NSLayoutAttribute::Leading);
        accessory.setSpacing(8.0);
        accessory.addArrangedSubview(&field);
        accessory.addArrangedSubview(&service);
        field.widthAnchor().constraintEqualToConstant(300.0).setActive(true);
        accessory.setFrame(NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(300.0, 56.0)));
        alert.setAccessoryView(Some(&accessory));
        alert.window().setInitialFirstResponder(Some(&field));
        let this = self.retain();
        crate::ci_panel::sheet(&alert, &window, move |response| {
            if response != NSAlertFirstButtonReturn {
                return;
            }
            let kind = if service.indexOfSelectedItem() == 0 {
                Kind::GitHub
            } else {
                Kind::GitLab
            };
            this.add_rule(&field.stringValue().to_string(), kind);
        });
    }

    /// The tab's view.
    pub fn view(&self) -> Retained<NSView> {
        let mtm = self.mtm();
        let (hosts, hosts_scroll) = table(mtm, self, [t("Host"), t("Dienst")]);
        let (apps, apps_scroll) = table(mtm, self, [t("Host"), t("App-ID")]);
        let add_host = list_bar::button(mtm, true, t("CI-Host hinzufügen"), self, sel!(ideAddCiHost:));
        let remove_host = list_bar::button(mtm, false, t("CI-Host entfernen"), self, sel!(ideRemoveCiHost:));
        let remove_app = list_bar::button(mtm, false, t("OAuth-App entfernen"), self, sel!(ideRemoveCiApp:));
        let problem = NSTextField::wrappingLabelWithString(&ns(""), mtm);
        problem.setTextColor(Some(&NSColor::systemRedColor()));
        problem.setPreferredMaxLayoutWidth(TABLE_WIDTH);
        problem.setHidden(true);
        let stack = NSStackView::new(mtm);
        stack.setOrientation(NSUserInterfaceLayoutOrientation::Vertical);
        stack.setAlignment(NSLayoutAttribute::Leading);
        stack.setSpacing(8.0);
        let views: [Retained<NSView>; 9] = [
            heading(mtm, t("Hosts mit CI-Dienst")),
            note(
                mtm,
                t(
                    "Für Hosts, deren Name nicht verrät, welcher Dienst dort läuft. Nur du legst sie fest: ein Projekt könnte sonst einen Token an einen fremden Host schicken.",
                ),
            ),
            hosts_scroll,
            list_bar::bar(mtm, &[&add_host, &remove_host], &[]),
            problem.clone().into_super().into_super(),
            heading(mtm, t("OAuth-Apps für die Browser-Anmeldung")),
            note(
                mtm,
                t(
                    "Auf anderen Hosts als github.com und gitlab.com fragt die Browser-Anmeldung einmal nach der ID der dort registrierten App. Entfernst du sie, fragt sie beim nächsten Mal neu.",
                ),
            ),
            apps_scroll,
            list_bar::bar(mtm, &[&remove_app], &[]),
        ];
        for view in &views {
            stack.addArrangedSubview(view);
        }
        stack.setCustomSpacing_afterView(18.0, &views[4]);
        list_bar::attach(&stack);
        let _ = self.ivars().views.set(Views {
            hosts,
            apps,
            remove_host,
            remove_app,
            problem,
        });
        self.update_buttons();
        let view = NSView::new(mtm);
        view.addSubview(&stack);
        stack.setTranslatesAutoresizingMaskIntoConstraints(false);
        for constraint in [
            stack
                .leadingAnchor()
                .constraintEqualToAnchor_constant(&view.leadingAnchor(), 30.0),
            stack
                .trailingAnchor()
                .constraintEqualToAnchor_constant(&view.trailingAnchor(), -30.0),
            stack
                .topAnchor()
                .constraintEqualToAnchor_constant(&view.topAnchor(), 24.0),
            stack
                .bottomAnchor()
                .constraintEqualToAnchor_constant(&view.bottomAnchor(), -24.0),
        ] {
            constraint.setActive(true);
        }
        view
    }
}

fn service_name(kind: Kind) -> &'static str {
    match kind {
        Kind::GitHub => "GitHub",
        Kind::GitLab => "GitLab",
    }
}

fn heading(mtm: MainThreadMarker, text: &str) -> Retained<NSView> {
    let heading = label(mtm, text);
    heading.setFont(Some(&NSFont::boldSystemFontOfSize(NSFont::systemFontSize())));
    heading.into_super().into_super()
}

fn note(mtm: MainThreadMarker, text: &str) -> Retained<NSView> {
    let note = NSTextField::wrappingLabelWithString(&ns(text), mtm);
    note.setFont(Some(&NSFont::systemFontOfSize(NSFont::smallSystemFontSize())));
    note.setTextColor(Some(&NSColor::secondaryLabelColor()));
    note.setPreferredMaxLayoutWidth(TABLE_WIDTH);
    note.into_super().into_super()
}

/// A table with two columns titled `titles`, whose rows `owner` gives, in its scroll view.
fn table(mtm: MainThreadMarker, owner: &CiSettings, titles: [&str; 2]) -> (Retained<NSTableView>, Retained<NSView>) {
    let frame = NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(TABLE_WIDTH, TABLE_HEIGHT));
    let table = NSTableView::initWithFrame(NSTableView::alloc(mtm), frame);
    for (identifier, title, width) in [("first", titles[0], 260.0), ("second", titles[1], 140.0)] {
        let column = NSTableColumn::initWithIdentifier(NSTableColumn::alloc(mtm), &ns(identifier));
        column.setTitle(&ns(title));
        column.setWidth(width);
        table.addTableColumn(&column);
    }
    table.setUsesAlternatingRowBackgroundColors(true);
    table.setAllowsEmptySelection(true);
    table.setAccessibilityLabel(Some(&ns(titles[0])));
    // SAFETY: The table holds its data source and delegate weakly; the settings controller keeps `owner` alive.
    unsafe {
        table.setDataSource(Some(ProtocolObject::from_ref(owner)));
        table.setDelegate(Some(ProtocolObject::from_ref(owner)));
    }
    let scroll = NSScrollView::initWithFrame(NSScrollView::alloc(mtm), frame);
    scroll.setHasVerticalScroller(true);
    scroll.setAutohidesScrollers(true);
    scroll.setBorderType(objc2_app_kit::NSBorderType::BezelBorder);
    scroll.setDocumentView(Some(&table));
    scroll
        .widthAnchor()
        .constraintEqualToConstant(TABLE_WIDTH)
        .setActive(true);
    scroll
        .heightAnchor()
        .constraintEqualToConstant(TABLE_HEIGHT)
        .setActive(true);
    (table, scroll.into_super())
}

/// What the self-test reads from and does with the CI tab, through the code its controls run.
#[cfg(feature = "selftest")]
pub mod selftest {
    use objc2::DefinedClass;
    use objc2_app_kit::NSModalResponse;

    use super::{CiSettings, List};
    use crate::util::ns;

    /// The rows the table of `list` shows, and how many it counts.
    pub fn rows(owner: &CiSettings, list: List) -> (Vec<[String; 2]>, isize) {
        let shown = owner.ivars().views.get().map_or(0, |views| match list {
            List::Hosts => views.hosts.numberOfRows(),
            List::Apps => views.apps.numberOfRows(),
        });
        (owner.rows(list), shown)
    }

    /// Selects `row` of `list` and clicks its remove button.
    pub fn remove(owner: &CiSettings, list: List, row: usize) {
        let Some(views) = owner.ivars().views.get() else { return };
        let (table, button) = match list {
            List::Hosts => (&views.hosts, &views.remove_host),
            List::Apps => (&views.apps, &views.remove_app),
        };
        table.selectRowIndexes_byExtendingSelection(&objc2_foundation::NSIndexSet::indexSetWithIndex(row), false);
        // SAFETY: `set_action` gave the button a target that answers its action with the sender.
        unsafe { button.performClick(None) };
    }

    /// Asks for a host to add, as the add button does.
    pub fn click_add(owner: &CiSettings) {
        owner.ask_host();
    }

    /// Types `place` into the add sheet, picks the service at `service`, and ends it with `response`.
    pub fn answer_add(owner: &CiSettings, place: &str, service: isize, response: NSModalResponse) -> bool {
        let Some(window) = owner.ivars().views.get().and_then(|v| v.hosts.window()) else {
            return false;
        };
        let Some(sheet) = window.attachedSheet() else {
            return false;
        };
        fn collect(view: &objc2_app_kit::NSView, into: &mut Vec<objc2::rc::Retained<objc2_app_kit::NSView>>) {
            for subview in view.subviews().iter() {
                collect(&subview, into);
                into.push(subview);
            }
        }
        let mut views = vec![];
        if let Some(content) = sheet.contentView() {
            collect(&content, &mut views);
        }
        for view in &views {
            if let Some(field) = view.downcast_ref::<objc2_app_kit::NSTextField>()
                && field.isEditable()
            {
                field.setStringValue(&ns(place));
            }
            if let Some(popup) = view.downcast_ref::<objc2_app_kit::NSPopUpButton>() {
                popup.selectItemAtIndex(service);
            }
        }
        window.endSheet_returnCode(&sheet, response);
        true
    }

    /// The problem the tab shows, if any.
    pub fn problem(owner: &CiSettings) -> Option<String> {
        let views = owner.ivars().views.get()?;
        (!views.problem.isHidden()).then(|| views.problem.stringValue().to_string())
    }
}

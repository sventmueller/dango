//! The language servers tab of the settings (E10): where this Mac has the server of each built-in language, and the
//! servers the user sets up for further languages or in place of a built-in one. Only the user sets these; a project
//! cannot (E22).

use std::cell::{OnceCell, RefCell};

use ide_core::settings::Settings;
use ide_lsp::CustomServer;
use objc2::rc::Retained;
use objc2::runtime::{AnyObject, NSObject};
use objc2::{DefinedClass, MainThreadMarker, MainThreadOnly, Message, define_class, msg_send, sel};
use objc2_app_kit::{
    NSAlert, NSAlertFirstButtonReturn, NSButton, NSControlTextEditingDelegate, NSLayoutAttribute, NSStackView,
    NSTableColumn, NSTableView, NSTableViewDataSource, NSTableViewDelegate, NSTextField,
    NSUserInterfaceLayoutOrientation, NSView,
};
use objc2_foundation::{NSNotification, NSObjectProtocol, NSPoint, NSRect, NSSize};
use splat_i18n::t;

use super::{form, list_bar};
use crate::util::{guarded, ns};
use ide_ui_lsp::LANGUAGE_SERVERS;

const BUILT_IN_HEIGHT: f64 = 190.0;
const CUSTOM_HEIGHT: f64 = 110.0;

pub struct ServersIvars {
    /// The built-in languages with where their server is, once the survey came back.
    found: RefCell<Vec<(String, String)>>,
    custom: RefCell<Vec<CustomServer>>,
    views: OnceCell<Views>,
}

struct Views {
    built_in: Retained<NSTableView>,
    custom: Retained<NSTableView>,
    remove: Retained<NSButton>,
    problem: Retained<NSTextField>,
}

define_class!(
    // SAFETY: NSObject has no subclassing requirements and `ServersSettings` does not implement Drop.
    #[unsafe(super(NSObject))]
    #[thread_kind = MainThreadOnly]
    #[name = "IDEServersSettings"]
    #[ivars = ServersIvars]
    pub struct ServersSettings;

    unsafe impl NSObjectProtocol for ServersSettings {}

    unsafe impl NSControlTextEditingDelegate for ServersSettings {}

    unsafe impl NSTableViewDataSource for ServersSettings {
        #[unsafe(method(numberOfRowsInTableView:))]
        fn row_count(&self, table: &NSTableView) -> isize {
            guarded(0, || isize::try_from(self.rows(self.is_custom(table)).len()).unwrap_or(0))
        }
    }

    unsafe impl NSTableViewDelegate for ServersSettings {
        #[unsafe(method_id(tableView:viewForTableColumn:row:))]
        fn view_for(&self, table: &NSTableView, column: Option<&NSTableColumn>, row: isize) -> Option<Retained<NSView>> {
            guarded(None, || {
                let index: usize = column?.identifier().to_string().parse().ok()?;
                let row = self.rows(self.is_custom(table)).into_iter().nth(usize::try_from(row).ok()?)?;
                Some(form::cell(self.mtm(), row.get(index)?))
            })
        }

        #[unsafe(method(tableViewSelectionDidChange:))]
        fn selection_changed(&self, _notification: &NSNotification) {
            guarded((), || self.update_buttons());
        }
    }

    impl ServersSettings {
        #[unsafe(method(ideAddLanguageServer:))]
        fn add_action(&self, _sender: Option<&AnyObject>) {
            guarded((), || self.ask());
        }

        #[unsafe(method(ideRemoveLanguageServer:))]
        fn remove_action(&self, _sender: Option<&AnyObject>) {
            guarded((), || self.remove_selected());
        }
    }
);

impl ServersSettings {
    pub fn new(mtm: MainThreadMarker) -> Retained<ServersSettings> {
        let this = Self::alloc(mtm).set_ivars(ServersIvars {
            found: RefCell::new(Vec::new()),
            custom: RefCell::new(Vec::new()),
            views: OnceCell::new(),
        });
        // SAFETY: NSObject's initializer, called once on the newly allocated object.
        let this: Retained<ServersSettings> = unsafe { msg_send![super(this), init] };
        this.read();
        this
    }

    /// Reads the user's servers again and looks anew for the built-in ones, which may have been installed meanwhile.
    pub fn reload(&self) {
        self.read();
        if let Some(views) = self.ivars().views.get() {
            views.custom.reloadData();
        }
        self.update_buttons();
        ide_ui_lsp::survey(super::servers_found);
    }

    /// The built-in languages and where their servers are, from a survey that came back.
    pub fn found(&self, lines: Vec<(String, String)>) {
        *self.ivars().found.borrow_mut() = lines;
        if let Some(views) = self.ivars().views.get() {
            views.built_in.reloadData();
        }
    }

    fn read(&self) {
        *self.ivars().custom.borrow_mut() = Settings::load_user().get(&LANGUAGE_SERVERS);
    }

    fn is_custom(&self, table: &NSTableView) -> bool {
        self.ivars()
            .views
            .get()
            .is_some_and(|views| std::ptr::eq(table, &*views.custom))
    }

    /// The rows of the user's servers or of the built-in languages, as their table shows them.
    pub fn rows(&self, custom: bool) -> Vec<Vec<String>> {
        if !custom {
            return self
                .ivars()
                .found
                .borrow()
                .iter()
                .map(|(language, server)| vec![language.clone(), server.clone()])
                .collect();
        }
        self.ivars()
            .custom
            .borrow()
            .iter()
            .map(|server| {
                vec![
                    server.name.clone(),
                    server.extensions.join(", "),
                    server.command.clone(),
                ]
            })
            .collect()
    }

    fn update_buttons(&self) {
        let Some(views) = self.ivars().views.get() else { return };
        views.remove.setEnabled(views.custom.selectedRow() >= 0);
    }

    fn set_problem(&self, problem: Option<&str>) {
        let Some(views) = self.ivars().views.get() else { return };
        views.problem.setStringValue(&ns(problem.unwrap_or("")));
        form::show(&views.problem, problem.is_some());
    }

    fn store(&self, servers: &[CustomServer]) {
        if let Err(error) = Settings::load_user().set(&LANGUAGE_SERVERS, &servers.to_vec()) {
            crate::diag::report("saving the language servers", error);
        }
        self.read();
        if let Some(views) = self.ivars().views.get() {
            views.custom.reloadData();
        }
        self.update_buttons();
        ide_ui_lsp::servers_changed();
    }

    fn remove_selected(&self) {
        let Some(views) = self.ivars().views.get() else { return };
        let Ok(row) = usize::try_from(views.custom.selectedRow()) else {
            return;
        };
        let mut servers = self.ivars().custom.borrow().clone();
        if row < servers.len() {
            servers.remove(row);
            self.store(&servers);
        }
    }

    /// Keeps the server `name` for `extensions`, started by `command`, in place of one of the same name; else says
    /// what is missing.
    fn add(&self, name: &str, extensions: &str, command: &str) {
        let extensions: Vec<String> = extensions
            .split([',', ' '])
            .map(|e| e.trim().trim_start_matches('.').to_string())
            .filter(|e| !e.is_empty())
            .collect();
        let (name, command) = (name.trim(), command.trim());
        if name.is_empty() || extensions.is_empty() || command.is_empty() {
            self.set_problem(Some(t("Name, Dateiendungen und Befehl sind nötig.")));
            return;
        }
        self.set_problem(None);
        let mut servers: Vec<CustomServer> = self
            .ivars()
            .custom
            .borrow()
            .iter()
            .filter(|s| s.name != name)
            .cloned()
            .collect();
        servers.push(CustomServer {
            name: name.to_string(),
            extensions,
            command: command.to_string(),
        });
        self.store(&servers);
    }

    /// The sheet that asks for a server's name, extensions and command.
    fn ask(&self) {
        let Some(window) = self.ivars().views.get().and_then(|v| v.custom.window()) else {
            return;
        };
        let mtm = self.mtm();
        let alert = NSAlert::new(mtm);
        alert.setMessageText(&ns(t("Sprachserver hinzufügen")));
        alert.setInformativeText(&ns(t(
            "Etwa „Elixir“, „ex, exs“ und „elixir-ls“. Der Befehl ist ein Programm auf dem PATH der Login-Shell oder ein Pfad, gefolgt von seinen Argumenten.",
        )));
        alert.addButtonWithTitle(&ns(t("Hinzufügen")));
        alert.addButtonWithTitle(&ns(t("Abbrechen")));
        let field = |placeholder: &str| {
            let field = NSTextField::textFieldWithString(&ns(""), mtm);
            field.setPlaceholderString(Some(&ns(placeholder)));
            field.widthAnchor().constraintEqualToConstant(300.0).setActive(true);
            field
        };
        let (name, extensions, command) = (field(t("Name")), field(t("Dateiendungen")), field(t("Befehl")));
        let accessory = NSStackView::new(mtm);
        accessory.setOrientation(NSUserInterfaceLayoutOrientation::Vertical);
        accessory.setAlignment(NSLayoutAttribute::Leading);
        accessory.setSpacing(ide_ui_common::style::space::M);
        for view in [&name, &extensions, &command] {
            accessory.addArrangedSubview(view);
        }
        accessory.setFrame(NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(300.0, 88.0)));
        alert.setAccessoryView(Some(&accessory));
        alert.window().setInitialFirstResponder(Some(&name));
        let this = self.retain();
        crate::ci_panel::sheet(&alert, &window, move |response| {
            if response == NSAlertFirstButtonReturn {
                this.add(
                    &name.stringValue().to_string(),
                    &extensions.stringValue().to_string(),
                    &command.stringValue().to_string(),
                );
            }
        });
    }

    /// The tab's view.
    pub fn view(&self) -> Retained<NSView> {
        let mtm = self.mtm();
        let (built_in, built_in_scroll) = form::table(
            mtm,
            self,
            &[(t("Sprache"), 110.0), (t("Server"), 400.0)],
            BUILT_IN_HEIGHT,
        );
        let (custom, custom_scroll) = form::table(
            mtm,
            self,
            &[(t("Name"), 110.0), (t("Dateiendungen"), 120.0), (t("Befehl"), 280.0)],
            CUSTOM_HEIGHT,
        );
        let add = list_bar::button(
            mtm,
            true,
            t("Sprachserver hinzufügen"),
            self,
            sel!(ideAddLanguageServer:),
        );
        let remove = list_bar::button(
            mtm,
            false,
            t("Sprachserver entfernen"),
            self,
            sel!(ideRemoveLanguageServer:),
        );
        let problem = form::problem(mtm);
        let views: [Retained<NSView>; 8] = [
            form::heading(mtm, t("Eingebaute Sprachserver")),
            form::note(
                mtm,
                t(
                    "Wo die App den Server jeder Sprache auf diesem Mac findet. Ein Server startet erst, wenn ein Editor eine Datei seiner Sprache in einem vertrauten Ordner öffnet.",
                ),
            ),
            built_in_scroll,
            form::heading(mtm, t("Eigene Sprachserver")),
            form::note(
                mtm,
                t(
                    "Für weitere Sprachen oder statt eines eingebauten Servers: Dateien mit diesen Endungen gehen an den Befehl, der den Server über Standardein- und -ausgabe startet.",
                ),
            ),
            custom_scroll,
            list_bar::bar(mtm, &[&add, &remove], &[]),
            problem.clone().into_super().into_super(),
        ];
        let stack = form::stack(mtm, &views);
        let _ = self.ivars().views.set(Views {
            built_in,
            custom,
            remove,
            problem,
        });
        self.update_buttons();
        form::page(mtm, &stack)
    }
}

/// What the self-test reads from and does with the tab, through the code its controls run.
#[cfg(feature = "selftest")]
pub mod selftest {
    use objc2::DefinedClass;
    use objc2_app_kit::NSModalResponse;

    use super::ServersSettings;
    use crate::util::ns;

    /// The rows the table of the user's servers or of the built-in languages shows, and how many it counts.
    pub fn rows(owner: &ServersSettings, custom: bool) -> (Vec<Vec<String>>, isize) {
        let shown = owner.ivars().views.get().map_or(0, |views| {
            if custom {
                views.custom.numberOfRows()
            } else {
                views.built_in.numberOfRows()
            }
        });
        (owner.rows(custom), shown)
    }

    /// Selects `row` of the user's servers and clicks the remove button.
    pub fn remove(owner: &ServersSettings, row: usize) {
        let Some(views) = owner.ivars().views.get() else { return };
        views
            .custom
            .selectRowIndexes_byExtendingSelection(&objc2_foundation::NSIndexSet::indexSetWithIndex(row), false);
        // SAFETY: `set_action` gave the button a target that answers its action with the sender.
        unsafe { views.remove.performClick(None) };
    }

    /// Asks for a server to add, as the add button does.
    pub fn click_add(owner: &ServersSettings) {
        owner.ask();
    }

    /// Types `values` into the add sheet's fields in their order and ends it with `response`.
    pub fn answer_add(owner: &ServersSettings, values: [&str; 3], response: NSModalResponse) -> bool {
        let Some(window) = owner.ivars().views.get().and_then(|v| v.custom.window()) else {
            return false;
        };
        let Some(sheet) = window.attachedSheet() else {
            return false;
        };
        fn collect(view: &objc2_app_kit::NSView, into: &mut Vec<objc2::rc::Retained<objc2_app_kit::NSTextField>>) {
            for subview in view.subviews().iter() {
                if let Some(field) = subview.downcast_ref::<objc2_app_kit::NSTextField>()
                    && field.isEditable()
                {
                    into.push(objc2::Message::retain(field));
                }
                collect(&subview, into);
            }
        }
        let mut fields = vec![];
        if let Some(content) = sheet.contentView() {
            collect(&content, &mut fields);
        }
        fields.sort_by(|a, b| {
            let y = |f: &objc2_app_kit::NSTextField| f.convertRect_toView(f.bounds(), None).origin.y;
            y(b).total_cmp(&y(a))
        });
        for (field, value) in fields.iter().zip(values) {
            field.setStringValue(&ns(value));
        }
        window.endSheet_returnCode(&sheet, response);
        fields.len() == 3
    }

    /// The problem the tab shows, if any.
    pub fn problem(owner: &ServersSettings) -> Option<String> {
        let views = owner.ivars().views.get()?;
        (!views.problem.isHidden()).then(|| views.problem.stringValue().to_string())
    }
}

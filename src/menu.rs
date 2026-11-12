use ide_core::keymap::Keymap;
use objc2::rc::Retained;
use objc2::runtime::{ProtocolObject, Sel};
use objc2::{MainThreadMarker, MainThreadOnly, sel};
use objc2_app_kit::{NSApplication, NSMenu, NSMenuItem};
use splat_i18n::{t, tf};

use crate::app::AppDelegate;
use crate::util::{item, ns, separator, submenu, tagged};
use crate::window::Panel;

/// The shortcuts from the user's keymap, by command.
struct Keys(Keymap);

impl Keys {
    fn get(&self, command: &str) -> String {
        self.0.key(command).unwrap_or("").to_string()
    }
}

pub fn install(app: &NSApplication, delegate: &AppDelegate, mtm: MainThreadMarker) {
    let (keymap, problems) = Keymap::load(&Keymap::user_path());
    for problem in problems {
        crate::diag::report("keybindings", problem);
    }
    let keys = Keys(keymap);
    let main = NSMenu::initWithTitle(NSMenu::alloc(mtm), &ns(""));
    let (window_item, window_menu) = window_menu(mtm, &keys);
    let (help_item, help_menu) = submenu(mtm, t("Hilfe"), vec![]);
    for entry in [
        app_menu(app, mtm, &keys),
        file_menu(delegate, mtm, &keys),
        edit_menu(mtm, &keys),
        crate::markdown::format_menu(mtm, &|command| keys.get(command)),
        crate::editor::with_view_items(mtm, view_menu(mtm, &keys)),
        go_menu(mtm, &keys),
        crate::editor::predict::with_menu_items(mtm, code_menu(mtm, &keys)),
        crate::git_panel::menu(mtm),
        crate::debug::menu(mtm, &|command| keys.get(command), &delegate.context()),
        window_item,
        help_item,
    ] {
        main.addItem(&entry);
    }
    app.setMainMenu(Some(&main));
    app.setWindowsMenu(Some(&window_menu));
    app.setHelpMenu(Some(&help_menu));
}

fn app_menu(app: &NSApplication, mtm: MainThreadMarker, keys: &Keys) -> Retained<NSMenuItem> {
    let name = ide_product::NAME;
    let (services_item, services) = submenu(mtm, t("Dienste"), vec![]);
    app.setServicesMenu(Some(&services));
    let about = item(
        mtm,
        &tf("Über {}", &[name]),
        Some(sel!(orderFrontStandardAboutPanel:)),
        "",
    );
    let head = std::iter::once(about).chain(crate::updates::menu_item(mtm));
    submenu(
        mtm,
        name,
        head.chain([
            separator(mtm),
            item(
                mtm,
                t("Einstellungen …"),
                Some(sel!(ideShowSettings:)),
                &keys.get("app.settings"),
            ),
            item(
                mtm,
                &tf("Befehl „{}“ installieren …", &[ide_product::COMMAND]),
                Some(sel!(ideInstallCommand:)),
                "",
            ),
            separator(mtm),
            services_item,
            separator(mtm),
            item(mtm, &tf("{} ausblenden", &[name]), Some(sel!(hide:)), "cmd+h"),
            item(
                mtm,
                t("Andere ausblenden"),
                Some(sel!(hideOtherApplications:)),
                "alt+cmd+h",
            ),
            item(mtm, t("Alle einblenden"), Some(sel!(unhideAllApplications:)), ""),
            separator(mtm),
            item(mtm, &tf("{} beenden", &[name]), Some(sel!(terminate:)), "cmd+q"),
        ])
        .collect(),
    )
    .0
}

fn file_menu(delegate: &AppDelegate, mtm: MainThreadMarker, keys: &Keys) -> Retained<NSMenuItem> {
    let (recent_item, recent_menu) = submenu(mtm, t("Zuletzt geöffnet"), vec![]);
    recent_menu.setDelegate(Some(ProtocolObject::from_ref(delegate)));
    let entry = |title: &str, action: Sel, command: &str| item(mtm, title, Some(action), &keys.get(command));
    submenu(
        mtm,
        t("Ablage"),
        vec![
            entry(t("Neue Datei"), sel!(ideNewFile:), "file.new"),
            entry(t("Neues Fenster"), sel!(ideNewWindow:), "window.new"),
            entry(t("Neuer Tab …"), sel!(ideNewTab:), "tab.new"),
            entry(t("Neues Terminal"), sel!(ideNewTerminal:), "terminal.new"),
            entry(t("Ordner öffnen …"), sel!(ideOpenFolder:), "project.open"),
            recent_item,
            entry(t("Schnell öffnen …"), sel!(ideQuickOpen:), "project.quickOpen"),
            separator(mtm),
            entry(t("Sichern"), sel!(ideSave:), "file.save"),
            entry(t("Sichern unter …"), sel!(ideSaveAs:), "file.saveAs"),
            separator(mtm),
        ]
        .into_iter()
        .chain(crate::markdown::file_items(mtm))
        .chain([
            separator(mtm),
            entry(t("Tab schließen"), sel!(ideCloseTab:), "tab.close"),
            entry(t("Fenster schließen"), sel!(ideCloseWindow:), "window.close"),
        ])
        .collect(),
    )
    .0
}

fn edit_menu(mtm: MainThreadMarker, keys: &Keys) -> Retained<NSMenuItem> {
    let items = vec![
        item(mtm, t("Widerrufen"), Some(sel!(undo:)), "cmd+z"),
        item(mtm, t("Wiederholen"), Some(sel!(redo:)), "shift+cmd+z"),
        separator(mtm),
        item(mtm, t("Ausschneiden"), Some(sel!(cut:)), "cmd+x"),
        item(mtm, t("Kopieren"), Some(sel!(copy:)), "cmd+c"),
        item(mtm, t("Einsetzen"), Some(sel!(paste:)), "cmd+v"),
        crate::markdown::paste_plain_item(mtm, &|command| keys.get(command)),
        item(mtm, t("Löschen"), Some(sel!(delete:)), ""),
        item(mtm, t("Alles auswählen"), Some(sel!(selectAll:)), "cmd+a"),
        separator(mtm),
        find_menu(mtm, keys),
        separator(mtm),
        item(
            mtm,
            t("Kommentar ein/aus"),
            Some(sel!(ideToggleComment:)),
            &keys.get("edit.toggleComment"),
        ),
        item(
            mtm,
            t("Gehe zu Zeile …"),
            Some(sel!(ideGoToLine:)),
            &keys.get("edit.goToLine"),
        ),
        item(
            mtm,
            t("Plan kommentieren …"),
            Some(sel!(idePlanComment:)),
            &keys.get("plan.comment"),
        ),
        separator(mtm),
        item(
            mtm,
            t("Terminal leeren"),
            Some(sel!(ideClearTerminal:)),
            &keys.get("terminal.clear"),
        ),
    ];
    #[cfg(feature = "dictation")]
    let items = [
        items,
        vec![
            separator(mtm),
            crate::dictation::menu_item(mtm, &keys.get("dictation.toggle")),
        ],
    ]
    .concat();
    submenu(mtm, t("Bearbeiten"), items).0
}

/// The find commands of text views, which NSTextView's find bar answers, and the search of the window's folder.
fn find_menu(mtm: MainThreadMarker, keys: &Keys) -> Retained<NSMenuItem> {
    let find = |title: &str, tag: isize, key: &str| tagged(mtm, title, sel!(performTextFinderAction:), key, tag);
    submenu(
        mtm,
        t("Suchen"),
        vec![
            find(t("Suchen …"), 1, "cmd+f"),
            find(t("Suchen und ersetzen …"), 12, "alt+cmd+f"),
            item(
                mtm,
                t("Im Projekt suchen …"),
                Some(sel!(ideSearchProject:)),
                &keys.get("project.search"),
            ),
            find(t("Weitersuchen"), 2, "cmd+g"),
            find(t("Rückwärts suchen"), 3, "shift+cmd+g"),
            find(t("Auswahl suchen"), 7, "cmd+e"),
        ],
    )
    .0
}

fn view_menu(mtm: MainThreadMarker, keys: &Keys) -> Retained<NSMenuItem> {
    let panel =
        |panel: Panel, command: &str| tagged(mtm, panel.title(), sel!(ideShowPanel:), &keys.get(command), panel.tag());
    submenu(
        mtm,
        t("Darstellung"),
        vec![
            panel(Panel::Files, "panel.files"),
            panel(Panel::Search, "panel.search"),
            panel(Panel::Agents, "panel.agents"),
            panel(Panel::Git, "panel.git"),
            panel(Panel::Ci, "panel.ci"),
            panel(Panel::Debug, "panel.debug"),
            separator(mtm),
            item(
                mtm,
                t("Claude fokussieren"),
                Some(sel!(ideFocusClaude:)),
                &keys.get("agent.focus"),
            ),
            item(
                mtm,
                t("Subagenten dieser Sitzung"),
                Some(sel!(ideShowSessionSubagents:)),
                "",
            ),
            item(
                mtm,
                t("Hintergrundprozesse dieser Sitzung"),
                Some(sel!(ideShowSessionBackground:)),
                "",
            ),
            separator(mtm),
            item(
                mtm,
                t("Zeilenumbruch"),
                Some(sel!(ideToggleWrap:)),
                &keys.get("view.wrap"),
            ),
            item(
                mtm,
                t("Leerzeichen anzeigen"),
                Some(sel!(ideToggleWhitespace:)),
                &keys.get("view.whitespace"),
            ),
            item(
                mtm,
                t("Vorschau daneben ein-/ausblenden"),
                Some(sel!(ideShowPreview:)),
                &keys.get("view.preview"),
            ),
            item(
                mtm,
                t("Seitenleiste ein-/ausblenden"),
                Some(sel!(toggleSidebar:)),
                &keys.get("sidebar.toggle"),
            ),
            item(
                mtm,
                t("Inspektor ein-/ausblenden"),
                Some(sel!(toggleInspector:)),
                &keys.get("view.inspector"),
            ),
            separator(mtm),
            item(
                mtm,
                t("Adresse öffnen …"),
                Some(sel!(ideBrowserFocusAddress:)),
                &keys.get("browser.focusAddress"),
            ),
            item(
                mtm,
                t("Seite neu laden"),
                Some(sel!(ideBrowserReload:)),
                &keys.get("browser.reload"),
            ),
            item(mtm, t("Zurück"), Some(sel!(ideBrowserBack:)), &keys.get("browser.back")),
            item(
                mtm,
                t("Vorwärts"),
                Some(sel!(ideBrowserForward:)),
                &keys.get("browser.forward"),
            ),
            separator(mtm),
            item(
                mtm,
                t("Symbolleiste ausblenden"),
                Some(sel!(toggleToolbarShown:)),
                "alt+cmd+t",
            ),
            item(mtm, t("Vollbildmodus"), Some(sel!(toggleFullScreen:)), "ctrl+cmd+f"),
        ],
    )
    .0
}

/// Places in the code, which the language server of the editor in front knows.
fn go_menu(mtm: MainThreadMarker, keys: &Keys) -> Retained<NSMenuItem> {
    let entry = |title: &str, action: Sel, command: &str| item(mtm, title, Some(action), &keys.get(command));
    let navigator = crate::navigation::navigator(mtm);
    let [back, forward] = crate::navigation::actions().map(|(title, action, command)| {
        let entry = entry(title, action, command);
        crate::util::set_target(&entry, &navigator);
        entry
    });
    submenu(
        mtm,
        t("Gehe zu"),
        vec![
            back,
            forward,
            separator(mtm),
            entry(t("Gehe zu Definition"), sel!(ideDefinition:), "code.definition"),
            item(mtm, t("Gehe zu Implementierung"), Some(sel!(ideImplementation:)), ""),
            item(mtm, t("Gehe zu Typdefinition"), Some(sel!(ideTypeDefinition:)), ""),
            item(mtm, t("Gehe zu Deklaration"), Some(sel!(ideDeclaration:)), ""),
            entry(t("Verwendungen suchen"), sel!(ideReferences:), "code.references"),
            entry(t("Gehe zu Symbol …"), sel!(ideSymbols:), "code.symbols"),
            entry(
                t("Gehe zu Symbol im Projekt …"),
                sel!(ideProjectSymbols:),
                "project.symbols",
            ),
            separator(mtm),
            entry(t("Aufrufhierarchie"), sel!(ideCallHierarchy:), "code.callHierarchy"),
            item(mtm, t("Typhierarchie"), Some(sel!(ideTypeHierarchy:)), ""),
            separator(mtm),
            entry(t("Nächstes Problem"), sel!(ideNextProblem:), "code.nextProblem"),
            entry(
                t("Vorheriges Problem"),
                sel!(idePreviousProblem:),
                "code.previousProblem",
            ),
        ],
    )
    .0
}

/// What the editor in front does to the code: the language server's commands, folding and further carets.
fn code_menu(mtm: MainThreadMarker, keys: &Keys) -> Retained<NSMenuItem> {
    let entry = |title: &str, action: Sel, command: &str| item(mtm, title, Some(action), &keys.get(command));
    submenu(
        mtm,
        t("Editor"),
        vec![
            entry(t("Vervollständigen"), sel!(ideComplete:), "code.complete"),
            entry(t("Info zum Symbol"), sel!(ideHover:), "code.hover"),
            item(mtm, t("Parameter zeigen"), Some(sel!(ideSignatureHelp:)), ""),
            separator(mtm),
            entry(t("Schnellkorrektur …"), sel!(ideQuickFix:), "code.quickFix"),
            item(mtm, t("Refactoring …"), Some(sel!(ideRefactor:)), ""),
            item(mtm, t("Quellaktionen …"), Some(sel!(ideSourceActions:)), ""),
            separator(mtm),
            entry(t("Umbenennen …"), sel!(ideRename:), "code.rename"),
            entry(t("Dokument formatieren"), sel!(ideFormat:), "code.format"),
            separator(mtm),
            entry(t("Bereich einklappen"), sel!(ideFold:), "edit.fold"),
            entry(t("Bereich ausklappen"), sel!(ideUnfold:), "edit.unfold"),
            entry(t("Alle einklappen"), sel!(ideFoldAll:), "edit.foldAll"),
            entry(t("Alle ausklappen"), sel!(ideUnfoldAll:), "edit.unfoldAll"),
            separator(mtm),
            entry(
                t("Cursor darüber hinzufügen"),
                sel!(ideAddCaretAbove:),
                "edit.addCaretAbove",
            ),
            entry(
                t("Cursor darunter hinzufügen"),
                sel!(ideAddCaretBelow:),
                "edit.addCaretBelow",
            ),
            entry(
                t("Nächstes Vorkommen hinzufügen"),
                sel!(ideAddNextOccurrence:),
                "edit.addNextOccurrence",
            ),
        ],
    )
    .0
}

fn window_menu(mtm: MainThreadMarker, keys: &Keys) -> (Retained<NSMenuItem>, Retained<NSMenu>) {
    submenu(
        mtm,
        t("Fenster"),
        vec![
            item(mtm, t("Im Dock ablegen"), Some(sel!(performMiniaturize:)), "cmd+m"),
            item(mtm, t("Zoomen"), Some(sel!(performZoom:)), ""),
            separator(mtm),
            item(
                mtm,
                t("Nächsten Tab zeigen"),
                Some(sel!(ideNextTab:)),
                &keys.get("tab.next"),
            ),
            item(
                mtm,
                t("Vorherigen Tab zeigen"),
                Some(sel!(idePreviousTab:)),
                &keys.get("tab.previous"),
            ),
            item(
                mtm,
                t("Zum Dokument daneben"),
                Some(sel!(ideFocusNextPanel:)),
                &keys.get("window.nextPanel"),
            ),
            separator(mtm),
            item(mtm, t("Alle nach vorne bringen"), Some(sel!(arrangeInFront:)), ""),
        ]
        .into_iter()
        .chain(tab_stand_ins(mtm))
        .collect(),
    )
}

/// Hidden items with the actions of AppKit's "Show Next Tab" and "Show Previous Tab", so that AppKit adds neither to
/// the Window menu with ⌃Tab and ⌃⇧Tab, which the HIG keeps for moving the focus (E101).
fn tab_stand_ins(mtm: MainThreadMarker) -> [Retained<NSMenuItem>; 2] {
    [sel!(selectNextTab:), sel!(selectPreviousTab:)].map(|action| {
        let entry = item(mtm, "", Some(action), "");
        entry.setHidden(true);
        entry
    })
}

//! The rules Claude stored per project with "Immer erlauben" (E27), in the Agents tab of the settings: read from each
//! project's `.claude` settings files and removable one by one. ACP agents keep theirs where the app cannot reach
//! them, which the section says.

use std::cell::{OnceCell, RefCell};
use std::path::{Path, PathBuf};

use ide_claude::rules::{self, Scope};
use ide_core::settings::{Key, State, remembered};
use objc2::rc::Retained;
use objc2::runtime::{AnyObject, NSObject, ProtocolObject};
use objc2::{DefinedClass, MainThreadMarker, MainThreadOnly, define_class, msg_send, sel};
use objc2_app_kit::{
    NSAccessibility, NSAlert, NSButton, NSColor, NSControlTextEditingDelegate, NSFont, NSLineBreakMode, NSScrollView,
    NSTableColumn, NSTableView, NSTableViewDataSource, NSTableViewDelegate, NSTextField, NSView,
};
use objc2_foundation::{NSNotification, NSObjectProtocol, NSPoint, NSRect, NSSize};
use splat_i18n::t;

use super::list_bar;
use crate::util::{guarded, label, ns};

/// The folders in which the user let Claude store a rule from the app, besides the recent and open ones.
const FOLDERS: Key<Vec<PathBuf>> = Key::user("claudeRuleFolders", Vec::new);

const WIDTH: f64 = 520.0;
const HEIGHT: f64 = 110.0;

/// Notes `folder` as one where Claude stored a rule, so the settings list its rules even once it left the recent
/// folders.
pub fn remember(folder: &Path) {
    let mut state = State::load_user();
    let mut folders = state.get(&FOLDERS);
    if folders.iter().any(|f| f == folder) {
        return;
    }
    folders.push(folder.to_path_buf());
    if let Err(error) = state.set(&FOLDERS, &folders) {
        crate::diag::report("noting a folder with Claude's rules", error);
    }
}

/// The folders whose rules the section lists: those noted, the open and the recent ones, each once.
fn folders() -> Vec<PathBuf> {
    let state = State::load_user();
    let mut folders: Vec<PathBuf> = Vec::new();
    let open = crate::window::open_projects();
    let known = state
        .get(&FOLDERS)
        .into_iter()
        .chain(open)
        .chain(state.get(&remembered::RECENT_PROJECTS));
    for folder in known {
        if !folders.contains(&folder) {
            folders.push(folder);
        }
    }
    folders
}

/// One rule of the list.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Row {
    pub project: PathBuf,
    pub scope: Scope,
    pub rule: String,
}

/// Every allow rule Claude keeps in the listed folders.
fn rows() -> Vec<Row> {
    folders()
        .into_iter()
        .flat_map(|project| {
            rules::allow_rules(&project).into_iter().map(move |stored| Row {
                project: project.clone(),
                scope: stored.scope,
                rule: stored.rule,
            })
        })
        .collect()
}

pub struct StoredRulesIvars {
    rows: RefCell<Vec<Row>>,
    table: OnceCell<Retained<NSTableView>>,
    remove: OnceCell<Retained<NSButton>>,
    /// The section's last line, which the self-test scrolls to.
    #[cfg(feature = "selftest")]
    end: OnceCell<Retained<NSView>>,
}

define_class!(
    // SAFETY: NSObject has no subclassing requirements and `StoredRules` does not implement Drop.
    #[unsafe(super(NSObject))]
    #[thread_kind = MainThreadOnly]
    #[name = "IDEStoredRules"]
    #[ivars = StoredRulesIvars]
    pub struct StoredRules;

    unsafe impl NSObjectProtocol for StoredRules {}

    unsafe impl NSControlTextEditingDelegate for StoredRules {}

    unsafe impl NSTableViewDataSource for StoredRules {
        #[unsafe(method(numberOfRowsInTableView:))]
        fn row_count(&self, _table: &NSTableView) -> isize {
            guarded(0, || isize::try_from(self.ivars().rows.borrow().len()).unwrap_or(0))
        }
    }

    unsafe impl NSTableViewDelegate for StoredRules {
        #[unsafe(method_id(tableView:viewForTableColumn:row:))]
        fn view_for(&self, _table: &NSTableView, column: Option<&NSTableColumn>, row: isize) -> Option<Retained<NSView>> {
            guarded(None, || {
                let index: usize = column?.identifier().to_string().parse().ok()?;
                let text = self.cells().into_iter().nth(usize::try_from(row).ok()?)?.into_iter().nth(index)?;
                let cell = label(self.mtm(), &text);
                cell.setLineBreakMode(NSLineBreakMode::ByTruncatingMiddle);
                cell.setToolTip(Some(&ns(&text)));
                Some(cell.into_super().into_super())
            })
        }

        #[unsafe(method(tableViewSelectionDidChange:))]
        fn selection_changed(&self, _notification: &NSNotification) {
            guarded((), || self.update_button());
        }
    }

    impl StoredRules {
        #[unsafe(method(ideRemoveStoredRule:))]
        fn remove_action(&self, _sender: Option<&AnyObject>) {
            guarded((), || self.remove_selected());
        }
    }
);

impl StoredRules {
    pub fn new(mtm: MainThreadMarker) -> Retained<StoredRules> {
        let this = Self::alloc(mtm).set_ivars(StoredRulesIvars {
            rows: RefCell::new(Vec::new()),
            table: OnceCell::new(),
            remove: OnceCell::new(),
            #[cfg(feature = "selftest")]
            end: OnceCell::new(),
        });
        // SAFETY: NSObject's initializer, called once on the newly allocated object.
        unsafe { msg_send![super(this), init] }
    }

    /// The section: its heading and note, the list with its remove button, and what holds for ACP agents.
    pub fn views(&self) -> Vec<Retained<NSView>> {
        let mtm = self.mtm();
        let heading = label(mtm, t("Gespeicherte Regeln von Claude"));
        heading.setFont(Some(&NSFont::boldSystemFontOfSize(NSFont::systemFontSize())));
        let (table, scroll) = self.table(mtm);
        let remove = list_bar::button(mtm, false, t("Regel entfernen"), self, sel!(ideRemoveStoredRule:));
        let views = vec![
            heading.into_super().into_super(),
            note(
                mtm,
                t(
                    "Was du Claude mit „Immer erlauben“ für ein Projekt freigegeben hast. Zerstörerische Git-Befehle fragen trotzdem jedes Mal.",
                ),
            ),
            scroll,
            list_bar::bar(mtm, &[&remove], &[]),
            note(
                mtm,
                t("ACP-Agenten speichern ihre Regeln selbst, wo die App sie weder sieht noch löschen kann."),
            ),
        ];
        let _ = self.ivars().table.set(table);
        let _ = self.ivars().remove.set(remove);
        #[cfg(feature = "selftest")]
        if let Some(end) = views.last() {
            let _ = self.ivars().end.set(end.clone());
        }
        self.reload();
        views
    }

    /// Reads the rules again from the projects' files and shows them.
    pub fn reload(&self) {
        *self.ivars().rows.borrow_mut() = rows();
        if let Some(table) = self.ivars().table.get() {
            table.reloadData();
        }
        self.update_button();
    }

    /// The rows as the list shows them: project, rule, where it applies.
    pub fn cells(&self) -> Vec<Vec<String>> {
        self.ivars()
            .rows
            .borrow()
            .iter()
            .map(|row| {
                vec![
                    crate::app::display_path(&row.project),
                    row.rule.clone(),
                    crate::agent::scope_name(row.scope.destination()),
                ]
            })
            .collect()
    }

    /// Selects the row `row` and removes its rule, as the user does, for the self-test.
    #[cfg(feature = "selftest")]
    pub fn remove_row(&self, row: usize) {
        if let Some(table) = self.ivars().table.get() {
            table.selectRowIndexes_byExtendingSelection(&objc2_foundation::NSIndexSet::indexSetWithIndex(row), false);
        }
        self.remove_selected();
    }

    /// Scrolls the Agents tab to the section's last line, for the self-test's picture.
    #[cfg(feature = "selftest")]
    pub fn reveal_end(&self) {
        if let Some(end) = self.ivars().end.get() {
            end.scrollRectToVisible(end.bounds());
        }
    }

    fn selected(&self) -> Option<Row> {
        let row = usize::try_from(self.ivars().table.get()?.selectedRow()).ok()?;
        self.ivars().rows.borrow().get(row).cloned()
    }

    fn update_button(&self) {
        if let Some(remove) = self.ivars().remove.get() {
            remove.setEnabled(self.selected().is_some());
        }
    }

    fn remove_selected(&self) {
        let Some(row) = self.selected() else { return };
        if let Err(error) = rules::remove_allow_rule(&row.project, row.scope, &row.rule) {
            let alert = NSAlert::new(self.mtm());
            alert.setMessageText(&ns(t("Die Regel ließ sich nicht entfernen.")));
            alert.setInformativeText(&ns(&format!(
                "{}\n{error}",
                crate::app::display_path(&row.scope.file(&row.project))
            )));
            alert.runModal();
        }
        self.reload();
    }

    fn table(&self, mtm: MainThreadMarker) -> (Retained<NSTableView>, Retained<NSView>) {
        let frame = NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(WIDTH, HEIGHT));
        let table = NSTableView::initWithFrame(NSTableView::alloc(mtm), frame);
        for (n, (title, width)) in [(t("Projekt"), 150.0), (t("Regel"), 150.0), (t("Gilt für"), 190.0)]
            .into_iter()
            .enumerate()
        {
            let column = NSTableColumn::initWithIdentifier(NSTableColumn::alloc(mtm), &ns(&n.to_string()));
            column.setTitle(&ns(title));
            column.setWidth(width);
            table.addTableColumn(&column);
        }
        table.setUsesAlternatingRowBackgroundColors(true);
        table.setAllowsEmptySelection(true);
        table.setAccessibilityLabel(Some(&ns(t("Gespeicherte Regeln von Claude"))));
        // SAFETY: The table holds its data source and delegate weakly; the Agents tab keeps `self` alive.
        unsafe {
            table.setDataSource(Some(ProtocolObject::from_ref(self)));
            table.setDelegate(Some(ProtocolObject::from_ref(self)));
        }
        let scroll = NSScrollView::initWithFrame(NSScrollView::alloc(mtm), frame);
        scroll.setHasVerticalScroller(true);
        scroll.setAutohidesScrollers(true);
        scroll.setBorderType(objc2_app_kit::NSBorderType::BezelBorder);
        scroll.setDocumentView(Some(&table));
        scroll.widthAnchor().constraintEqualToConstant(WIDTH).setActive(true);
        scroll.heightAnchor().constraintEqualToConstant(HEIGHT).setActive(true);
        (table, scroll.into_super())
    }
}

fn note(mtm: MainThreadMarker, text: &str) -> Retained<NSView> {
    let note = NSTextField::wrappingLabelWithString(&ns(text), mtm);
    note.setFont(Some(&NSFont::systemFontOfSize(NSFont::smallSystemFontSize())));
    note.setTextColor(Some(&NSColor::secondaryLabelColor()));
    note.setPreferredMaxLayoutWidth(WIDTH);
    note.widthAnchor().constraintEqualToConstant(WIDTH).setActive(true);
    note.into_super().into_super()
}

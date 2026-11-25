//! The rules Claude stored per project with "Immer erlauben" (E27), in the Agents tab of the settings: read from each
//! project's `.claude` settings files and removed one by one through Claude's own writer (E146). ACP agents keep
//! theirs where the app cannot reach them, which the section says.

use std::cell::{OnceCell, RefCell};
use std::path::{Path, PathBuf};

use ide_claude::rules::{self, Scope};
use ide_core::settings::{State, remembered};
use ide_ui_common::alert::Alert;
use objc2::rc::Retained;
use objc2::runtime::{AnyObject, NSObject};
use objc2::{DefinedClass, MainThreadMarker, MainThreadOnly, define_class, msg_send, sel};
use objc2_app_kit::{
    NSAccessibility, NSButton, NSControlTextEditingDelegate, NSTableColumn, NSTableView, NSTableViewDataSource,
    NSTableViewDelegate, NSView,
};
use objc2_foundation::{NSNotification, NSObjectProtocol};
use splat_i18n::t;

use super::{form, list_bar};
use crate::util::{guarded, ns};
#[cfg(feature = "selftest")]
mod selftest_probe;

/// The folders in which the user let Claude store a rule from the app, besides the recent and open ones.
const FOLDERS: ide_core::settings::StateKey<Vec<PathBuf>> =
    ide_core::settings::StateKey::new("claudeRuleFolders", Vec::new);

const HEIGHT: f64 = 110.0;

/// Notes `folder` as one where Claude stored a rule, so the settings list its rules even once it left the recent
/// folders.
pub(crate) fn remember(folder: &Path) {
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
    once_each(
        state
            .get(&FOLDERS)
            .into_iter()
            .chain(crate::window::open_projects(&crate::context::app()))
            .chain(state.get(&remembered::RECENT_PROJECTS)),
    )
}

fn once_each(folders: impl IntoIterator<Item = PathBuf>) -> Vec<PathBuf> {
    let mut distinct: Vec<PathBuf> = Vec::new();
    for folder in folders {
        if !distinct.contains(&folder) {
            distinct.push(folder);
        }
    }
    distinct
}

/// One rule of the list.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Row {
    pub(crate) project: PathBuf,
    pub(crate) scope: Scope,
    pub(crate) rule: String,
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

/// The texts of `row` as the list shows them: project, rule, where it applies.
fn row_cells(row: &Row) -> [String; 3] {
    [
        crate::app::display_path(&row.project),
        row.rule.clone(),
        ide_ui_agent::scope_name(&row.scope.into()),
    ]
}

pub(crate) struct StoredRulesIvars {
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
    pub(crate) struct StoredRules;

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
                let text = self.cell(usize::try_from(row).ok()?, index)?;
                Some(form::cell(self.mtm(), &text))
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
    pub(crate) fn new(mtm: MainThreadMarker) -> Retained<StoredRules> {
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
    pub(crate) fn views(&self) -> Vec<Retained<NSView>> {
        let mtm = self.mtm();
        let (table, scroll) = form::table(
            mtm,
            self,
            &[(t("Projekt"), 170.0), (t("Regel"), 170.0), (t("Gilt für"), 0.0)],
            HEIGHT,
        );
        table.setAccessibilityLabel(Some(&ns(t("Gespeicherte Regeln von Claude"))));
        let remove = list_bar::button(mtm, false, t("Regel entfernen"), self, sel!(ideRemoveStoredRule:));
        let views = vec![
            form::heading(mtm, t("Gespeicherte Regeln von Claude")),
            form::note(
                mtm,
                t(
                    "Was du Claude mit „Immer erlauben“ für ein Projekt freigegeben hast. Zerstörerische Git-Befehle fragen trotzdem jedes Mal.",
                ),
            ),
            scroll,
            list_bar::bar(mtm, &[&remove], &[]),
            form::note(
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
    pub(crate) fn reload(&self) {
        *self.ivars().rows.borrow_mut() = rows();
        if let Some(table) = self.ivars().table.get() {
            table.reloadData();
        }
        self.update_button();
    }

    /// What column `column` of row `row` shows.
    fn cell(&self, row: usize, column: usize) -> Option<String> {
        let rows = self.ivars().rows.borrow();
        row_cells(rows.get(row)?).into_iter().nth(column)
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
        if let Some(remove) = self.ivars().remove.get() {
            remove.setEnabled(false);
        }
        let this = objc2::Message::retain(self);
        let file = crate::app::display_path(&row.scope.file(&row.project));
        ide_ui_agent::remove_claude_rule(self.mtm(), row.project, row.scope, row.rule, move |removed| {
            if let Err(why) = removed {
                Alert::new(this.mtm(), t("Die Regel ließ sich nicht entfernen."))
                    .informative(&format!("{file}\n{}", why.0))
                    .run_modal();
            }
            this.reload();
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_folder_is_listed_once_where_it_first_appears() {
        let folders = ["/b", "/a", "/b", "/c", "/a"].map(PathBuf::from);
        assert_eq!(once_each(folders), ["/b", "/a", "/c"].map(PathBuf::from));
    }
}

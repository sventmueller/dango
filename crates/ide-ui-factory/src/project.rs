//! The dialog "Leitstand für <Ordner>" (E183), a sheet on the window: what the project sets in `.dango/factory.toml`
//! beside the user's defaults, every value with its origin and "Auf Vorgabe zurücksetzen", which takes the key out of
//! the file with its comments and the rest kept; the role lists read only, the reviewer rule, limits, budgets, copying
//! and setup, risk patterns and the files that count for the review editable; the gate commands and the prompt
//! templates to open in the editor, and the file's errors in the user's words (E141, E190).

use std::cell::{OnceCell, RefCell};

use ide_core::explain::Explained;
use ide_factory::event::PromptKind;
use ide_factory::workflow::{TEMPLATE_DIR, WORKFLOW_FILE};
use ide_ui_common::util::{guarded, label, ns};
use ide_ui_common::window::Files;
use objc2::rc::{Retained, Weak};
use objc2::runtime::{AnyObject, NSObject};
use objc2::{DefinedClass, MainThreadMarker, MainThreadOnly, Message, define_class, msg_send, sel};
use objc2_app_kit::{
    NSBackingStoreType, NSButton, NSColor, NSGridView, NSModalResponse, NSPopUpButton, NSStackView, NSTextField,
    NSView, NSWindow, NSWindowStyleMask,
};
use objc2_foundation::{NSObjectProtocol, NSPoint, NSRect, NSSize};
use splat_i18n::{t, tf};

use crate::actions::Target;
use crate::host::host;
use crate::model::settings::{LIMITS, Origin, ProjectFile, ProjectValue, reviewer_rules};
use crate::ui;
use ide_factory::station::Role;
use ide_ui_common::style::space;

const WIDTH: f64 = 560.0;

/// How wide the fields of lists, patterns and the setup command are.
const WIDE: f64 = 260.0;

/// The templates the dialog opens, with their names.
const TEMPLATES: [PromptKind; 6] = [
    PromptKind::Build,
    PromptKind::Rework,
    PromptKind::Resume,
    PromptKind::Plan,
    PromptKind::Review,
    PromptKind::Handoff,
];

pub(crate) struct ProjectIvars {
    target: Target,
    file: RefCell<ProjectFile>,
    ui: OnceCell<ProjectUi>,
}

struct ProjectUi {
    panel: Retained<NSWindow>,
    problems: Retained<NSTextField>,
    rows: Vec<ValueRow>,
    template: Retained<NSPopUpButton>,
}

/// A value's row: its control, what the control showed of the file, its origin and its reset button.
struct ValueRow {
    value: ProjectValue,
    control: Control,
    shown: RefCell<String>,
    origin: Retained<NSTextField>,
    reset: Retained<NSButton>,
}

/// What shows a value in its row.
enum Control {
    /// A field the user types the value into.
    Field(Retained<NSTextField>),
    /// The value as words, changed only in the file.
    Shown(Retained<NSTextField>),
    /// One of a few choices.
    Choice(Retained<NSPopUpButton>),
}

define_class!(
    // SAFETY: NSObject has no subclassing requirements and `ProjectSheet` does not implement Drop.
    #[unsafe(super(NSObject))]
    #[thread_kind = MainThreadOnly]
    #[name = "IDEFactoryProjectSheet"]
    #[ivars = ProjectIvars]
    pub(crate) struct ProjectSheet;

    unsafe impl NSObjectProtocol for ProjectSheet {}

    impl ProjectSheet {
        #[unsafe(method(ideFactoryProjectReset:))]
        fn reset_action(&self, sender: Option<&AnyObject>) {
            guarded((), || {
                let value = sender
                    .and_then(|s| s.downcast_ref::<NSButton>())
                    .and_then(|b| ProjectValue::of_tag(b.tag()));
                if let Some(value) = value {
                    self.reset(value);
                }
            });
        }

        #[unsafe(method(ideFactoryProjectOpenFile:))]
        fn open_file_action(&self, _sender: Option<&AnyObject>) {
            guarded((), || self.open(WORKFLOW_FILE));
        }

        #[unsafe(method(ideFactoryProjectOpenTemplate:))]
        fn open_template_action(&self, _sender: Option<&AnyObject>) {
            guarded((), || self.open_template());
        }

        #[unsafe(method(ideFactoryProjectCancel:))]
        fn cancel_action(&self, _sender: Option<&AnyObject>) {
            guarded((), || self.close());
        }

        #[unsafe(method(ideFactoryProjectSave:))]
        fn save_action(&self, _sender: Option<&AnyObject>) {
            guarded((), || self.save());
        }
    }
);

/// Shows "Leitstand für <Ordner>" on `target`'s window.
pub(crate) fn show(target: &Target) {
    let window = target.window();
    let Some(parent) = window.window() else { return };
    if parent.attachedSheet().is_some() {
        return;
    }
    let mtm = window.mtm();
    let ivars = ProjectIvars {
        target: target.clone(),
        file: RefCell::new(host().project_file(target.root())),
        ui: OnceCell::new(),
    };
    // SAFETY: NSObject's initializer, called once on the newly allocated object.
    let sheet: Retained<ProjectSheet> = unsafe { msg_send![super(ProjectSheet::alloc(mtm).set_ivars(ivars)), init] };
    let _ = sheet.ivars().ui.set(sheet.build(mtm));
    sheet.show_values();
    if let Some(state) = target.state() {
        let number = state.projects.next_id();
        state.projects.insert(number, Weak::from_retained(&sheet));
    }
    let kept = sheet.clone();
    // The handler holds the sheet until it ends, since the panel holds its controls' target only weakly.
    let done = block2::RcBlock::new(move |_response: NSModalResponse| guarded((), || drop(kept.retain())));
    parent.beginSheet_completionHandler(&sheet.ui().panel, Some(&done));
}

impl ProjectSheet {
    fn build(&self, mtm: MainThreadMarker) -> ProjectUi {
        let folder = self
            .ivars()
            .target
            .root()
            .file_name()
            .map_or_else(String::new, |name| name.to_string_lossy().into_owned());
        let column = ui::column(mtm, space::M);
        column.setEdgeInsets(objc2_foundation::NSEdgeInsets {
            top: space::GROUP,
            left: space::GROUP,
            bottom: space::GROUP,
            right: space::GROUP,
        });
        column.addArrangedSubview(&ui::heading(mtm, &tf("Leitstand für {}", &[&folder])));
        for note in self.ivars().file.borrow().notes() {
            column.addArrangedSubview(&ui::note(mtm, note, WIDTH));
        }
        let problems = ui::note(mtm, "", WIDTH);
        problems.setTextColor(Some(&NSColor::systemRedColor()));
        column.addArrangedSubview(&problems);
        let mut rows = Vec::new();
        for (heading, note, values) in sections() {
            column.addArrangedSubview(&ui::heading(mtm, heading));
            if let Some(note) = note {
                column.addArrangedSubview(&ui::note(mtm, note, WIDTH));
            }
            let grid = NSGridView::new(mtm);
            grid.setRowSpacing(space::S);
            grid.setColumnSpacing(space::M);
            rows.extend(values.into_iter().map(|value| self.value_row(mtm, &grid, value)));
            column.addArrangedSubview(&grid);
        }
        let template = self.files_part(mtm, &column);
        let panel = panel(mtm, &folder);
        panel.setContentView(Some(&ui::scrolling(mtm, &column)));
        ProjectUi {
            panel,
            problems,
            rows,
            template,
        }
    }

    /// The gate commands, the workflow file and the templates to open, and the dialog's buttons; the templates'
    /// menu.
    fn files_part(&self, mtm: MainThreadMarker, column: &NSStackView) -> Retained<NSPopUpButton> {
        let me: &AnyObject = self.as_ref();
        column.addArrangedSubview(&ui::heading(mtm, t("Prüfbefehle")));
        let gates = self.ivars().file.borrow().gates();
        if gates.is_empty() {
            column.addArrangedSubview(&ui::note(
                mtm,
                &tf("Keine; {} erkennt das Testwerkzeug des Projekts.", &[ide_product::NAME]),
                WIDTH,
            ));
        }
        for gate in gates {
            column.addArrangedSubview(&ui::code(mtm, &gate, WIDTH));
        }
        let open = ui::button(
            mtm,
            t("Workflow-Datei im Editor öffnen"),
            me,
            sel!(ideFactoryProjectOpenFile:),
            0,
        );
        column.addArrangedSubview(&open);
        let template = NSPopUpButton::new(mtm);
        for kind in TEMPLATES {
            template.addItemWithTitle(&ns(ide_factory::prompt::template_file(kind)));
        }
        ide_ui_common::util::set_label(&template, Some(t("Prompt-Vorlage")));
        let open_template = ui::button(mtm, t("Im Editor öffnen"), me, sel!(ideFactoryProjectOpenTemplate:), 0);
        let line = ui::row(mtm, space::M);
        line.addArrangedSubview(&label(mtm, t("Prompt-Vorlage:")));
        line.addArrangedSubview(&template);
        line.addArrangedSubview(&open_template);
        column.addArrangedSubview(&line);
        let buttons = ui::row(mtm, space::M);
        let cancel = ui::button(mtm, t("Abbrechen"), me, sel!(ideFactoryProjectCancel:), 0);
        cancel.setKeyEquivalent(&ns("\u{1b}"));
        let save = ui::button(mtm, t("Sichern"), me, sel!(ideFactoryProjectSave:), 0);
        save.setKeyEquivalent(&ns("\r"));
        buttons.addArrangedSubview(&cancel);
        buttons.addArrangedSubview(&save);
        column.addArrangedSubview(&buttons);
        template
    }

    /// A row of `grid`: the value's title, its control, its origin and "Auf Vorgabe zurücksetzen".
    fn value_row(&self, mtm: MainThreadMarker, grid: &NSGridView, value: ProjectValue) -> ValueRow {
        let title = value.title();
        let caption = label(mtm, &title);
        let control = match value {
            ProjectValue::Role(_) => {
                let shown = ui::text(mtm, "", WIDE);
                ide_ui_common::util::set_title_element(&shown, &caption);
                Control::Shown(shown)
            }
            ProjectValue::ReviewerRule => {
                let choice = NSPopUpButton::new(mtm);
                for words in reviewer_rules() {
                    choice.addItemWithTitle(&ns(words));
                }
                ide_ui_common::util::set_title_element(&choice, &caption);
                Control::Choice(choice)
            }
            _ => {
                let field = NSTextField::textFieldWithString(&ns(""), mtm);
                let width = if value.is_list() || value == ProjectValue::Setup {
                    WIDE
                } else {
                    80.0
                };
                field.widthAnchor().constraintEqualToConstant(width).setActive(true);
                ide_ui_common::util::set_title_element(&field, &caption);
                Control::Field(field)
            }
        };
        let origin = ui::note(mtm, "", 100.0);
        let reset = ui::button(
            mtm,
            t("Auf Vorgabe zurücksetzen"),
            self.as_ref(),
            sel!(ideFactoryProjectReset:),
            value.tag(),
        );
        reset.setControlSize(objc2_app_kit::NSControlSize::Small);
        ide_ui_common::util::set_label(
            &reset,
            Some(&tf("{} auf Vorgabe zurücksetzen", &[title.trim_end_matches(':')])),
        );
        let shown: &NSView = match &control {
            Control::Field(field) | Control::Shown(field) => field,
            Control::Choice(choice) => choice,
        };
        let views: [&NSView; 4] = [&caption, shown, &origin, &reset];
        grid.addRowWithViews(&objc2_foundation::NSArray::from_slice(&views));
        ValueRow {
            value,
            control,
            shown: RefCell::new(String::new()),
            origin,
            reset,
        }
    }

    fn ui(&self) -> &ProjectUi {
        self.ivars().ui.get().expect("the sheet builds its views in `show`")
    }

    /// Shows every value of the file as edited, with its origin, and the file's errors.
    fn show_values(&self) {
        let file = self.ivars().file.borrow();
        let ui = self.ui();
        for row in &ui.rows {
            let shown = match &row.control {
                Control::Field(field) | Control::Shown(field) => {
                    let text = file.field_text(row.value);
                    field.setStringValue(&ns(&text));
                    field.setPlaceholderString(file.placeholder(row.value).as_deref().map(ns).as_deref());
                    text
                }
                Control::Choice(choice) => {
                    choice.selectItemAtIndex(isize::from(file.reviewer_preferred().0));
                    choice.indexOfSelectedItem().to_string()
                }
            };
            *row.shown.borrow_mut() = shown;
            let origin = file.origin(row.value);
            row.origin.setStringValue(&ns(origin.words()));
            row.reset.setEnabled(origin == Origin::File);
        }
        let errors = file.errors();
        ui.problems.setStringValue(&ns(&errors.join("\n")));
        ui.problems.setHidden(errors.is_empty());
    }

    fn reset(&self, value: ProjectValue) {
        let result = self.ivars().file.borrow_mut().reset(value);
        self.show_values();
        if let Err(error) = result {
            self.problem(&error.0);
        }
    }

    fn problem(&self, words: &str) {
        let ui = self.ui();
        ui.problems.setStringValue(&ns(words));
        ui.problems.setHidden(words.is_empty());
    }

    /// Takes what the user changed in the controls into the file; the first problem, in the user's words.
    fn take_fields(&self) -> Result<(), Explained> {
        let mut file = self.ivars().file.borrow_mut();
        for row in &self.ui().rows {
            let shown = row.shown.borrow();
            match &row.control {
                Control::Shown(_) => {}
                Control::Choice(choice) => {
                    let index = choice.indexOfSelectedItem();
                    if index.to_string() != *shown {
                        file.set_reviewer_preferred(index == 1)?;
                    }
                }
                Control::Field(field) => file.take(row.value, &shown, &field.stringValue().to_string())?,
            }
        }
        Ok(())
    }

    fn save(&self) {
        let before = self.ivars().file.borrow().errors();
        if let Err(error) = self.take_fields() {
            self.problem(&error.0);
            return;
        }
        let after = self.ivars().file.borrow().errors();
        if !after.is_empty() && after != before {
            self.problem(&after.join("\n"));
            return;
        }
        let text = self.ivars().file.borrow().text().to_string();
        let target = self.ivars().target.clone();
        match host().save_project_file(target.root(), &text) {
            Ok(()) => {
                self.close();
                target.refresh();
            }
            Err(error) => self.problem(&error.0),
        }
    }

    fn open(&self, relative: &str) {
        let target = self.ivars().target.clone();
        let path = target.root().join(relative);
        self.close();
        target.window().open_file(&path);
    }

    /// Opens the chosen template in the editor, written out first from the built-in one if the project has none.
    fn open_template(&self) {
        let index = usize::try_from(self.ui().template.indexOfSelectedItem()).unwrap_or(0);
        let kind = TEMPLATES.get(index).copied().unwrap_or(PromptKind::Build);
        let relative = format!("{TEMPLATE_DIR}/{}", ide_factory::prompt::template_file(kind));
        let path = self.ivars().target.root().join(&relative);
        if !path.exists() {
            let written = path
                .parent()
                .map_or(Ok(()), std::fs::create_dir_all)
                .and_then(|()| std::fs::write(&path, ide_factory::prompt::default_template(kind)));
            if let Err(error) = written {
                self.problem(&tf("Die Vorlage ließ sich nicht anlegen: {}", &[&error.to_string()]));
                return;
            }
        }
        self.open(&relative);
    }

    fn close(&self) {
        let panel = &self.ui().panel;
        if let Some(parent) = panel.sheetParent() {
            parent.endSheet(panel);
        }
        panel.orderOut(None);
    }
}

/// The dialog's sections: heading, note and the values in them.
fn sections() -> Vec<(&'static str, Option<&'static str>, Vec<ProjectValue>)> {
    let mut roles: Vec<ProjectValue> = Role::ALL.into_iter().map(ProjectValue::Role).collect();
    roles.push(ProjectValue::ReviewerRule);
    let mut limits: Vec<ProjectValue> = LIMITS.into_iter().map(ProjectValue::Limit).collect();
    limits.extend([ProjectValue::OrderBudget, ProjectValue::DayBudget]);
    vec![
        (
            t("Rollen"),
            Some(t(
                "Die Ranglisten stehen in der Workflow-Datei; „Anderer Hersteller bevorzugt“ gilt erst nach deiner Bestätigung.",
            )),
            roles,
        ),
        (
            t("Grenzen und Budget"),
            Some(t(
                "Ein leeres Budget lässt die Vorgabe gelten. Ein Projekt kann Grenzen und Budgets nur verschärfen; ohne Grenze ist ein Budget hier nur, wenn auch die Vorgabe keine setzt.",
            )),
            limits,
        ),
        (t("Worktree"), None, vec![ProjectValue::Copy, ProjectValue::Setup]),
        (
            t("Risikoklassen"),
            Some(t(
                "Pfadmuster, eins je Zeile oder durch Kommas getrennt. Die gemessene Klasse kann die erwartete nur anheben.",
            )),
            vec![ProjectValue::RiskHigh, ProjectValue::RiskMedium],
        ),
        (
            t("Prüfrelevante Dateien"),
            Some(t(
                "Änderungen an diesen Pfaden heben die gemessene Risikoklasse auf hoch; die Vorgaben je Testwerkzeug gelten immer.",
            )),
            vec![ProjectValue::Tests, ProjectValue::Config],
        ),
    ]
}

fn panel(mtm: MainThreadMarker, folder: &str) -> Retained<NSWindow> {
    let frame = NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(WIDTH + 2.0 * space::GROUP, 640.0));
    // SAFETY: NSWindow's designated initializer on a newly allocated window.
    let panel = unsafe {
        NSWindow::initWithContentRect_styleMask_backing_defer(
            NSWindow::alloc(mtm),
            frame,
            NSWindowStyleMask::Titled | NSWindowStyleMask::Resizable,
            NSBackingStoreType::Buffered,
            false,
        )
    };
    // SAFETY: Switching it off keeps AppKit from releasing the window behind Rust's back.
    unsafe { panel.setReleasedWhenClosed(false) };
    panel.setTitle(&ns(&tf("Leitstand für {}", &[folder])));
    panel
}

#[cfg(feature = "selftest")]
mod selftest_probe;

//! The dialog "Software Factory für <Ordner>" (E183), a sheet on the window: what the project sets in `.dango/factory.toml`
//! beside the user's defaults. A field left empty takes the default, which it shows grey; a value the project sets
//! has a bold title, and emptying the field takes its key out of the file with its comments and the rest kept. The
//! roles' ranked lists with model and effort, the reviewer rule, limits, budgets, copying and setup, risk patterns
//! and the files that count for the review are editable; the gate commands and the prompt templates open in the
//! editor, and the file's errors show in the user's words in the fixed head (E141, E190).

use std::cell::{OnceCell, Ref, RefCell};

use ide_core::explain::Explained;
use ide_factory::event::PromptKind;
use ide_factory::workflow::{TEMPLATE_DIR, WORKFLOW_FILE};
use ide_ui_common::util::{guarded, ns, set_action};
use ide_ui_common::window::Files;
use objc2::rc::{Retained, Weak};
use objc2::runtime::{AnyObject, NSObject};
use objc2::{DefinedClass, MainThreadMarker, MainThreadOnly, Message, define_class, msg_send, sel};
use objc2_app_kit::{
    NSBackingStoreType, NSBox, NSBoxType, NSColor, NSFont, NSLayoutConstraintOrientation, NSMenuItem, NSModalResponse,
    NSPopUpButton, NSStackView, NSTextField, NSUserInterfaceLayoutOrientation, NSView, NSWindow, NSWindowStyleMask,
};
use objc2_foundation::{NSEdgeInsets, NSObjectProtocol, NSPoint, NSRect, NSSize};
use splat_i18n::{t, tf};

use crate::actions::Target;
use crate::host::host;
use crate::model::settings::{Origin, ProjectFile, ProjectValue, reviewer_rules};
use crate::ui;
use form::{CONTENT, Form};
use ide_factory::station::Role;
use ide_ui_common::style::space;

/// How wide the fields of lists, patterns and the setup command are.
const WIDE: f64 = 360.0;

/// How wide the fields of limits and budgets are.
const NUMBER: f64 = 100.0;

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
    roles: Retained<roles::RolesEditor>,
    problems: Retained<NSTextField>,
    rows: Vec<ValueRow>,
    template: Retained<NSPopUpButton>,
}

/// A value's row: its control, what the control showed of the file, and its title, bold while the project sets it.
struct ValueRow {
    value: ProjectValue,
    control: Control,
    shown: RefCell<String>,
    caption: Retained<NSTextField>,
}

/// What shows a value in its row.
enum Control {
    /// A field the user types the value into.
    Field(Retained<NSTextField>),
    /// One of a few choices, the default first.
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
        #[unsafe(method(ideFactoryProjectField:))]
        fn field_action(&self, _sender: Option<&AnyObject>) {
            guarded((), || self.field_done());
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

/// Shows "Software Factory für <Ordner>" on `target`'s window.
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
        let (header, problems) = self.header(mtm, &folder);
        let form = Form::new(mtm);
        let roles = roles::RolesEditor::new(mtm, self);
        let mut rows = Vec::new();
        form.heading(t("Rollen"), true);
        roles.add_to(&form);
        rows.push(self.value_row(mtm, &form, ProjectValue::ReviewerRule));
        form.group();
        form.heading(t("Grenzen und Budget"), false);
        form.note(t(
            "Ein leeres Budget lässt die Vorgabe gelten. Ein Projekt kann Grenzen und Budgets nur verschärfen; ohne Grenze ist ein Budget hier nur, wenn auch die Vorgabe keine setzt.",
        ));
        for (index, group) in limit_groups().into_iter().enumerate() {
            for (n, value) in group.into_iter().enumerate() {
                rows.push(self.value_row(mtm, &form, value));
                if n == 0 && index > 0 {
                    form.group();
                }
            }
        }
        for (heading, note, values) in path_sections() {
            form.heading(heading, false);
            if let Some(note) = note {
                form.note(note);
            }
            rows.extend(values.into_iter().map(|value| self.value_row(mtm, &form, value)));
        }
        let template = self.files_part(mtm, &form);
        let body = ui::column(mtm, space::M);
        body.setEdgeInsets(insets(space::M, space::GROUP));
        body.addArrangedSubview(&form.grid);
        let panel = panel(mtm, &folder);
        panel.setContentView(Some(&self.layout(mtm, &header, &body)));
        ProjectUi {
            panel,
            roles,
            problems,
            rows,
            template,
        }
    }

    /// The fixed head: the dialog's title, what to know about the file, how fields and defaults work, and the
    /// problems; the problems' label.
    fn header(&self, mtm: MainThreadMarker, folder: &str) -> (Retained<NSStackView>, Retained<NSTextField>) {
        let header = ui::column(mtm, space::S);
        header.setEdgeInsets(insets(space::GROUP, space::GROUP));
        header.addArrangedSubview(&ui::title(mtm, &tf("Software Factory für {}", &[folder])));
        for note in self.ivars().file.borrow().notes() {
            header.addArrangedSubview(&ui::note(mtm, note, CONTENT));
        }
        header.addArrangedSubview(&ui::note(
            mtm,
            t("Ein leeres Feld übernimmt deine Vorgabe, die grau darin steht; fett ist, was das Projekt selbst setzt."),
            CONTENT,
        ));
        let problems = ui::note(mtm, "", CONTENT);
        problems.setTextColor(Some(&NSColor::systemRedColor()));
        problems.setHidden(true);
        header.addArrangedSubview(&problems);
        (header, problems)
    }

    /// The sheet's content: the fixed head, the sections that scroll, a line, and the fixed buttons.
    fn layout(&self, mtm: MainThreadMarker, header: &NSStackView, body: &NSStackView) -> Retained<NSView> {
        let me: &AnyObject = self.as_ref();
        let scroll = ui::scrolling(mtm, body);
        scroll.setContentHuggingPriority_forOrientation(1.0, NSLayoutConstraintOrientation::Vertical);
        let open = ui::button(
            mtm,
            t("Workflow-Datei im Editor öffnen"),
            me,
            sel!(ideFactoryProjectOpenFile:),
            0,
        );
        let cancel = ui::button(mtm, t("Abbrechen"), me, sel!(ideFactoryProjectCancel:), 0);
        cancel.setKeyEquivalent(&ns("\u{1b}"));
        let save = ui::button(mtm, t("Sichern"), me, sel!(ideFactoryProjectSave:), 0);
        save.setKeyEquivalent(&ns("\r"));
        let footer = ui::row(mtm, space::M);
        footer.setEdgeInsets(insets(space::M, space::GROUP));
        footer.addArrangedSubview(&open);
        let spacer = NSView::new(mtm);
        spacer.setContentHuggingPriority_forOrientation(1.0, NSLayoutConstraintOrientation::Horizontal);
        footer.addArrangedSubview(&spacer);
        footer.addArrangedSubview(&cancel);
        footer.addArrangedSubview(&save);
        let line = NSBox::new(mtm);
        line.setBoxType(NSBoxType::Separator);
        let content = NSStackView::new(mtm);
        content.setOrientation(NSUserInterfaceLayoutOrientation::Vertical);
        content.setSpacing(space::NONE);
        let parts: [&NSView; 4] = [header, &scroll, &line, &footer];
        for view in parts {
            content.addArrangedSubview(view);
            view.widthAnchor()
                .constraintEqualToAnchor(&content.widthAnchor())
                .setActive(true);
        }
        Retained::into_super(content)
    }

    /// The gate commands, read only, and the prompt templates to open; the templates' menu.
    fn files_part(&self, mtm: MainThreadMarker, form: &Form) -> Retained<NSPopUpButton> {
        let me: &AnyObject = self.as_ref();
        form.heading(t("Prüfbefehle"), false);
        let gates = self.ivars().file.borrow().gates();
        if gates.is_empty() {
            form.note(&tf(
                "Keine; {} erkennt das Testwerkzeug des Projekts.",
                &[ide_product::NAME],
            ));
        }
        for gate in gates {
            form.wide(&ui::code(mtm, &gate, CONTENT));
        }
        let template = NSPopUpButton::new(mtm);
        for kind in TEMPLATES {
            template.addItemWithTitle(&ns(ide_factory::prompt::template_file(kind)));
        }
        let open_template = ui::button(mtm, t("Im Editor öffnen"), me, sel!(ideFactoryProjectOpenTemplate:), 0);
        let line = ui::row(mtm, space::S);
        line.addArrangedSubview(&template);
        line.addArrangedSubview(&open_template);
        let title = form.row(t("Prompt-Vorlage:"), &line);
        form.group();
        ide_ui_common::util::set_title_element(&template, &title);
        template
    }

    /// The row of `value` in `form`: its title and its control, a field whose placeholder is the default or a menu
    /// whose first item is.
    fn value_row(&self, mtm: MainThreadMarker, form: &Form, value: ProjectValue) -> ValueRow {
        let me: &AnyObject = self.as_ref();
        let control = if value == ProjectValue::ReviewerRule {
            let choice = NSPopUpButton::new(mtm);
            choice.addItemWithTitle(&ns(t("Vorgabe")));
            if let Some(menu) = choice.menu() {
                menu.addItem(&NSMenuItem::separatorItem(mtm));
            }
            for words in reviewer_rules() {
                choice.addItemWithTitle(&ns(words));
            }
            set_action(&choice, me, sel!(ideFactoryProjectField:));
            Control::Choice(choice)
        } else {
            let field = NSTextField::textFieldWithString(&ns(""), mtm);
            let wide = value.is_list() || value == ProjectValue::Setup;
            if wide {
                field.setFont(Some(&ide_ui_common::style::mono(ide_ui_common::style::CODE)));
            } else {
                field.setAlignment(objc2_app_kit::NSTextAlignment::Right);
            }
            field
                .widthAnchor()
                .constraintEqualToConstant(if wide { WIDE } else { NUMBER })
                .setActive(true);
            set_action(&field, me, sel!(ideFactoryProjectField:));
            Control::Field(field)
        };
        let shown: &NSView = match &control {
            Control::Field(field) => field,
            Control::Choice(choice) => choice,
        };
        let caption = form.row(&value.title(), shown);
        match &control {
            Control::Field(field) => ide_ui_common::util::set_title_element(field, &caption),
            Control::Choice(choice) => ide_ui_common::util::set_title_element(choice, &caption),
        }
        ValueRow {
            value,
            control,
            shown: RefCell::new(String::new()),
            caption,
        }
    }

    fn ui(&self) -> &ProjectUi {
        self.ivars().ui.get().expect("the sheet builds its views in `show`")
    }

    /// Shows every value of the file as edited, the titles of those the project sets in bold, and the file's errors.
    fn show_values(&self) {
        let file = self.ivars().file.borrow();
        let ui = self.ui();
        for row in &ui.rows {
            let shown = match &row.control {
                Control::Field(field) => {
                    let text = file.field_text(row.value);
                    field.setStringValue(&ns(&text));
                    field.setPlaceholderString(file.placeholder(row.value).as_deref().map(ns).as_deref());
                    text
                }
                Control::Choice(choice) => {
                    let default = reviewer_rules()[usize::from(file.default_reviewer_preferred())];
                    if let Some(first) = choice.itemAtIndex(0) {
                        first.setTitle(&ns(&tf("Vorgabe ({})", &[default])));
                    }
                    let index = match file.reviewer_preferred() {
                        (_, Origin::Default) => 0,
                        (preferred, Origin::File) => 2 + isize::from(preferred),
                    };
                    choice.selectItemAtIndex(index);
                    index.to_string()
                }
            };
            *row.shown.borrow_mut() = shown;
            let size = NSFont::systemFontSize();
            let font = match file.origin(row.value) {
                Origin::File => NSFont::boldSystemFontOfSize(size),
                Origin::Default => NSFont::systemFontOfSize(size),
            };
            row.caption.setFont(Some(&font));
        }
        let errors = file.errors();
        ui.problems.setStringValue(&ns(&errors.join("\n")));
        ui.problems.setHidden(errors.is_empty());
    }

    /// Takes what the user typed or chose into the file and shows the rows again; a value it cannot take stays as
    /// typed with the reason.
    fn field_done(&self) {
        match self.take_fields() {
            Ok(()) => self.show_values(),
            Err(error) => self.problem(&error.0),
        }
    }

    /// The workflow file as edited.
    pub(crate) fn file(&self) -> Ref<'_, ProjectFile> {
        self.ivars().file.borrow()
    }

    /// Writes `list` as `role`'s ranked list into the file as edited, and shows it in the rows.
    pub(crate) fn set_ranks(&self, role: Role, list: &[ide_factory::routing::RankEntry]) {
        let result = self.ivars().file.borrow_mut().set_ranks(role, list);
        self.show_values();
        if let Err(error) = result {
            self.problem(&error.0);
        }
    }

    /// Takes `value` out of the file, so the default applies again, and shows it.
    pub(crate) fn reset(&self, value: ProjectValue) {
        let result = self.ivars().file.borrow_mut().reset(value);
        self.show_values();
        if let Some(roles) = self.ivars().ui.get().map(|ui| &ui.roles) {
            roles.reload();
        }
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
                Control::Choice(choice) => {
                    let index = choice.indexOfSelectedItem();
                    if index.to_string() != *shown {
                        match index {
                            2 | 3 => file.set_reviewer_preferred(index == 3)?,
                            _ => file.reset(row.value)?,
                        }
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
                .and_then(|()| write_new(&path, ide_factory::prompt::default_template(kind)));
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

/// The limits and budgets in their groups: how many run at once, how long things take, how often they are tried, and
/// what they may cost.
fn limit_groups() -> Vec<Vec<ProjectValue>> {
    use ide_factory::limits::Limit;
    let limits = |list: &[Limit]| list.iter().map(|l| ProjectValue::Limit(*l)).collect::<Vec<_>>();
    vec![
        limits(&[Limit::ParallelOrders, Limit::WindowStartPercent]),
        limits(&[
            Limit::TurnMinutes,
            Limit::StallMinutes,
            Limit::ToolCallMinutes,
            Limit::GateMinutes,
            Limit::SetupMinutes,
        ]),
        limits(&[Limit::TurnsPerStation, Limit::Attempts, Limit::ReviewRounds]),
        vec![ProjectValue::OrderBudget, ProjectValue::DayBudget],
    ]
}

/// The sections of paths and patterns: heading, note and the values in them.
fn path_sections() -> Vec<(&'static str, Option<&'static str>, Vec<ProjectValue>)> {
    vec![
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

/// Writes `text` as a new file at `path`; a file or a link already there, even one that leads nowhere, is left alone,
/// so a link the project put in place of a template never makes the app write where it leads.
fn write_new(path: &std::path::Path, text: &str) -> std::io::Result<()> {
    let mut file = std::fs::OpenOptions::new().write(true).create_new(true).open(path)?;
    std::io::Write::write_all(&mut file, text.as_bytes())
}

/// Room of `vertical` above and below and `horizontal` at the sides.
fn insets(vertical: f64, horizontal: f64) -> NSEdgeInsets {
    NSEdgeInsets {
        top: vertical,
        left: horizontal,
        bottom: vertical,
        right: horizontal,
    }
}

fn panel(mtm: MainThreadMarker, folder: &str) -> Retained<NSWindow> {
    let frame = NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(CONTENT + 2.0 * space::GROUP, 640.0));
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
    panel.setTitle(&ns(&tf("Software Factory für {}", &[folder])));
    panel
}

mod form;
pub(crate) mod roles;
#[cfg(feature = "selftest")]
mod selftest_probe;

#[cfg(test)]
mod tests {
    use super::write_new;

    #[test]
    fn a_template_is_written_only_where_nothing_stands() {
        let dir = std::env::temp_dir().join(format!("factory-template-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("build.md");
        write_new(&file, "Build {{title}}").unwrap();
        assert_eq!(std::fs::read_to_string(&file).unwrap(), "Build {{title}}");
        assert!(write_new(&file, "other").is_err());
        assert_eq!(std::fs::read_to_string(&file).unwrap(), "Build {{title}}");
        let elsewhere = dir.join("elsewhere.md");
        let link = dir.join("review.md");
        std::os::unix::fs::symlink(&elsewhere, &link).unwrap();
        assert!(write_new(&link, "Review").is_err());
        assert!(!elsewhere.exists(), "nothing is written where a dangling link leads");
        let _ = std::fs::remove_dir_all(&dir);
    }
}

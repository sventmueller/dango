//! The dialog "Leitstand für <Ordner>" (E183), a sheet on the window: what the project sets in `.dango/factory.toml`
//! beside the user's defaults, each value with its origin and a way back to the default, the gate commands and the
//! prompt templates to open in the editor, and the file's errors in the user's words (E141, E190).

use std::cell::{OnceCell, RefCell};

use ide_core::explain::Explained;
use ide_factory::event::PromptKind;
use ide_factory::workflow::{TEMPLATE_DIR, WORKFLOW_FILE};
use ide_ui_common::util::{guarded, label, ns};
use ide_ui_common::window::Files;
use objc2::rc::Retained;
use objc2::runtime::{AnyObject, NSObject};
use objc2::{DefinedClass, MainThreadMarker, MainThreadOnly, Message, define_class, msg_send, sel};
use objc2_app_kit::{
    NSBackingStoreType, NSButton, NSColor, NSGridView, NSModalResponse, NSPopUpButton, NSTextField, NSView, NSWindow,
    NSWindowStyleMask,
};
use objc2_foundation::{NSObjectProtocol, NSPoint, NSRect, NSSize};
use splat_i18n::{t, tf};

use crate::actions::Target;
use crate::host::host;
use crate::model::settings::{LIMITS, Origin, ProjectFile, limit_title, read_dollars};
use crate::ui;
use ide_ui_common::style::space;

const WIDTH: f64 = 520.0;

/// The templates the dialog opens, with their names.
const TEMPLATES: [PromptKind; 6] = [
    PromptKind::Build,
    PromptKind::Rework,
    PromptKind::Resume,
    PromptKind::Plan,
    PromptKind::Review,
    PromptKind::Handoff,
];

/// The tag of the order budget's reset button; the limits' are their place in `LIMITS`.
const BUDGET_TAG: isize = 100;

pub(crate) struct ProjectIvars {
    target: Target,
    file: RefCell<ProjectFile>,
    ui: OnceCell<ProjectUi>,
}

struct ProjectUi {
    panel: Retained<NSWindow>,
    problems: Retained<NSTextField>,
    limits: Vec<(Retained<NSTextField>, Retained<NSTextField>)>,
    budget: (Retained<NSTextField>, Retained<NSTextField>),
    copy: Retained<NSTextField>,
    setup: Retained<NSTextField>,
    template: Retained<NSPopUpButton>,
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
                if let Some(button) = sender.and_then(|s| s.downcast_ref::<NSButton>()) {
                    self.reset(button.tag());
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
    let kept = sheet.clone();
    // The handler holds the sheet until it ends, since the panel holds its controls' target only weakly.
    let done = block2::RcBlock::new(move |_response: NSModalResponse| guarded((), || drop(kept.retain())));
    parent.beginSheet_completionHandler(&sheet.ui().panel, Some(&done));
}

impl ProjectSheet {
    fn build(&self, mtm: MainThreadMarker) -> ProjectUi {
        let me: &AnyObject = self.as_ref();
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
        let (builders, origin) = self.ivars().file.borrow().builders();
        column.addArrangedSubview(&ui::heading(mtm, t("Builder")));
        column.addArrangedSubview(&ui::text(
            mtm,
            &tf("{} ({})", &[&builders.join(", "), origin.words()]),
            WIDTH,
        ));
        column.addArrangedSubview(&ui::heading(mtm, t("Grenzen und Budget")));
        let grid = NSGridView::new(mtm);
        grid.setRowSpacing(space::S);
        grid.setColumnSpacing(space::M);
        let mut limits = Vec::new();
        for (index, limit) in LIMITS.into_iter().enumerate() {
            let tag = isize::try_from(index).unwrap_or(0);
            limits.push(self.value_row(mtm, &grid, limit_title(limit), tag));
        }
        let budget = self.value_row(mtm, &grid, t("Budget je Auftrag ($):"), BUDGET_TAG);
        column.addArrangedSubview(&grid);
        column.addArrangedSubview(&ui::heading(mtm, t("Worktree")));
        let copy = crate::actions::line_field(mtm, "", t("Mitkopieren, etwa node_modules, .env.local"));
        copy.widthAnchor().constraintEqualToConstant(WIDTH).setActive(true);
        column.addArrangedSubview(&copy);
        let setup = crate::actions::line_field(mtm, "", t("Einrichtungsbefehl, etwa npm ci"));
        setup.widthAnchor().constraintEqualToConstant(WIDTH).setActive(true);
        column.addArrangedSubview(&setup);
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
        let panel = panel(mtm, &folder);
        panel.setContentView(Some(&ui::scrolling(mtm, &column)));
        ProjectUi {
            panel,
            problems,
            limits,
            budget,
            copy,
            setup,
            template,
        }
    }

    /// A row of `grid`: `title`, the field, its origin and "Auf Vorgabe zurücksetzen" tagged `tag`.
    fn value_row(
        &self,
        mtm: MainThreadMarker,
        grid: &NSGridView,
        title: &str,
        tag: isize,
    ) -> (Retained<NSTextField>, Retained<NSTextField>) {
        let caption = label(mtm, title);
        let field = NSTextField::textFieldWithString(&ns(""), mtm);
        field.widthAnchor().constraintEqualToConstant(80.0).setActive(true);
        ide_ui_common::util::set_title_element(&field, &caption);
        let origin = ui::note(mtm, "", 120.0);
        let reset = ui::button(
            mtm,
            t("Auf Vorgabe zurücksetzen"),
            self.as_ref(),
            sel!(ideFactoryProjectReset:),
            tag,
        );
        reset.setControlSize(objc2_app_kit::NSControlSize::Small);
        ide_ui_common::util::set_label(
            &reset,
            Some(&tf("{} auf Vorgabe zurücksetzen", &[title.trim_end_matches(':')])),
        );
        let views: [&NSView; 4] = [&caption, &field, &origin, &reset];
        grid.addRowWithViews(&objc2_foundation::NSArray::from_slice(&views));
        (field, origin)
    }

    fn ui(&self) -> &ProjectUi {
        self.ivars().ui.get().expect("the sheet builds its views in `show`")
    }

    /// Shows every value of the file as edited, with its origin, and the file's errors.
    fn show_values(&self) {
        let file = self.ivars().file.borrow();
        let ui = self.ui();
        for (limit, (field, origin)) in LIMITS.into_iter().zip(&ui.limits) {
            let (value, from) = file.limit(limit);
            field.setStringValue(&ns(&value.to_string()));
            origin.setStringValue(&ns(from.words()));
        }
        let (dollars, from) = file.order_budget();
        ui.budget
            .0
            .setStringValue(&ns(&dollars.map(|d| format!("{d:.2}")).unwrap_or_default()));
        ui.budget.1.setStringValue(&ns(from.words()));
        ui.copy.setStringValue(&ns(&file.copy().0.join(", ")));
        ui.setup.setStringValue(&ns(&file.setup().0));
        let errors = file.errors();
        ui.problems.setStringValue(&ns(&errors.join("\n")));
        ui.problems.setHidden(errors.is_empty());
    }

    fn reset(&self, tag: isize) {
        let result = {
            let mut file = self.ivars().file.borrow_mut();
            if tag == BUDGET_TAG {
                file.set_order_budget(None)
            } else {
                match usize::try_from(tag).ok().and_then(|i| LIMITS.get(i)) {
                    Some(limit) => file.reset_limit(*limit),
                    None => return,
                }
            }
        };
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

    /// Takes what the fields say into the file; the first problem, in the user's words.
    fn take_fields(&self) -> Result<(), Explained> {
        let ui = self.ui();
        let mut file = self.ivars().file.borrow_mut();
        for (limit, (field, _)) in LIMITS.into_iter().zip(&ui.limits) {
            let typed = field.stringValue().to_string();
            let Ok(value) = typed.trim().parse::<u64>() else {
                return Err(Explained(tf("„{}“ ist keine ganze Zahl.", &[&typed])));
            };
            let (now, origin) = file.limit(limit);
            if value != now || origin == Origin::File {
                file.set_limit(limit, value)?;
            }
        }
        let typed = ui.budget.0.stringValue().to_string();
        let dollars = read_dollars(&typed).map_err(|_| Explained(tf("„{}“ ist kein Betrag in Dollar.", &[&typed])))?;
        let (now, origin) = file.order_budget();
        if dollars != now || origin == Origin::File {
            file.set_order_budget(dollars)?;
        }
        file.set_copy(&ui.copy.stringValue().to_string())?;
        file.set_setup(&ui.setup.stringValue().to_string())
    }

    fn save(&self) {
        if let Err(error) = self.take_fields() {
            self.problem(&error.0);
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

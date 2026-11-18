//! The sheet "Neuer Auftrag" (E185) on a window: title, goal and criteria, the options folded away below them (risk
//! class, path scope, budget, specification without approval), what keeps the order from starting checked as the
//! user types, and the note that the order starts at HEAD without uncommitted changes (F51). Taken from an issue
//! (E176), it says that the text is the issue's and offers the issue's open tasks as suggestions to leave out.

use std::cell::{OnceCell, RefCell};

use ide_ui_common::util::{guarded, ns, set_label};
use objc2::rc::Retained;
use objc2::runtime::{AnyObject, NSObject, ProtocolObject};
use objc2::{DefinedClass, MainThreadMarker, MainThreadOnly, Message, define_class, msg_send, sel};
use objc2_app_kit::{
    NSAccessibility, NSBackingStoreType, NSButton, NSColor, NSControlStateValueOn, NSControlTextEditingDelegate,
    NSModalResponse, NSPopUpButton, NSScrollView, NSStackView, NSTextDelegate, NSTextField, NSTextFieldDelegate,
    NSTextView, NSTextViewDelegate, NSView, NSWindow, NSWindowStyleMask,
};
use objc2_foundation::{NSNotification, NSObjectProtocol, NSPoint, NSRect, NSSize};
use splat_i18n::{t, tf};

use crate::actions::Target;
use crate::host::host;
use crate::model::sheet::{Prefill, RISKS, SheetContext, SheetForm, risk_title};
use crate::model::words;
use crate::ui;
use ide_ui_common::style::space;

const WIDTH: f64 = 440.0;

pub(crate) struct SheetIvars {
    target: Target,
    cx: SheetContext,
    form: RefCell<SheetForm>,
    ui: OnceCell<SheetUi>,
}

struct SheetUi {
    panel: Retained<NSWindow>,
    title: Retained<NSTextField>,
    goal: Retained<NSTextView>,
    criteria: Retained<NSTextView>,
    disclosure: Retained<NSButton>,
    options: Retained<NSStackView>,
    risk: Retained<NSPopUpButton>,
    scope: Retained<NSTextField>,
    budget: Retained<NSTextField>,
    skip: Retained<NSButton>,
    suggestions: Vec<Retained<NSButton>>,
    messages: Retained<NSTextField>,
    start: Retained<NSButton>,
}

define_class!(
    // SAFETY: NSObject has no subclassing requirements and `OrderSheet` does not implement Drop.
    #[unsafe(super(NSObject))]
    #[thread_kind = MainThreadOnly]
    #[name = "IDEFactoryOrderSheet"]
    #[ivars = SheetIvars]
    pub(crate) struct OrderSheet;

    unsafe impl NSObjectProtocol for OrderSheet {}

    unsafe impl NSControlTextEditingDelegate for OrderSheet {
        #[unsafe(method(controlTextDidChange:))]
        fn control_text_did_change(&self, _notification: &NSNotification) {
            guarded((), || self.changed());
        }
    }

    unsafe impl NSTextFieldDelegate for OrderSheet {}

    unsafe impl NSTextDelegate for OrderSheet {
        #[unsafe(method(textDidChange:))]
        fn text_did_change(&self, _notification: &NSNotification) {
            guarded((), || self.changed());
        }
    }

    unsafe impl NSTextViewDelegate for OrderSheet {}

    impl OrderSheet {
        #[unsafe(method(ideFactorySheetChanged:))]
        fn changed_action(&self, _sender: Option<&AnyObject>) {
            guarded((), || self.changed());
        }

        #[unsafe(method(ideFactorySheetOptions:))]
        fn options_action(&self, _sender: Option<&AnyObject>) {
            guarded((), || {
                self.ivars().form.borrow_mut().toggle_options();
                self.show_options();
            });
        }

        #[unsafe(method(ideFactorySheetCancel:))]
        fn cancel_action(&self, _sender: Option<&AnyObject>) {
            guarded((), || self.close());
        }

        #[unsafe(method(ideFactorySheetStart:))]
        fn start_action(&self, _sender: Option<&AnyObject>) {
            guarded((), || self.start());
        }
    }
);

/// Shows the sheet "Neuer Auftrag" on `target`'s window.
pub(crate) fn show(target: &Target) {
    show_prefilled(target, None);
}

/// Shows the sheet on `target`'s window, filled in from an issue when `prefill` says.
pub(crate) fn show_prefilled(target: &Target, prefill: Option<Prefill>) {
    let window = target.window();
    let Some(parent) = window.window() else { return };
    if parent.attachedSheet().is_some() {
        return;
    }
    let mtm = window.mtm();
    let ivars = SheetIvars {
        target: target.clone(),
        cx: host().sheet(target.root()),
        form: RefCell::new(prefill.as_ref().map(SheetForm::prefilled).unwrap_or_default()),
        ui: OnceCell::new(),
    };
    // SAFETY: NSObject's initializer, called once on the newly allocated object.
    let sheet: Retained<OrderSheet> = unsafe { msg_send![super(OrderSheet::alloc(mtm).set_ivars(ivars)), init] };
    let _ = sheet.ivars().ui.set(sheet.build(mtm));
    if let Some(prefill) = &prefill {
        sheet.ui().title.setStringValue(&ns(prefill.title()));
        sheet.ui().goal.setString(&ns(prefill.goal()));
    }
    sheet.changed();
    sheet.show_options();
    let kept = sheet.clone();
    // The handler holds the sheet until it ends, since the panel holds its controls' target only weakly.
    let done = block2::RcBlock::new(move |_response: NSModalResponse| guarded((), || drop(kept.retain())));
    let panel = &sheet.ui().panel;
    panel.setInitialFirstResponder(Some(&sheet.ui().title));
    parent.beginSheet_completionHandler(panel, Some(&done));
}

impl OrderSheet {
    fn build(&self, mtm: MainThreadMarker) -> SheetUi {
        let me: &AnyObject = self.as_ref();
        let column = ui::column(mtm, space::M);
        column.setEdgeInsets(objc2_foundation::NSEdgeInsets {
            top: space::GROUP,
            left: space::GROUP,
            bottom: space::GROUP,
            right: space::GROUP,
        });
        column.addArrangedSubview(&ui::heading(mtm, t("Neuer Auftrag")));
        let title = crate::actions::line_field(mtm, "", t("Titel"));
        // SAFETY: The field holds its delegate weakly; the sheet outlives its panel through the completion handler.
        unsafe { title.setDelegate(Some(ProtocolObject::from_ref(self))) };
        title.widthAnchor().constraintEqualToConstant(WIDTH).setActive(true);
        labelled(mtm, &column, t("Titel"), &title);
        let (goal_box, goal) = self.text_box(mtm, t("Ziel"));
        labelled(mtm, &column, t("Ziel"), &goal_box);
        let suggestions = self.issue_part(mtm, &column);
        let (criteria_box, criteria) = self.text_box(mtm, t("Kriterien, eines je Zeile"));
        labelled(mtm, &column, t("Kriterien, eines je Zeile"), &criteria_box);
        let disclosure = ui::button(mtm, t("Optionen"), me, sel!(ideFactorySheetOptions:), 0);
        disclosure.setBezelStyle(objc2_app_kit::NSBezelStyle::Disclosure);
        disclosure.setButtonType(objc2_app_kit::NSButtonType::PushOnPushOff);
        disclosure.setTitle(&ns(""));
        set_label(&disclosure, Some(t("Optionen")));
        let line = ui::row(mtm, space::XS);
        line.addArrangedSubview(&disclosure);
        line.addArrangedSubview(&ide_ui_common::util::label(mtm, t("Optionen")));
        column.addArrangedSubview(&line);
        let options = ui::column(mtm, space::S);
        let risk = NSPopUpButton::new(mtm);
        for choice in RISKS {
            risk.addItemWithTitle(&ns(risk_title(choice)));
        }
        ide_ui_common::util::set_action(&risk, me, sel!(ideFactorySheetChanged:));
        labelled(mtm, &options, t("Risikoklasse"), &risk);
        let scope = crate::actions::line_field(mtm, "", t("Pfadbereich, etwa src/export/** (freiwillig)"));
        // SAFETY: As for the title's field.
        unsafe { scope.setDelegate(Some(ProtocolObject::from_ref(self))) };
        scope.widthAnchor().constraintEqualToConstant(WIDTH).setActive(true);
        labelled(mtm, &options, t("Pfadbereich"), &scope);
        let budget = crate::actions::line_field(mtm, "", t("Vorgabe"));
        // SAFETY: As for the title's field.
        unsafe { budget.setDelegate(Some(ProtocolObject::from_ref(self))) };
        labelled(mtm, &options, t("Budget in Dollar"), &budget);
        // SAFETY: The sheet answers the action and lives as long as its panel.
        let skip = unsafe {
            NSButton::checkboxWithTitle_target_action(
                &ns(t("Spezifikation ohne Freigabe")),
                Some(me),
                Some(sel!(ideFactorySheetChanged:)),
                mtm,
            )
        };
        let issue = self.ivars().form.borrow().issue().is_some();
        skip.setEnabled(self.ivars().cx.skip_allowed() && !issue);
        options.addArrangedSubview(&skip);
        if !self.ivars().cx.skip_allowed() {
            options.addArrangedSubview(&ui::note(
                mtm,
                t("Ohne Freigabe nur bei Risikoklasse niedrig und nur, wenn das Projekt es erlaubt."),
                WIDTH,
            ));
        }
        column.addArrangedSubview(&options);
        let messages = ui::note(mtm, "", WIDTH);
        column.addArrangedSubview(&messages);
        let buttons = ui::row(mtm, space::M);
        let cancel = ui::button(mtm, t("Abbrechen"), me, sel!(ideFactorySheetCancel:), 0);
        cancel.setKeyEquivalent(&ns("\u{1b}"));
        let start = ui::button(mtm, t("Starten"), me, sel!(ideFactorySheetStart:), 0);
        start.setKeyEquivalent(&ns("\r"));
        buttons.addArrangedSubview(&cancel);
        buttons.addArrangedSubview(&start);
        column.addArrangedSubview(&buttons);
        column.setAlignment(objc2_app_kit::NSLayoutAttribute::Leading);
        let panel = panel(mtm);
        panel.setContentView(Some(&column));
        SheetUi {
            panel,
            title,
            goal,
            criteria,
            disclosure,
            options,
            risk,
            scope,
            budget,
            skip,
            suggestions,
            messages,
            start,
        }
    }

    /// For an order from an issue: the note that its text is foreign, and a box per open task of the issue, ticked.
    fn issue_part(&self, mtm: MainThreadMarker, column: &NSStackView) -> Vec<Retained<NSButton>> {
        let form = self.ivars().form.borrow();
        let Some(number) = form.issue() else {
            return Vec::new();
        };
        let note = tf(
            "Titel und Ziel stammen aus Issue #{}. Der Text geht als Fremdtext an die Agenten, und die Spezifikation braucht immer die Freigabe.",
            &[&number.to_string()],
        );
        column.addArrangedSubview(&ui::note(mtm, &note, WIDTH));
        if form.suggestions().is_empty() {
            return Vec::new();
        }
        let me: &AnyObject = self.as_ref();
        let caption = ide_ui_common::util::label(mtm, t("Vorschläge aus dem Issue"));
        column.addArrangedSubview(&caption);
        column.setCustomSpacing_afterView(space::XS, &caption);
        let boxes: Vec<Retained<NSButton>> = form
            .suggestions()
            .iter()
            .map(|(text, on)| {
                // SAFETY: The sheet answers the action and lives as long as its panel.
                let tick = unsafe {
                    NSButton::checkboxWithTitle_target_action(
                        &ns(text),
                        Some(me),
                        Some(sel!(ideFactorySheetChanged:)),
                        mtm,
                    )
                };
                tick.setState(if *on { NSControlStateValueOn } else { 0 });
                tick.setAccessibilityHelp(Some(&ns(t("Vorschlag aus dem Issue"))));
                column.addArrangedSubview(&tick);
                tick
            })
            .collect();
        boxes
    }

    fn text_box(&self, mtm: MainThreadMarker, name: &str) -> (Retained<NSScrollView>, Retained<NSTextView>) {
        let frame = NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(WIDTH, 72.0));
        let scroll = NSScrollView::initWithFrame(NSScrollView::alloc(mtm), frame);
        scroll.setHasVerticalScroller(true);
        scroll.setBorderType(objc2_app_kit::NSBorderType::BezelBorder);
        let view = NSTextView::initWithFrame(NSTextView::alloc(mtm), frame);
        view.setRichText(false);
        view.setFont(Some(&ide_ui_common::style::body()));
        view.setAutoresizingMask(objc2_app_kit::NSAutoresizingMaskOptions::ViewWidthSizable);
        view.setAccessibilityLabel(Some(&ns(name)));
        view.setDelegate(Some(ProtocolObject::from_ref(self)));
        scroll.setDocumentView(Some(&view));
        scroll.widthAnchor().constraintEqualToConstant(WIDTH).setActive(true);
        scroll.heightAnchor().constraintEqualToConstant(72.0).setActive(true);
        (scroll, view)
    }

    fn ui(&self) -> &SheetUi {
        self.ivars().ui.get().expect("the sheet builds its views in `show`")
    }

    /// Reads every field into the form and shows what keeps the order back.
    fn changed(&self) {
        let ui = self.ui();
        let mut form = self.ivars().form.borrow_mut();
        form.set_texts(
            &ui.title.stringValue().to_string(),
            &ui.goal.string().to_string(),
            &ui.criteria.string().to_string(),
        );
        form.set_options(
            usize::try_from(ui.risk.indexOfSelectedItem()).unwrap_or(0),
            &ui.scope.stringValue().to_string(),
            &ui.budget.stringValue().to_string(),
            ui.skip.state() == NSControlStateValueOn,
        );
        for (index, tick) in ui.suggestions.iter().enumerate() {
            form.choose_suggestion(index, tick.state() == NSControlStateValueOn);
        }
        let cx = &self.ivars().cx;
        let lines = form.messages(cx);
        let ok = form.can_start(cx);
        ui.messages.setStringValue(&ns(&lines.join("\n")));
        let color = if ok {
            NSColor::secondaryLabelColor()
        } else {
            NSColor::systemRedColor()
        };
        ui.messages.setTextColor(Some(&color));
        ui.messages.setHidden(lines.is_empty());
        ui.start.setEnabled(ok);
    }

    fn show_options(&self) {
        let open = self.ivars().form.borrow().options_open();
        let ui = self.ui();
        ui.options.setHidden(!open);
        ui.disclosure.setState(if open { NSControlStateValueOn } else { 0 });
    }

    fn start(&self) {
        self.changed();
        let form = self.ivars().form.borrow().clone();
        if !form.can_start(&self.ivars().cx) {
            return;
        }
        let target = self.ivars().target.clone();
        match host().new_order(target.root(), form.order()) {
            Ok(id) => {
                self.close();
                target.refresh();
                crate::order::open(&target, &id);
            }
            Err(refused) => {
                let lines: Vec<String> = refused.iter().map(words::refusal).collect();
                let ui = self.ui();
                ui.messages.setStringValue(&ns(&lines.join("\n")));
                ui.messages.setTextColor(Some(&NSColor::systemRedColor()));
                ui.messages.setHidden(false);
            }
        }
    }

    fn close(&self) {
        let panel = &self.ui().panel;
        if let Some(parent) = panel.sheetParent() {
            parent.endSheet(panel);
        }
        panel.orderOut(None);
    }
}

/// `view` under its title `title` in `stack`.
fn labelled(mtm: MainThreadMarker, stack: &NSStackView, title: &str, view: &NSView) {
    let caption = ide_ui_common::util::label(mtm, title);
    if let Some(control) = view.downcast_ref::<objc2_app_kit::NSControl>() {
        ide_ui_common::util::set_title_element(control, &caption);
    }
    stack.addArrangedSubview(&caption);
    stack.setCustomSpacing_afterView(space::XS, &caption);
    stack.addArrangedSubview(view);
}

fn panel(mtm: MainThreadMarker) -> Retained<NSWindow> {
    let frame = NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(WIDTH + 2.0 * space::GROUP, 420.0));
    // SAFETY: NSWindow's designated initializer on a newly allocated window.
    let panel = unsafe {
        NSWindow::initWithContentRect_styleMask_backing_defer(
            NSWindow::alloc(mtm),
            frame,
            NSWindowStyleMask::Titled,
            NSBackingStoreType::Buffered,
            false,
        )
    };
    // SAFETY: Switching it off keeps AppKit from releasing the window behind Rust's back.
    unsafe { panel.setReleasedWhenClosed(false) };
    panel.setTitle(&ns(t("Neuer Auftrag")));
    panel
}

#[cfg(feature = "selftest")]
mod selftest_probe;

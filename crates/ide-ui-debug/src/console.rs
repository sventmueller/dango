//! The debug console as a tab of its own (E60, E94): what the program and the adapter write, and a field that
//! evaluates an expression in the paused frame. Once its session ended, or after a restart, it shows its history only
//! and says so (Review C18).

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use ide_core::debugging::{ConsoleKind, ConsoleLine};
use objc2::rc::Retained;
use objc2::runtime::{AnyObject, ProtocolObject};
use objc2::{AnyThread, DefinedClass, MainThreadMarker, MainThreadOnly, sel};
use objc2_app_kit::{
    NSAccessibility, NSColor, NSScrollView, NSStackView, NSTextField, NSTextView, NSUserInterfaceLayoutOrientation,
    NSView,
};
use objc2_foundation::{NSAttributedString, NSDictionary, NSPoint, NSRange, NSRect, NSSize, NSString};
use splat_i18n::{t, tf};

use super::model::console::Console;
use super::panel::DebugPanel;
use ide_ui_common::constant;
use ide_ui_common::util::{label, ns, set_action, with_editing};
use ide_ui_common::window::{TabKind, Window};
#[cfg(feature = "selftest")]
mod selftest_probe;

/// A debug console tab.
pub struct ConsoleTab {
    /// The tab's document number.
    pub tab: isize,
    /// The session it belongs to; none once it came back after a restart.
    pub session: Cell<Option<u64>>,
    pub title: RefCell<String>,
    model: RefCell<Console>,
    text: Retained<NSTextView>,
    input: Retained<NSTextField>,
    finished: Retained<NSTextField>,
    /// How far ↑ went back through what was asked.
    back: Cell<Option<usize>>,
}

impl ConsoleTab {
    /// Opens a console tab for the session `session` named `title`, beside nothing, in `controller`'s window.
    pub fn open(panel: &DebugPanel, controller: &Window, session: u64, title: &str) -> Rc<ConsoleTab> {
        let console = ConsoleTab::build(panel, controller, title, Console::default());
        console.session.set(Some(session));
        console
    }

    /// Opens the finished console `title` with `history`, as a restart brings it back; no session starts.
    pub fn reopen(panel: &DebugPanel, controller: &Window, title: &str, history: Vec<ConsoleLine>) -> Rc<ConsoleTab> {
        ConsoleTab::build(panel, controller, title, Console::finished(history))
    }

    fn build(panel: &DebugPanel, controller: &Window, title: &str, model: Console) -> Rc<ConsoleTab> {
        let mtm = controller.mtm();
        let (root, text, input, finished) = views(mtm, panel);
        let tab = controller
            .add_tab(TabKind::Console, None, &tf("Debug-Konsole: {}", &[title]), root)
            .unwrap_or(-1);
        input.setTag(tab);
        let console = Rc::new(ConsoleTab {
            tab,
            session: Cell::new(None),
            title: RefCell::new(title.to_string()),
            model: RefCell::new(model),
            text,
            input,
            finished,
            back: Cell::new(None),
        });
        console.show_all();
        console
    }

    pub fn lines(&self) -> Vec<ConsoleLine> {
        self.model.borrow().lines().to_vec()
    }

    pub fn is_finished(&self) -> bool {
        self.model.borrow().finished
    }

    /// Adds `text` of `kind` and shows it.
    pub fn push(&self, kind: ConsoleKind, text: &str) {
        self.model.borrow_mut().push(kind, text);
        self.show_all();
    }

    /// Takes the expression the user typed; `None` when there is nothing to evaluate.
    pub fn ask(&self, expression: &str) -> Option<String> {
        let asked = self.model.borrow_mut().ask(expression)?;
        self.back.set(None);
        self.input.setStringValue(&ns(""));
        self.show_all();
        Some(asked)
    }

    /// Ends the console with `note`: it takes no more input and says it is over.
    pub fn finish(&self, note: &str) {
        self.model.borrow_mut().finish(note);
        self.session.set(None);
        self.show_all();
    }

    /// Empties a finished console for a new session named `title`, which writes to it from now on.
    pub fn start_over(&self, controller: &Window, title: &str) {
        *self.model.borrow_mut() = Console::default();
        *self.title.borrow_mut() = title.to_string();
        self.session.set(None);
        controller.set_tab_title(self.tab, &tf("Debug-Konsole: {}", &[title]));
        self.show_all();
    }

    /// Puts what was asked one step further back (↑) or forward (↓) into the field.
    pub fn step_history(&self, older: bool) {
        let back = match (self.back.get(), older) {
            (None, true) => Some(0),
            (None, false) => None,
            (Some(back), true) => Some(back + 1),
            (Some(0), false) => None,
            (Some(back), false) => Some(back - 1),
        };
        let text = match back {
            Some(back) => match self.model.borrow().asked(back) {
                Some(text) => text.to_string(),
                None => return,
            },
            None => String::new(),
        };
        self.back.set(back);
        self.input.setStringValue(&ns(&text));
    }

    /// Shows the whole history and, once it is over, the note instead of the field.
    fn show_all(&self) {
        let model = self.model.borrow();
        // SAFETY: The view keeps its text storage alive; asking for it keeps a TextKit 2 view on TextKit 2.
        let Some(storage) = (unsafe { self.text.textStorage() }) else {
            return;
        };
        with_editing(&storage, || {
            storage.setAttributedString(&NSAttributedString::new());
            for line in model.lines() {
                let mut text = match line.kind {
                    ConsoleKind::Input => format!("› {}", line.text),
                    _ => line.text.clone(),
                };
                if !text.ends_with('\n') {
                    text.push('\n');
                }
                storage.appendAttributedString(&styled(&text, line.kind));
            }
        });
        let end = storage.length();
        self.text.scrollRangeToVisible(NSRange::new(end, 0));
        self.input.setHidden(model.finished);
        self.finished.setHidden(!model.finished);
        self.text.setAccessibilityLabel(Some(&ns(&if model.finished {
            tf("Debug-Konsole: {}, beendet", &[&self.title.borrow()])
        } else {
            tf("Debug-Konsole: {}", &[&self.title.borrow()])
        })));
    }
}

/// A line of the console in the colour of its kind.
fn styled(text: &str, kind: ConsoleKind) -> Retained<NSAttributedString> {
    let color = match kind {
        ConsoleKind::Input | ConsoleKind::Output => NSColor::labelColor(),
        ConsoleKind::Result => NSColor::secondaryLabelColor(),
        ConsoleKind::Error => NSColor::systemRedColor(),
        ConsoleKind::Note => NSColor::tertiaryLabelColor(),
    };
    let font = ide_ui_common::style::mono(12.0);
    let keys = [
        constant!(objc2_app_kit::NSFontAttributeName),
        constant!(objc2_app_kit::NSForegroundColorAttributeName),
    ];
    let values: [&AnyObject; 2] = [&font, &color];
    let attributes = NSDictionary::<NSString, AnyObject>::from_slices(&keys, &values);
    // SAFETY: The attributes map the font and colour keys to a font and a colour.
    unsafe { NSAttributedString::initWithString_attributes(NSAttributedString::alloc(), &ns(text), Some(&attributes)) }
}

/// The console's views: the history, the field below it, and the note that takes the field's place once it is over.
fn views(
    mtm: MainThreadMarker,
    panel: &DebugPanel,
) -> (
    Retained<NSView>,
    Retained<NSTextView>,
    Retained<NSTextField>,
    Retained<NSTextField>,
) {
    let frame = NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(600.0, 400.0));
    let scroll = NSScrollView::initWithFrame(NSScrollView::alloc(mtm), frame);
    scroll.setHasVerticalScroller(true);
    scroll.setAutohidesScrollers(true);
    let text = NSTextView::initWithFrame(NSTextView::alloc(mtm), frame);
    text.setEditable(false);
    text.setSelectable(true);
    text.setRichText(false);
    text.setTextContainerInset(NSSize::new(8.0, 8.0));
    text.setAutoresizingMask(objc2_app_kit::NSAutoresizingMaskOptions::ViewWidthSizable);
    scroll.setDocumentView(Some(&text));
    let input = NSTextField::new(mtm);
    input.setPlaceholderString(Some(&ns(t("Ausdruck im angehaltenen Frame auswerten"))));
    input.setAccessibilityLabel(Some(&ns(t("Ausdruck auswerten"))));
    input.setFont(Some(&ide_ui_common::style::mono(12.0)));
    set_action(&input, panel, sel!(ideDebugEvaluate:));
    // SAFETY: The field holds its delegate weakly; the panel registry keeps the panel alive.
    unsafe { input.setDelegate(Some(ProtocolObject::from_ref(panel))) };
    let finished = label(mtm, t("Beendet: nur der Verlauf, ohne laufende Sitzung."));
    finished.setTextColor(Some(&NSColor::secondaryLabelColor()));
    finished.setFont(Some(&ide_ui_common::style::small_system()));
    let stack = NSStackView::new(mtm);
    stack.setOrientation(NSUserInterfaceLayoutOrientation::Vertical);
    stack.setSpacing(ide_ui_common::style::space::S);
    stack.setEdgeInsets(objc2_foundation::NSEdgeInsets {
        top: 0.0,
        left: 0.0,
        bottom: 8.0,
        right: 0.0,
    });
    stack.addArrangedSubview(&scroll);
    for below in [&input, &finished] {
        stack.addArrangedSubview(below);
        below
            .leadingAnchor()
            .constraintEqualToAnchor_constant(&stack.leadingAnchor(), 8.0)
            .setActive(true);
        below
            .trailingAnchor()
            .constraintEqualToAnchor_constant(&stack.trailingAnchor(), -8.0)
            .setActive(true);
    }
    scroll
        .leadingAnchor()
        .constraintEqualToAnchor(&stack.leadingAnchor())
        .setActive(true);
    scroll
        .trailingAnchor()
        .constraintEqualToAnchor(&stack.trailingAnchor())
        .setActive(true);
    scroll.setContentHuggingPriority_forOrientation(1.0, objc2_app_kit::NSLayoutConstraintOrientation::Vertical);
    (stack.into_super(), text, input, finished)
}

impl DebugPanel {
    /// The console of the tab `tab`.
    pub fn console_of_tab(&self, tab: isize) -> Option<Rc<ConsoleTab>> {
        self.ivars()
            .links
            .borrow()
            .consoles
            .iter()
            .find(|c| c.tab == tab)
            .cloned()
    }

    /// The console of the session `session`.
    pub fn console_of(&self, session: u64) -> Option<Rc<ConsoleTab>> {
        self.ivars()
            .links
            .borrow()
            .consoles
            .iter()
            .find(|c| c.session.get() == Some(session))
            .cloned()
    }

    /// The tab of a console closed.
    pub fn console_closed(&self, tab: isize) {
        self.ivars().links.borrow_mut().consoles.retain(|c| c.tab != tab);
    }

    /// Opens the finished console `title` with `history`, as the layout brings it back.
    pub fn reopen_console(&self, title: &str, history: Vec<ConsoleLine>) -> bool {
        let Some(controller) = self.controller() else {
            return false;
        };
        let console = ConsoleTab::reopen(self, &controller, title, history);
        self.ivars().links.borrow_mut().consoles.push(console);
        true
    }

    /// The expression typed into the console of the tab the field belongs to, evaluated in the paused frame.
    pub(super) fn console_typed(&self, sender: Option<&AnyObject>) {
        let Some(field) = sender.and_then(|s| s.downcast_ref::<NSTextField>()) else {
            return;
        };
        let Some(console) = self.console_of_tab(field.tag()) else {
            return;
        };
        let text = field.stringValue().to_string();
        let Some(expression) = console.ask(&text) else { return };
        let Some(session) = console.session.get() else { return };
        self.evaluate_in_console(session, &expression);
    }

    /// ↑ and ↓ in a console's field go through what was asked.
    pub(super) fn console_key(&self, field: &NSTextField, older: bool) -> bool {
        match self
            .console_of_tab(field.tag())
            .filter(|c| std::ptr::eq(&*c.input, field))
        {
            Some(console) => {
                console.step_history(older);
                true
            }
            None => false,
        }
    }
}

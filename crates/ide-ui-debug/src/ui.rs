//! The views of the debug panel and the inspector: the controls in the panel's title row, the start row with its
//! configurations, the list of sessions and breakpoints, the note of a folder not trusted, and the inspector's list
//! with the field for a new watch expression.

use objc2::rc::Retained;
use objc2::runtime::{AnyObject, ProtocolObject, Sel};
use objc2::{MainThreadMarker, MainThreadOnly, define_class, msg_send, sel};
use objc2_app_kit::{
    NSAccessibility, NSButton, NSColor, NSControl, NSEvent, NSLayoutAttribute, NSLayoutConstraintOrientation, NSMenu,
    NSOutlineView, NSPopUpButton, NSScrollView, NSStackView, NSStackViewDistribution, NSTextField,
    NSUserInterfaceLayoutOrientation, NSView,
};
use objc2_foundation::{NSObjectProtocol, NSPoint, NSRect, NSSize};
use splat_i18n::t;

use super::model::Command;
use super::panel::DebugPanel;
use ide_ui_common::util::{label, ns, pin_edges, set_action};

/// How far the panel's content keeps from the sidebar's edges: as far as the panel's title.
const MARGIN: f64 = 8.0;
/// How far AppKit's source list sets its disclosure triangles in past the panel's title.
const OUTDENT: f64 = 5.5;

/// Return and the keypad's Enter.
const RETURN_KEYS: [u16; 2] = [36, 76];
/// Delete and forward delete.
const DELETE_KEYS: [u16; 2] = [51, 117];

define_class!(
    // SAFETY: NSOutlineView has no subclassing requirements and `DebugOutlineView` does not implement Drop.
    /// Return shows a frame or opens a row, Delete removes a breakpoint or a watch expression.
    #[unsafe(super(NSOutlineView))]
    #[thread_kind = MainThreadOnly]
    #[name = "IDEDebugOutlineView"]
    pub struct DebugOutlineView;

    unsafe impl NSObjectProtocol for DebugOutlineView {}

    impl DebugOutlineView {
        /// The disclosure triangle where the panel's title begins, not in the source list's margin; a row below a
        /// section has its triangle where the section has its own.
        #[unsafe(method(frameOfOutlineCellAtRow:))]
        fn outline_cell_frame(&self, row: isize) -> NSRect {
            // SAFETY: The superclass's implementation, with this method's signature.
            let frame: NSRect = unsafe { msg_send![super(self), frameOfOutlineCellAtRow: row] };
            let frame = ide_ui_common::panel::outdent(self, frame, OUTDENT, false);
            if self.levelForRow(row) > 0 {
                self.toward_leading(frame, false)
            } else {
                frame
            }
        }

        /// The rows moved along with the triangles: a section's rows begin where its title does, and a note or a
        /// switch, which has no triangle, where the section's triangle does.
        #[unsafe(method(frameOfCellAtColumn:row:))]
        fn cell_frame(&self, column: isize, row: isize) -> NSRect {
            // SAFETY: The superclass's implementation, with this method's signature.
            let frame: NSRect = unsafe { msg_send![super(self), frameOfCellAtColumn: column, row: row] };
            if column != 0 {
                return frame;
            }
            let frame = ide_ui_common::panel::outdent(self, frame, OUTDENT, true);
            if self.levelForRow(row) <= 0 {
                frame
            } else if self.is_flush(row) {
                self.flush_with_section(frame, row)
            } else {
                self.toward_leading(frame, true)
            }
        }

        #[unsafe(method(keyDown:))]
        fn key_down(&self, event: &NSEvent) {
            let handled = ide_ui_common::util::guarded(false, || {
                let Some(panel) = self.panel() else { return false };
                let code = event.keyCode();
                if RETURN_KEYS.contains(&code) {
                    panel.activate_in(self, self.selectedRow());
                    true
                } else if DELETE_KEYS.contains(&code) {
                    panel.delete_in(self, self.selectedRow())
                } else {
                    false
                }
            });
            if !handled {
                // SAFETY: The superclass's implementation, with this method's signature.
                let _: () = unsafe { msg_send![super(self), keyDown: event] };
            }
        }
    }
);

impl DebugOutlineView {
    fn new(mtm: MainThreadMarker, frame: NSRect) -> Retained<DebugOutlineView> {
        // SAFETY: NSOutlineView's designated initializer, called once on the newly allocated object.
        unsafe { msg_send![DebugOutlineView::alloc(mtm), initWithFrame: frame] }
    }

    fn panel(&self) -> Option<Retained<DebugPanel>> {
        // SAFETY: Only `outline` below makes this view, and it sets the panel as the data source.
        let source = unsafe { self.dataSource() }?;
        let object: Retained<AnyObject> = source.into();
        object.downcast::<DebugPanel>().ok()
    }

    /// Whether the row at `row` is a note or a switch right below a section.
    fn is_flush(&self, row: isize) -> bool {
        use super::model::rows::Key;
        use super::model::variables::Row;
        let Some(item) = self.itemAtRow(row) else {
            return false;
        };
        matches!(
            super::panel::key_of(&item),
            Some(Key::Empty(_) | Key::Filter(_) | Key::Breakpoint(..))
        ) || matches!(super::panel::row_of(&item), Some(Row::Empty(_)))
    }

    /// `frame` moved one level toward the leading edge; a cell keeps its far edge.
    fn toward_leading(&self, frame: NSRect, cell: bool) -> NSRect {
        ide_ui_common::panel::outdent(self, frame, self.indentationPerLevel(), cell)
    }

    /// The cell at `row` beginning at the leading edge of its section's triangle.
    fn flush_with_section(&self, frame: NSRect, row: isize) -> NSRect {
        let Some(item) = self.itemAtRow(row) else {
            return frame;
        };
        // SAFETY: The item comes from this outline view, which knows it and its parent.
        let section = unsafe {
            self.parentForItem(Some(&item))
                .map(|parent| self.rowForItem(Some(&parent)))
        };
        let Some(section) = section.filter(|row| *row >= 0) else {
            return self.toward_leading(frame, true);
        };
        // SAFETY: This class's own method, with its signature.
        let chevron: NSRect = unsafe { msg_send![self, frameOfOutlineCellAtRow: section] };
        if chevron.size.width <= 0.0 {
            return self.toward_leading(frame, true);
        }
        let rtl = self.userInterfaceLayoutDirection() == objc2_app_kit::NSUserInterfaceLayoutDirection::RightToLeft;
        let (left, right) = if rtl {
            (frame.origin.x, chevron.origin.x + chevron.size.width)
        } else {
            (chevron.origin.x, frame.origin.x + frame.size.width)
        };
        NSRect::new(
            NSPoint::new(left, frame.origin.y),
            NSSize::new((right - left).max(0.0), frame.size.height),
        )
    }
}

/// The views of the panel.
pub struct Ui {
    pub page: Retained<NSView>,
    /// The controls for the panel's title row.
    pub header: Retained<NSStackView>,
    /// The controls in the order of `Command::CONTROLS`.
    pub controls: Vec<Retained<NSButton>>,
    pub configurations: Retained<NSPopUpButton>,
    pub start: Retained<NSButton>,
    pub outline: Retained<NSOutlineView>,
    /// Why nothing starts in a folder the user does not trust, with the way to trust it.
    pub untrusted: Retained<NSStackView>,
    pub inspector: InspectorUi,
}

/// The views of the inspector.
pub struct InspectorUi {
    pub page: Retained<NSView>,
    pub outline: Retained<NSOutlineView>,
    /// The field for a new watch expression, which the self-test types into.
    #[cfg(feature = "selftest")]
    pub watch: Retained<NSTextField>,
}

/// The symbol and the name of a control.
pub fn control_look(command: Command) -> (&'static str, &'static str) {
    match command {
        Command::Continue => ("play.fill", t("Fortsetzen")),
        Command::StepOver => ("arrow.uturn.right", t("Überspringen")),
        Command::StepInto => ("arrow.down.to.line", t("Hineinspringen")),
        Command::StepOut => ("arrow.up.to.line", t("Heraus")),
        Command::Restart => ("arrow.clockwise", t("Neu starten")),
        Command::Stop => ("stop.fill", t("Stopp")),
        Command::ToggleBreakpoints => ("arrowshape.right", t("Haltepunkte ausschalten")),
        Command::ClearBreakpoints => ("trash", t("Alle Haltepunkte entfernen")),
    }
}

/// The keymap's command of a control, whose shortcut its tooltip names.
fn shortcut_of(command: Command) -> Option<&'static str> {
    match command {
        Command::Continue => Some("debug.continue"),
        Command::StepOver => Some("debug.stepOver"),
        Command::StepInto => Some("debug.stepInto"),
        Command::StepOut => Some("debug.stepOut"),
        Command::Stop => Some("debug.stop"),
        Command::Restart | Command::ToggleBreakpoints | Command::ClearBreakpoints => None,
    }
}

/// A shortcut of the keymap, such as `ctrl+cmd+y`, as macOS writes it: ⌃⌘Y.
pub fn key_symbols(keys: &str) -> String {
    let Some((modifiers, key)) = ide_core::keymap::split(keys) else {
        return keys.to_string();
    };
    let mut text: String = ["ctrl", "alt", "shift", "cmd"]
        .into_iter()
        .filter(|m| modifiers.contains(m))
        .map(|m| match m {
            "ctrl" => "⌃",
            "alt" => "⌥",
            "shift" => "⇧",
            _ => "⌘",
        })
        .collect();
    text.push_str(&key.to_uppercase());
    text
}

/// The command of a control that sent an action.
pub fn command_of(sender: Option<&AnyObject>) -> Option<Command> {
    let tag = sender?.downcast_ref::<NSControl>()?.tag();
    Command::CONTROLS.get(usize::try_from(tag).ok()?).copied()
}

impl Ui {
    pub fn new(mtm: MainThreadMarker, panel: &DebugPanel) -> Ui {
        let (header, controls) = header(mtm, panel);
        let page = NSView::new(mtm);
        let content = NSStackView::new(mtm);
        content.setOrientation(NSUserInterfaceLayoutOrientation::Vertical);
        content.setAlignment(NSLayoutAttribute::Leading);
        content.setDistribution(NSStackViewDistribution::Fill);
        content.setSpacing(ide_ui_common::style::space::S);
        content.setEdgeInsets(objc2_foundation::NSEdgeInsets {
            top: 0.0,
            left: 0.0,
            bottom: 0.0,
            right: 0.0,
        });
        page.addSubview(&content);
        pin_edges(&content, &page);
        let (start_row, configurations, start) = start_row(mtm, panel);
        let untrusted = untrusted(mtm, panel);
        let (list, outline) = outline(
            mtm,
            panel,
            t("Debug-Sitzungen und Haltepunkte"),
            Some(sel!(ideDebugDoubleClicked:)),
        );
        // The list is the first subview though it shows below the start row, so ⌘5 puts the focus there (E54).
        content.addArrangedSubview(&list);
        content.insertArrangedSubview_atIndex(&start_row, 0);
        content.insertArrangedSubview_atIndex(&untrusted, 1);
        for (view, inset) in [(&*list as &NSView, 0.0), (&untrusted, MARGIN), (&start_row, MARGIN)] {
            view.leadingAnchor()
                .constraintEqualToAnchor_constant(&content.leadingAnchor(), inset)
                .setActive(true);
            view.trailingAnchor()
                .constraintEqualToAnchor_constant(&content.trailingAnchor(), -inset)
                .setActive(true);
            view.setContentHuggingPriority_forOrientation(750.0, NSLayoutConstraintOrientation::Vertical);
        }
        list.setContentHuggingPriority_forOrientation(1.0, NSLayoutConstraintOrientation::Vertical);
        untrusted.setHidden(true);
        Ui {
            page,
            header,
            controls,
            configurations,
            start,
            outline,
            untrusted,
            inspector: InspectorUi::new(mtm, panel),
        }
    }

    /// Greys out the controls that cannot run now (E101) and names Stop "Trennen" for a program the session only
    /// attached to.
    pub fn enable_controls(&self, can: &dyn Fn(Command) -> bool, detaches: bool) {
        for (command, button) in Command::CONTROLS.into_iter().zip(&self.controls) {
            button.setEnabled(can(command));
            if command == Command::Stop {
                let (_, stop) = control_look(Command::Stop);
                let name = if detaches { t("Trennen") } else { stop };
                let tip = button.toolTip().map(|t| t.to_string()).unwrap_or_default();
                let keys = tip.rfind(" (").map_or("", |at| &tip[at..]);
                button.setToolTip(Some(&ns(&format!("{name}{keys}"))));
                ide_ui_common::util::set_label(button, Some(name));
            }
        }
    }
}

impl InspectorUi {
    fn new(mtm: MainThreadMarker, panel: &DebugPanel) -> InspectorUi {
        let page = NSView::new(mtm);
        let (list, outline) = outline(mtm, panel, t("Variablen und überwachte Ausdrücke"), None);
        let watch = NSTextField::new(mtm);
        watch.setPlaceholderString(Some(&ns(t("Ausdruck überwachen …"))));
        watch.setAccessibilityLabel(Some(&ns(t("Neuer überwachter Ausdruck"))));
        watch.setFont(Some(&ide_ui_common::style::small_system()));
        set_action(&watch, panel, sel!(ideDebugAddWatch:));
        // SAFETY: The field holds its delegate weakly; the panel registry keeps the panel alive.
        unsafe { watch.setDelegate(Some(ProtocolObject::from_ref(panel))) };
        // The list draws no grey of its own, so the inspector reads as one surface below the window's tab bar.
        outline.setBackgroundColor(&NSColor::clearColor());
        list.setDrawsBackground(false);
        let title = ide_ui_common::panel::heading(mtm, t("Inspektor"));
        // Nil target: the window controller in the responder chain answers it, as the menu item does.
        let close = ide_ui_common::panel::header_button(mtm, "xmark", t("Schließen"), None, sel!(ideToggleInspector:));
        page.addSubview(&title);
        page.addSubview(&close);
        page.addSubview(&list);
        page.addSubview(&watch);
        for view in [&*title as &NSView, &close, &list, &watch] {
            view.setTranslatesAutoresizingMaskIntoConstraints(false);
        }
        let guide = page.safeAreaLayoutGuide();
        for constraint in [
            title
                .topAnchor()
                .constraintEqualToAnchor_constant(&guide.topAnchor(), MARGIN),
            title
                .leadingAnchor()
                .constraintEqualToAnchor_constant(&page.leadingAnchor(), MARGIN + 4.0),
            title
                .trailingAnchor()
                .constraintLessThanOrEqualToAnchor_constant(&close.leadingAnchor(), -4.0),
            close.centerYAnchor().constraintEqualToAnchor(&title.centerYAnchor()),
            close
                .trailingAnchor()
                .constraintEqualToAnchor_constant(&page.trailingAnchor(), -ide_ui_common::panel::ACCESSORY_INSET),
            list.topAnchor()
                .constraintEqualToAnchor_constant(&title.bottomAnchor(), 4.0),
            list.leadingAnchor().constraintEqualToAnchor(&page.leadingAnchor()),
            list.trailingAnchor().constraintEqualToAnchor(&page.trailingAnchor()),
            list.bottomAnchor()
                .constraintEqualToAnchor_constant(&watch.topAnchor(), -6.0),
            watch
                .leadingAnchor()
                .constraintEqualToAnchor_constant(&page.leadingAnchor(), MARGIN),
            watch
                .trailingAnchor()
                .constraintEqualToAnchor_constant(&page.trailingAnchor(), -MARGIN),
            watch
                .bottomAnchor()
                .constraintEqualToAnchor_constant(&page.bottomAnchor(), -MARGIN),
        ] {
            constraint.setActive(true);
        }
        InspectorUi {
            page,
            outline,
            #[cfg(feature = "selftest")]
            watch,
        }
    }
}

/// The six controls of the title row (E94), SF Symbols named for VoiceOver.
fn header(mtm: MainThreadMarker, panel: &DebugPanel) -> (Retained<NSStackView>, Vec<Retained<NSButton>>) {
    let stack = ide_ui_common::panel::header_row(mtm);
    let keymap = ide_ui_common::window::keymap();
    let controls: Vec<Retained<NSButton>> = Command::CONTROLS
        .into_iter()
        .enumerate()
        .map(|(index, command)| {
            let (symbol, name) = control_look(command);
            let button = ide_ui_common::panel::header_button(mtm, symbol, name, Some(panel), sel!(ideDebugControl:));
            button.setTag(isize::try_from(index).unwrap_or(-1));
            if let Some(keys) = shortcut_of(command).and_then(|c| keymap.key(c)) {
                button.setToolTip(Some(&ns(&format!("{name} ({})", key_symbols(keys)))));
            }
            button.setEnabled(false);
            stack.addArrangedSubview(&button);
            button
        })
        .collect();
    (stack, controls)
}

/// The configurations to start with and the button that starts the chosen one.
fn start_row(
    mtm: MainThreadMarker,
    panel: &DebugPanel,
) -> (Retained<NSStackView>, Retained<NSPopUpButton>, Retained<NSButton>) {
    let row = NSStackView::new(mtm);
    row.setOrientation(NSUserInterfaceLayoutOrientation::Horizontal);
    row.setSpacing(ide_ui_common::style::space::S);
    let choices = NSPopUpButton::initWithFrame_pullsDown(
        NSPopUpButton::alloc(mtm),
        NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(160.0, 24.0)),
        false,
    );
    choices.setControlSize(objc2_app_kit::NSControlSize::Small);
    choices.setFont(Some(&ide_ui_common::style::small_system()));
    choices.setAccessibilityLabel(Some(&ns(t("Startkonfiguration"))));
    choices.setToolTip(Some(&ns(t("Startkonfiguration"))));
    set_action(&choices, panel, sel!(ideDebugChoose:));
    choices.setContentCompressionResistancePriority_forOrientation(200.0, NSLayoutConstraintOrientation::Horizontal);
    let image = ide_ui_common::util::symbol("play.fill", t("Starten")).unwrap_or_default();
    // SAFETY: A nil target sends the action along the responder chain; `set_action` aims it at the panel.
    let start = unsafe { NSButton::buttonWithImage_target_action(&image, None, Some(sel!(ideDebugStart:)), mtm) };
    set_action(&start, panel, sel!(ideDebugStart:));
    start.setControlSize(objc2_app_kit::NSControlSize::Small);
    start.setToolTip(Some(&ns(t("Starten"))));
    ide_ui_common::util::set_label(&start, Some(t("Starten")));
    row.addArrangedSubview(&choices);
    row.addArrangedSubview(&start);
    (row, choices, start)
}

/// The note of a folder the user does not trust: nothing it configures starts, the debugger neither (E22).
fn untrusted(mtm: MainThreadMarker, panel: &DebugPanel) -> Retained<NSStackView> {
    let stack = NSStackView::new(mtm);
    stack.setOrientation(NSUserInterfaceLayoutOrientation::Vertical);
    stack.setAlignment(NSLayoutAttribute::Leading);
    stack.setSpacing(ide_ui_common::style::space::XS);
    let note = NSTextField::wrappingLabelWithString(
        &ns(t(
            "Debuggen startet Programme des Ordners und geht erst, wenn du ihm vertraust.",
        )),
        mtm,
    );
    note.setTextColor(Some(&NSColor::secondaryLabelColor()));
    note.setFont(Some(&ide_ui_common::style::small_system()));
    let trust = ide_ui_common::util::borderless_button(mtm, t("Ordner vertrauen …"), Some(panel), sel!(ideDebugTrust:));
    trust.setContentTintColor(Some(&NSColor::controlAccentColor()));
    stack.addArrangedSubview(&note);
    stack.addArrangedSubview(&trust);
    stack
}

/// A list of `panel`'s, in a scroll view, named `name` for VoiceOver; `double` runs on a double click.
fn outline(
    mtm: MainThreadMarker,
    panel: &DebugPanel,
    name: &str,
    double: Option<Sel>,
) -> (Retained<NSScrollView>, Retained<NSOutlineView>) {
    let view = DebugOutlineView::new(mtm, ide_ui_common::panel::LIST_FRAME);
    let scroll = ide_ui_common::panel::source_list_outline(mtm, &view, "debug", name);
    // SAFETY: The outline view holds its data source and delegate weakly; the panel registry keeps the panel alive.
    unsafe {
        view.setDataSource(Some(ProtocolObject::from_ref(panel)));
        view.setDelegate(Some(ProtocolObject::from_ref(panel)));
    }
    if let Some(double) = double {
        set_action(&view, panel, double);
        // SAFETY: The panel answers the double action with a method that takes the sender; `set_action` checked it.
        unsafe { view.setDoubleAction(Some(double)) };
    }
    // SAFETY: A plain action does nothing on a single click; selecting a row is what a click does.
    unsafe { view.setAction(None) };
    let menu = NSMenu::new(mtm);
    menu.setDelegate(Some(ProtocolObject::from_ref(panel)));
    // SAFETY: The view keeps the menu; its delegate, the panel, outlives the view through the registry.
    unsafe { view.setMenu(Some(&menu)) };
    (scroll, view.into_super())
}

/// A row of two texts after a symbol: a name and, dimmer, what it is or where.
pub fn row(mtm: MainThreadMarker, symbol: Option<(&str, &NSColor)>, title: &str, detail: &str) -> Retained<NSView> {
    // The parts sit directly in a table cell view, which hands the selection's background style on to them, so the
    // texts turn light on a selected row.
    let cell = objc2_app_kit::NSTableCellView::new(mtm);
    let mut leading = cell.leadingAnchor();
    let mut gap = 0.0;
    if let Some((name, tint)) = symbol {
        let icon = objc2_app_kit::NSImageView::new(mtm);
        icon.setImage(ide_ui_common::util::symbol(name, "").as_deref());
        icon.setContentTintColor(Some(tint));
        icon.setSymbolConfiguration(Some(
            &objc2_app_kit::NSImageSymbolConfiguration::configurationWithPointSize_weight(
                11.0,
                // SAFETY: AppKit sets its font weight constants when it loads.
                unsafe { objc2_app_kit::NSFontWeightRegular },
            ),
        ));
        ide_ui_common::util::hide_from_voiceover(&icon);
        cell.addSubview(&icon);
        icon.setTranslatesAutoresizingMaskIntoConstraints(false);
        for constraint in [
            icon.leadingAnchor().constraintEqualToAnchor(&leading),
            icon.centerYAnchor().constraintEqualToAnchor(&cell.centerYAnchor()),
            icon.widthAnchor().constraintEqualToConstant(16.0),
        ] {
            constraint.setActive(true);
        }
        // SAFETY: The cell holds its image view weakly; the view is its subview and lives as long.
        unsafe { cell.setImageView(Some(&icon)) };
        leading = icon.trailingAnchor();
        gap = 4.0;
    }
    let name = label(mtm, title);
    name.setLineBreakMode(objc2_app_kit::NSLineBreakMode::ByTruncatingTail);
    name.setContentCompressionResistancePriority_forOrientation(300.0, NSLayoutConstraintOrientation::Horizontal);
    name.setContentHuggingPriority_forOrientation(750.0, NSLayoutConstraintOrientation::Horizontal);
    cell.addSubview(&name);
    name.setTranslatesAutoresizingMaskIntoConstraints(false);
    for constraint in [
        name.leadingAnchor().constraintEqualToAnchor_constant(&leading, gap),
        name.centerYAnchor().constraintEqualToAnchor(&cell.centerYAnchor()),
        name.trailingAnchor()
            .constraintLessThanOrEqualToAnchor_constant(&cell.trailingAnchor(), -4.0),
    ] {
        constraint.setActive(true);
    }
    // SAFETY: The cell holds its text field weakly; the field is its subview and lives as long.
    unsafe { cell.setTextField(Some(&name)) };
    if !detail.is_empty() {
        let more = label(mtm, detail);
        more.setTextColor(Some(&NSColor::secondaryLabelColor()));
        more.setLineBreakMode(objc2_app_kit::NSLineBreakMode::ByTruncatingMiddle);
        more.setContentCompressionResistancePriority_forOrientation(200.0, NSLayoutConstraintOrientation::Horizontal);
        more.setContentHuggingPriority_forOrientation(750.0, NSLayoutConstraintOrientation::Horizontal);
        cell.addSubview(&more);
        more.setTranslatesAutoresizingMaskIntoConstraints(false);
        for constraint in [
            more.leadingAnchor()
                .constraintEqualToAnchor_constant(&name.trailingAnchor(), 5.0),
            more.firstBaselineAnchor()
                .constraintEqualToAnchor(&name.firstBaselineAnchor()),
            more.trailingAnchor()
                .constraintLessThanOrEqualToAnchor_constant(&cell.trailingAnchor(), -4.0),
        ] {
            constraint.setActive(true);
        }
    }
    cell.into_super()
}

/// The height of a session's row, its name above how it stands.
pub const SESSION_HEIGHT: f64 = 36.0;

/// A row with `title` above `detail`, after a symbol, for a session whose state may be a long sentence.
pub fn two_lines(mtm: MainThreadMarker, symbol: (&str, &NSColor), title: &str, detail: &str) -> Retained<NSView> {
    let cell = objc2_app_kit::NSTableCellView::new(mtm);
    let icon = objc2_app_kit::NSImageView::new(mtm);
    icon.setImage(ide_ui_common::util::symbol(symbol.0, "").as_deref());
    icon.setContentTintColor(Some(symbol.1));
    ide_ui_common::util::hide_from_voiceover(&icon);
    let name = label(mtm, title);
    name.setLineBreakMode(objc2_app_kit::NSLineBreakMode::ByTruncatingTail);
    let more = label(mtm, detail);
    more.setTextColor(Some(&NSColor::secondaryLabelColor()));
    more.setFont(Some(&ide_ui_common::style::small_system()));
    more.setLineBreakMode(objc2_app_kit::NSLineBreakMode::ByTruncatingTail);
    for view in [&*icon as &NSView, &name, &more] {
        cell.addSubview(view);
        view.setTranslatesAutoresizingMaskIntoConstraints(false);
    }
    for field in [&name, &more] {
        field.setContentCompressionResistancePriority_forOrientation(250.0, NSLayoutConstraintOrientation::Horizontal);
    }
    for constraint in [
        icon.leadingAnchor().constraintEqualToAnchor(&cell.leadingAnchor()),
        icon.centerYAnchor().constraintEqualToAnchor(&cell.centerYAnchor()),
        icon.widthAnchor().constraintEqualToConstant(16.0),
        name.leadingAnchor()
            .constraintEqualToAnchor_constant(&icon.trailingAnchor(), 4.0),
        name.trailingAnchor()
            .constraintLessThanOrEqualToAnchor_constant(&cell.trailingAnchor(), -4.0),
        name.bottomAnchor().constraintEqualToAnchor(&cell.centerYAnchor()),
        more.leadingAnchor().constraintEqualToAnchor(&name.leadingAnchor()),
        more.trailingAnchor()
            .constraintLessThanOrEqualToAnchor_constant(&cell.trailingAnchor(), -4.0),
        more.topAnchor().constraintEqualToAnchor(&cell.centerYAnchor()),
    ] {
        constraint.setActive(true);
    }
    // SAFETY: The cell holds its text field and image view weakly; both are its subviews and live as long.
    unsafe {
        cell.setTextField(Some(&name));
        cell.setImageView(Some(&icon));
    }
    cell.into_super()
}

/// The height of a row that says why a list is empty, in two lines.
pub const NOTE_HEIGHT: f64 = 40.0;

/// A row that says why a list is empty, wrapping onto a second line.
pub fn note(mtm: MainThreadMarker, text: &str) -> Retained<NSView> {
    let cell = objc2_app_kit::NSTableCellView::new(mtm);
    let field = NSTextField::wrappingLabelWithString(&ns(text), mtm);
    field.setTextColor(Some(&NSColor::secondaryLabelColor()));
    field.setFont(Some(&ide_ui_common::style::small_system()));
    field.setContentCompressionResistancePriority_forOrientation(250.0, NSLayoutConstraintOrientation::Horizontal);
    cell.addSubview(&field);
    field.setTranslatesAutoresizingMaskIntoConstraints(false);
    for constraint in [
        field.leadingAnchor().constraintEqualToAnchor(&cell.leadingAnchor()),
        field
            .trailingAnchor()
            .constraintEqualToAnchor_constant(&cell.trailingAnchor(), -4.0),
        field.centerYAnchor().constraintEqualToAnchor(&cell.centerYAnchor()),
    ] {
        constraint.setActive(true);
    }
    // SAFETY: The cell holds its text field weakly; the field is its subview and lives as long.
    unsafe { cell.setTextField(Some(&field)) };
    cell.into_super()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_shortcut_reads_as_macos_writes_it() {
        assert_eq!(key_symbols("ctrl+cmd+y"), "⌃⌘Y");
        assert_eq!(key_symbols("shift+alt+f11"), "⌥⇧F11");
        assert_eq!(key_symbols("cmd++"), "⌘+");
        assert_eq!(key_symbols(""), "");
    }

    #[test]
    fn every_command_has_a_symbol_and_a_name_of_its_own_and_the_steps_a_shortcut() {
        let all = [
            Command::Continue,
            Command::StepOver,
            Command::StepInto,
            Command::StepOut,
            Command::Restart,
            Command::Stop,
            Command::ToggleBreakpoints,
            Command::ClearBreakpoints,
        ];
        let looks: Vec<(&str, &str)> = all.iter().map(|c| control_look(*c)).collect();
        for (n, look) in looks.iter().enumerate() {
            assert!(
                looks[n + 1..]
                    .iter()
                    .all(|other| other.0 != look.0 && other.1 != look.1),
                "{look:?}"
            );
        }
        let with_shortcut: Vec<Command> = all.into_iter().filter(|c| shortcut_of(*c).is_some()).collect();
        assert_eq!(
            with_shortcut,
            [
                Command::Continue,
                Command::StepOver,
                Command::StepInto,
                Command::StepOut,
                Command::Stop
            ]
        );
    }
}

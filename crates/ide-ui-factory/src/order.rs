//! The tab of one order (E175): its condition, the next step with the commands beside it and every command in a
//! pull-down; the specification with inline comments while it waits for G1 (E178); the request that waits for the user as
//! a card it answers without the AI tab (F72); its goal and criteria;
//! the timeline from the log; the evidence with the deviations first; and the diff's sections, each of which G2 can
//! reject (C55).

use std::cell::{Cell, OnceCell, RefCell};
use std::collections::BTreeSet;

use ide_factory::{HunkId, InputKind, OrderId};
use ide_ui_common::util::{guarded, ns};
use ide_ui_common::window::{TabDecor, TabId, TabKind, Tabs};
use objc2::rc::Retained;
use objc2::runtime::AnyObject;
use objc2::{DefinedClass, MainThreadMarker, MainThreadOnly, define_class, msg_send, sel};
use objc2_app_kit::{
    NSBox, NSButton, NSColor, NSControlStateValueOff, NSControlStateValueOn, NSLayoutConstraintOrientation, NSMenuItem,
    NSPopUpButton, NSStackView, NSStackViewVisibilityPriorityDetachOnlyIfNecessary,
    NSStackViewVisibilityPriorityMustHold, NSTextField, NSView,
};
use objc2_foundation::{NSObjectProtocol, NSPoint, NSRect, NSSize};
use splat_i18n::{t, tf};

use crate::actions::Target;
use crate::host::{SpecAction, host};
use crate::model::detail::{AcceptBlock, Command, Detail};
use crate::ui;
use ide_ui_common::style::space;

/// How wide the tab's text wraps before the tab knows its width.
const WIDTH: f64 = 640.0;

/// How tall the specification's view is at G1: its text and comments scroll inside it.
const SPEC_HEIGHT: f64 = 460.0;

/// The key of the tab of the order `id`, which the window finds it by and the layout keeps (E64).
pub(crate) fn tab_key(id: &OrderId) -> String {
    format!("order:{id}")
}

/// The order of the tab key `key`.
pub(crate) fn order_of_key(key: &str) -> Option<OrderId> {
    key.strip_prefix("order:").map(|id| OrderId(id.to_string()))
}

pub(crate) struct OrderIvars {
    target: Target,
    order: OrderId,
    tab: Cell<Option<TabId>>,
    detail: RefCell<Option<Detail>>,
    /// Whether the views show `detail`, which a step that leaves the order as it was does not build again.
    shown: Cell<bool>,
    /// The sections the user turned down for G2.
    rejected: RefCell<BTreeSet<HunkId>>,
    /// The specification the G1 view shows, which a new one replaces.
    spec: RefCell<Option<String>>,
    /// The width the labels wrap at, from the tab's width.
    wrap: Cell<f64>,
    ui: OnceCell<OrderUi>,
}

struct OrderUi {
    title: Retained<NSTextField>,
    condition: Retained<NSStackView>,
    /// The next step and the commands beside it, from `buttons`, then `more`.
    bar: Retained<NSStackView>,
    /// A button per command, in the order of `Command::ALL`.
    buttons: Vec<Retained<NSButton>>,
    more: Retained<NSPopUpButton>,
    /// The pull-down's item per command, in the order of `Command::ALL`.
    items: Vec<Retained<NSMenuItem>>,
    accept_note: Retained<NSTextField>,
    /// Holds the specification's view at G1.
    spec: Retained<NSView>,
    request: Retained<NSBox>,
    answer: Retained<NSTextField>,
    body: Retained<NSStackView>,
}

define_class!(
    // SAFETY: NSView has no subclassing requirements and `OrderTab` does not implement Drop.
    #[unsafe(super(NSView))]
    #[thread_kind = MainThreadOnly]
    #[name = "IDEFactoryOrderTab"]
    #[ivars = OrderIvars]
    pub(crate) struct OrderTab;

    unsafe impl NSObjectProtocol for OrderTab {}

    impl OrderTab {
        #[unsafe(method(ideFactoryOrderCommand:))]
        fn command_action(&self, sender: Option<&AnyObject>) {
            guarded((), || {
                if let Some(command) = sender.and_then(command_of) {
                    self.run(command);
                }
            });
        }

        #[unsafe(method(setFrameSize:))]
        fn set_frame_size(&self, size: NSSize) {
            // SAFETY: The overridden method with arguments of its types, which objc2 checks in debug builds.
            let _: () = unsafe { msg_send![super(self), setFrameSize: size] };
            guarded((), || self.width_changed(size.width));
        }

        #[unsafe(method(ideFactoryAnswer:))]
        fn answer_action(&self, _sender: Option<&AnyObject>) {
            guarded((), || self.answer());
        }

        #[unsafe(method(ideFactoryAllow:))]
        fn allow_action(&self, _sender: Option<&AnyObject>) {
            guarded((), || self.decide(true));
        }

        #[unsafe(method(ideFactoryRefuse:))]
        fn refuse_action(&self, _sender: Option<&AnyObject>) {
            guarded((), || self.decide(false));
        }

        #[unsafe(method(ideFactoryTakeIssue:))]
        fn take_issue_action(&self, _sender: Option<&AnyObject>) {
            guarded((), || self.take_issue());
        }

        #[unsafe(method(ideFactorySection:))]
        fn section_action(&self, sender: Option<&AnyObject>) {
            guarded((), || {
                if let Some(button) = sender.and_then(|s| s.downcast_ref::<NSButton>()) {
                    self.section_switched(button);
                }
            });
        }
    }
);

/// Opens the tab of the order `id` in `target`'s window, or brings it to the front; whether it shows.
pub(crate) fn open(target: &Target, id: &OrderId) -> bool {
    host().check_issue(target.root(), id);
    let window = target.window();
    let key = tab_key(id);
    if window.select_tab(&key) {
        return true;
    }
    let Some(state) = target.state() else { return false };
    let tab = OrderTab::new(window.mtm(), target.clone(), id.clone());
    let title = host()
        .order(target.root(), id)
        .map_or_else(|| id.to_string(), |d| d.title().to_string());
    let Some(tab_id) = window.add_tab(TabKind::Order, Some(key), &title, Retained::into_super(tab.clone())) else {
        return false;
    };
    tab.ivars().tab.set(Some(tab_id));
    let number = state.orders.next_id();
    state.orders.insert(number, objc2::rc::Weak::from_retained(&tab));
    tab.reload();
    true
}

impl OrderTab {
    fn new(mtm: MainThreadMarker, target: Target, order: OrderId) -> Retained<OrderTab> {
        let ivars = OrderIvars {
            target,
            order,
            tab: Cell::new(None),
            detail: RefCell::new(None),
            shown: Cell::new(false),
            rejected: RefCell::new(BTreeSet::new()),
            spec: RefCell::new(None),
            wrap: Cell::new(WIDTH),
            ui: OnceCell::new(),
        };
        // SAFETY: NSView's initializer, called once on the newly allocated object.
        let tab: Retained<OrderTab> = unsafe { msg_send![super(OrderTab::alloc(mtm).set_ivars(ivars)), init] };
        let _ = tab.ivars().ui.set(tab.build(mtm));
        tab
    }

    fn build(&self, mtm: MainThreadMarker) -> OrderUi {
        let me: &AnyObject = self.as_ref();
        let title = ui::title(mtm, "");
        title.setContentCompressionResistancePriority_forOrientation(
            ui::GIVES_WAY,
            NSLayoutConstraintOrientation::Horizontal,
        );
        let condition = ui::row(mtm, space::S);
        let buttons: Vec<Retained<NSButton>> = Command::ALL
            .iter()
            .map(|command| {
                let button = ui::button(mtm, command.title(), me, sel!(ideFactoryOrderCommand:), command.tag());
                if let Some(tip) = tooltip(*command) {
                    button.setToolTip(Some(&ns(tip)));
                }
                button
            })
            .collect();
        let (more, items) = more_menu(mtm, me);
        let bar = ui::row(mtm, space::M);
        let accept_note = ui::note(mtm, "", WIDTH);
        let spec = NSView::new(mtm);
        spec.setHidden(true);
        let (request, answer) = request_card(mtm, me);
        let body = ui::column(mtm, space::M);
        let column = ui::column(mtm, space::M);
        column.setEdgeInsets(objc2_foundation::NSEdgeInsets {
            top: space::M,
            left: space::GROUP,
            bottom: space::GROUP,
            right: space::GROUP,
        });
        for view in [
            &*title as &NSView,
            &condition,
            &bar,
            &accept_note,
            &spec,
            &request,
            &body,
        ] {
            column.addArrangedSubview(view);
        }
        column.setCustomSpacing_afterView(space::XS, &title);
        column.setCustomSpacing_afterView(space::XL, &condition);
        let inner = -2.0 * space::GROUP;
        ide_ui_common::util::activate([
            spec.heightAnchor().constraintEqualToConstant(SPEC_HEIGHT),
            spec.widthAnchor()
                .constraintEqualToAnchor_constant(&column.widthAnchor(), inner),
            request
                .widthAnchor()
                .constraintEqualToAnchor_constant(&column.widthAnchor(), inner),
            body.widthAnchor()
                .constraintEqualToAnchor_constant(&column.widthAnchor(), inner),
        ]);
        let scroll = ui::scrolling(mtm, &column);
        self.addSubview(&scroll);
        ide_ui_common::util::auto_layout(&[&scroll]);
        ide_ui_common::util::pin_edges(&scroll, self);
        OrderUi {
            title,
            condition,
            bar,
            buttons,
            more,
            items,
            accept_note,
            spec,
            request,
            answer,
            body,
        }
    }

    fn ui(&self) -> &OrderUi {
        self.ivars().ui.get().expect("the tab builds its views in `new`")
    }

    /// The order it shows.
    pub(crate) fn order(&self) -> &OrderId {
        &self.ivars().order
    }

    /// Its tab.
    pub(crate) fn tab(&self) -> Option<TabId> {
        self.ivars().tab.get()
    }

    /// The folder of its window.
    pub(crate) fn root(&self) -> &std::path::Path {
        self.ivars().target.root()
    }

    /// The order's model as the tab shows it.
    pub(crate) fn detail(&self) -> Option<Detail> {
        self.ivars().detail.borrow().clone()
    }

    /// Reads the order again and shows it.
    pub(crate) fn reload(&self) {
        let detail = host().order(self.root(), self.order());
        if let Some(detail) = &detail {
            let live: BTreeSet<HunkId> = detail.sections().iter().map(|s| s.id().clone()).collect();
            self.ivars().rejected.borrow_mut().retain(|id| live.contains(id));
        }
        if self.ivars().shown.replace(true) && *self.ivars().detail.borrow() == detail {
            return;
        }
        *self.ivars().detail.borrow_mut() = detail.clone();
        self.show(detail.as_ref());
    }

    fn show(&self, detail: Option<&Detail>) {
        let mtm = self.mtm();
        let ui = self.ui();
        let window = self.ivars().target.window();
        ui::clear(&ui.condition);
        ui::clear(&ui.body);
        let Some(detail) = detail else {
            ui.title.setStringValue(&ns(&self.order().to_string()));
            ui.body
                .addArrangedSubview(&ui::note(mtm, t("Diesen Auftrag gibt es nicht mehr."), WIDTH));
            self.show_commands(None);
            ui.request.setHidden(true);
            ui.accept_note.setHidden(true);
            self.show_spec(None);
            return;
        };
        ui.title.setStringValue(&ns(detail.title()));
        ui.condition.addArrangedSubview(&ui::tone_image(mtm, detail.tone()));
        ui.condition.addArrangedSubview(&ui::text(
            mtm,
            &tf(
                "{} – {}",
                &[crate::model::words::station(detail.station()), detail.condition()],
            ),
            WIDTH,
        ));
        ui.condition.addArrangedSubview(&ui::note(mtm, detail.risk(), WIDTH));
        self.show_commands(Some(detail));
        let accept = match detail.accept_block() {
            Some(AcceptBlock::OpenDiffFirst) => Some(t(
                "Bei hoher Risikoklasse geht „Abnehmen“ erst, wenn der Diff geöffnet wurde.",
            )),
            Some(AcceptBlock::NoValidEvidence) => Some(t("Abnehmen geht erst mit einem gültigen Nachweis.")),
            None => None,
        };
        let notes: Vec<String> = accept
            .map(str::to_string)
            .into_iter()
            .chain(detail.publish_block())
            .collect();
        ui.accept_note.setStringValue(&ns(&notes.join("\n")));
        ui.accept_note.setHidden(notes.is_empty());
        ui.request.setHidden(detail.request().is_none());
        self.show_spec(Some(detail));
        crate::order_view::fill(self, &ui.body, detail, &self.ivars().rejected.borrow());
        self.rewrap();
        if let Some(tab) = self.tab() {
            window.set_tab_title(tab, detail.title());
            let red = NSColor::systemRedColor();
            let waits = detail.tone() == crate::model::words::Tone::Waiting;
            window.set_tab_status(tab, waits.then_some(&*red));
        }
    }

    /// Puts the next step and the commands beside it into the bar and greys out the pull-down's commands that do not
    /// fit (E101).
    fn show_commands(&self, detail: Option<&Detail>) {
        let ui = self.ui();
        ui::clear(&ui.bar);
        let primary = detail.and_then(Detail::primary);
        let shown: Vec<Command> = primary
            .into_iter()
            .chain(detail.map(Detail::companions).unwrap_or_default())
            .collect();
        for (index, command) in shown.iter().enumerate() {
            let Some(button) = ui.buttons.get(usize::try_from(command.tag()).unwrap_or(usize::MAX)) else {
                continue;
            };
            button.setEnabled(true);
            let accent = (Some(*command) == primary).then(NSColor::controlAccentColor);
            button.setBezelColor(accent.as_deref());
            ui.bar.addArrangedSubview(button);
            let keep = if index == 0 {
                NSStackViewVisibilityPriorityMustHold
            } else {
                NSStackViewVisibilityPriorityDetachOnlyIfNecessary - f32::from(u8::try_from(index).unwrap_or(0))
            };
            ui.bar.setVisibilityPriority_forView(keep, button);
        }
        ui.bar.addArrangedSubview(&ui.more);
        for item in &ui.items {
            let enabled = Command::of_tag(item.tag()).is_some_and(|c| detail.is_some_and(|d| d.can(c)));
            item.setEnabled(enabled);
        }
        ui.more.setEnabled(detail.is_some());
    }

    /// Wraps the tab's labels at the width its column has now.
    fn width_changed(&self, width: f64) {
        if width < 1.0 {
            return;
        }
        let wrap = width - 2.0 * space::GROUP;
        if (wrap - self.ivars().wrap.replace(wrap)).abs() < 0.5 {
            return;
        }
        if self.ivars().ui.get().is_some() {
            self.rewrap();
        }
    }

    fn rewrap(&self) {
        let ui = self.ui();
        let width = self.ivars().wrap.get();
        for view in [&*ui.condition as &NSView, &ui.accept_note, &ui.request, &ui.body] {
            ui::wrap(view, width);
        }
    }

    /// Shows the specification while it waits for G1, built again only when a new one replaces it.
    fn show_spec(&self, detail: Option<&Detail>) {
        let wanted = detail.and_then(Detail::spec).map(str::to_string);
        if *self.ivars().spec.borrow() == wanted {
            return;
        }
        let holder = &self.ui().spec;
        for view in holder.subviews().iter() {
            view.removeFromSuperview();
        }
        holder.setHidden(true);
        *self.ivars().spec.borrow_mut() = None;
        let (Some(detail), Some(_)) = (detail, &wanted) else {
            return;
        };
        let target = self.ivars().target.clone();
        let id = detail.id().clone();
        let then = std::rc::Rc::new(move |action: SpecAction| match action {
            SpecAction::Approve => crate::actions::run_on(&target, &id, Command::Approve),
            SpecAction::Comment(text) => crate::actions::send(&target, InputKind::CommentSpec(id.clone(), text)),
            SpecAction::Discard => crate::actions::run_on(&target, &id, Command::Abort),
        });
        let window = self.ivars().target.window().id();
        let Some(view) = host().spec_view(window, self.root(), detail.id(), then) else {
            return;
        };
        holder.addSubview(&view);
        ide_ui_common::util::auto_layout(&[&view]);
        ide_ui_common::util::pin_edges(&view, holder);
        holder.setHidden(false);
        *self.ivars().spec.borrow_mut() = wanted;
    }

    fn run(&self, command: Command) {
        let Some(detail) = self.detail() else { return };
        let rejected = self.ivars().rejected.borrow().clone();
        crate::actions::run(&self.ivars().target, &detail, command, &rejected);
    }

    fn answer(&self) {
        let Some(detail) = self.detail() else { return };
        let Some(request) = detail.request().cloned() else {
            return;
        };
        let text = self.ui().answer.stringValue().to_string().trim().to_string();
        if text.is_empty() {
            return;
        }
        self.ui().answer.setStringValue(&ns(""));
        crate::actions::send(
            &self.ivars().target,
            InputKind::Answer {
                order: detail.id().clone(),
                request,
                text,
            },
        );
    }

    /// Takes the change of the order's issue over, on the user's word (E176).
    fn take_issue(&self) {
        let Some(input) = self.detail().and_then(|d| d.take_issue_change()) else {
            return;
        };
        crate::actions::send(&self.ivars().target, input);
    }

    fn decide(&self, allow: bool) {
        let Some(detail) = self.detail() else { return };
        let Some(request) = detail.request().cloned() else {
            return;
        };
        let target = &self.ivars().target;
        let refused = crate::host::host().decide(target.root(), detail.id(), &request, allow);
        target.refresh();
        crate::actions::refusals(target, &refused);
    }

    fn section_switched(&self, button: &NSButton) {
        let Some(detail) = self.detail() else { return };
        let Some(section) = usize::try_from(button.tag())
            .ok()
            .and_then(|i| detail.sections().get(i))
        else {
            return;
        };
        let mut rejected = self.ivars().rejected.borrow_mut();
        if button.state() == NSControlStateValueOn {
            rejected.remove(section.id());
        } else {
            rejected.insert(section.id().clone());
        }
    }

    /// The target the section checkboxes and buttons send to.
    pub(crate) fn me(&self) -> &AnyObject {
        self.as_ref()
    }

    /// Whether the section `id` is turned down.
    pub(crate) fn state_of(rejected: &BTreeSet<HunkId>, id: &HunkId) -> isize {
        if rejected.contains(id) {
            NSControlStateValueOff
        } else {
            NSControlStateValueOn
        }
    }
}

/// What a command's button says on hover where its title cannot say what leaves the Mac.
fn tooltip(command: Command) -> Option<&'static str> {
    match command {
        Command::MergeRequest => Some(t(
            "Öffnet die Seite für einen neuen Merge Request im Browser-Tab. Den Branch vorher selbst pushen; die App schreibt nichts zu GitHub oder GitLab.",
        )),
        Command::AcceptAsChange | Command::Publish => Some(t(
            "Pusht den Branch mit deinen Zugangsdaten, legt einen Merge Request mit einer Zusammenfassung des Nachweises an und kommentiert das Issue des Auftrags. Vorher prüft gitleaks alles, was hinausgeht.",
        )),
        _ => None,
    }
}

/// The card of a request that waits for the user, with the field of the answer and "Antwort senden".
fn request_card(mtm: MainThreadMarker, me: &AnyObject) -> (Retained<NSBox>, Retained<NSTextField>) {
    let card = NSBox::new(mtm);
    card.setTitle(&ns(t("Anfrage des Agenten")));
    let column = ui::column(mtm, space::S);
    column.addArrangedSubview(&ui::note(
        mtm,
        t("Der Auftrag wartet auf deine Antwort oder Freigabe. Frage und Befehl stehen in der Sitzung des Agenten."),
        WIDTH,
    ));
    let answer = crate::actions::line_field(mtm, "", t("Antwort"));
    answer
        .widthAnchor()
        .constraintGreaterThanOrEqualToConstant(160.0)
        .setActive(true);
    answer.setContentHuggingPriority_forOrientation(1.0, NSLayoutConstraintOrientation::Horizontal);
    let send = ui::button(mtm, t("Antwort senden"), me, sel!(ideFactoryAnswer:), 0);
    let line = ui::row(mtm, space::M);
    line.addArrangedSubview(&answer);
    line.addArrangedSubview(&send);
    column.addArrangedSubview(&line);
    let permission = ui::row(mtm, space::M);
    permission.addArrangedSubview(&ui::button(mtm, t("Erlauben"), me, sel!(ideFactoryAllow:), 0));
    permission.addArrangedSubview(&ui::button(mtm, t("Ablehnen"), me, sel!(ideFactoryRefuse:), 0));
    column.addArrangedSubview(&permission);
    card.setContentView(Some(&column));
    card.setHidden(true);
    (card, answer)
}

/// "Weitere Befehle": a pull-down with every command in the order of [`Command::ALL`], the item per command beside.
fn more_menu(mtm: MainThreadMarker, me: &AnyObject) -> (Retained<NSPopUpButton>, Vec<Retained<NSMenuItem>>) {
    let more = NSPopUpButton::initWithFrame_pullsDown(
        NSPopUpButton::alloc(mtm),
        NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(44.0, 24.0)),
        true,
    );
    more.setAutoenablesItems(false);
    let heading = ide_ui_common::util::item(mtm, "", None, "");
    heading.setTag(-1);
    heading.setImage(ide_ui_common::util::symbol("ellipsis.circle", t("Weitere Befehle")).as_deref());
    let menu = more.menu().expect("a pop-up button has a menu");
    menu.addItem(&heading);
    let mut items = Vec::new();
    for command in Command::ALL {
        if matches!(
            command,
            Command::OpenDiff | Command::Publish | Command::BackToBuilder | Command::Abort
        ) {
            menu.addItem(&ide_ui_common::util::separator(mtm));
        }
        let item = ide_ui_common::util::tagged(mtm, command.title(), sel!(ideFactoryOrderCommand:), "", command.tag());
        // SAFETY: The tab answers the action and holds the menu, so it outlives every item.
        unsafe { item.setTarget(Some(me)) };
        if let Some(tip) = tooltip(command) {
            item.setToolTip(Some(&ns(tip)));
        }
        menu.addItem(&item);
        items.push(item);
    }
    more.setToolTip(Some(&ns(t("Weitere Befehle"))));
    ide_ui_common::util::set_label(&more, Some(t("Weitere Befehle")));
    (more, items)
}

/// The command a button, a menu item or the pull-down that sent an action stands for.
fn command_of(sender: &AnyObject) -> Option<Command> {
    let tag = if let Some(popup) = sender.downcast_ref::<NSPopUpButton>() {
        popup.selectedItem()?.tag()
    } else if let Some(item) = sender.downcast_ref::<NSMenuItem>() {
        item.tag()
    } else {
        sender.downcast_ref::<NSButton>()?.tag()
    };
    Command::of_tag(tag)
}

/// The order tabs of the folder `root` in `state`.
pub(crate) fn for_root(state: &crate::host::State, root: &std::path::Path) -> Vec<Retained<OrderTab>> {
    state
        .orders
        .live()
        .into_iter()
        .filter(|tab| tab.root() == root)
        .collect()
}

#[cfg(feature = "selftest")]
mod selftest_probe;

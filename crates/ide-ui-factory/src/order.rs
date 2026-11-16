//! The tab of one order (E175): its condition and buttons G1, G2, back to the builder, change criteria, by hand,
//! abort; the specification with inline comments while it waits for G1 (E178); the request that waits for the user as
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
    NSBox, NSButton, NSColor, NSControlStateValueOff, NSControlStateValueOn, NSStackView, NSTextField, NSView,
};
use objc2_foundation::NSObjectProtocol;
use splat_i18n::{t, tf};

use crate::actions::Target;
use crate::host::{SpecAction, host};
use crate::model::detail::{AcceptBlock, Command, Detail};
use crate::ui;
use ide_ui_common::style::space;

/// How wide the tab's text wraps.
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
    /// The sections the user turned down for G2.
    rejected: RefCell<BTreeSet<HunkId>>,
    /// The specification the G1 view shows, which a new one replaces.
    spec: RefCell<Option<String>>,
    ui: OnceCell<OrderUi>,
}

struct OrderUi {
    title: Retained<NSTextField>,
    condition: Retained<NSStackView>,
    buttons: Vec<Retained<NSButton>>,
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
                let command = sender
                    .and_then(|s| s.downcast_ref::<NSButton>())
                    .and_then(|b| Command::of_tag(b.tag()));
                if let Some(command) = command {
                    self.run(command);
                }
            });
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
            rejected: RefCell::new(BTreeSet::new()),
            spec: RefCell::new(None),
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
        let condition = ui::row(mtm, space::S);
        let primary = ui::row(mtm, space::M);
        let secondary = ui::row(mtm, space::M);
        let mut buttons = Vec::new();
        for command in Command::ALL {
            let button = ui::button(mtm, command.title(), me, sel!(ideFactoryOrderCommand:), command.tag());
            let into = match command {
                Command::Approve
                | Command::Accept
                | Command::OpenDiff
                | Command::OpenSession
                | Command::Continue
                | Command::Resume => &primary,
                _ => &secondary,
            };
            into.addArrangedSubview(&button);
            buttons.push(button);
        }
        let accept_note = ui::note(mtm, "", WIDTH);
        let spec = NSView::new(mtm);
        spec.setHidden(true);
        let (request, answer) = request_card(mtm, me);
        let body = ui::column(mtm, space::M);
        let column = ui::column(mtm, space::M);
        column.setEdgeInsets(objc2_foundation::NSEdgeInsets {
            top: space::GROUP,
            left: space::GROUP,
            bottom: space::GROUP,
            right: space::GROUP,
        });
        for view in [
            &*title as &NSView,
            &condition,
            &primary,
            &spec,
            &secondary,
            &accept_note,
            &request,
            &body,
        ] {
            column.addArrangedSubview(view);
        }
        ide_ui_common::util::activate([
            spec.heightAnchor().constraintEqualToConstant(SPEC_HEIGHT),
            spec.widthAnchor()
                .constraintEqualToAnchor_constant(&column.widthAnchor(), -2.0 * space::GROUP),
        ]);
        let scroll = ui::scrolling(mtm, &column);
        self.addSubview(&scroll);
        ide_ui_common::util::auto_layout(&[&scroll]);
        ide_ui_common::util::pin_edges(&scroll, self);
        OrderUi {
            title,
            condition,
            buttons,
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
            for button in &ui.buttons {
                button.setEnabled(false);
            }
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
        for button in &ui.buttons {
            let enabled = Command::of_tag(button.tag()).is_some_and(|c| detail.can(c));
            button.setEnabled(enabled);
        }
        let note = match detail.accept_block() {
            Some(AcceptBlock::OpenDiffFirst) => Some(t(
                "Bei hoher Risikoklasse geht „Abnehmen“ erst, wenn der Diff geöffnet wurde.",
            )),
            Some(AcceptBlock::NoValidEvidence) => Some(t("Abnehmen geht erst mit einem gültigen Nachweis.")),
            None => None,
        };
        ui.accept_note.setStringValue(&ns(note.unwrap_or("")));
        ui.accept_note.setHidden(note.is_none());
        ui.request.setHidden(detail.request().is_none());
        self.show_spec(Some(detail));
        crate::order_view::fill(self, &ui.body, detail, &self.ivars().rejected.borrow());
        if let Some(tab) = self.tab() {
            window.set_tab_title(tab, detail.title());
            let red = NSColor::systemRedColor();
            let waits = detail.tone() == crate::model::words::Tone::Waiting;
            window.set_tab_status(tab, waits.then_some(&*red));
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
        .constraintGreaterThanOrEqualToConstant(320.0)
        .setActive(true);
    let send = ui::button(mtm, t("Antwort senden"), me, sel!(ideFactoryAnswer:), 0);
    let open = ui::button(
        mtm,
        Command::OpenSession.title(),
        me,
        sel!(ideFactoryOrderCommand:),
        Command::OpenSession.tag(),
    );
    let line = ui::row(mtm, space::M);
    line.addArrangedSubview(&answer);
    line.addArrangedSubview(&send);
    line.addArrangedSubview(&open);
    column.addArrangedSubview(&line);
    let permission = ui::row(mtm, space::M);
    permission.addArrangedSubview(&ui::button(mtm, t("Erlauben"), me, sel!(ideFactoryAllow:), 0));
    permission.addArrangedSubview(&ui::button(mtm, t("Ablehnen"), me, sel!(ideFactoryRefuse:), 0));
    column.addArrangedSubview(&permission);
    card.setContentView(Some(&column));
    card.setHidden(true);
    (card, answer)
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

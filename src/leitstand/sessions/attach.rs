//! The AI tab on an order's session (B37b, E184): the session of the order's current role opens as an AI tab that
//! follows its feed in the shared registry; what the user does there reaches the order's factory as an intervention,
//! a model switch or an answer, each on the next turn of the main thread, so no factory step runs inside the tab's
//! own call. Closing the tab ends its feed, never the session.

use std::path::Path;
use std::rc::Rc;

use ide_factory::{InputKind, OrderId, RequestId, SessionEvent};
use ide_session::{Driver, PathKind, SessionId};
use ide_ui_common::window::WindowId;
use splat_i18n::tf;

use super::Sessions;
use crate::context::AppContext;
use crate::leitstand::{Back, Leitstand};

#[cfg(feature = "selftest")]
mod selftest_probe;

/// An order's session as its AI tab shows it.
struct Found {
    id: SessionId,
    badge: String,
    title: String,
}

impl Sessions {
    fn driver_of(&mut self, id: SessionId) -> Option<&mut Driver> {
        self.registry.as_mut()?.get_mut(id)
    }

    /// Session `id` as its AI tab shows it, while it runs on a path the tab follows: Claude, and the scripted
    /// session of the self-test.
    fn found(&self, id: SessionId) -> Option<Found> {
        let link = self.links.get(&id)?;
        let driver = self.registry.as_ref()?.get(id)?;
        if driver.is_ended() || !matches!(driver.kind(), PathKind::Claude | PathKind::Scripted) {
            return None;
        }
        Some(Found {
            id,
            badge: tf(
                "Auftrag {} · {}",
                &[&link.title, ide_ui_factory::model::words::role(link.role)],
            ),
            title: link.title.clone(),
        })
    }
}

impl Leitstand {
    /// The running session of the current role of `order` in `root`, on a path the AI tab follows.
    fn order_session(&self, root: &Path, order: &OrderId) -> Option<Found> {
        let key = self
            .with_order(root, order, |o| o.current().map(|slot| slot.session().clone()))
            .flatten()?;
        let sessions = self.sessions.borrow();
        sessions.find(&key).and_then(|id| sessions.found(id))
    }

    /// What `f` gives of the driver of session `id`, while the registry holds it.
    fn with_session<R>(&self, id: SessionId, f: impl FnOnce(&mut Driver) -> R) -> Option<R> {
        self.sessions.borrow_mut().driver_of(id).map(f)
    }

    /// Hands the factory of session `id`'s order `event` of that session.
    fn session_event(&self, cx: &Rc<AppContext>, id: SessionId, event: SessionEvent) {
        let target = {
            let sessions = self.sessions.borrow();
            let Some(link) = sessions.links.get(&id) else { return };
            if let SessionEvent::Answered { request } = &event
                && !link.requests.contains_key(&request.0)
            {
                return;
            }
            (link.root.clone(), link.order.clone(), link.key.clone())
        };
        let (root, order, key) = target;
        self.input(cx, &root, InputKind::Session(order, key, event));
    }

    /// The AI tabs on session `id` take what its driver published for them, after its flush; on a session the
    /// registry let go they show it ended.
    pub(in crate::leitstand) fn views_fed(&self, cx: &Rc<AppContext>, id: SessionId) {
        ide_ui_agent::session_fed(cx.agent(), id);
    }
}

impl Leitstand {
    /// Opens the session of the current role of `order` in `root` as an AI tab of `window`, or brings it to the
    /// front; false when the order has no running session the AI tab can follow.
    pub(crate) fn open_tab(&self, cx: &Rc<AppContext>, window: WindowId, root: &Path, order: &OrderId) -> bool {
        let Some(controller) = cx.windows().get(window.get()) else {
            return false;
        };
        let Some(found) = self.order_session(root, order) else {
            return false;
        };
        self.attach_tab(cx, &controller, found)
    }

    fn attach_tab(&self, cx: &Rc<AppContext>, controller: &crate::window::WindowController, found: Found) -> bool {
        let attach = ide_ui_agent::Attach {
            session: found.id,
            badge: found.badge,
            title: found.title,
            sessions: Rc::new(OrderSessions(self.back())),
        };
        ide_ui_agent::attach(controller, cx.agent(), attach).is_some()
    }
}

/// The registry as the AI tabs on orders' sessions reach it, through the way back to the app context (E143).
struct OrderSessions(Back);

impl OrderSessions {
    /// Hands `event` of `session` to its order's factory on the next turn of the main thread.
    fn later(&self, session: SessionId, event: SessionEvent) {
        let back = self.0.clone();
        crate::util::main_soon(move || {
            back.with(|cx| cx.leitstand().session_event(cx, session, event));
        });
    }
}

impl ide_ui_agent::Sessions for OrderSessions {
    fn with_driver(&self, session: SessionId, f: &mut dyn FnMut(&mut Driver)) -> bool {
        (self.0)
            .with(|cx| cx.leitstand().with_session(session, |driver| f(driver)))
            .flatten()
            .is_some()
    }

    fn intervened(&self, session: SessionId) {
        self.later(session, SessionEvent::Intervened);
    }

    fn model_switched(&self, session: SessionId, model: &str) {
        let model = model.to_string();
        self.later(session, SessionEvent::ModelSwitched { model });
    }

    fn answered(&self, session: SessionId, request: &str) {
        let request = RequestId(request.to_string());
        self.later(session, SessionEvent::Answered { request });
    }
}

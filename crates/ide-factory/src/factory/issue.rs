//! Later changes of an order's issue (E176): the app reads the issue again, the order notes how it reads now, and only
//! the user's word takes the change over, as a change of goal and criteria that voids the evidence (E184).

use super::{Cx, Factory};
use crate::effect::Effect;
use crate::event::{Event, Phase};
use crate::ids::OrderId;
use crate::intake::{IssueOrigin, spec_of, updated_criteria};
use crate::order::Order;
use crate::prompt::untrusted;
use crate::station::{Stage, Station};

impl Factory {
    /// The issue of order `id` reads `body` now; a change from what the order froze waits for the user.
    pub(super) fn issue_read(&mut self, cx: &mut Cx, id: &OrderId, body: String) {
        let Some(order) = self.orders.get(id).filter(|o| !o.finished()) else {
            return;
        };
        let Some(origin) = &order.options().issue else { return };
        let noted = if origin.unchanged(&body) {
            order.issue_update().is_none()
        } else {
            order.issue_update() == Some(body.as_str())
        };
        if !noted {
            self.emit(cx, id, Event::IssueChanged { body });
        }
    }

    /// Takes the change of the issue over: the issue's new text becomes the order's goal and origin, its task list
    /// updates the criteria, and the station that used the old text does its work again.
    pub(super) fn take_issue_change(&mut self, cx: &mut Cx, id: &OrderId) {
        let fits = |o: &Order| o.issue_update().is_some() && o.station() != Station::TakeOver;
        if !self.fitting(cx, id, fits) {
            return;
        }
        let Some(order) = self.orders.get(id) else { return };
        let (Some(origin), Some(body)) = (order.options().issue.clone(), order.issue_update().map(str::to_string))
        else {
            return;
        };
        let criteria = updated_criteria(order.criteria(), &origin.body, &body);
        let changed = criteria != order.criteria();
        let taken = IssueOrigin::freeze(&origin.url, origin.number, origin.author.as_deref(), &body);
        self.emit(cx, id, Event::IssueTaken { origin: taken });
        if changed {
            self.emit(cx, id, Event::CriteriaChanged { criteria });
        }
        match self.orders.get(id).map(Order::station) {
            Some(Station::Spec) => self.respecify(cx, id),
            Some(Station::Intake) | None => {}
            Some(_) => {
                let text = untrusted(&format!("the new text of issue #{}", origin.number), &body);
                let comment = format!(
                    "The order's issue changed and the user took the change over; the goal and the acceptance \
                     criteria are updated. Check your work against them.\n\n{text}"
                );
                self.rework_criteria(cx, id, comment);
            }
        }
    }

    /// The specification of order `id` follows its issue's new text: in stage 1 it is the issue's text again, from
    /// stage 2 on the planner writes it again; either way it waits for G1 anew.
    fn respecify(&mut self, cx: &mut Cx, id: &OrderId) {
        let Some(order) = self.orders.get(id) else { return };
        if order.stage() == Stage::One {
            let Some(origin) = order.options().issue.clone() else {
                return;
            };
            let markdown = spec_of(order.title(), order.goal(), order.criteria(), &origin);
            self.emit(
                cx,
                id,
                Event::Specified {
                    markdown: markdown.clone(),
                    risk: None,
                },
            );
            cx.effect(Effect::WriteSpec {
                order: id.clone(),
                markdown,
            });
            return;
        }
        let role = order.spec_role();
        self.stop_work(cx, id);
        let text = "The order's issue changed; write the specification again for its new text.".to_string();
        self.emit(cx, id, Event::Commented { role, text });
        self.run(cx, id);
        self.set_phase(cx, id, Phase::Idle);
        self.advance(cx, id);
    }
}

//! Who answers what the agent asks: the rule a driver is handed decides each permission request, question and plan
//! as it comes, or leaves it to the user (E165, E173, E189).

use ide_thread::{PermissionRequest, PlanApproval, Question};

use crate::launch::Contract;

/// What the model reads when an order refuses a call; for the model, so not translated.
pub const OUTSIDE_ORDER: &str = "This work order does not allow this call: it reaches beyond the order's worktree, \
build folder or network. Carry on within those bounds, or say in your answer what you could not do.";

/// What the model reads when an order refuses a plan; for the model, so not translated.
pub const NO_PLANS: &str = "A work order approves no plans. Carry on with the order and its acceptance criteria \
without leaving plan mode.";

/// A request of the agent that waits for an answer.
#[derive(Debug, Clone, Copy)]
pub enum Request<'a> {
    /// It asks to use a tool.
    Permission(&'a PermissionRequest),
    /// It asks the user questions.
    Question(&'a Question),
    /// It asks to carry out a plan.
    Plan(&'a PlanApproval),
}

/// What sort of request waits.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RequestKind {
    /// A permission request.
    Permission,
    /// Questions.
    Question,
    /// A plan.
    Plan,
}

impl Request<'_> {
    /// What sort of request it is.
    pub fn kind(&self) -> RequestKind {
        match self {
            Request::Permission(_) => RequestKind::Permission,
            Request::Question(_) => RequestKind::Question,
            Request::Plan(_) => RequestKind::Plan,
        }
    }
}

/// What the rule says to a request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Verdict {
    /// Allow the tool once, approve the plan; for questions the same as `User`.
    Allow,
    /// Refuse it and tell the agent this, where the path carries a reason: a denied tool, a plan sent back, or the
    /// answer to every question.
    Deny(String),
    /// Leave it to the user, who answers through the driver.
    User,
}

/// Decides the requests of one session; it runs on the main thread, as the driver does.
pub trait RequestRule {
    /// What to do about `request`, which the driver hands over once.
    fn decide(&self, request: &Request<'_>) -> Verdict;
}

/// Everything goes to the user, as in an AI tab today.
#[derive(Debug, Clone, Copy, Default)]
pub struct UserRule;

impl RequestRule for UserRule {
    fn decide(&self, _: &Request<'_>) -> Verdict {
        Verdict::User
    }
}

/// The rule of a work order (E173, E189): the path's bounds let through what lies inside them, so a permission
/// request that still comes reaches beyond them and is refused with a reason, or goes to the user without the owner's
/// decision O1 (C72). Questions always wait for the user, plans are refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OrderRule {
    /// Whether requests outside the bounds are refused rather than left to the user.
    pub refuse_outside: bool,
}

impl RequestRule for OrderRule {
    fn decide(&self, request: &Request<'_>) -> Verdict {
        match request {
            Request::Permission(_) if self.refuse_outside => Verdict::Deny(OUTSIDE_ORDER.into()),
            Request::Permission(_) | Request::Question(_) => Verdict::User,
            Request::Plan(_) => Verdict::Deny(NO_PLANS.into()),
        }
    }
}

/// The rule `contract` implies: the user's or the order's.
pub fn for_contract(contract: &Contract) -> Box<dyn RequestRule> {
    match contract {
        Contract::User(_) => Box::new(UserRule),
        Contract::Order(order) => Box::new(OrderRule {
            refuse_outside: order.refuse_outside,
        }),
    }
}

#[cfg(test)]
mod tests {
    use ide_thread::RequestState;
    use serde_json::Value;

    use super::*;

    fn permission() -> PermissionRequest {
        PermissionRequest {
            request_id: "r1".into(),
            tool_call: None,
            tool_name: "Bash".into(),
            title: None,
            description: None,
            reason: None,
            blocked_path: Some("/etc".into()),
            input: Value::Null,
            options: Vec::new(),
            default_to_no: false,
            state: RequestState::Pending,
        }
    }

    #[test]
    fn an_order_refuses_what_lies_outside_asks_the_user_for_questions_and_refuses_plans() {
        let permission = permission();
        let question = Question {
            request_id: Some("q1".into()),
            tool_call: "c1".into(),
            items: Vec::new(),
            state: RequestState::Pending,
        };
        let plan = PlanApproval {
            request_id: Some("p1".into()),
            tool_call: "c2".into(),
            plan: "Do it".into(),
            state: RequestState::Pending,
        };
        let with_o1 = OrderRule { refuse_outside: true };
        assert_eq!(
            with_o1.decide(&Request::Permission(&permission)),
            Verdict::Deny(OUTSIDE_ORDER.into())
        );
        assert_eq!(with_o1.decide(&Request::Question(&question)), Verdict::User);
        assert_eq!(with_o1.decide(&Request::Plan(&plan)), Verdict::Deny(NO_PLANS.into()));
        let without = OrderRule { refuse_outside: false };
        assert_eq!(without.decide(&Request::Permission(&permission)), Verdict::User);
        assert_eq!(without.decide(&Request::Plan(&plan)), Verdict::Deny(NO_PLANS.into()));
        assert_eq!(UserRule.decide(&Request::Plan(&plan)), Verdict::User);
        assert_eq!(Request::Question(&question).kind(), RequestKind::Question);
    }
}

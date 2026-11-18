//! Factories for the models' tests, driven as the app drives them.

use std::collections::VecDeque;

use ide_factory::event::{BaseRef, OpResult};
use ide_factory::options::NewOrder;
use ide_factory::workflow::rules::Rules;
use ide_factory::{Effect, Factory, Input, InputKind, OrderId, Stage, Stamp};

/// A factory whose one order, "CSV export", waits for the user, as nothing is ready to build it.
pub(crate) fn waiting() -> Factory {
    let mut factory = Factory::new(Stage::One, Rules::default());
    let mut wall = 1_700_000_000_000;
    let mut pending = VecDeque::from([InputKind::NewOrder {
        id: OrderId("o1".into()),
        order: NewOrder {
            title: "CSV export".into(),
            goal: "Export the table as CSV.".into(),
            criteria: "- exports CSV\n".into(),
            ..NewOrder::default()
        },
        base: BaseRef {
            branch: "main".into(),
            commit: "c0".into(),
        },
        dirty: false,
    }]);
    while let Some(kind) = pending.pop_front() {
        wall += 1;
        let step = factory.handle(Input::new(Stamp::new(wall, wall), kind));
        for effect in step.effects {
            match effect {
                Effect::PrepareWorktree { order, op, .. } => pending.push_back(InputKind::Done {
                    result: OpResult::Prepared {
                        path: format!("/tmp/{order}"),
                        tree: "tree-base".into(),
                        setup: None,
                        copied: Vec::new(),
                    },
                    order,
                    op,
                }),
                Effect::FreezeConfig { order, .. } => {
                    let frozen = format!("/tmp/frozen-{order}");
                    pending.push_back(InputKind::Frozen(order, Ok(frozen)));
                }
                _ => {}
            }
        }
    }
    factory
}

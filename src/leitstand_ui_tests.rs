//! What the views send, as the runtime takes it: each user command becomes the runtime's, anything else is refused as
//! "not now", and "Weiter" stays itself unless its order waits for changed gate commands.

use ide_factory::options::OptionChange;
use ide_factory::publish::Outgoing;
use ide_factory::{HunkId, RequestId};

use super::*;

fn id() -> OrderId {
    OrderId("o-1".into())
}

fn hunks() -> Vec<HunkId> {
    vec![HunkId("src/lib.rs#1".into())]
}

#[test]
fn every_user_command_of_the_views_becomes_the_runtimes() {
    let texts = Outgoing::new("Title", "Body", "Comment");
    let pairs = [
        (InputKind::Approve(id()), Command::Approve(id())),
        (
            InputKind::CommentSpec(id(), "shorter".into()),
            Command::CommentSpec(id(), "shorter".into()),
        ),
        (InputKind::DiffOpened(id()), Command::DiffOpened(id())),
        (
            InputKind::Accept {
                order: id(),
                fingerprint: "f".into(),
                rejected: hunks(),
            },
            Command::Accept {
                order: id(),
                fingerprint: "f".into(),
                rejected: hunks(),
            },
        ),
        (
            InputKind::KeepBranch {
                order: id(),
                fingerprint: "f".into(),
                rejected: hunks(),
            },
            Command::KeepBranch {
                order: id(),
                fingerprint: "f".into(),
                rejected: hunks(),
            },
        ),
        (
            InputKind::Publish {
                order: id(),
                fingerprint: Some("f".into()),
                rejected: hunks(),
                texts: texts.clone(),
            },
            Command::Publish {
                order: id(),
                fingerprint: Some("f".into()),
                rejected: hunks(),
                texts,
            },
        ),
        (
            InputKind::BackToBuilder(id(), "again".into()),
            Command::BackToBuilder(id(), "again".into()),
        ),
        (
            InputKind::ChangeCriteria(id(), "- one\n".into()),
            Command::ChangeCriteria(id(), "- one\n".into()),
        ),
        (
            InputKind::ChangeOption(id(), OptionChange::Scope(vec!["src".into()])),
            Command::ChangeOption(id(), OptionChange::Scope(vec!["src".into()])),
        ),
        (InputKind::ByHand(id()), Command::ByHand(id())),
        (InputKind::BackToLeitstand(id()), Command::BackToLeitstand(id())),
        (InputKind::Abort(id()), Command::Abort(id())),
        (InputKind::Delete(id()), Command::Delete(id())),
        (InputKind::Resume(Some(id())), Command::Resume(Some(id()))),
        (InputKind::Continue(id()), Command::Continue(id())),
        (
            InputKind::Answer {
                order: id(),
                request: RequestId("r-1".into()),
                text: "yes".into(),
            },
            Command::Answer {
                order: id(),
                request: RequestId("r-1".into()),
                text: "yes".into(),
            },
        ),
        (InputKind::ConfirmGates("h".into()), Command::ConfirmGates("h".into())),
        (InputKind::TakeIssueChange(id()), Command::TakeIssueChange(id())),
    ];
    for (kind, expected) in pairs {
        assert_eq!(command(kind.clone()), Some(expected), "{kind:?}");
    }
}

#[test]
fn what_is_no_user_command_is_refused_as_not_now() {
    assert_eq!(command(InputKind::Tick), None);
    assert_eq!(
        not_now(),
        [Refusal {
            order: None,
            why: Refused::NotNow,
        }]
    );
}

#[test]
fn continue_stays_itself_unless_its_order_waits_for_changed_gate_commands() {
    let leitstand = crate::leitstand::Leitstand::default();
    let root = Path::new("/no/such/folder");
    assert_eq!(
        gates_confirmed(&leitstand, root, Command::Continue(id())),
        Command::Continue(id())
    );
    assert_eq!(
        gates_confirmed(&leitstand, root, Command::Abort(id())),
        Command::Abort(id())
    );
}

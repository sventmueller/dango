use super::*;

#[test]
fn every_command_comes_back_from_its_tag() {
    for command in Command::ALL {
        assert_eq!(Command::of_tag(command.tag()), Some(command));
    }
    assert_eq!(Command::of_tag(-1), None);
}

#[test]
fn the_next_step_is_the_command_that_moves_the_order_on() {
    let only = |allowed: &'static [Command]| move |c: Command| allowed.contains(&c);
    let running = only(&[
        Command::OpenDiff,
        Command::OpenSession,
        Command::ChangeCriteria,
        Command::Abort,
    ]);
    assert_eq!(primary(&running, None), None);
    assert_eq!(
        companions(&running, None, false),
        [Command::OpenDiff, Command::OpenSession]
    );
    let acceptance = only(&[
        Command::Accept,
        Command::AcceptAsChange,
        Command::KeepBranch,
        Command::OpenDiff,
        Command::Abort,
    ]);
    assert_eq!(primary(&acceptance, None), Some(Command::Accept));
    assert_eq!(
        companions(&acceptance, None, false),
        [Command::AcceptAsChange, Command::OpenDiff]
    );
    let unread = only(&[Command::OpenDiff, Command::Abort]);
    assert_eq!(
        primary(&unread, Some(AcceptBlock::OpenDiffFirst)),
        Some(Command::OpenDiff)
    );
    assert!(companions(&unread, Some(AcceptBlock::OpenDiffFirst), false).is_empty());
    let objected = only(&[
        Command::Continue,
        Command::ChangeCriteria,
        Command::OpenSession,
        Command::Abort,
    ]);
    assert_eq!(primary(&objected, None), Some(Command::Continue));
    assert_eq!(
        companions(&objected, None, true),
        [Command::ChangeCriteria, Command::OpenSession]
    );
    assert_eq!(companions(&objected, None, false), [Command::OpenSession]);
    let checkable = only(&[
        Command::Accept,
        Command::OpenReport,
        Command::OpenTerminal,
        Command::OpenDiff,
    ]);
    assert_eq!(
        companions(&checkable, None, false),
        [Command::OpenReport, Command::OpenTerminal, Command::OpenDiff]
    );
    let built = only(&[Command::OpenReport, Command::OpenTerminal, Command::OpenDiff]);
    assert_eq!(companions(&built, None, false), [Command::OpenDiff]);
}

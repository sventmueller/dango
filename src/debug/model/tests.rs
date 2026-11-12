use std::path::PathBuf;

use ide_core::debugging::{Breakpoint, Debugging};

use super::session::{Caps, Frame, Origin, Phase, Session};
use super::variables::{Node, Path};
use super::*;

fn frame(id: i64, line: usize) -> Frame {
    Frame {
        id,
        name: format!("frame {id}"),
        path: Some(PathBuf::from("/p/src/main.rs")),
        line,
        column: 0,
    }
}

fn caps(restart: bool) -> Caps {
    Caps {
        restart,
        terminate: true,
        exception_filters: vec![("panic".into(), "Rust: on panic".into(), true)],
        ..Caps::default()
    }
}

/// A model with a running session 1 that can or cannot restart.
fn running(restart: bool) -> Model {
    let mut model = Model::new(Debugging {
        watches: vec!["n".into()],
        ..Debugging::default()
    });
    model.started(Session::new(1, "shop", Origin::Launched));
    model.capabilities(1, caps(restart));
    model
}

#[test]
fn without_a_session_every_control_is_greyed_out() {
    let model = Model::new(Debugging::default());
    for command in Command::CONTROLS {
        assert!(!model.can(command), "{command:?}");
    }
    assert!(
        !model.can(Command::ToggleBreakpoints),
        "nothing to switch without breakpoints"
    );
}

#[test]
fn a_running_session_can_stop_and_only_a_paused_one_step() {
    let mut model = running(false);
    assert!(model.can(Command::Stop));
    assert!(!model.can(Command::Continue));
    assert!(!model.can(Command::StepOver));
    assert!(!model.can(Command::Restart), "the adapter cannot restart");
    model.stopped(1, 7, "breakpoint");
    for command in [
        Command::Continue,
        Command::StepOver,
        Command::StepInto,
        Command::StepOut,
        Command::Stop,
    ] {
        assert!(model.can(command), "{command:?}");
    }
    assert!(!model.can(Command::Restart));
    assert_eq!(model.run(Command::StepOver), [Effect::Run(1, Command::StepOver, 7)]);
    model.continued(1);
    assert!(!model.can(Command::StepInto));
    model.ended(1, Some(0), None);
    for command in Command::CONTROLS {
        assert!(!model.can(command), "{command:?} after the end");
    }
    assert!(model.run(Command::Continue).is_empty());
}

#[test]
fn an_adapter_that_can_restart_enables_restart() {
    let model = running(true);
    assert!(model.can(Command::Restart));
}

#[test]
fn a_stop_asks_for_the_stack_and_its_frame_shows_with_variables_and_watches() {
    let mut model = running(false);
    model.threads(1, vec![(7, "main".into()), (8, "worker".into())]);
    assert_eq!(
        model.stopped(1, 7, "breakpoint"),
        [Effect::Threads(1), Effect::Stack(1, 7)]
    );
    let effects = model.stack(1, 7, vec![frame(1000, 11), frame(1001, 40)]);
    let generation = model.variables.generation;
    assert_eq!(
        effects,
        [
            Effect::Reveal {
                path: "/p/src/main.rs".into(),
                line: 11
            },
            Effect::Scopes {
                session: 1,
                frame: 1000,
                generation
            },
            Effect::Watch {
                session: 1,
                frame: 1000,
                expression: "n".into(),
                generation
            },
        ]
    );
    assert_eq!(model.paused_at().map(|f| f.line), Some(11));
    let effects = model.scopes(generation, vec![Node::new("Locals", "", 3)]);
    assert_eq!(
        effects,
        [Effect::Children {
            session: 1,
            reference: 3,
            path: Path::Scope(vec!["Locals".into()]),
            generation
        }]
    );
    let effects = model.select(1, 7, 1);
    assert!(matches!(effects.first(), Some(Effect::Reveal { line: 40, .. })));
    assert_eq!(model.frame().map(|(_, f)| f.id), Some(1001));
    assert_eq!(
        model.paused_at().map(|f| f.line),
        Some(11),
        "the arrow stays on the innermost frame"
    );
    assert_eq!(
        model.select(1, 8, 0),
        [Effect::Stack(1, 8)],
        "another thread's stack is asked for"
    );
}

#[test]
fn running_on_forgets_stacks_and_variables() {
    let mut model = running(false);
    model.stopped(1, 7, "step");
    model.stack(1, 7, vec![frame(1000, 3)]);
    model.continued(1);
    assert!(model.frame().is_none());
    assert!(model.paused_at().is_none());
    assert!(model.variables.frame.is_none());
    assert!(
        model.stack(1, 7, vec![frame(1000, 3)]).is_empty(),
        "a late stack of a running session is dropped"
    );
}

#[test]
fn exception_filters_keep_the_users_choice_across_sessions() {
    let mut model = running(false);
    assert_eq!(model.enabled_filters(), ["panic"]);
    assert_eq!(
        model.set_filter("panic", false),
        [Effect::SendExceptionFilters, Effect::Save]
    );
    assert!(model.set_filter("panic", false).is_empty());
    model.started(Session::new(2, "again", Origin::Launched));
    model.capabilities(2, caps(false));
    assert!(
        model.enabled_filters().is_empty(),
        "the new session takes the user's choice, not the default"
    );
    assert_eq!(model.kept().exception_filters.len(), 1);
}

#[test]
fn switching_breakpoints_off_sends_every_file_and_keeps_them() {
    let mut model = Model::new(Debugging {
        breakpoints: vec![Breakpoint::new("/p/a.rs", 1), Breakpoint::new("/p/b.rs", 2)],
        ..Debugging::default()
    });
    assert!(model.can(Command::ToggleBreakpoints));
    let effects = model.run(Command::ToggleBreakpoints);
    assert_eq!(
        effects,
        [
            Effect::SendBreakpoints("/p/a.rs".into()),
            Effect::SendBreakpoints("/p/b.rs".into()),
            Effect::Save
        ]
    );
    assert!(!model.kept().active);
    assert_eq!(model.kept().breakpoints.len(), 2);
}

#[test]
fn what_is_kept_comes_back_the_same() {
    let mut model = running(false);
    model.breakpoints.toggle(std::path::Path::new("/p/a.rs"), 3);
    model.add_watch("items.len()");
    model.fold("breakpoints", true);
    model.configuration = Some("Debug shop".into());
    let kept = model.kept();
    let again = Model::new(kept.clone());
    assert_eq!(again.kept(), kept);
    assert!(again.sessions.is_empty(), "no session comes back");
    assert!(again.is_folded("breakpoints"));
}

#[test]
fn a_new_session_replaces_the_ended_ones_and_is_selected() {
    let mut model = running(false);
    model.ended(1, Some(1), None);
    model.started(Session::new(2, "again", Origin::Attached));
    assert_eq!(model.sessions.len(), 1);
    assert_eq!(model.current().map(|s| s.id), Some(2));
    assert!(stops_by_detaching(&model));
    assert_eq!(model.current().map(|s| s.phase.clone()), Some(Phase::Starting));
}

#[test]
fn a_watch_added_while_paused_is_evaluated_at_once() {
    let mut model = running(false);
    model.stopped(1, 7, "breakpoint");
    model.stack(1, 7, vec![frame(1000, 3)]);
    let effects = model.add_watch("total");
    assert!(
        effects
            .iter()
            .any(|e| matches!(e, Effect::Watch { expression, .. } if expression == "total"))
    );
    assert_eq!(model.remove_watch(0), [Effect::Save]);
    assert_eq!(model.variables.expressions(), ["total"]);
}

#[test]
fn a_start_that_goes_unanswered_waits_and_can_be_stopped() {
    let mut model = Model::new(Debugging::default());
    model.started(Session::new(0, "shop", Origin::Launched));
    assert!(model.can(Command::Stop), "a start under way can be stopped");
    assert!(model.still_starting(0));
    assert!(!model.still_starting(0), "it says so once");
    assert!(model.current().is_some_and(|s| s.waiting));
    model.renumber(0, 9);
    assert_eq!(model.current().map(|s| s.id), Some(9));
    model.continued(9);
    assert!(model.current().is_some_and(|s| !s.waiting && s.phase == Phase::Running));
    assert!(!model.still_starting(9), "a running session waits for nothing");
}

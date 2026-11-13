use super::*;
use ide_thread::{HookOutcome, Notice, ThreadId, Update};

fn hook(thread: &mut Thread, id: &str, outcome: HookOutcome) {
    thread.apply(Update::Put {
        lane: Lane::Main,
        key: Some(format!("hook:{id}")),
        at: std::time::UNIX_EPOCH,
        entry: EntryKind::Notice(Notice::Hook {
            id: id.into(),
            name: "PostToolUse:Bash".into(),
            event: "PostToolUse".into(),
            output: String::new(),
            exit_code: None,
            outcome,
        }),
    });
}

#[test]
fn running_hooks_get_no_row_until_they_fail() {
    let mut thread = Thread::new(ThreadId::from("t"), "test", "/tmp");
    hook(&mut thread, "a", HookOutcome::Success);
    hook(&mut thread, "b", HookOutcome::Running);
    assert_eq!(shown(&thread, false).len(), 1);
    hook(&mut thread, "b", HookOutcome::Success);
    assert_eq!(shown(&thread, false).len(), 1);
    hook(&mut thread, "c", HookOutcome::Running);
    assert_eq!(shown(&thread, false).len(), 1);
    hook(&mut thread, "c", HookOutcome::Error);
    assert_eq!(shown(&thread, false).len(), 2);
}

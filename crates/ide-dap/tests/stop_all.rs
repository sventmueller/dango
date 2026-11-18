//! Quitting the app stops every session at once: in a test binary of its own, as it reaches every session of the
//! process.

use std::sync::mpsc::channel;
use std::time::Duration;

use ide_dap::fake::Script;
use ide_dap::{Adapter, EndReason, Event, Request, Session, Setup, State, stop_all};
use ide_process::Supervisor;

#[test]
fn stop_all_ends_launched_sessions_and_detaches_attached_ones() {
    let supervisor = Supervisor::new();
    let script = Script {
        lines: vec![1, 1, 1, 1, 1, 1, 1, 1, 1, 1],
        ..Script::default()
    };
    let mut setup = Setup::default();
    setup
        .breakpoints
        .insert(script.path.clone(), vec![script.breakpoint(1)]);
    let start = |config| {
        let (tx, rx) = channel();
        let session = Session::start(
            &supervisor,
            Adapter::fake(script.clone()),
            config,
            setup.clone(),
            move |_, e| {
                let _ = tx.send(e);
            },
        )
        .unwrap();
        (session, rx)
    };
    let (launched, launched_events) = start(script.launch_config());
    let (attached, attached_events) = start(script.attach_config(4242));
    for session in [&launched, &attached] {
        assert!(session.wait_for(Duration::from_secs(10), |s| matches!(s, State::Stopped(_))));
    }
    assert_eq!(attached.request(), Request::Attach);

    stop_all(Duration::from_secs(10));

    for (session, events) in [(&launched, launched_events), (&attached, attached_events)] {
        assert_eq!(session.end().map(|e| e.reason), Some(EndReason::Stopped));
        let ended = std::iter::from_fn(|| events.recv_timeout(Duration::from_secs(2)).ok())
            .filter(|e| matches!(e, Event::Ended(_)))
            .count();
        assert_eq!(ended, 1);
    }
}

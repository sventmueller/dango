#![no_main]

use std::time::{Duration, SystemTime};

use ide_acp::Capabilities;
use ide_acp::mapping::Mapper;
use ide_thread::fuzzing::Replay;
use ide_thread::{Thread, ThreadId};
use libfuzzer_sys::fuzz_target;

// The parameters of `session/update` notifications, one JSON value per line, through the mapping into a thread whose
// reported changes must rebuild its timelines.
fuzz_target!(|data: &[u8]| {
    let text = String::from_utf8_lossy(data);
    let mut mapper = Mapper::new();
    let mut caps = Capabilities::default();
    let mut replay = Replay::new(Thread::new(ThreadId::new(), "acp", "/"));
    let mut now = SystemTime::UNIX_EPOCH;
    for line in text.lines() {
        now += Duration::from_millis(250);
        let Ok(params) = serde_json::from_str(line) else {
            continue;
        };
        let (updates, _) = mapper.apply_json(&params, &mut caps, now);
        for update in updates {
            replay.apply(update);
        }
    }
});

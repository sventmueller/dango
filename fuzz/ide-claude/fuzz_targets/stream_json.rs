#![no_main]

use std::time::{Duration, SystemTime};

use ide_claude::mapping::Mapper;
use ide_thread::fuzzing::Replay;
use ide_thread::{Thread, ThreadId};
use libfuzzer_sys::fuzz_target;

// The CLI's stream-json output, line by line, through the mapping into a thread whose reported changes must rebuild
// its timelines.
fuzz_target!(|data: &[u8]| {
    let text = String::from_utf8_lossy(data);
    let mut mapper = Mapper::new();
    let mut replay = Replay::new(Thread::new(ThreadId::new(), ide_claude::AGENT, "/"));
    let mut now = SystemTime::UNIX_EPOCH;
    for line in text.lines() {
        now += Duration::from_millis(250);
        if let Ok(message) = ide_claude::protocol::parse(line) {
            for update in mapper.map(&message, now) {
                replay.apply(update);
            }
        }
    }
    for update in mapper.ended(now) {
        replay.apply(update);
    }
});

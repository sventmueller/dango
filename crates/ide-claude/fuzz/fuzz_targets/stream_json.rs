#![no_main]

use std::time::{Duration, SystemTime};

use ide_claude::mapping::Mapper;
use ide_thread::{Thread, ThreadId};
use libfuzzer_sys::fuzz_target;

// The CLI's stream-json output, line by line, through the mapping into a thread.
fuzz_target!(|data: &[u8]| {
    let text = String::from_utf8_lossy(data);
    let mut mapper = Mapper::new();
    let mut thread = Thread::new(ThreadId::new(), ide_claude::AGENT, "/");
    let mut now = SystemTime::UNIX_EPOCH;
    for line in text.lines() {
        now += Duration::from_millis(250);
        if let Ok(message) = ide_claude::protocol::parse(line) {
            for update in mapper.map(&message, now) {
                thread.apply(update);
            }
        }
    }
    for update in mapper.ended(now) {
        thread.apply(update);
    }
});

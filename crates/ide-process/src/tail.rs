//! The latest output of a process, kept within a limit however much it writes.

use std::collections::VecDeque;
use std::io::Read;
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;

/// The last `limit` bytes a process wrote.
#[derive(Debug, Clone)]
pub struct Tail {
    limit: usize,
    bytes: VecDeque<u8>,
    dropped: u64,
}

impl Tail {
    /// An empty tail that keeps up to `limit` bytes.
    pub fn new(limit: usize) -> Tail {
        Tail {
            limit,
            bytes: VecDeque::new(),
            dropped: 0,
        }
    }

    /// Adds `data`, dropping the oldest bytes beyond the limit.
    pub fn push(&mut self, data: &[u8]) {
        let data = if data.len() > self.limit {
            self.dropped += (data.len() - self.limit) as u64;
            &data[data.len() - self.limit..]
        } else {
            data
        };
        let overflow = (self.bytes.len() + data.len()).saturating_sub(self.limit);
        self.bytes.drain(..overflow);
        self.dropped += overflow as u64;
        self.bytes.extend(data);
    }

    /// How many bytes were dropped so far.
    pub fn dropped(&self) -> u64 {
        self.dropped
    }

    /// The kept output as text; after a drop it starts at the next full line.
    pub fn text(&self) -> String {
        let bytes: Vec<u8> = self.bytes.iter().copied().collect();
        let start = if self.dropped > 0 {
            bytes.iter().position(|&b| b == b'\n').map_or(0, |i| i + 1)
        } else {
            0
        };
        String::from_utf8_lossy(&bytes[start..]).into_owned()
    }
}

/// Reads `reader` to its end into `tail` on a thread of its own.
pub fn collect(mut reader: impl Read + Send + 'static, tail: Arc<Mutex<Tail>>) -> std::io::Result<JoinHandle<()>> {
    std::thread::Builder::new().name("tail".into()).spawn(move || {
        let mut buffer = [0u8; 8192];
        while let Ok(n) = reader.read(&mut buffer) {
            if n == 0 {
                break;
            }
            tail.lock().unwrap_or_else(|e| e.into_inner()).push(&buffer[..n]);
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_tail_keeps_the_latest_bytes_and_starts_at_a_line() {
        let mut tail = Tail::new(10);
        tail.push(b"first\nsecond\nthird");
        assert_eq!(tail.dropped(), 8);
        assert_eq!(tail.text(), "third");
        let mut small = Tail::new(64);
        small.push(b"a\n");
        small.push(b"b\n");
        assert_eq!(small.text(), "a\nb\n");
        assert_eq!(small.dropped(), 0);
    }

    #[test]
    fn a_reader_is_collected_to_its_end() {
        let tail = Arc::new(Mutex::new(Tail::new(4)));
        collect(&b"0123456789"[..], tail.clone()).unwrap().join().unwrap();
        assert_eq!(tail.lock().unwrap().dropped(), 6);
    }
}

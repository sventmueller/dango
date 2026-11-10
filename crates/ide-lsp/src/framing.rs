//! The base protocol's framing: each message is a header block with its byte length, a blank line, then the JSON.

use std::io::{self, BufRead};

/// The largest message accepted; a header claiming more means the stream is broken.
const MAX_MESSAGE: usize = 256 * 1024 * 1024;

/// The next message body, or `None` when the stream ends between messages.
///
/// Lines before a header that are not headers are skipped: some servers print stray lines to standard output.
pub(crate) fn read(reader: &mut impl BufRead) -> io::Result<Option<Vec<u8>>> {
    let mut length = None;
    let mut line = Vec::new();
    let mut in_header = false;
    loop {
        line.clear();
        if reader.read_until(b'\n', &mut line)? == 0 {
            return if in_header {
                Err(io::Error::new(
                    io::ErrorKind::UnexpectedEof,
                    "the stream ended inside a header",
                ))
            } else {
                Ok(None)
            };
        }
        let text = String::from_utf8_lossy(&line);
        let text = text.trim_end_matches(['\r', '\n']);
        if text.is_empty() {
            if in_header {
                break;
            }
            continue;
        }
        let Some((name, value)) = text.split_once(':') else {
            if in_header {
                return Err(invalid("a header line without a colon"));
            }
            continue;
        };
        in_header = true;
        if name.trim().eq_ignore_ascii_case("content-length") {
            let n: usize = value
                .trim()
                .parse()
                .map_err(|_| invalid("Content-Length is not a number"))?;
            if n > MAX_MESSAGE {
                return Err(invalid("the message is too large"));
            }
            length = Some(n);
        }
    }
    let length = length.ok_or_else(|| invalid("a message without Content-Length"))?;
    let mut body = vec![0; length];
    reader.read_exact(&mut body)?;
    Ok(Some(body))
}

/// `body` with its header, ready to write in one piece.
pub(crate) fn frame(body: &[u8]) -> Vec<u8> {
    let header = format!("Content-Length: {}\r\n\r\n", body.len());
    let mut bytes = Vec::with_capacity(header.len() + body.len());
    bytes.extend_from_slice(header.as_bytes());
    bytes.extend_from_slice(body);
    bytes
}

fn invalid(why: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, why.to_string())
}

#[cfg(test)]
mod tests {
    use std::io::{BufReader, Read};

    use super::*;

    /// Hands out its bytes a few at a time, as a pipe does under load.
    struct Trickle {
        bytes: Vec<u8>,
        at: usize,
        step: usize,
    }

    impl Read for Trickle {
        fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
            let n = self.step.min(buf.len()).min(self.bytes.len() - self.at);
            buf[..n].copy_from_slice(&self.bytes[self.at..self.at + n]);
            self.at += n;
            Ok(n)
        }
    }

    fn trickle(bytes: Vec<u8>, step: usize) -> BufReader<Trickle> {
        BufReader::with_capacity(4, Trickle { bytes, at: 0, step })
    }

    #[test]
    fn messages_arriving_in_pieces_are_put_back_together() {
        let first = r#"{"jsonrpc":"2.0","method":"a"}"#;
        let second = r#"{"jsonrpc":"2.0","method":"b"}"#;
        let mut bytes = frame(first.as_bytes());
        bytes.extend(frame(second.as_bytes()));
        for step in [1, 2, 3, 7, 64] {
            let mut reader = trickle(bytes.clone(), step);
            assert_eq!(read(&mut reader).unwrap().unwrap(), first.as_bytes());
            assert_eq!(read(&mut reader).unwrap().unwrap(), second.as_bytes());
            assert_eq!(read(&mut reader).unwrap(), None);
        }
    }

    #[test]
    fn the_length_counts_bytes_so_multibyte_text_survives() {
        let body = r#"{"text":"Grüße 😀 ∑"}"#;
        assert_ne!(body.len(), body.chars().count());
        let mut reader = trickle(frame(body.as_bytes()), 1);
        assert_eq!(read(&mut reader).unwrap().unwrap(), body.as_bytes());
    }

    #[test]
    fn other_headers_and_their_case_do_not_matter() {
        let bytes = b"content-length: 2\r\nContent-Type: application/vscode-jsonrpc; charset=utf-8\r\n\r\n{}".to_vec();
        assert_eq!(read(&mut trickle(bytes, 5)).unwrap().unwrap(), b"{}");
    }

    #[test]
    fn stray_lines_before_a_message_are_skipped() {
        let mut bytes = b"\r\nnode warning printed to stdout\n".to_vec();
        bytes.extend(frame(b"{}"));
        assert_eq!(read(&mut trickle(bytes, 3)).unwrap().unwrap(), b"{}");
    }

    #[test]
    fn a_broken_header_is_an_error_not_a_hang() {
        let missing = b"Content-Type: x\r\n\r\n{}".to_vec();
        assert!(read(&mut trickle(missing, 4)).is_err());
        let huge = format!("Content-Length: {}\r\n\r\n", MAX_MESSAGE + 1).into_bytes();
        assert!(read(&mut trickle(huge, 4)).is_err());
        let cut = b"Content-Length: 10\r\n\r\n{}".to_vec();
        assert!(read(&mut trickle(cut, 4)).is_err());
        let cut_header = b"Content-Length: 10\r\n".to_vec();
        assert!(read(&mut trickle(cut_header, 4)).is_err());
    }
}

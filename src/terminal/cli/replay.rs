//! Replays the sessions in `fixtures/` through a CLI thread's status, with the output watched as the session watches
//! it, the programs in front looked at once a second as the view does, and the status checked where a fixture
//! expects one.

use std::time::{Duration, Instant};

use ide_terminal::OutputWatch;

use super::{CliState, CliStatus, Launch};

#[derive(Debug, Clone)]
enum Step {
    Out(Vec<u8>),
    Key,
    Programs(Vec<String>),
    Exit,
    Expect(CliStatus),
}

/// The text of a quoted fixture string with its escapes resolved.
fn unescape(text: &str) -> Vec<u8> {
    let mut out = String::new();
    let mut chars = text.chars();
    while let Some(c) = chars.next() {
        if c != '\\' {
            out.push(c);
            continue;
        }
        match chars.next() {
            Some('e') => out.push('\x1b'),
            Some('a') => out.push('\x07'),
            Some('r') => out.push('\r'),
            Some('n') => out.push('\n'),
            Some(other) => out.push(other),
            None => {}
        }
    }
    out.into_bytes()
}

/// The words of a fixture line, quoted strings as one word each without their quotes.
fn words(line: &str) -> Vec<(String, bool)> {
    let mut words = Vec::new();
    let mut chars = line.chars().peekable();
    while let Some(&c) = chars.peek() {
        if c.is_whitespace() {
            chars.next();
        } else if c == '"' {
            chars.next();
            let mut word = String::new();
            while let Some(c) = chars.next() {
                match c {
                    '\\' => {
                        word.push('\\');
                        word.extend(chars.next());
                    }
                    '"' => break,
                    c => word.push(c),
                }
            }
            words.push((word, true));
        } else {
            let word: String = std::iter::from_fn(|| chars.next_if(|c| !c.is_whitespace())).collect();
            words.push((word, false));
        }
    }
    words
}

/// The steps of a fixture in time order, `every` lines spread out into their frames.
fn steps(fixture: &str) -> Vec<(u64, Step)> {
    let mut steps: Vec<(u64, usize, Step)> = Vec::new();
    for (order, line) in fixture.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let words = words(line);
        let at: u64 = words[0].0.parse().expect("a time");
        let frames = || {
            words
                .iter()
                .filter(|(_, quoted)| *quoted)
                .map(|(w, _)| unescape(w))
                .collect::<Vec<_>>()
        };
        match words[1].0.as_str() {
            "out" => steps.extend(frames().into_iter().map(|frame| (at, order, Step::Out(frame)))),
            "every" => {
                let period: u64 = words[2].0.parse().expect("a period");
                let until: u64 = words[4].0.parse().expect("an end");
                let frames = frames();
                for (n, time) in (at..until)
                    .step_by(usize::try_from(period).expect("a period"))
                    .enumerate()
                {
                    steps.push((time, order, Step::Out(frames[n % frames.len()].clone())));
                }
            }
            "key" => steps.push((at, order, Step::Key)),
            "programs" => steps.push((
                at,
                order,
                Step::Programs(words[2..].iter().map(|(w, _)| w.clone()).collect()),
            )),
            "exit" => steps.push((at, order, Step::Exit)),
            "expect" => {
                let status = match words[2].0.as_str() {
                    "working" => CliStatus::Working,
                    "waiting" => CliStatus::Waiting,
                    _ => CliStatus::Finished,
                };
                steps.push((at, order, Step::Expect(status)));
            }
            other => panic!("unknown step {other}"),
        }
    }
    steps.sort_by_key(|(at, order, _)| (*at, *order));
    steps.into_iter().map(|(at, _, step)| (at, step)).collect()
}

/// Where the replayed status differed from what the fixture expects.
fn replay(fixture: &str) -> Vec<String> {
    let state = CliState::new(Launch {
        name: "cli".into(),
        program: "/usr/bin/true".into(),
    });
    let start = Instant::now();
    let at = |ms: u64| start + Duration::from_millis(ms);
    let mut watch = OutputWatch::default();
    let mut content = 0;
    let mut programs: Vec<String> = Vec::new();
    let mut next_tick = 1000;
    let mut wrong = Vec::new();
    let look = |state: &CliState, programs: &[String], ms: u64| {
        let now = at(ms);
        let seen = state.quiet(now).then(|| programs.to_vec());
        state.evaluate(seen, now);
    };
    for (ms, step) in steps(fixture) {
        while next_tick <= ms {
            look(&state, &programs, next_tick);
            next_tick += 1000;
        }
        match step {
            Step::Out(bytes) => {
                let seen = watch.feed(&bytes);
                content += u64::from(seen.content);
                state.output(content, at(ms));
                for title in &seen.titles {
                    state.title(Some(title));
                }
                if seen.bell {
                    state.bell(at(ms));
                }
                state.evaluate(None, at(ms));
            }
            Step::Key => state.key(at(ms)),
            Step::Programs(now_running) => programs = now_running,
            Step::Exit => state.exited(),
            Step::Expect(expected) => {
                look(&state, &programs, ms);
                if state.status() != expected {
                    wrong.push(format!("at {ms} ms: {:?}, expected {expected:?}", state.status()));
                }
            }
        }
    }
    wrong
}

#[test]
fn a_claude_code_session_reads_as_it_went() {
    let wrong = replay(include_str!("fixtures/claude.trace"));
    assert!(wrong.is_empty(), "{wrong:#?}");
}

#[test]
fn a_codex_session_reads_as_it_went_also_while_its_title_alone_animates() {
    let wrong = replay(include_str!("fixtures/codex.trace"));
    assert!(wrong.is_empty(), "{wrong:#?}");
}

#[test]
fn an_agy_session_reads_as_it_went_despite_its_helper_process() {
    let wrong = replay(include_str!("fixtures/agy.trace"));
    assert!(wrong.is_empty(), "{wrong:#?}");
}

#[test]
fn fixture_strings_and_frames_are_read_as_written() {
    assert_eq!(unescape(r"\e]0;✳ x\a\\"), "\x1b]0;✳ x\x07\\".as_bytes());
    let steps = steps("100 every 50 until 200 out \"a\" \"b\"\n150 key");
    let kinds: Vec<String> = steps
        .iter()
        .map(|(at, step)| match step {
            Step::Out(bytes) => format!("{at} {}", String::from_utf8_lossy(bytes)),
            other => format!("{at} {other:?}"),
        })
        .collect();
    assert_eq!(kinds, ["100 a", "150 b", "150 Key"]);
}

#[test]
fn a_status_other_than_expected_is_reported() {
    let fixture = "0 out \"text\"\n1000 expect waiting\n5000 expect working\n";
    assert_eq!(
        replay(fixture),
        [
            "at 1000 ms: Working, expected Waiting",
            "at 5000 ms: Waiting, expected Working"
        ]
    );
}

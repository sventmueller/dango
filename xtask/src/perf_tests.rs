use super::*;
use crate::shell::fake::scratch;

fn time(name: &str, value: f64) -> Measurement {
    Measurement {
        name: name.into(),
        value,
        unit: "ms".into(),
        lower_is_better: true,
        samples: Vec::new(),
    }
}

fn rate(name: &str, value: f64) -> Measurement {
    Measurement {
        unit: "MB/s".into(),
        lower_is_better: false,
        ..time(name, value)
    }
}

fn baseline(machine: &str, measurements: Vec<Measurement>) -> Baseline {
    Baseline {
        machine: Some(machine.into()),
        measurements,
    }
}

#[test]
fn a_time_fails_only_beyond_its_baseline_and_a_fifth() {
    let accepted = time("key", 10.0);
    assert_eq!(verdict(&time("key", 11.9), Some(&accepted)), Verdict::Within);
    assert_eq!(verdict(&time("key", 5.0), Some(&accepted)), Verdict::Within);
    let Verdict::Worse(percent) = verdict(&time("key", 13.0), Some(&accepted)) else {
        panic!("13 ms against 10 ms is worse");
    };
    assert!((percent - 30.0).abs() < 1e-9, "{percent}");
}

#[test]
fn a_rate_fails_when_it_falls_beyond_a_fifth() {
    let accepted = rate("cat", 120.0);
    assert_eq!(verdict(&rate("cat", 101.0), Some(&accepted)), Verdict::Within);
    assert_eq!(verdict(&rate("cat", 500.0), Some(&accepted)), Verdict::Within);
    assert!(matches!(
        verdict(&rate("cat", 90.0), Some(&accepted)),
        Verdict::Worse(_)
    ));
}

#[test]
fn a_measurement_without_an_accepted_value_of_its_unit_is_new() {
    assert_eq!(verdict(&time("key", 99.0), None), Verdict::New);
    assert_eq!(verdict(&time("key", 99.0), Some(&rate("key", 1.0))), Verdict::New);
}

#[test]
fn only_a_baseline_of_this_mac_fails_the_run() {
    let measured = [time("key", 30.0), time("fresh", 1.0)];
    let here = baseline("Mac16,1", vec![time("key", 10.0), time("gone", 1.0)]);
    let (lines, worse) = comparison(&measured, &here, "Mac16,1");
    assert_eq!(worse, ["key"]);
    assert!(
        lines.iter().any(|l| l.contains("new") && l.contains("fresh")),
        "{lines:?}"
    );
    assert!(
        lines.iter().any(|l| l.contains("unmeasured") && l.contains("gone")),
        "{lines:?}"
    );
    let (lines, worse) = comparison(&measured, &here, "Mac15,3");
    assert!(worse.is_empty());
    assert!(lines[0].contains("reporting only"), "{lines:?}");
    let (_, worse) = comparison(&measured, &Baseline::default(), "Mac16,1");
    assert!(worse.is_empty());
}

#[test]
fn accepting_keeps_this_macs_unmeasured_values_and_drops_another_macs() {
    let measured = vec![Measurement {
        samples: vec![1.0, 2.0, 3.0],
        ..time("key", 2.0)
    }];
    let old = baseline("Mac16,1", vec![time("key", 9.0), time("other", 4.0)]);
    let kept = accepted(old, measured.clone(), "Mac16,1".into());
    assert_eq!(kept, baseline("Mac16,1", vec![time("key", 2.0), time("other", 4.0)]));
    let old = baseline("Mac16,1", vec![time("other", 4.0)]);
    let moved = accepted(old, measured, "Mac15,3".into());
    assert_eq!(moved, baseline("Mac15,3", vec![time("key", 2.0)]));
}

#[test]
fn the_files_ide_core_writes_are_read_by_name() {
    let dir = scratch("perf-read");
    files::write(
        dir.join("b.json"),
        r#"{"name": "b", "value": 2.5, "unit": "ms", "lower_is_better": true, "samples": [2.0, 2.5, 3.0]}"#,
    )
    .unwrap();
    files::write(
        dir.join("a.json"),
        r#"{"name": "a", "value": 80.0, "unit": "MB/s", "lower_is_better": false}"#,
    )
    .unwrap();
    files::write(dir.join("notes.txt"), "not a measurement").unwrap();
    let read = read_measurements(&dir).unwrap();
    assert_eq!(read.iter().map(|m| m.name.as_str()).collect::<Vec<_>>(), ["a", "b"]);
    assert_eq!(read[1].samples, [2.0, 2.5, 3.0]);
    assert!(read_measurements(&dir.join("missing")).unwrap().is_empty());
}

#[test]
fn the_checked_in_baseline_parses() {
    read_baseline(&baseline_file()).unwrap();
}

#[test]
fn a_time_under_a_millisecond_slower_is_noise_however_large_its_share() {
    let accepted = time("tiny", 0.04);
    assert_eq!(verdict(&time("tiny", 0.09), Some(&accepted)), Verdict::Within);
    let small = time("small", 2.0);
    assert_eq!(verdict(&time("small", 2.9), Some(&small)), Verdict::Within);
    assert!(matches!(verdict(&time("small", 3.5), Some(&small)), Verdict::Worse(_)));
}

#[test]
fn the_minute_load_is_the_first_number_sysctl_prints() {
    assert_eq!(super::minute_load("{ 1.87 3.21 4.06 }\n"), Some(1.87));
    assert_eq!(super::minute_load(""), None);
}

//! Percentiles of the latencies and the evenness of a scroll.

use serde::Serialize;

/// The spread of one measurement's samples, in milliseconds.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub(crate) struct Summary {
    pub(crate) samples: usize,
    /// Samples that saw no change before their time limit, left out of the figures.
    pub(crate) missed: usize,
    pub(crate) min_ms: f64,
    pub(crate) p50_ms: f64,
    pub(crate) p95_ms: f64,
    pub(crate) max_ms: f64,
    pub(crate) mean_ms: f64,
}

/// The summary of `values`, or none without a value.
pub(crate) fn summarize(values: &[f64], missed: usize) -> Option<Summary> {
    let mut sorted: Vec<f64> = values.iter().copied().filter(|v| v.is_finite()).collect();
    if sorted.is_empty() {
        return None;
    }
    sorted.sort_by(f64::total_cmp);
    let len = sorted.len();
    Some(Summary {
        samples: len,
        missed,
        min_ms: sorted[0],
        p50_ms: percentile(&sorted, 50.0),
        p95_ms: percentile(&sorted, 95.0),
        max_ms: sorted[len - 1],
        mean_ms: sorted.iter().sum::<f64>() / len as f64,
    })
}

/// The nearest-rank percentile `p` of the sorted, non-empty `sorted`.
fn percentile(sorted: &[f64], p: f64) -> f64 {
    let rank = (p / 100.0 * sorted.len() as f64).ceil();
    let index = whole(rank).saturating_sub(1).min(sorted.len() - 1);
    sorted[index]
}

/// `value` rounded towards zero as a count, 0 for a negative one.
pub(crate) fn whole(value: f64) -> usize {
    if !value.is_finite() || value <= 0.0 {
        return 0;
    }
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "positive and finite, and a count beyond usize is no count of frames"
    )]
    let n = value as usize;
    n
}

/// How evenly the picture followed a scroll.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub(crate) struct Smoothness {
    /// Between consecutive changed frames.
    pub(crate) intervals: Summary,
    /// Intervals longer than one and a half refreshes.
    pub(crate) long_intervals: usize,
    /// The refreshes those long intervals skipped.
    pub(crate) dropped_frames: usize,
    pub(crate) changed_frames: usize,
    /// One changed frame per refresh for the length of the scroll.
    pub(crate) expected_frames: usize,
}

/// The smoothness of the changed frames at `times` (ns, ascending) during a scroll of `duration_ns` on a display that
/// refreshes `refresh_hz` times a second.
pub(crate) fn smoothness(times: &[u64], refresh_hz: f64, duration_ns: u64) -> Option<Smoothness> {
    if refresh_hz <= 0.0 {
        return None;
    }
    let refresh_ms = 1000.0 / refresh_hz;
    let intervals: Vec<f64> = times.windows(2).map(|w| ms(w[1].saturating_sub(w[0]))).collect();
    let long: Vec<f64> = intervals.iter().copied().filter(|i| *i > 1.5 * refresh_ms).collect();
    Some(Smoothness {
        intervals: summarize(&intervals, 0)?,
        long_intervals: long.len(),
        dropped_frames: long
            .iter()
            .map(|i| whole((i / refresh_ms).round()).saturating_sub(1))
            .sum(),
        changed_frames: times.len(),
        expected_frames: whole((ms(duration_ns) / refresh_ms).round()),
    })
}

/// Several scrolls as one: their intervals together, the counts added up.
pub(crate) fn combine(parts: &[Smoothness], all_intervals: &[f64]) -> Option<Smoothness> {
    Some(Smoothness {
        intervals: summarize(all_intervals, 0)?,
        long_intervals: parts.iter().map(|p| p.long_intervals).sum(),
        dropped_frames: parts.iter().map(|p| p.dropped_frames).sum(),
        changed_frames: parts.iter().map(|p| p.changed_frames).sum(),
        expected_frames: parts.iter().map(|p| p.expected_frames).sum(),
    })
}

/// Nanoseconds as milliseconds.
pub(crate) fn ms(ns: u64) -> f64 {
    ns as f64 / 1_000_000.0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn percentiles_take_the_nearest_rank() {
        let values: Vec<f64> = (1..=20).map(f64::from).collect();
        let summary = summarize(&values, 2).unwrap();
        assert_eq!((summary.samples, summary.missed), (20, 2));
        assert_eq!(
            (summary.min_ms, summary.p50_ms, summary.p95_ms, summary.max_ms),
            (1.0, 10.0, 19.0, 20.0)
        );
        assert!((summary.mean_ms - 10.5).abs() < 1e-9);
    }

    #[test]
    fn one_sample_is_every_percentile_and_none_is_no_summary() {
        let one = summarize(&[7.5], 0).unwrap();
        assert_eq!((one.p50_ms, one.p95_ms, one.max_ms), (7.5, 7.5, 7.5));
        assert!(summarize(&[], 3).is_none());
        assert!(summarize(&[f64::NAN], 0).is_none());
    }

    #[test]
    fn the_order_of_the_samples_does_not_matter() {
        assert_eq!(summarize(&[3.0, 1.0, 2.0], 0), summarize(&[1.0, 2.0, 3.0], 0));
    }

    #[test]
    fn an_even_scroll_drops_nothing_and_a_gap_counts_its_missing_frames() {
        let frame = 16_666_667;
        let even: Vec<u64> = (0..60).map(|i| i * frame).collect();
        let smooth = smoothness(&even, 60.0, 60 * frame).unwrap();
        assert_eq!((smooth.long_intervals, smooth.dropped_frames), (0, 0));
        assert_eq!((smooth.changed_frames, smooth.expected_frames), (60, 60));
        assert!((smooth.intervals.p50_ms - 16.666_667).abs() < 1e-3);

        let mut gap = even.clone();
        gap.drain(10..13);
        let rough = smoothness(&gap, 60.0, 60 * frame).unwrap();
        assert_eq!((rough.long_intervals, rough.dropped_frames), (1, 3));
        assert!((rough.intervals.max_ms - 66.666_668).abs() < 1e-3);
    }

    #[test]
    fn a_scroll_without_two_changed_frames_has_no_smoothness() {
        assert!(smoothness(&[5], 60.0, 1).is_none());
        assert!(smoothness(&[1, 2], 0.0, 1).is_none());
    }

    #[test]
    fn scrolls_add_up() {
        let frame = 8_333_333;
        let one = smoothness(&[0, frame, 3 * frame], 120.0, 3 * frame).unwrap();
        let both = combine(&[one.clone(), one], &[8.3, 16.7, 8.3, 16.7]).unwrap();
        assert_eq!(
            (both.long_intervals, both.dropped_frames, both.changed_frames),
            (2, 2, 6)
        );
        assert_eq!(both.expected_frames, 6);
    }

    #[test]
    fn counts_round_down_and_never_below_zero() {
        assert_eq!((whole(2.9), whole(-1.0), whole(f64::NAN)), (2, 0, 0));
    }
}

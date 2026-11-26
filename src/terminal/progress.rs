//! The progress a terminal's program reports (OSC 9;4) as its tab's ring and the Dock icon's bar show it, as Tapa's
//! tabs show it (Tapa's F56): filled as far as the work is, red after an error, yellow while paused, and turning
//! while the program does not know how far it is.

use splat_i18n::{t, tf};
use term_session::event::{Progress, ProgressState};

/// What a ring or a bar shows of a program's progress.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Ring {
    /// How much of it is filled, from 0 to 1; none while the program does not know how far it is.
    pub(crate) filled: Option<f64>,
    pub(crate) tone: Tone,
    /// The percentage the program reported, which VoiceOver reads.
    pub(crate) percent: Option<u8>,
}

/// The colour of a ring.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Tone {
    /// The work goes on.
    Normal,
    /// It ran into an error.
    Error,
    /// It is paused.
    Paused,
}

impl Ring {
    /// The ring of `progress`; an error or a pause without a percentage fills it.
    pub(crate) fn of(progress: Progress) -> Ring {
        let filled = progress.percent.map(|percent| f64::from(percent.min(100)) / 100.0);
        let (filled, tone) = match progress.state {
            ProgressState::Normal => (Some(filled.unwrap_or(0.0)), Tone::Normal),
            ProgressState::Indeterminate => (None, Tone::Normal),
            ProgressState::Error => (Some(filled.unwrap_or(1.0)), Tone::Error),
            ProgressState::Paused => (Some(filled.unwrap_or(1.0)), Tone::Paused),
        };
        let percent = filled.and(progress.percent).map(|percent| percent.min(100));
        Ring { filled, tone, percent }
    }

    /// The one ring the Dock icon shows for the `rings` of every terminal: filled as far as those that know it are on
    /// average, red when one of them failed, yellow when all are paused; none without a ring.
    pub(crate) fn combined(rings: &[Ring]) -> Option<Ring> {
        if rings.is_empty() {
            return None;
        }
        let tone = if rings.iter().any(|ring| ring.tone == Tone::Error) {
            Tone::Error
        } else if rings.iter().all(|ring| ring.tone == Tone::Paused) {
            Tone::Paused
        } else {
            Tone::Normal
        };
        let fills: Vec<f64> = rings.iter().filter_map(|ring| ring.filled).collect();
        let percents: Vec<f64> = rings.iter().filter_map(|ring| ring.percent.map(f64::from)).collect();
        let (filled, percent) = (mean(&fills), mean(&percents).map(whole_percent));
        Some(Ring { filled, tone, percent })
    }

    /// What VoiceOver reads for the ring.
    pub(crate) fn spoken(self) -> String {
        let percent = self.percent.map(|percent| percent.to_string());
        match (self.tone, percent) {
            (Tone::Normal, Some(percent)) if self.filled.is_some() => tf("{} % erledigt", &[&percent]),
            (Tone::Normal, _) => t("in Arbeit").to_string(),
            (Tone::Error, Some(percent)) => tf("Fehler bei {} %", &[&percent]),
            (Tone::Error, None) => t("Fehler").to_string(),
            (Tone::Paused, Some(percent)) => tf("angehalten bei {} %", &[&percent]),
            (Tone::Paused, None) => t("angehalten").to_string(),
        }
    }
}

fn mean(values: &[f64]) -> Option<f64> {
    (!values.is_empty()).then(|| values.iter().sum::<f64>() / values.len() as f64)
}

#[expect(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "clamped to 0 to 100 first"
)]
fn whole_percent(percent: f64) -> u8 {
    percent.round().clamp(0.0, 100.0) as u8
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ring(state: ProgressState, percent: Option<u8>) -> Ring {
        Ring::of(Progress { state, percent })
    }

    #[test]
    fn the_ring_fills_as_far_as_the_work_is_and_shows_how_it_goes() {
        let normal = ring(ProgressState::Normal, Some(40));
        assert_eq!(
            (normal.filled, normal.tone, normal.percent),
            (Some(0.4), Tone::Normal, Some(40))
        );
        let unknown = ring(ProgressState::Indeterminate, None);
        assert_eq!(
            (unknown.filled, unknown.tone, unknown.percent),
            (None, Tone::Normal, None)
        );
        let failed = ring(ProgressState::Error, None);
        assert_eq!(
            (failed.filled, failed.tone, failed.percent),
            (Some(1.0), Tone::Error, None)
        );
        let paused = ring(ProgressState::Paused, Some(70));
        assert_eq!(
            (paused.filled, paused.tone, paused.percent),
            (Some(0.7), Tone::Paused, Some(70))
        );
    }

    #[test]
    fn voiceover_reads_the_percentage_and_how_the_work_goes() {
        assert_eq!(ring(ProgressState::Normal, Some(40)).spoken(), "40 % erledigt");
        assert_eq!(ring(ProgressState::Indeterminate, None).spoken(), "in Arbeit");
        assert_eq!(ring(ProgressState::Error, Some(30)).spoken(), "Fehler bei 30 %");
        assert_eq!(ring(ProgressState::Error, None).spoken(), "Fehler");
        assert_eq!(ring(ProgressState::Paused, None).spoken(), "angehalten");
    }

    #[test]
    fn the_dock_shows_the_average_and_an_error_before_everything_else() {
        assert_eq!(Ring::combined(&[]), None);
        let both = Ring::combined(&[
            ring(ProgressState::Normal, Some(20)),
            ring(ProgressState::Paused, Some(60)),
        ])
        .unwrap();
        assert_eq!(both.tone, Tone::Normal);
        assert!(both.filled.is_some_and(|filled| (filled - 0.4).abs() < 1e-9));
        assert_eq!(both.percent, Some(40));
        let failed = Ring::combined(&[ring(ProgressState::Normal, Some(20)), ring(ProgressState::Error, None)]);
        assert_eq!(failed.map(|ring| ring.tone), Some(Tone::Error));
        let unknown = Ring::combined(&[ring(ProgressState::Indeterminate, None)]).unwrap();
        assert_eq!((unknown.filled, unknown.tone), (None, Tone::Normal));
        let paused = Ring::combined(&[ring(ProgressState::Paused, Some(10)), ring(ProgressState::Paused, None)]);
        assert_eq!(paused.map(|ring| ring.tone), Some(Tone::Paused));
    }
}

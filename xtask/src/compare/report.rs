//! The results of a comparison as `report.md` and `results.json`.

use std::fmt::Write as _;
use std::path::Path;

use serde::Serialize;

use crate::compare::stats::{Smoothness, Summary};
use crate::files;

/// The Mac and the conditions of a run.
#[derive(Debug, Clone, Default, Serialize)]
pub(crate) struct Machine {
    pub(crate) macos: String,
    pub(crate) model: String,
    pub(crate) keyboard_layout: String,
    pub(crate) refresh_hz: f64,
    /// Where `refresh_hz` came from.
    pub(crate) refresh_source: String,
    /// x, y, width and height of both apps' window, in points from the top left of the main display.
    pub(crate) window: [f64; 4],
    pub(crate) runs: usize,
    pub(crate) cold_runs: usize,
    pub(crate) started_unix: u64,
}

/// Every sample, in milliseconds, as measured.
#[derive(Debug, Clone, Default, Serialize)]
pub(crate) struct Raw {
    pub(crate) typing_rust_ms: Vec<f64>,
    pub(crate) typing_markdown_ms: Vec<f64>,
    pub(crate) tab_switch_ms: Vec<f64>,
    pub(crate) scroll_intervals_ms: Vec<f64>,
    pub(crate) cold_window_ms: Vec<f64>,
    pub(crate) cold_content_ms: Vec<f64>,
    pub(crate) cold_key_ms: Vec<f64>,
}

/// What one app measured.
#[derive(Debug, Clone, Default, Serialize)]
pub(crate) struct AppResult {
    pub(crate) app: String,
    pub(crate) version: String,
    pub(crate) bundle_id: String,
    pub(crate) typing_rust: Option<Summary>,
    pub(crate) typing_markdown: Option<Summary>,
    pub(crate) scroll: Option<Smoothness>,
    pub(crate) tab_switch: Option<Summary>,
    pub(crate) cold_window: Option<Summary>,
    pub(crate) cold_content: Option<Summary>,
    pub(crate) cold_key: Option<Summary>,
    /// What went wrong or differently along the way.
    pub(crate) notes: Vec<String>,
    pub(crate) raw: Raw,
}

#[derive(Debug, Clone, Default, Serialize)]
pub(crate) struct Report {
    pub(crate) machine: Machine,
    pub(crate) apps: Vec<AppResult>,
}

/// Writes `report.md` and `results.json` into `dir` and names the first.
pub(crate) fn write(dir: &Path, report: &Report) -> Result<std::path::PathBuf, String> {
    files::create_dir_all(dir)?;
    let json = serde_json::to_string_pretty(report).map_err(|e| format!("results.json: {e}"))?;
    files::write(dir.join("results.json"), json + "\n")?;
    let path = dir.join("report.md");
    files::write(&path, markdown(report))?;
    Ok(path)
}

fn number(value: f64) -> String {
    format!("{value:.1}")
}

fn latency_rows(out: &mut String, title: &str, pick: impl Fn(&AppResult) -> Option<&Summary>, apps: &[AppResult]) {
    for app in apps {
        let _ = match pick(app) {
            Some(s) => writeln!(
                out,
                "| {title} | {} | {} | {} | {} | {} | {} |",
                app.app,
                s.samples,
                s.missed,
                number(s.p50_ms),
                number(s.p95_ms),
                number(s.max_ms)
            ),
            None => writeln!(out, "| {title} | {} | 0 | – | – | – | – |", app.app),
        };
    }
}

fn latencies(out: &mut String, apps: &[AppResult]) {
    out.push_str("## Reaktion auf Eingaben (ms)\n\n");
    out.push_str("| Messung | App | Proben | ohne Änderung | p50 | p95 | max |\n|---|---|---:|---:|---:|---:|---:|\n");
    latency_rows(out, "Taste → Bild, Rust Mitte", |a| a.typing_rust.as_ref(), apps);
    latency_rows(out, "Taste → Bild, Markdown oben", |a| a.typing_markdown.as_ref(), apps);
    latency_rows(out, "Tab-Wechsel", |a| a.tab_switch.as_ref(), apps);
    out.push('\n');
}

fn scrolling(out: &mut String, apps: &[AppResult]) {
    out.push_str("## Weiches Scrollen (Pixel-Ereignisse alle 8,33 ms, je 3 s)\n\n");
    out.push_str(
        "| App | Abstand p50 | p95 | max | lange Abstände | verlorene Frames | geänderte / erwartete Frames |\n\
         |---|---:|---:|---:|---:|---:|---:|\n",
    );
    for app in apps {
        let _ = match &app.scroll {
            Some(s) => writeln!(
                out,
                "| {} | {} | {} | {} | {} | {} | {} / {} |",
                app.app,
                number(s.intervals.p50_ms),
                number(s.intervals.p95_ms),
                number(s.intervals.max_ms),
                s.long_intervals,
                s.dropped_frames,
                s.changed_frames,
                s.expected_frames
            ),
            None => writeln!(out, "| {} | – | – | – | – | – | – |", app.app),
        };
    }
    out.push('\n');
}

fn cold(out: &mut String, apps: &[AppResult]) {
    out.push_str("## Kaltstart ab `open` (ms)\n\n");
    out.push_str("| Messung | App | Proben | ohne Ergebnis | p50 | p95 | max |\n|---|---|---:|---:|---:|---:|---:|\n");
    latency_rows(out, "Fenster gelistet", |a| a.cold_window.as_ref(), apps);
    latency_rows(out, "erstes Bild mit Inhalt", |a| a.cold_content.as_ref(), apps);
    latency_rows(out, "erste angenommene Taste", |a| a.cold_key.as_ref(), apps);
    out.push('\n');
}

fn machine(out: &mut String, report: &Report) {
    let m = &report.machine;
    out.push_str("## Umgebung\n\n");
    let _ = writeln!(
        out,
        "- macOS {}, {}, Tastaturbelegung {}",
        m.macos, m.model, m.keyboard_layout
    );
    let _ = writeln!(
        out,
        "- Bildwiederholrate {} Hz ({}), ein Frame {} ms",
        number(m.refresh_hz),
        m.refresh_source,
        number(if m.refresh_hz > 0.0 { 1000.0 / m.refresh_hz } else { 0.0 })
    );
    let [x, y, w, h] = m.window;
    let _ = writeln!(out, "- Fenster beider Apps bei ({x}, {y}), {w} × {h} pt");
    let _ = writeln!(out, "- {} Proben je Messung, {} Kaltstarts", m.runs, m.cold_runs);
    for app in &report.apps {
        let _ = writeln!(out, "- {} {} ({})", app.app, app.version, app.bundle_id);
    }
    out.push('\n');
}

fn caveats(out: &mut String, apps: &[AppResult]) {
    out.push_str("## Grenzen der Messung\n\n");
    out.push_str(
        "- Gemessen wird von außen: vom Zeitpunkt direkt vor dem Senden des Ereignisses bis zur Anzeigezeit des ersten \
         Frames von ScreenCaptureKit, der im Fensterinhalt unter der Titelleiste anders aussieht. Die Auflösung ist durch das \
         Frame-Intervall der Anzeige begrenzt; ein Wert ist also bis zu einem Frame zu groß, und Unterschiede unter \
         einem Frame sind Rauschen.\n\
         - Zellen, die sich vor einer Eingabe von allein ändern (ein blinkender Cursor), zählen bei dieser Eingabe nicht.\n\
         - Der Kaltstart misst das erste Bild mit Inhalt erst ab dem Start der Aufnahme, die das gelistete Fenster \
         braucht; die erste angenommene Taste erkennt eine flache, breite Änderung wie eine getippte Zeile.\n\
         - Beide Apps laufen mit eigenem, leerem Home-Ordner, ohne Sprachserver, Vorschläge, Telemetrie und Updates.\n",
    );
    for app in apps {
        for note in &app.notes {
            let _ = writeln!(out, "- {}: {note}", app.app);
        }
    }
}

/// The report as Markdown.
pub(crate) fn markdown(report: &Report) -> String {
    let mut out = String::from("# Reaktionszeiten im Vergleich\n\n");
    machine(&mut out, report);
    latencies(&mut out, &report.apps);
    scrolling(&mut out, &report.apps);
    cold(&mut out, &report.apps);
    caveats(&mut out, &report.apps);
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::compare::stats::summarize;

    #[test]
    fn the_report_shows_each_app_with_its_figures_and_a_dash_for_a_missing_one() {
        let report = Report {
            machine: Machine {
                macos: "27.2".into(),
                refresh_hz: 60.0,
                ..Machine::default()
            },
            apps: vec![
                AppResult {
                    app: "One".into(),
                    typing_rust: summarize(&[4.0, 8.0, 12.0], 1),
                    notes: vec!["went fine".into()],
                    ..AppResult::default()
                },
                AppResult {
                    app: "Two".into(),
                    ..AppResult::default()
                },
            ],
        };
        let text = markdown(&report);
        assert!(
            text.contains("| Taste → Bild, Rust Mitte | One | 3 | 1 | 8.0 | 12.0 | 12.0 |"),
            "{text}"
        );
        assert!(text.contains("| Taste → Bild, Rust Mitte | Two | 0 | – | – | – | – |"));
        assert!(text.contains("ein Frame 16.7 ms"));
        assert!(text.contains("- One: went fine"));
        assert!(serde_json::to_string(&report).unwrap().contains("\"typing_rust_ms\""));
    }
}

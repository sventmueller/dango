//! The history of the timing and memory budgets: `make perf` and the perf self-tests write one measurement per file
//! to `target/perf/` (`ide_core::perf`), `perf-compare` holds them against the checked-in `perf/baseline.json` and
//! `perf-accept` takes them as the new baseline. See docs/development.md, "Timing budgets".

use std::path::{Path, PathBuf};
use std::process::Command;

use serde::{Deserialize, Serialize};

use crate::*;

/// The variable that tells the app of a self-test where its measurements go, as `ide_core::perf` reads it.
pub(crate) const DIR_VARIABLE: &str = "IDE_PERF_DIR";

/// How much worse than its baseline a measurement may be before it fails: 20 %.
const TOLERANCE: f64 = 1.2;

/// How many milliseconds a time must also lose before it fails: runs of the same code on the same idle Mac differ by up
/// to about that much, which for a value under a few milliseconds is far beyond the tolerance.
const NOISE_MS: f64 = 1.0;

/// One measurement, as `ide_core::perf::Measurement` writes it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct Measurement {
    name: String,
    value: f64,
    unit: String,
    lower_is_better: bool,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    samples: Vec<f64>,
}

/// The accepted values and the Mac they were measured on, by `sysctl hw.model`.
#[derive(Debug, Default, PartialEq, Serialize, Deserialize)]
struct Baseline {
    machine: Option<String>,
    measurements: Vec<Measurement>,
}

/// What a measurement is next to its baseline.
#[derive(Debug, PartialEq)]
enum Verdict {
    /// No accepted value of this name and unit.
    New,
    Within,
    /// Beyond the tolerance, with how much worse in percent.
    Worse(f64),
}

/// Where the measurements of a run go.
pub(crate) fn dir() -> PathBuf {
    target_dir().join("perf")
}

fn baseline_file() -> PathBuf {
    root().join("perf/baseline.json")
}

/// Empties `target/perf/`, so that a run compares only what it measured.
pub(crate) fn clear() -> Result<(), String> {
    let dir = dir();
    if dir.exists() {
        files::remove_dir_all(&dir)?;
    }
    settle();
    Ok(())
}

/// The load of the last minute below which the Mac counts as idle enough to measure.
const IDLE_LOAD: f64 = 2.0;
/// How long to wait for that at most; a busy Mac still gets measured, and its slower numbers then say so.
const SETTLE_LIMIT: std::time::Duration = std::time::Duration::from_secs(300);

/// Waits until the Mac is idle, as after `/ci`'s coverage stage the measurements would otherwise carry its heat and load.
pub(crate) fn settle() {
    let started = std::time::Instant::now();
    loop {
        let load = std::process::Command::new("sysctl")
            .args(["-n", "vm.loadavg"])
            .output()
            .ok()
            .and_then(|out| minute_load(&String::from_utf8_lossy(&out.stdout)));
        match load {
            Some(load) if load >= IDLE_LOAD && started.elapsed() < SETTLE_LIMIT => {
                eprintln!("perf: waiting for the Mac to settle, load {load:.1}");
                std::thread::sleep(std::time::Duration::from_secs(10));
            }
            _ => return,
        }
    }
}

/// The load of the last minute from `sysctl -n vm.loadavg`, which prints `{ 1.23 1.45 1.67 }`.
fn minute_load(text: &str) -> Option<f64> {
    text.split_whitespace().find_map(|word| word.parse().ok())
}

/// Compares the measurements in `target/perf/` with the baseline; fails for one worse than its baseline by more than
/// the tolerance, unless the baseline is of another Mac, where it only reports.
pub(crate) fn compare() -> Result<(), String> {
    let measured = read_measurements(&dir())?;
    if measured.is_empty() {
        return Err(format!("no measurements in {}; make perf writes them", dir().display()));
    }
    let baseline = read_baseline(&baseline_file())?;
    let (lines, worse) = comparison(&measured, &baseline, &machine()?);
    for line in lines {
        println!("{line}");
    }
    if worse.is_empty() {
        Ok(())
    } else {
        Err(format!(
            "worse than the baseline by more than 20 %: {}",
            worse.join(", ")
        ))
    }
}

/// Writes the measurements in `target/perf/` as `perf/baseline.json` for this Mac, keeping the accepted values of
/// this Mac the run did not measure.
pub(crate) fn accept() -> Result<(), String> {
    let measured = read_measurements(&dir())?;
    if measured.is_empty() {
        return Err(format!("no measurements in {}; make perf writes them", dir().display()));
    }
    let file = baseline_file();
    let old = if file.exists() {
        read_baseline(&file)?
    } else {
        Baseline::default()
    };
    let baseline = accepted(old, measured, machine()?);
    if let Some(folder) = file.parent() {
        files::create_dir_all(folder)?;
    }
    let mut text = serde_json::to_string_pretty(&baseline).map_err(|e| e.to_string())?;
    text.push('\n');
    files::write(&file, text)?;
    println!(
        "{}: {} measurements of {}",
        file.display(),
        baseline.measurements.len(),
        baseline.machine.as_deref().unwrap_or("?")
    );
    Ok(())
}

/// The model of this Mac, such as `Mac16,1`.
fn machine() -> Result<String, String> {
    Ok(stdout(Command::new("sysctl").args(["-n", "hw.model"]))?
        .trim()
        .to_string())
}

fn read_baseline(file: &Path) -> Result<Baseline, String> {
    serde_json::from_str(&files::read_to_string(file)?).map_err(|e| format!("{}: {e}", file.display()))
}

/// Every measurement in `dir`, by name; none if it does not exist.
fn read_measurements(dir: &Path) -> Result<Vec<Measurement>, String> {
    if !dir.exists() {
        return Ok(Vec::new());
    }
    let mut measured = Vec::new();
    for entry in files::read_dir(dir)?.flatten() {
        let path = entry.path();
        if path.extension().is_none_or(|e| e != "json") {
            continue;
        }
        let text = files::read_to_string(&path)?;
        let measurement: Measurement = serde_json::from_str(&text).map_err(|e| format!("{}: {e}", path.display()))?;
        measured.push(measurement);
    }
    measured.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(measured)
}

fn verdict(current: &Measurement, accepted: Option<&Measurement>) -> Verdict {
    let Some(accepted) = accepted.filter(|a| a.unit == current.unit && a.lower_is_better == current.lower_is_better)
    else {
        return Verdict::New;
    };
    let (worse, base) = if current.lower_is_better {
        let noise = if current.unit == "ms" { NOISE_MS } else { 0.0 };
        (
            current.value > accepted.value * TOLERANCE && current.value - accepted.value > noise,
            accepted.value,
        )
    } else {
        (current.value * TOLERANCE < accepted.value, current.value)
    };
    if !worse {
        return Verdict::Within;
    }
    let ratio = if current.lower_is_better {
        current.value / base
    } else {
        accepted.value / base
    };
    Verdict::Worse((ratio - 1.0) * 100.0)
}

/// The report of `measured` against `baseline` on `machine`, a line each, and the names of those that fail.
fn comparison(measured: &[Measurement], baseline: &Baseline, machine: &str) -> (Vec<String>, Vec<String>) {
    let same_mac = baseline.machine.as_deref() == Some(machine);
    let mut lines = Vec::new();
    if !same_mac {
        lines.push(format!(
            "perf: the baseline is of {}, this is {machine}; reporting only",
            baseline.machine.as_deref().unwrap_or("no Mac")
        ));
    }
    let mut worse = Vec::new();
    for current in measured {
        let accepted = baseline.measurements.iter().find(|a| a.name == current.name);
        let value = format!("{:.2} {}", current.value, current.unit);
        let line = match (verdict(current, accepted), accepted) {
            (Verdict::New, _) | (_, None) => format!("  new       {value:>14}  {}", current.name),
            (Verdict::Within, Some(a)) => {
                format!("  ok        {value:>14}  {} (baseline {:.2})", current.name, a.value)
            }
            (Verdict::Worse(percent), Some(a)) => {
                if same_mac {
                    worse.push(current.name.clone());
                }
                let mark = if same_mac { "WORSE" } else { "worse" };
                format!(
                    "  {mark:<9} {value:>14}  {} (baseline {:.2}, {percent:.0} % worse)",
                    current.name, a.value
                )
            }
        };
        lines.push(line);
    }
    for missing in baseline
        .measurements
        .iter()
        .filter(|a| !measured.iter().any(|m| m.name == a.name))
    {
        lines.push(format!("  unmeasured {:>13}  {}", "", missing.name));
    }
    (lines, worse)
}

/// The baseline after accepting `measured` on `machine` over `old`, by name and without the samples.
fn accepted(old: Baseline, measured: Vec<Measurement>, machine: String) -> Baseline {
    let mut measurements: Vec<Measurement> = if old.machine.as_deref() == Some(machine.as_str()) {
        old.measurements
            .into_iter()
            .filter(|a| !measured.iter().any(|m| m.name == a.name))
            .collect()
    } else {
        Vec::new()
    };
    measurements.extend(measured.into_iter().map(|m| Measurement {
        samples: Vec::new(),
        ..m
    }));
    measurements.sort_by(|a, b| a.name.cmp(&b.name));
    Baseline {
        machine: Some(machine),
        measurements,
    }
}

#[cfg(test)]
#[path = "perf_tests.rs"]
mod tests;

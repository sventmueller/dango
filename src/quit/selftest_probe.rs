//! The self-test's hooks of `quit.rs`.

/// Every self-test quits at its end with whatever its tabs left running; only the scenario about this question asks.
pub(crate) fn asks_in_selftest() -> bool {
    std::env::var("IDE_SELFTEST").map_or(true, |scenario| scenario == "quit")
}

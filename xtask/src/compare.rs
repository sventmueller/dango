//! `cargo xtask compare`: how fast this app and Zed answer input, measured from outside and the same way for both.
//! Each app runs as an instance of its own with a scratch home; events go to that process alone and ScreenCaptureKit
//! watches its windows.

#[cfg(any(target_os = "macos", test))]
#[cfg_attr(
    target_os = "macos",
    allow(unsafe_code, reason = "mach_absolute_time and mach_timebase_info")
)]
mod clock;
#[cfg(any(target_os = "macos", test))]
mod detect;
#[cfg(any(target_os = "macos", test))]
mod fingerprint;
#[cfg(any(target_os = "macos", test))]
mod fixtures;
#[cfg(any(target_os = "macos", test))]
#[cfg_attr(
    target_os = "macos",
    allow(unsafe_code, reason = "the keyboard layout through Carbon")
)]
mod keys;
mod options;
#[cfg(any(target_os = "macos", test))]
mod report;
#[cfg(any(target_os = "macos", test))]
mod stats;

#[cfg(target_os = "macos")]
#[allow(unsafe_code, reason = "Accessibility, window list and process calls")]
mod apps;
#[cfg(target_os = "macos")]
#[allow(unsafe_code, reason = "ScreenCaptureKit, CoreMedia and CoreVideo")]
mod capture;
#[cfg(target_os = "macos")]
mod cold;
#[cfg(target_os = "macos")]
mod debug;
#[cfg(target_os = "macos")]
#[allow(unsafe_code, reason = "NSUserDefaults of another domain")]
mod defaults;
#[cfg(target_os = "macos")]
#[allow(unsafe_code, reason = "the characters of a key event")]
mod input;
#[cfg(target_os = "macos")]
#[allow(unsafe_code, reason = "a signal handler")]
mod interrupt;
#[cfg(target_os = "macos")]
mod measure;
#[cfg(target_os = "macos")]
#[allow(unsafe_code, reason = "the permission queries")]
mod permissions;
#[cfg(target_os = "macos")]
mod run;
#[cfg(target_os = "macos")]
#[allow(unsafe_code, reason = "Accessibility values and arrays")]
mod tree;

/// Runs the comparison the arguments describe.
pub(crate) fn compare(args: &[String]) -> Result<(), String> {
    if args.iter().any(|a| a == "--help" || a == "-h") {
        println!("usage: cargo xtask {}", help());
        return Ok(());
    }
    let options = options::parse(args, crate::target_dir().join("compare"))?;
    #[cfg(target_os = "macos")]
    {
        let options = options::Options {
            out: crate::root().join(&options.out),
            ..options
        };
        if options.check {
            run::check(&options)
        } else {
            run::run(&options)
        }
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = options;
        Err("compare drives macOS apps and runs only on macOS".into())
    }
}

/// The lines of the task list that describe `compare`.
fn help() -> String {
    let lines: Vec<&str> = crate::USAGE.lines().collect();
    let start = lines.iter().position(|l| l.starts_with("  compare ")).unwrap_or(0);
    let length = lines[start + 1..]
        .iter()
        .take_while(|l| l.starts_with("      "))
        .count()
        + 1;
    lines[start..start + length]
        .iter()
        .map(|l| l.trim_start())
        .collect::<Vec<_>>()
        .join("\n")
}

#[cfg(test)]
mod tests {
    #[test]
    fn the_help_is_the_task_lists_entry() {
        let help = super::help();
        assert!(help.starts_with("compare [--apps"), "{help}");
        assert!(help.contains("--check only checks permissions and apps"));
        assert!(!help.contains("compile assets"));
    }
}

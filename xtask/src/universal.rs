//! The universal programs the disk image carries, for Apple Silicon and Intel in one file each (E16).

use std::path::{Path, PathBuf};
use std::process::Command;

use crate::*;

/// Each processor's target for cargo and its architecture for `lipo`.
pub(crate) const UNIVERSAL: [(&str, &str); 2] = [("aarch64-apple-darwin", "arm64"), ("x86_64-apple-darwin", "x86_64")];

/// Builds the release of the programs `names` of the app's package with `features` for each processor and joins each
/// into `target/universal/<name>`, through rustup with the toolchain rust-toolchain.toml names, since Homebrew's Rust
/// ships the standard library for the host processor alone.
pub(crate) fn universal_programs(names: &[&str], features: &[&str]) -> Result<Vec<PathBuf>, String> {
    let dir = universal_dir();
    for (target, _) in UNIVERSAL {
        let mut build = cargo_for(target)?;
        build.args([
            "build",
            "--locked",
            "--release",
            "--package",
            PACKAGE,
            "--target",
            target,
        ]);
        for name in names {
            build.args(["--bin", name]);
        }
        if !features.is_empty() {
            build.args(["--features", &features.join(",")]);
        }
        run(&mut build)?;
    }
    names
        .iter()
        .map(|name| {
            let mut join = Command::new("lipo");
            join.arg("-create");
            for (target, _) in UNIVERSAL {
                join.arg(dir.join(target).join("release").join(name));
            }
            let program = dir.join(name);
            run(join.arg("-output").arg(&program))?;
            Ok(program)
        })
        .collect()
}

/// The universal programs build in a target folder of their own: whisper-rs-sys, under dictation, hands the `GGML_*`
/// settings to whisper.cpp but does not rebuild when they change, so other builds for these targets must not share
/// its output.
fn universal_dir() -> PathBuf {
    target_dir().join("universal")
}

/// cargo of rust-toolchain.toml with what a build of the distributed program for `target` needs, as Lowspeak's.
fn cargo_for(target: &str) -> Result<Command, String> {
    let mut cargo = rustup_cargo()?;
    cargo
        .env("CARGO_TARGET_DIR", universal_dir())
        // whisper.cpp otherwise tunes itself for the processor it is built on, which other Macs may lack.
        .env("GGML_NATIVE", "OFF");
    if target.starts_with("x86_64") {
        // Every Intel Mac that runs macOS 14 has these.
        for flag in [
            "GGML_SSE42",
            "GGML_AVX",
            "GGML_AVX2",
            "GGML_BMI2",
            "GGML_FMA",
            "GGML_F16C",
        ] {
            cargo.env(flag, "ON");
        }
    }
    Ok(cargo)
}

/// cargo of the toolchain rust-toolchain.toml names, with its targets.
fn rustup_cargo() -> Result<Command, String> {
    let root = root();
    let rustup = rustup()?;
    run(Command::new(&rustup).current_dir(&root).args(["toolchain", "install"]))?;
    let tool = |name: &str| {
        stdout(Command::new(&rustup).current_dir(&root).args(["which", name])).map(|path| path.trim().to_string())
    };
    let mut cargo = Command::new(tool("cargo")?);
    // Cargo would otherwise run the rustc first on the PATH, which may be Homebrew's.
    cargo.current_dir(&root).env("RUSTC", tool("rustc")?);
    Ok(cargo)
}

/// rustup from the PATH, or from Homebrew, which keeps it off the PATH beside its own Rust.
fn rustup() -> Result<PathBuf, String> {
    let on_path = Command::new("rustup")
        .arg("--version")
        .output()
        .is_ok_and(|out| out.status.success());
    if on_path {
        return Ok("rustup".into());
    }
    stdout(Command::new("brew").args(["--prefix", "rustup"]))
        .map(|prefix| Path::new(prefix.trim()).join("bin/rustup"))
        .ok()
        .filter(|path| path.exists())
        .ok_or_else(|| "this needs rustup; install it with `brew install rustup`".into())
}

/// The architectures of `UNIVERSAL` that are not in the output of `lipo -archs`.
pub(crate) fn missing_architectures(listed: &str) -> Vec<&'static str> {
    UNIVERSAL
        .iter()
        .map(|(_, arch)| *arch)
        .filter(|arch| !listed.split_whitespace().any(|name| name == *arch))
        .collect()
}

/// A program without the code for one processor goes unnoticed on the other and keeps the app from starting there:
/// checks the app's program, its services' and Sparkle's.
pub(crate) fn check_universal(app: &Path) -> Result<(), String> {
    let mut programs = vec![executable(app)];
    programs.extend(SERVICES.iter().map(|service| service.executable(app)));
    if updates::framework(app).exists() {
        programs.extend(updates::sparkle_programs(app));
    }
    for program in programs {
        let missing = missing_architectures(&stdout(Command::new("lipo").arg("-archs").arg(&program))?);
        if !missing.is_empty() {
            return Err(format!("{} lacks {}", program.display(), missing.join(" and ")));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_program_is_universal_only_with_the_code_for_both_processors() {
        assert!(missing_architectures("x86_64 arm64\n").is_empty());
        assert_eq!(missing_architectures("arm64\n"), ["x86_64"]);
        assert_eq!(missing_architectures("arm64e x86_64h\n"), ["arm64", "x86_64"]);
    }

    #[test]
    fn the_toolchain_file_names_both_processors() {
        let toolchain = files::read_to_string(root().join("rust-toolchain.toml")).unwrap();
        for (target, _) in UNIVERSAL {
            assert!(toolchain.contains(&format!("\"{target}\"")), "{target}");
        }
    }
}

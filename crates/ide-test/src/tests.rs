//! Each adapter against what its tool printed for the same small project: a passing, a failing, a skipped and a
//! parametrised or nested test, recorded under `tests/fixtures/<tool>/` with the tool's version in the file name.

use std::path::{Path, PathBuf};

use super::*;

fn fixture(tool: &str, name: &str) -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(tool)
        .join(name);
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}

fn sample(tool: &str) -> PathBuf {
    PathBuf::from("/private/tmp/dango-samples").join(tool)
}

fn state(reports: &[Report], name: &str) -> State {
    reports
        .iter()
        .find(|r| r.id.name == name)
        .map_or(State::NotRun, |r| r.state)
}

fn report<'a>(reports: &'a [Report], name: &str) -> &'a Report {
    reports
        .iter()
        .find(|r| r.id.name == name)
        .unwrap_or_else(|| panic!("no {name} in {reports:#?}"))
}

#[test]
fn cargo_test_lists_by_test_program_and_reads_libtest_text_of_rust_1_99() {
    let root = sample("rust");
    let tests = Tool::Cargo.tests(&root, &fixture("cargo", "list-1.99.txt"));
    let names: Vec<(&str, &str)> = tests
        .iter()
        .map(|t| (t.id.package.as_str(), t.id.name.as_str()))
        .collect();
    assert_eq!(
        names,
        [
            ("sample", "tests::adds"),
            ("sample", "tests::fails"),
            ("sample", "tests::nested::inner"),
            ("sample", "tests::slow"),
            ("api", "api_works"),
        ]
    );
    let reports = Tool::Cargo.reports(&root, &fixture("cargo", "run-1.99.txt"));
    assert_eq!(state(&reports, "tests::adds"), State::Passed);
    assert_eq!(state(&reports, "tests::slow"), State::Skipped);
    assert_eq!(state(&reports, "api_works"), State::Passed);
    let failed = report(&reports, "tests::fails");
    assert_eq!(failed.state, State::Failed);
    assert_eq!(
        failed.failure,
        Some(Location {
            path: root.join("src/lib.rs"),
            line: 16
        })
    );
    assert!(
        failed.message.as_deref().is_some_and(|m| m.contains("one and two")),
        "{failed:?}"
    );
    assert!(!Tool::Cargo.reports_starts() && Tool::Cargo.lists_by_building());
}

#[test]
fn nextest_lists_its_suites_and_reads_the_junit_report_of_0_9_151() {
    let root = sample("rust");
    let tests = Tool::Nextest.tests(&root, &fixture("nextest", "list-0.9.151.json"));
    assert!(
        tests
            .iter()
            .any(|t| t.id.package == "sample::api" && t.id.name == "api_works")
    );
    assert!(
        tests
            .iter()
            .any(|t| t.id.package == "sample" && t.id.name == "tests::nested::inner")
    );
    let reports = Tool::Nextest.reports(&root, &fixture("nextest", "run-0.9.151.junit.xml"));
    assert_eq!(state(&reports, "tests::adds"), State::Passed);
    let failed = report(&reports, "tests::fails");
    assert_eq!(failed.state, State::Failed);
    assert_eq!(
        failed.failure,
        Some(Location {
            path: root.join("src/lib.rs"),
            line: 16
        })
    );
    let run = Tool::Nextest.run(&root, &Scope::All, Some(Path::new("/tmp/s")));
    assert!(
        run.args
            .windows(2)
            .any(|w| w[0] == "--tool-config-file" && w[1] == "dango:/tmp/s/dango-nextest.toml")
    );
    assert_eq!(run.files[0].1, nextest::CONFIG);
}

#[test]
fn go_lists_top_level_tests_and_reads_the_json_events_of_go_1_27() {
    let root = sample("go");
    // The recorded events name files by the module path the recording's go.mod declared.
    std::fs::create_dir_all(&root).unwrap();
    std::fs::write(root.join("go.mod"), "module example.com/sample\n\ngo 1.27\n").unwrap();
    let tests = Tool::Go.tests(&root, &fixture("go", "list-1.27.jsonl"));
    let names: Vec<&str> = tests.iter().map(|t| t.id.name.as_str()).collect();
    assert_eq!(names, ["TestPass", "TestFail", "TestSkip", "TestTable"]);
    let reports = Tool::Go.reports(&root, &fixture("go", "run-1.27.jsonl"));
    assert_eq!(state(&reports, "TestPass"), State::Passed);
    assert_eq!(state(&reports, "TestSkip"), State::Skipped);
    assert_eq!(state(&reports, "TestTable/case_one"), State::Passed);
    let failed = report(&reports, "TestTable/case_two");
    assert_eq!(failed.state, State::Failed);
    assert_eq!(
        failed.failure,
        Some(Location {
            path: root.join("sample_test.go"),
            line: 23
        })
    );
    assert_eq!(failed.message.as_deref(), Some("second case fails"));
    assert!(Tool::Go.reports_starts());
}

#[test]
fn pytest_reads_collection_starts_and_results_from_dangos_plugin_with_pytest_9_2() {
    let root = sample("py");
    let tests = Tool::Pytest.tests(&root, &fixture("pytest", "list-9.2.jsonl"));
    assert_eq!(tests.len(), 5);
    let param = tests
        .iter()
        .find(|t| t.id.name == "test_sample.py::test_param[2]")
        .unwrap();
    assert_eq!(
        param.location,
        Some(Location {
            path: root.join("test_sample.py"),
            line: 17
        })
    );
    let reports = Tool::Pytest.reports(&root, &fixture("pytest", "run-9.2.jsonl"));
    assert_eq!(state(&reports, "test_sample.py::test_pass"), State::Passed);
    assert_eq!(state(&reports, "test_sample.py::test_skip"), State::Skipped);
    let failed = report(&reports, "test_sample.py::test_param[2]");
    assert_eq!(failed.state, State::Failed);
    assert_eq!(
        failed.failure,
        Some(Location {
            path: root.join("test_sample.py"),
            line: 19
        })
    );
    assert_eq!(failed.message.as_deref(), Some("assert 2 == 1"));
}

#[test]
fn vitest_lists_tests_with_their_place_and_reads_its_json_report_of_5_1() {
    let root = sample("vitest");
    let tests = Tool::Vitest.tests(&root, &fixture("vitest", "list-5.1.json"));
    let adds = tests.iter().find(|t| t.id.name == "math > adds").unwrap();
    assert_eq!(adds.location.as_ref().map(|l| l.line), Some(2));
    let reports = Tool::Vitest.reports(&root, &fixture("vitest", "run-5.1.json"));
    assert_eq!(state(&reports, "math > adds"), State::Passed);
    assert_eq!(state(&reports, "math > later"), State::Skipped);
    let failed = report(&reports, "math > fails");
    assert_eq!(failed.state, State::Failed);
    assert_eq!(failed.failure.as_ref().map(|f| f.line), Some(6));
}

#[test]
fn jest_lists_files_and_takes_its_tests_from_the_json_of_30_5() {
    let root = sample("jest");
    let files = Tool::Jest.tests(&root, &fixture("jest", "list-30.5.txt"));
    assert_eq!(files.len(), 1);
    assert!(files[0].id.name.is_empty());
    let reports = Tool::Jest.reports(&root, &fixture("jest", "run-30.5.json"));
    assert_eq!(state(&reports, "math > adds"), State::Passed);
    assert_eq!(state(&reports, "math > later"), State::Skipped);
    let failed = report(&reports, "math > fails");
    assert_eq!(failed.failure.as_ref().map(|f| f.line), Some(6));
    assert_eq!(failed.location.as_ref().map(|l| l.line), Some(5));
}

#[test]
fn one_test_runs_and_debugs_with_its_own_filter() {
    let root = Path::new("/p");
    let rust = TestId {
        package: "api".into(),
        file: None,
        name: "api_works".into(),
    };
    assert_eq!(
        Tool::Cargo.run(root, &Scope::Test(rust.clone()), None).args[2..],
        ["--", "api_works", "--exact"]
    );
    match Tool::Cargo.debug(root, &rust) {
        DebugTarget::Native { build, args } => {
            assert!(build.args.contains(&"ide-debug".to_string()) && build.args.contains(&"api".to_string()));
            assert_eq!(args, ["api_works", "--exact", "--nocapture"]);
        }
        other => panic!("{other:?}"),
    }
    let go = TestId {
        package: "example.com/m/sub".into(),
        file: None,
        name: "TestTable/case_one".into(),
    };
    let module = ide_core::testing::scratch("test-go");
    std::fs::write(module.join("go.mod"), "module example.com/m\n").unwrap();
    match Tool::Go.debug(&module, &go) {
        DebugTarget::Go { package, args } => {
            assert_eq!(package, module.join("sub"));
            assert_eq!(args, ["-test.run", "^TestTable$/^case_one$"]);
        }
        other => panic!("{other:?}"),
    }
    let py = TestId {
        package: String::new(),
        file: None,
        name: "t.py::test_a[1]".into(),
    };
    assert!(
        matches!(Tool::Pytest.debug(root, &py), DebugTarget::PythonModule { module, args, .. }
        if module == "pytest" && args == ["t.py::test_a[1]"])
    );
    let js = TestId {
        package: String::new(),
        file: Some("/p/a.test.js".into()),
        name: "math > adds".into(),
    };
    match Tool::Jest.debug(root, &js) {
        DebugTarget::Node { program, args, .. } => {
            assert_eq!(program, Path::new("/p/node_modules/jest/bin/jest.js"));
            assert_eq!(args, ["--runInBand", "/p/a.test.js", "-t", "^math adds$"]);
        }
        other => panic!("{other:?}"),
    }
}

#[test]
fn the_tools_of_a_project_follow_its_files() {
    let root = ide_core::testing::scratch("test-tools");
    std::fs::create_dir_all(root.join("bin")).unwrap();
    std::fs::write(root.join("Cargo.toml"), "").unwrap();
    std::fs::write(root.join("go.mod"), "module m\n").unwrap();
    std::fs::write(root.join("package.json"), r#"{"devDependencies":{"vitest":"^5"}}"#).unwrap();
    assert_eq!(tools(&root, None), [Tool::Cargo, Tool::Go, Tool::Vitest]);
    std::fs::write(root.join("bin/cargo-nextest"), "").unwrap();
    let path = root.join("bin").display().to_string();
    assert_eq!(tools(&root, Some(&path))[0], Tool::Nextest);
}

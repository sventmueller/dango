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
    let half: String = fixture("cargo", "run-1.99.txt")
        .lines()
        .take(6)
        .map(|l| format!("{l}\n"))
        .collect();
    let so_far = Tool::Cargo.reports(&root, &half);
    assert_eq!(
        state(&so_far, "tests::adds"),
        State::Passed,
        "libtest's text reads as it comes"
    );
}

#[test]
fn nextest_lists_its_suites_and_reads_the_libtest_json_of_0_9_152_as_it_goes() {
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
    let run = fixture("nextest", "run-0.9.152.jsonl");
    let reports = Tool::Nextest.reports(&root, &run);
    assert_eq!(state(&reports, "tests::adds"), State::Passed);
    assert_eq!(state(&reports, "tests::slow"), State::Skipped);
    assert_eq!(report(&reports, "api_works").id.package, "sample::api");
    let failed = report(&reports, "tests::fails");
    assert_eq!((failed.state, failed.id.package.as_str()), (State::Failed, "sample"));
    assert_eq!(
        failed.failure,
        Some(Location {
            path: root.join("src/lib.rs"),
            line: 16
        })
    );
    assert!(
        failed
            .message
            .as_deref()
            .is_some_and(|m| m.contains("one and two") && !m.contains("note:")),
        "{failed:?}"
    );
    let half: String = run.lines().take(6).map(|l| format!("{l}\n")).collect();
    let so_far = Tool::Nextest.reports(&root, &half);
    assert_eq!(state(&so_far, "tests::adds"), State::Running);
    assert!(Tool::Nextest.reports_starts() && Tool::Nextest.lists_by_building());
    let one = TestId {
        package: "sample::api".into(),
        file: None,
        name: "api_works".into(),
    };
    let run = Tool::Nextest.run(&root, &Scope::Test(one), None);
    assert!(
        run.args
            .windows(2)
            .any(|w| w == ["--message-format", "libtest-json-plus"])
    );
    assert!(
        run.args
            .ends_with(&["-E".into(), "binary_id(sample::api) & test(=api_works)".into()])
    );
    assert!(
        run.env
            .contains(&("NEXTEST_EXPERIMENTAL_LIBTEST_JSON".into(), "1".into()))
    );
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
fn vitest_lists_tests_with_their_place_and_reads_dangos_reporter_with_5_1() {
    let root = sample("vitest");
    let tests = Tool::Vitest.tests(&root, &fixture("vitest", "list-5.1.json"));
    let adds = tests.iter().find(|t| t.id.name == "math > adds").unwrap();
    assert_eq!(adds.location.as_ref().map(|l| l.line), Some(2));
    let run = fixture("vitest", "run-5.1.jsonl");
    let reports = Tool::Vitest.reports(&root, &run);
    assert_eq!(state(&reports, "math > adds"), State::Passed);
    assert_eq!(state(&reports, "math > later"), State::Skipped);
    let failed = report(&reports, "math > fails");
    assert_eq!(failed.state, State::Failed);
    assert_eq!(failed.failure.as_ref().map(|f| f.line), Some(6));
    assert_eq!(
        failed.message.as_deref(),
        Some("AssertionError: expected 3 to be 4 // Object.is equality")
    );
    assert_eq!(
        failed.id.file, adds.id.file,
        "the run names the file as the listing does"
    );
    let started: String = run.lines().take(3).map(|l| format!("{l}\n")).collect();
    assert_eq!(
        state(&Tool::Vitest.reports(&root, &started), "math > fails"),
        State::Running
    );
    let scratch = Path::new("/tmp/s");
    let one = Tool::Vitest.run(&root, &Scope::Test(adds.id.clone()), Some(scratch));
    assert!(
        one.args.windows(2).any(|w| w == ["-t", "^math > adds$"]),
        "{:?}",
        one.args
    );
    assert_eq!(
        one.args.last().map(String::as_str),
        Some("--reporter=/tmp/s/dango_vitest.mjs")
    );
    assert_eq!(
        one.results.as_deref(),
        Some(scratch.join("dango-js-events.jsonl").as_path())
    );
    assert!(Tool::Vitest.reports_starts());
}

#[test]
fn jest_lists_files_and_takes_its_tests_from_dangos_reporter_with_30_5() {
    let root = sample("jest");
    let files = Tool::Jest.tests(&root, &fixture("jest", "list-30.5.txt"));
    assert_eq!(files.len(), 1);
    assert!(files[0].id.name.is_empty());
    let reports = Tool::Jest.reports(&root, &fixture("jest", "run-30.5.jsonl"));
    assert_eq!(state(&reports, "math > adds"), State::Passed);
    assert_eq!(state(&reports, "math > later"), State::Skipped);
    let failed = report(&reports, "math > fails");
    assert_eq!(failed.failure.as_ref().map(|f| f.line), Some(6));
    assert_eq!(failed.location.as_ref().map(|l| l.line), Some(5));
    let one = Tool::Jest.run(&root, &Scope::Test(failed.id.clone()), Some(Path::new("/tmp/s")));
    let flags: Vec<&str> = one.args.iter().map(String::as_str).collect();
    assert!(
        flags.ends_with(&[
            "-t",
            "^math fails$",
            "--reporters=default",
            "--reporters=/tmp/s/dango_jest.cjs"
        ]),
        "the reporters come last, as Jest takes what follows its flag as reporters: {flags:?}"
    );
}

#[test]
fn bun_lists_files_and_reads_the_junit_parts_of_1_4() {
    let root = sample("bun");
    let files = Tool::Bun.tests(&root, &fixture("bun", "list-runner.txt"));
    let files: Vec<_> = files.iter().map(|t| (t.id.file.clone(), t.id.name.as_str())).collect();
    assert_eq!(
        files,
        [
            (Some(root.join("src/math.test.ts")), ""),
            (Some(root.join("src/view.test.tsx")), "")
        ],
        "a runner's own columns and summary are no files"
    );
    let reports = Tool::Bun.reports(&root, &fixture("bun", "run-1.4.xml"));
    assert_eq!(state(&reports, "math & <stuff> > adds"), State::Passed);
    assert_eq!(state(&reports, "math & <stuff> > skipped"), State::Skipped);
    assert_eq!(state(&reports, "math & <stuff> > inner > deep"), State::Passed);
    assert_eq!(state(&reports, "later"), State::Skipped);
    let failed = report(&reports, "math & <stuff> > fails \"quoted\"");
    assert_eq!(failed.state, State::Failed);
    assert_eq!(failed.location.as_ref().map(|l| l.line), Some(4));
    assert_eq!(failed.failure.as_ref().map(|f| f.line), Some(4));
    assert!(
        failed.message.as_deref().is_some_and(|m| m.contains("Received: 1")),
        "{failed:?}"
    );
    let second = report(&reports, "x");
    assert_eq!(
        second.failure.as_ref().map(|f| f.path.clone()),
        Some(root.join("other_test.ts"))
    );
    assert_eq!(
        second.message.as_deref(),
        Some("boom"),
        "the second document of the file counts too"
    );
}

#[test]
fn bun_runs_its_test_script_or_bun_test_itself() {
    let root = ide_core::testing::scratch("test-bun-modes");
    std::fs::create_dir_all(&root).unwrap();
    let scratch = Path::new("/tmp/s");
    let test = TestId {
        package: String::new(),
        file: Some(root.join("a.test.ts")),
        name: "math > adds".into(),
    };
    std::fs::write(root.join("package.json"), r#"{"scripts":{"test":"bun test"}}"#).unwrap();
    assert_eq!(tools(&root, None), [(Tool::Bun, root.to_path_buf())]);
    let direct = Tool::Bun.run(&root, &Scope::Test(test.clone()), Some(scratch));
    assert_eq!(direct.args[..2], ["test", "--reporter=junit"]);
    assert!(
        direct.args.ends_with(&["-t".into(), "^math adds$".into()]),
        "{:?}",
        direct.args
    );
    assert_eq!(direct.results.as_deref(), Some(Path::new("/tmp/s/dango-bun-junit.xml")));
    assert_eq!(Tool::Bun.list(&root, scratch).args, ["/tmp/s/dango_bun_list.mjs"]);
    std::fs::write(root.join("package.json"), r#"{"scripts":{"test":"bun tools/run.mjs"}}"#).unwrap();
    let script = Tool::Bun.run(&root, &Scope::All, Some(scratch));
    assert_eq!(script.args[..3], ["run", "test", "--reporter=junit"]);
    assert_eq!(Tool::Bun.list(&root, scratch).args, ["run", "test", "--list"]);
    assert!(Tool::Bun.debug(&root, &test).is_none());
    std::fs::write(root.join("package.json"), r#"{"scripts":{"test":"node --test"}}"#).unwrap();
    assert!(tools(&root, None).is_empty());
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
    match Tool::Cargo.debug(root, &rust).unwrap() {
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
    match Tool::Go.debug(&module, &go).unwrap() {
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
        matches!(Tool::Pytest.debug(root, &py).unwrap(), DebugTarget::PythonModule { module, args, .. }
        if module == "pytest" && args == ["t.py::test_a[1]"])
    );
    let js = TestId {
        package: String::new(),
        file: Some("/p/a.test.js".into()),
        name: "math > adds".into(),
    };
    match Tool::Jest.debug(root, &js).unwrap() {
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
    let found = |path: Option<&str>| tools(&root, path).into_iter().map(|(tool, _)| tool).collect::<Vec<_>>();
    assert_eq!(found(None), [Tool::Cargo, Tool::Go, Tool::Vitest]);
    std::fs::write(root.join("bin/cargo-nextest"), "").unwrap();
    let path = root.join("bin").display().to_string();
    assert_eq!(found(Some(&path))[0], Tool::Nextest);
}

#[test]
fn a_tool_whose_files_are_in_a_subfolder_runs_there() {
    let root = ide_core::testing::scratch("test-tools-below");
    for folder in ["backend", "tools/gg", "frontend/node_modules/dep", "testdata/go"] {
        std::fs::create_dir_all(root.join(folder)).unwrap();
    }
    std::fs::write(root.join("package.json"), "{}").unwrap();
    std::fs::write(root.join("backend/go.mod"), "module example.com/backend\n").unwrap();
    std::fs::write(root.join("tools/gg/go.mod"), "module example.com/gg\n").unwrap();
    std::fs::write(root.join("testdata/go/go.mod"), "module example.com/data\n").unwrap();
    let dep = r#"{"devDependencies":{"vitest":"^5"}}"#;
    std::fs::write(root.join("frontend/node_modules/dep/package.json"), dep).unwrap();
    assert_eq!(tools(&root, None), [(Tool::Go, root.join("backend"))]);
    std::fs::write(root.join("go.mod"), "module example.com/root\n").unwrap();
    assert_eq!(
        tools(&root, None),
        [(Tool::Go, root.to_path_buf())],
        "the project's own folder comes first"
    );
}

#[test]
fn a_run_read_in_pieces_ends_with_the_reports_of_the_whole() {
    for (tool, dir, name) in [
        (Tool::Cargo, "cargo", "run-1.99.txt"),
        (Tool::Nextest, "nextest", "run-0.9.152.jsonl"),
        (Tool::Go, "go", "run-1.27.jsonl"),
        (Tool::Pytest, "pytest", "run-9.2.jsonl"),
        (Tool::Vitest, "vitest", "run-5.1.jsonl"),
        (Tool::Jest, "jest", "run-30.5.jsonl"),
        (Tool::Bun, "bun", "run-1.4.xml"),
    ] {
        let text = fixture(dir, name);
        let root = ide_core::testing::scratch(&format!("test-{dir}-in-pieces"));
        std::fs::create_dir_all(&root).unwrap();
        std::fs::write(root.join("go.mod"), "module example.com/sample\n\ngo 1.27\n").unwrap();
        let whole = tool.reports(&root, &text);
        let mut reading = Reading::new(tool, &root);
        let mut merged: Vec<Option<Report>> = Vec::new();
        let mut take = |changed: Vec<(usize, Report)>| {
            for (index, report) in changed {
                if merged.len() <= index {
                    merged.resize(index + 1, None);
                }
                merged[index] = Some(report);
            }
        };
        let mut rest = text.as_str();
        while !rest.is_empty() {
            let mut cut = rest.len().min(97);
            while !rest.is_char_boundary(cut) {
                cut += 1;
            }
            reading.feed(&rest[..cut]);
            rest = &rest[cut..];
            take(reading.changed());
        }
        reading.end();
        take(reading.changed());
        assert!(reading.changed().is_empty(), "{dir}: nothing came after the last look");
        let merged: Vec<Report> = merged.into_iter().map(Option::unwrap).collect();
        assert!(!whole.is_empty(), "{dir}");
        assert_eq!(merged, whole, "{dir}");
    }
}

#[test]
fn test_files_are_told_apart_by_the_names_their_tools_give_them() {
    for test in [
        "a/b_test.go",
        "test_a.py",
        "a_test.py",
        "src/a.test.ts",
        "a.spec.jsx",
        "a_spec.mjs",
    ] {
        assert!(named_as_test(Path::new(test)), "{test}");
    }
    for other in [
        "a/b.go",
        "a.py",
        "testing.py",
        "src/a.ts",
        "Cargo.toml",
        "src/lib.rs",
        "test",
    ] {
        assert!(!named_as_test(Path::new(other)), "{other}");
    }
}

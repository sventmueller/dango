//! Sessions with real programs in real PTYs.

mod common;

use std::time::{Duration, Instant};

use common::{Harness, alive, command, gone_within, temporary};
use ide_process::Ending;
use ide_terminal::{
    Agent, CellFlags, CellSide, Clipboard, Event, Foreground, Key, NamedKey, Options, PasteAction, Point,
    SelectionKind, Size, WorkingDirectory,
};

fn interactive_sh(dir: &std::path::Path) -> Harness {
    interactive_sh_with(command("/bin/sh", &["-i"], dir), dir)
}

fn interactive_sh_with(options: Options, dir: &std::path::Path) -> Harness {
    let mut harness = Harness::start(options, dir);
    harness.session.write("PS1='$ '\n");
    assert!(harness.wait_until(|h| h.screen_line("$")), "the shell prompts");
    harness
}

#[test]
fn typed_keys_reach_the_shell_and_its_output_lands_on_the_screen() {
    let dir = temporary("round-trip");
    let mut harness = interactive_sh(&dir);
    for c in "printf '%s-%s\\n' round trip".chars() {
        assert!(harness.session.key(&Key::text(c)));
    }
    harness.session.key(&Key::named(NamedKey::Enter));
    assert!(
        harness.wait_until(|h| h.screen_line("round-trip")),
        "{:?}",
        harness.session.visible_text()
    );
    assert!(harness.saw(&Event::Redraw));
}

#[test]
fn clearing_drops_the_history_and_the_lines_above_the_prompt() {
    let dir = temporary("clear");
    let mut harness = interactive_sh(&dir);
    harness
        .session
        .write("i=0; while [ $i -lt 60 ]; do i=$((i+1)); echo row$i; done; echo done\n");
    assert!(harness.wait_until(|h| h.screen_line("done") && h.session.snapshot().history > 0));
    harness.session.write("printf 'kept'\n");
    assert!(harness.wait_until(|h| h.screen_line("kept$")));
    harness.session.clear(true);
    let screen = harness.session.snapshot();
    assert_eq!(screen.history, 0);
    assert_eq!(screen.cursor.point.line, 0);
    let text = harness.session.visible_text();
    assert_eq!(text[0], "kept$", "{text:?}");
    assert!(text[1..].iter().all(String::is_empty), "{text:?}");
}

#[test]
fn a_cli_session_is_interrupted_when_it_closes() {
    let dir = temporary("interrupt");
    let marker = dir.join("interrupted");
    let script = format!(
        "trap 'echo yes > {}; exit 0' INT; echo ready; while :; do sleep 0.1; done",
        marker.display()
    );
    let options = Options {
        interrupt_on_close: true,
        ..command("/bin/sh", &["-c", &script], &dir)
    };
    let mut harness = Harness::start(options, &dir);
    assert!(harness.wait_until(|h| h.screen_line("ready")));
    let shell = i32::try_from(harness.session.pid()).unwrap();
    harness.session.close();
    let deadline = Instant::now() + Duration::from_secs(5);
    while !marker.exists() && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(20));
    }
    assert!(marker.exists(), "Ctrl+C did not reach the program");
    assert!(gone_within(shell, Duration::from_secs(5)));
}

#[test]
fn a_title_alone_changes_no_text() {
    let dir = temporary("content");
    let script = "printf '\\033]0;quiet\\007'; sleep 1; printf x; sleep 30";
    let mut harness = Harness::start(command("/bin/sh", &["-c", script], &dir), &dir);
    assert!(harness.wait_until(|h| h.saw(&Event::Title(Some("quiet".into())))));
    assert_eq!(harness.session.content_changes(), 0);
    assert!(harness.wait_until(|h| h.screen_line("x")));
    assert!(harness.session.content_changes() > 0);
}

#[test]
fn the_programs_in_front_are_named() {
    let dir = temporary("programs");
    let mut harness = interactive_sh(&dir);
    harness.session.write("sleep 30\n");
    assert!(
        harness.wait_until(|h| h.session.foreground_programs().contains(&"sleep".to_string())),
        "{:?}",
        harness.session.foreground_programs()
    );
}

#[test]
fn closing_a_session_returns_at_once_and_ends_the_job_in_front() {
    let dir = temporary("close");
    let mut harness = interactive_sh(&dir);
    harness.session.write("/bin/sh -c 'echo pid=$$; exec sleep 30'\n");
    assert!(harness.wait_until(|h| h.number_after("pid=").is_some()));
    let sleeper = harness.number_after("pid=").unwrap();
    assert!(harness.wait_until(|h| h.session.foreground() == Foreground::Other("sleep".into())));
    let shell = i32::try_from(harness.session.pid()).unwrap();

    let started = Instant::now();
    harness.session.close();
    assert!(
        started.elapsed() < Duration::from_millis(50),
        "close took {:?}",
        started.elapsed()
    );

    assert!(gone_within(sleeper, Duration::from_secs(5)), "the job survived");
    assert!(gone_within(shell, Duration::from_secs(5)), "the shell survived");
    let deadline = Instant::now() + Duration::from_secs(5);
    while !harness.supervisor.running().is_empty() && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(20));
    }
    assert!(
        harness.supervisor.running().is_empty(),
        "the supervisor still lists the shell"
    );
}

#[test]
fn a_job_that_ignores_hang_up_and_sigterm_is_killed_after_the_grace_periods() {
    let dir = temporary("stubborn");
    let options = Options {
        close_grace: Duration::from_millis(200),
        ..command("/bin/sh", &["-i"], &dir)
    };
    let mut harness = Harness::start(options, &dir);
    harness
        .session
        .write("/bin/sh -c 'trap \"\" HUP TERM; echo pid=$$; exec sleep 30'\n");
    assert!(harness.wait_until(|h| h.number_after("pid=").is_some()));
    let stubborn = harness.number_after("pid=").unwrap();
    assert!(harness.wait_until(|h| h.session.foreground() == Foreground::Other("sleep".into())));
    harness.session.close();
    std::thread::sleep(Duration::from_millis(100));
    assert!(alive(stubborn), "hang-up alone should not have ended it");
    assert!(gone_within(stubborn, Duration::from_secs(3)), "SIGKILL did not come");
}

#[test]
fn the_end_of_the_program_is_reported_with_its_exit_code_and_the_supervisor_forgets_it() {
    let dir = temporary("exit");
    let mut harness = Harness::start(command("/bin/sh", &["-c", "exit 7"], &dir), &dir);
    assert!(
        harness.wait_until(|h| h.saw(&Event::Exited(Ending::Code(7)))),
        "{:?}",
        harness.events
    );
    assert!(!harness.session.is_running());
    assert!(harness.wait_until(|h| h.supervisor.running().is_empty()));
}

#[test]
fn osc_sequences_and_server_announcements_become_events() {
    let dir = temporary("osc");
    let script = r"printf '\033]0;hello\007\007\033]52;c;aGk=\007\033]52;c;?\007'; printf '\033]7;file:///tmp/a%%20b\007'; echo 'Local: http://localhost:5173/'";
    let mut harness = Harness::start(command("/bin/sh", &["-c", script], &dir), &dir);
    assert!(
        harness.wait_until(|h| h.saw(&Event::Exited(Ending::Code(0)))),
        "{:?}",
        harness.events
    );
    let wanted = [
        Event::Title(Some("hello".into())),
        Event::Bell,
        Event::ClipboardStore {
            clipboard: Clipboard::Clipboard,
            text: "hi".into(),
        },
        Event::WorkingDirectory(WorkingDirectory {
            host: String::new(),
            path: "/tmp/a b".into(),
        }),
        Event::DevServer("http://localhost:5173/".into()),
    ];
    for event in &wanted {
        assert!(harness.saw(event), "missing {event:?} in {:?}", harness.events);
    }
    assert!(
        !harness
            .events
            .iter()
            .any(|event| matches!(event, Event::ClipboardLoad(_))),
        "reading is denied"
    );
}

#[test]
fn a_clipboard_read_is_asked_for_only_when_allowed_and_answered_in_osc_52() {
    let dir = temporary("clipboard");
    let script = r"stty raw -echo; printf '\033]52;c;?\007'; reply=$(dd bs=1 count=12 2>/dev/null); stty sane; echo; echo got:$(printf '%s' $reply | tr -d '\033\007')";
    let options = Options {
        allow_clipboard_read: true,
        ..command("/bin/sh", &["-c", script], &dir)
    };
    let mut harness = Harness::start(options, &dir);
    assert!(harness.wait_until(|h| h.saw(&Event::ClipboardLoad(Clipboard::Clipboard))));
    harness.session.answer_clipboard("hi");
    assert!(
        harness.wait_until(|h| h.screen_has("got:")),
        "{:?}",
        harness.session.visible_text()
    );
    assert!(
        harness.screen_line("got:]52;c;aGk="),
        "{:?}",
        harness.session.visible_text()
    );
}

#[test]
fn the_snapshot_has_resolved_colours_attributes_cursor_and_selection() {
    let dir = temporary("snapshot");
    let script = r"printf '\033[1;31mred\033[0m \033[4mline\033[0m'; sleep 5";
    let mut harness = Harness::start(command("/bin/sh", &["-c", script], &dir), &dir);
    assert!(harness.wait_until(|h| h.screen_has("red line")));
    let palette = ide_terminal::Palette::default();
    let snapshot = harness.session.snapshot();
    assert_eq!((snapshot.columns, snapshot.lines), (80, 24));
    let first = &snapshot.rows[0][0];
    assert_eq!(first.ch, 'r');
    assert_eq!(first.fg, palette.ansi[9], "bold red is drawn bright");
    assert!(first.flags.contains(CellFlags::BOLD));
    assert!(snapshot.rows[0][4].flags.contains(CellFlags::UNDERLINE));
    assert_eq!(snapshot.rows[0][3].bg, palette.background);
    assert_eq!(snapshot.cursor.point, Point { line: 0, column: 8 });
    assert!(snapshot.cursor.visible);
    assert_eq!(snapshot.damage, ide_terminal::Damage::Full);

    harness
        .session
        .select(SelectionKind::Word, Point { line: 0, column: 5 }, CellSide::Left);
    assert_eq!(harness.session.selected_text().as_deref(), Some("line"));
    let selection = harness.session.snapshot().selection.expect("a visible selection");
    assert_eq!((selection.start.column, selection.end.column), (4, 7));
    harness
        .session
        .select(SelectionKind::Line, Point { line: 0, column: 0 }, CellSide::Left);
    assert_eq!(harness.session.selected_text().as_deref(), Some("red line\n"));
    harness.session.clear_selection();
    assert_eq!(harness.session.selected_text(), None);
}

#[test]
fn a_resize_reaches_the_program() {
    let dir = temporary("resize");
    let mut harness = interactive_sh(&dir);
    harness.session.resize(Size {
        columns: 100,
        lines: 30,
        cell_width: 9,
        cell_height: 18,
    });
    harness.session.write("stty size\n");
    assert!(
        harness.wait_until(|h| h.screen_line("30 100")),
        "{:?}",
        harness.session.visible_text()
    );
    assert_eq!(harness.session.snapshot().columns, 100);
}

#[test]
fn an_agent_in_front_gets_ctrl_v_and_a_shell_gets_the_images_path() {
    let dir = temporary("agent");
    let agent = dir.join("claude");
    std::os::unix::fs::symlink("/bin/sleep", &agent).unwrap();
    let images = dir.join("images");
    let wide = Options {
        size: Size {
            columns: 300,
            ..Size::default()
        },
        ..command("/bin/sh", &["-i"], &dir)
    };
    let mut harness = interactive_sh_with(wide, &dir);

    assert!(harness.wait_until(|h| h.session.foreground() == Foreground::Shell("sh".into())));
    let action = harness.session.paste_image(b"png", &images).unwrap();
    let PasteAction::InsertPath(path) = action else {
        panic!("a shell gets a path, not {action:?}");
    };
    let name = path.file_name().unwrap().to_string_lossy().into_owned();
    assert!(
        harness.wait_until(|h| h.screen_has(&name)),
        "{:?}",
        harness.session.visible_text()
    );
    harness.session.key(&Key::text('u').with(ide_terminal::Modifiers {
        control: true,
        ..Default::default()
    }));

    harness.session.write(format!("{} 30\n", agent.display()));
    assert!(harness.wait_until(|h| h.session.foreground() == Foreground::Agent(Agent::Claude)));
    assert_eq!(
        harness.session.paste_image(b"png", &images).unwrap(),
        PasteAction::SendCtrlV
    );
}

#[test]
fn zsh_integration_marks_commands_and_still_reads_the_users_startup_files() {
    let home = temporary("zsh-home");
    std::fs::write(home.join(".zshenv"), "export MARK_ENV=1\n").unwrap();
    std::fs::write(home.join(".zprofile"), "export MARK_PROFILE=1\n").unwrap();
    std::fs::write(
        home.join(".zshrc"),
        "export MARK_RC=1\nPS1='%# '\nprecmd() { USER_PRECMD=1 }\n",
    )
    .unwrap();
    let scripts = temporary("zsh-scripts");
    let options = Options {
        program: ide_terminal::Program::LoginShell("/bin/zsh".into()),
        shell_integration: Some(scripts.to_path_buf()),
        ..Options::new(&*home)
    };
    let mut harness = Harness::start(options, &home);
    assert!(
        harness.wait_until(|h| h.saw(&Event::Prompt)),
        "{:?}",
        harness.session.visible_text()
    );
    harness
        .session
        .write("echo \"marks:$MARK_ENV$MARK_PROFILE$MARK_RC$USER_PRECMD:${ZDOTDIR-unset}\"; (exit 3)\n");
    assert!(
        harness.wait_until(|h| h.saw(&Event::CommandFinished { exit_code: Some(3) })),
        "{:?}",
        harness.events
    );
    assert!(
        harness.screen_line("marks:1111:unset"),
        "{:?}",
        harness.session.visible_text()
    );
    assert!(harness.saw(&Event::CommandStarted));
    let cwd = harness.events.iter().find_map(|event| match event {
        Event::WorkingDirectory(directory) => Some(directory.clone()),
        _ => None,
    });
    let cwd = cwd.expect("zsh reports its directory");
    assert_eq!(cwd.path, *home);
    assert!(cwd.is_local());
}

fn bash_integration(bash: &str) {
    let home = temporary("bash-home");
    std::fs::write(home.join(".bash_profile"), "export MARK_PROFILE=1\n. ~/.bashrc\n").unwrap();
    let rc = "export MARK_RC=1\nPS1='$ '\nPROMPT_COMMAND='USER_PC=$((USER_PC+1))'\ntrap 'USER_TRAP=$((USER_TRAP+1))' DEBUG\n";
    std::fs::write(home.join(".bashrc"), rc).unwrap();
    let scripts = temporary("bash-scripts");
    let options = Options {
        program: ide_terminal::Program::LoginShell(bash.into()),
        shell_integration: Some(scripts.to_path_buf()),
        ..Options::new(&*home)
    };
    let mut harness = Harness::start(options, &home);
    assert!(
        harness.wait_until(|h| h.saw(&Event::Prompt)),
        "{:?}",
        harness.session.visible_text()
    );
    harness.session.write("trap_before=$USER_TRAP; pc_before=$USER_PC\n");
    harness.session.write(
        "echo \"marks:$MARK_PROFILE$MARK_RC:$((USER_TRAP > trap_before)):$((USER_PC > pc_before))\"; (exit 4)\n",
    );
    assert!(
        harness.wait_until(|h| h.saw(&Event::CommandFinished { exit_code: Some(4) })),
        "{:?}",
        harness.events
    );
    assert!(
        harness.screen_line("marks:11:1:1"),
        "{:?}",
        harness.session.visible_text()
    );
    assert!(harness.saw(&Event::CommandStarted));
    harness.events.clear();
    harness.session.write("\n");
    assert!(harness.wait_until(|h| h.saw(&Event::Prompt)));
    assert!(
        !harness
            .events
            .iter()
            .any(|event| matches!(event, Event::CommandFinished { .. })),
        "an empty line is no command: {:?}",
        harness.events
    );
    assert!(
        harness
            .events
            .iter()
            .any(|event| matches!(event, Event::WorkingDirectory(d) if d.path == *home))
    );
}

#[test]
fn bash_3_integration_marks_commands_and_keeps_the_users_prompt_command_and_debug_trap() {
    bash_integration("/bin/bash");
}

#[test]
fn bash_5_integration_marks_commands_and_keeps_the_users_prompt_command_and_debug_trap() {
    let bash = "/opt/homebrew/bin/bash";
    if !std::path::Path::new(bash).exists() {
        eprintln!("skipped: no {bash}");
        return;
    }
    bash_integration(bash);
}

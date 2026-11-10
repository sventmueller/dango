use super::*;

const SHIFT: Modifiers = Modifiers {
    shift: true,
    control: false,
    alt: false,
    command: false,
};
const CTRL: Modifiers = Modifiers {
    shift: false,
    control: true,
    alt: false,
    command: false,
};
const ALT: Modifiers = Modifiers {
    shift: false,
    control: false,
    alt: true,
    command: false,
};
const CTRL_SHIFT: Modifiers = Modifiers {
    shift: true,
    control: true,
    alt: false,
    command: false,
};
const COMMAND: Modifiers = Modifiers {
    shift: false,
    control: false,
    alt: false,
    command: true,
};

fn legacy() -> InputModes {
    InputModes::default()
}

fn kitty(flags: &[KittyFlags]) -> InputModes {
    InputModes {
        kitty: flags.iter().fold(KittyFlags::NONE, |all, flag| all.with(*flag)),
        ..InputModes::default()
    }
}

fn shifted(c: char, text: &str) -> Key {
    Key {
        code: KeyCode::Char(c),
        modifiers: SHIFT,
        text: Some(text.into()),
        keypad: false,
        action: KeyAction::Press,
    }
}

fn keypad(code: KeyCode, text: Option<&str>) -> Key {
    Key {
        code,
        modifiers: Modifiers::default(),
        text: text.map(Into::into),
        keypad: true,
        action: KeyAction::Press,
    }
}

fn released(key: Key) -> Key {
    Key {
        action: KeyAction::Release,
        ..key
    }
}

fn repeated(key: Key) -> Key {
    Key {
        action: KeyAction::Repeat,
        ..key
    }
}

fn check(cases: &[(Key, &str)], modes: &InputModes) {
    for (key, expected) in cases {
        let bytes = encode_key(key, modes);
        assert_eq!(String::from_utf8_lossy(&bytes), *expected, "{key:?} in {modes:?}");
    }
}

#[test]
fn legacy_keys_send_what_xterm_sends() {
    check(
        &[
            (Key::text('a'), "a"),
            (shifted('a', "A"), "A"),
            (Key::text('a').with(CTRL), "\x01"),
            (Key::text('a').with(CTRL_SHIFT), "\x01"),
            (Key::text(' ').with(CTRL), "\0"),
            (Key::text('[').with(CTRL), "\x1b"),
            (Key::text('/').with(CTRL), "\x1f"),
            (Key::text('1').with(CTRL), "1"),
            (Key::text('a').with(ALT), "\x1ba"),
            (Key::text('c').with(Modifiers { alt: true, ..CTRL }), "\x1b\x03"),
            (Key::text('a').with(COMMAND), ""),
            (Key::text('é'), "é"),
            (Key::named(NamedKey::Enter), "\r"),
            (Key::named(NamedKey::Enter).with(ALT), "\x1b\r"),
            (Key::named(NamedKey::Tab), "\t"),
            (Key::named(NamedKey::Tab).with(SHIFT), "\x1b[Z"),
            (Key::named(NamedKey::Backspace), "\x7f"),
            (Key::named(NamedKey::Backspace).with(CTRL), "\x08"),
            (Key::named(NamedKey::Backspace).with(ALT), "\x1b\x7f"),
            (Key::named(NamedKey::Escape), "\x1b"),
            (Key::named(NamedKey::Up), "\x1b[A"),
            (Key::named(NamedKey::Left).with(CTRL), "\x1b[1;5D"),
            (
                Key::named(NamedKey::Right).with(Modifiers { shift: true, ..ALT }),
                "\x1b[1;4C",
            ),
            (Key::named(NamedKey::Home), "\x1b[H"),
            (Key::named(NamedKey::End).with(SHIFT), "\x1b[1;2F"),
            (Key::named(NamedKey::Delete), "\x1b[3~"),
            (Key::named(NamedKey::PageUp).with(CTRL), "\x1b[5;5~"),
            (Key::named(NamedKey::F(1)), "\x1bOP"),
            (Key::named(NamedKey::F(4)).with(SHIFT), "\x1b[1;2S"),
            (Key::named(NamedKey::F(5)), "\x1b[15~"),
            (Key::named(NamedKey::F(12)).with(CTRL), "\x1b[24;5~"),
            (Key::named(NamedKey::F(20)), "\x1b[34~"),
            (Key::named(NamedKey::F(21)), ""),
            (Key::named(NamedKey::Modifier(ModifierKey::Shift, Side::Left)), ""),
            (released(Key::text('a')), ""),
            (repeated(Key::text('a')), "a"),
        ],
        &legacy(),
    );
}

#[test]
fn application_cursor_and_keypad_modes_switch_to_ss3() {
    let modes = InputModes {
        app_cursor: true,
        app_keypad: true,
        ..InputModes::default()
    };
    check(
        &[
            (Key::named(NamedKey::Up), "\x1bOA"),
            (Key::named(NamedKey::Home), "\x1bOH"),
            (Key::named(NamedKey::Up).with(CTRL), "\x1b[1;5A"),
            (keypad(KeyCode::Char('5'), Some("5")), "\x1bOu"),
            (keypad(KeyCode::Char('+'), Some("+")), "\x1bOk"),
            (keypad(KeyCode::Named(NamedKey::Enter), None), "\x1bOM"),
        ],
        &modes,
    );
    check(
        &[
            (keypad(KeyCode::Char('5'), Some("5")), "5"),
            (keypad(KeyCode::Named(NamedKey::Enter), None), "\r"),
        ],
        &legacy(),
    );
    let newline = InputModes {
        newline: true,
        ..InputModes::default()
    };
    check(&[(Key::named(NamedKey::Enter), "\r\n")], &newline);
}

#[test]
fn kitty_disambiguation_encodes_only_what_legacy_bytes_confuse() {
    check(
        &[
            (Key::text('a'), "a"),
            (shifted('a', "A"), "A"),
            (Key::text('a').with(CTRL), "\x1b[97;5u"),
            (Key::text('a').with(CTRL_SHIFT), "\x1b[97;6u"),
            (Key::text('a').with(ALT), "\x1b[97;3u"),
            (Key::text('v').with(CTRL), "\x1b[118;5u"),
            (Key::text('a').with(COMMAND), "\x1b[97;9u"),
            (Key::named(NamedKey::Escape), "\x1b[27u"),
            (Key::named(NamedKey::Enter), "\r"),
            (Key::named(NamedKey::Enter).with(SHIFT), "\x1b[13;2u"),
            (Key::named(NamedKey::Tab).with(SHIFT), "\x1b[9;2u"),
            (Key::named(NamedKey::Backspace).with(CTRL), "\x1b[127;5u"),
            (Key::named(NamedKey::Tab), "\t"),
            (Key::named(NamedKey::Backspace), "\x7f"),
            (Key::named(NamedKey::Up), "\x1b[A"),
            (Key::named(NamedKey::Up).with(SHIFT), "\x1b[1;2A"),
            (Key::named(NamedKey::F(1)), "\x1b[P"),
            (Key::named(NamedKey::F(3)), "\x1b[13~"),
            (Key::named(NamedKey::F(5)).with(CTRL), "\x1b[15;5~"),
            (Key::named(NamedKey::F(13)), "\x1b[57376u"),
            (Key::named(NamedKey::Delete), "\x1b[3~"),
            (keypad(KeyCode::Char('1'), Some("1")), "\x1b[57400u"),
            (keypad(KeyCode::Named(NamedKey::Enter), None), "\x1b[57414u"),
            (Key::named(NamedKey::CapsLock), ""),
            (
                Key::named(NamedKey::Modifier(ModifierKey::Control, Side::Left)).with(CTRL),
                "",
            ),
            (released(Key::text('a')), ""),
        ],
        &kitty(&[KittyFlags::DISAMBIGUATE]),
    );
}

#[test]
fn kitty_event_types_report_repeats_and_releases() {
    check(
        &[
            (Key::text('a'), "a"),
            (repeated(Key::text('a')), "a"),
            (released(Key::text('a')), "\x1b[97;1:3u"),
            (repeated(Key::text('a').with(CTRL)), "\x1b[97;5:2u"),
            (released(Key::named(NamedKey::Up)), "\x1b[1;1:3A"),
            (repeated(Key::named(NamedKey::Left)), "\x1b[1;1:2D"),
            (released(Key::named(NamedKey::Enter)), ""),
            (released(Key::named(NamedKey::Escape)), "\x1b[27;1:3u"),
        ],
        &kitty(&[KittyFlags::DISAMBIGUATE, KittyFlags::EVENT_TYPES]),
    );
}

#[test]
fn kitty_all_keys_mode_encodes_text_alternates_and_modifier_keys() {
    let modes = kitty(&[
        KittyFlags::DISAMBIGUATE,
        KittyFlags::ALL_KEYS,
        KittyFlags::ALTERNATE_KEYS,
    ]);
    check(
        &[
            (Key::text('a'), "\x1b[97u"),
            (shifted('a', "A"), "\x1b[97:65;2u"),
            (shifted('1', "!"), "\x1b[49:33;2u"),
            (Key::named(NamedKey::Enter), "\x1b[13u"),
            (Key::named(NamedKey::Backspace), "\x1b[127u"),
            (
                Key::named(NamedKey::Modifier(ModifierKey::Shift, Side::Left)).with(SHIFT),
                "\x1b[57441;2u",
            ),
            (
                Key::named(NamedKey::Modifier(ModifierKey::Command, Side::Right)).with(COMMAND),
                "\x1b[57450;9u",
            ),
            (Key::named(NamedKey::CapsLock), "\x1b[57358u"),
        ],
        &modes,
    );
    let with_text = kitty(&[KittyFlags::ALL_KEYS, KittyFlags::ASSOCIATED_TEXT]);
    check(
        &[
            (Key::text('a'), "\x1b[97;1;97u"),
            (shifted('a', "A"), "\x1b[97;2;65u"),
            (Key::text('a').with(CTRL), "\x1b[97;5u"),
        ],
        &with_text,
    );
}

#[test]
fn mouse_reports_follow_the_tracking_mode_and_encoding() {
    let at = |action, line, column, modifiers| MouseEvent {
        action,
        line,
        column,
        modifiers,
    };
    let press = at(MouseAction::Press(MouseButton::Left), 4, 9, Modifiers::default());
    let release = at(MouseAction::Release(MouseButton::Left), 4, 9, Modifiers::default());
    let drag = at(MouseAction::Move(Some(MouseButton::Left)), 5, 9, Modifiers::default());
    let hover = at(MouseAction::Move(None), 5, 9, Modifiers::default());
    let right_ctrl = at(MouseAction::Press(MouseButton::Right), 0, 0, CTRL);
    let wheel_up = at(MouseAction::Press(MouseButton::WheelUp), 0, 0, Modifiers::default());

    assert_eq!(encode_mouse(&press, &legacy()), None);

    let clicks = InputModes {
        mouse: MouseTracking::Clicks,
        ..InputModes::default()
    };
    assert_eq!(encode_mouse(&press, &clicks), Some(b"\x1b[M *%".to_vec()));
    assert_eq!(encode_mouse(&release, &clicks), Some(b"\x1b[M#*%".to_vec()));
    assert_eq!(encode_mouse(&right_ctrl, &clicks), Some(b"\x1b[M2!!".to_vec()));
    assert_eq!(encode_mouse(&drag, &clicks), None);
    let far = at(MouseAction::Press(MouseButton::Left), 0, 223, Modifiers::default());
    assert_eq!(encode_mouse(&far, &clicks), None);

    let utf8 = InputModes {
        mouse_encoding: MouseEncoding::Utf8,
        ..clicks
    };
    assert_eq!(encode_mouse(&far, &utf8), Some("\x1b[M \u{100}!".as_bytes().to_vec()));

    let drags = InputModes {
        mouse: MouseTracking::ButtonMotion,
        mouse_encoding: MouseEncoding::Sgr,
        ..InputModes::default()
    };
    assert_eq!(encode_mouse(&press, &drags), Some(b"\x1b[<0;10;5M".to_vec()));
    assert_eq!(encode_mouse(&release, &drags), Some(b"\x1b[<0;10;5m".to_vec()));
    assert_eq!(encode_mouse(&drag, &drags), Some(b"\x1b[<32;10;6M".to_vec()));
    assert_eq!(encode_mouse(&hover, &drags), None);
    assert_eq!(encode_mouse(&right_ctrl, &drags), Some(b"\x1b[<18;1;1M".to_vec()));
    assert_eq!(encode_mouse(&wheel_up, &drags), Some(b"\x1b[<64;1;1M".to_vec()));

    let any = InputModes {
        mouse: MouseTracking::AnyMotion,
        ..drags
    };
    assert_eq!(encode_mouse(&hover, &any), Some(b"\x1b[<35;10;6M".to_vec()));
}

#[test]
fn the_wheel_reports_scrolls_or_sends_arrows_on_the_alternate_screen() {
    let sgr = InputModes {
        mouse: MouseTracking::Clicks,
        mouse_encoding: MouseEncoding::Sgr,
        ..InputModes::default()
    };
    assert_eq!(
        encode_wheel(-2, 1, 1, Modifiers::default(), &sgr),
        Wheel::Send(b"\x1b[<65;2;2M\x1b[<65;2;2M".to_vec())
    );
    let pager = InputModes {
        alternate_screen: true,
        alternate_scroll: true,
        app_cursor: true,
        ..InputModes::default()
    };
    assert_eq!(
        encode_wheel(3, 0, 0, Modifiers::default(), &pager),
        Wheel::Send(b"\x1bOA\x1bOA\x1bOA".to_vec())
    );
    assert_eq!(encode_wheel(3, 0, 0, SHIFT, &pager), Wheel::Scroll(3));
    assert_eq!(
        encode_wheel(-1, 0, 0, Modifiers::default(), &legacy()),
        Wheel::Scroll(-1)
    );
}

#[test]
fn focus_changes_are_reported_only_when_asked_for() {
    let focus = InputModes {
        focus_reporting: true,
        ..InputModes::default()
    };
    assert_eq!(encode_focus(true, &focus), Some(&b"\x1b[I"[..]));
    assert_eq!(encode_focus(false, &focus), Some(&b"\x1b[O"[..]));
    assert_eq!(encode_focus(true, &legacy()), None);
}

#[test]
fn bracketed_paste_wraps_the_text_and_strips_every_end_marker() {
    let bracketed = InputModes {
        bracketed_paste: true,
        ..InputModes::default()
    };
    assert_eq!(encode_paste("ls\n", &bracketed), b"\x1b[200~ls\n\x1b[201~");
    assert_eq!(
        encode_paste("a\x1b[201~rm -rf ~\n", &bracketed),
        b"\x1b[200~arm -rf ~\n\x1b[201~"
    );
    assert_eq!(
        encode_paste("x\x1b[20\x1b[201~1~y", &bracketed),
        b"\x1b[200~xy\x1b[201~",
        "a marker hidden inside another one"
    );
    assert_eq!(encode_paste("one\r\ntwo\nthree", &legacy()), b"one\rtwo\rthree");
}

#[test]
fn modes_are_read_from_the_terminal() {
    let mode = TermMode::APP_CURSOR
        | TermMode::BRACKETED_PASTE
        | TermMode::MOUSE_DRAG
        | TermMode::SGR_MOUSE
        | TermMode::DISAMBIGUATE_ESC_CODES
        | TermMode::REPORT_EVENT_TYPES;
    let modes = InputModes::from_term(mode);
    assert!(modes.app_cursor && modes.bracketed_paste && !modes.app_keypad);
    assert_eq!(modes.mouse, MouseTracking::ButtonMotion);
    assert_eq!(modes.mouse_encoding, MouseEncoding::Sgr);
    assert_eq!(modes.kitty.bits(), 3);
}

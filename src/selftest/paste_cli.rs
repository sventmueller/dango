//! The `paste-cli` scenario, run by hand for the acceptance of E9: every coding agent's CLI that is installed starts in
//! a terminal tab, gets an image pasted while it waits for input and must show it as an attachment. No prompt is ever
//! sent; folder-trust questions at start are answered with their default. The user's pasteboard is put back
//! afterwards, also when a step fails.

use std::cell::{Cell, RefCell};
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::time::{Duration, Instant};

use ide_terminal::{Foreground, PasteAction};
use objc2::rc::Retained;
use objc2::runtime::ProtocolObject;
use objc2_app_kit::{
    NSBitmapImageRep, NSEventModifierFlags, NSPasteboard, NSPasteboardItem, NSPasteboardTypePNG, NSPasteboardTypeTIFF,
};
use objc2_foundation::{NSArray, NSData};

use super::{check, finish, helpers};

mod forget;
use crate::terminal::inspect;
use crate::util::{constant, main_soon, ns};
use crate::window::{self, WindowController};

const NAME: &str = "paste-cli";

/// The agents E9 names, by their command, without Gemini, which E67 no longer offers.
const AGENTS: [&str; 3] = ["claude", "codex", "agy"];

/// How long the CLIs get to start and settle, together.
const START: Duration = Duration::from_secs(45);
/// A screen unchanged this long counts as waiting for input.
const SETTLE: Duration = Duration::from_secs(2);
/// How long a CLI gets to show a pasted image.
const ATTACH: Duration = Duration::from_secs(8);
/// How long the CLIs get to end once their tabs closed.
const GONE: Duration = Duration::from_secs(10);
/// The time between two cleanings of the CLIs' settings, in which another running CLI may write back an entry it read
/// before.
const REWRITE_GAP: Duration = Duration::from_secs(4);

/// The general pasteboard's items, each as its types with their data.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Contents(Vec<Vec<(String, Vec<u8>)>>);

impl Contents {
    fn read() -> Contents {
        let items = NSPasteboard::generalPasteboard().pasteboardItems().unwrap_or_default();
        Contents(
            items
                .iter()
                .map(|item| {
                    item.types()
                        .iter()
                        .filter_map(|kind| Some((kind.to_string(), item.dataForType(&kind)?.to_vec())))
                        .collect()
                })
                .collect(),
        )
    }

    fn write(&self) {
        let items: Vec<Retained<ProtocolObject<dyn objc2_app_kit::NSPasteboardWriting>>> = self
            .0
            .iter()
            .map(|entries| {
                let item = NSPasteboardItem::new();
                for (kind, data) in entries {
                    item.setData_forType(&NSData::with_bytes(data), &ns(kind));
                }
                ProtocolObject::from_retained(item)
            })
            .collect();
        let pasteboard = NSPasteboard::generalPasteboard();
        pasteboard.clearContents();
        pasteboard.writeObjects(&NSArray::from_retained_slice(&items));
    }
}

/// The user's pasteboard, put back after each paste and when dropped, which also happens when a step panics.
struct Restore {
    saved: Contents,
    done: bool,
}

impl Restore {
    fn put_back(&self) {
        self.saved.write();
    }

    /// Puts the pasteboard back a last time; whether it then holds what it held at the start.
    fn finish(mut self) -> bool {
        self.done = true;
        self.put_back();
        Contents::read() == self.saved
    }
}

impl Drop for Restore {
    fn drop(&mut self) {
        if !self.done {
            self.put_back();
        }
    }
}

/// Where a CLI is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Phase {
    /// The shell starts.
    Shell,
    /// The CLI starts; once its screen settles it waits for input.
    Starting,
    Ready,
}

#[derive(Debug, Clone)]
struct Run {
    name: &'static str,
    id: u64,
    phase: Phase,
    screen: Vec<String>,
    since: Instant,
    trusted: bool,
}

pub fn run() {
    let _ = helpers::make_project("paste-cli-project", &[]);
    let root = helpers::root("paste-cli-project");
    helpers::open_trusted(&root);
    std::thread::spawn(move || {
        let path = crate::environment::wait(Duration::from_secs(30))
            .and_then(|env| {
                env.iter()
                    .find(|(name, _)| name == "PATH")
                    .map(|(_, value)| value.clone())
            })
            .unwrap_or_default();
        main_soon(move || start(&root, &path));
    });
}

fn controller_of(root: &Path) -> Option<Retained<WindowController>> {
    window::open_windows()
        .into_iter()
        .find(|controller| controller.project().as_deref() == Some(root))
}

fn installed(path: &str, name: &str) -> bool {
    use std::os::unix::fs::PermissionsExt;
    path.split(':').any(|dir| {
        std::fs::metadata(Path::new(dir).join(name))
            .is_ok_and(|meta| meta.is_file() && meta.permissions().mode() & 0o111 != 0)
    })
}

fn start(root: &Path, path: &str) {
    let (present, missing): (Vec<&'static str>, Vec<&'static str>) =
        AGENTS.into_iter().partition(|name| installed(path, name));
    check(
        "agent CLIs are installed",
        !present.is_empty(),
        format!("present: {present:?}, missing: {missing:?}"),
    );
    let Some(controller) = controller_of(root).filter(|_| !present.is_empty()) else {
        return finish(NAME);
    };
    let restore = Restore {
        saved: Contents::read(),
        done: false,
    };
    let mut runs = vec![];
    for name in present {
        crate::terminal::open_tab(&controller, Some(root));
        if let Some((id, _)) = inspect::latest() {
            runs.push(Run {
                name,
                id,
                phase: Phase::Shell,
                screen: vec![],
                since: Instant::now(),
                trusted: false,
            });
        }
    }
    wait_ready(runs, restore, root.to_path_buf());
}

/// A PNG of the scenario's picture, with a TIFF of it for programs that read only that.
fn put_picture() {
    let item = NSPasteboardItem::new();
    let png = NSData::with_bytes(&super::terminal::PICTURE);
    item.setData_forType(&png, constant!(NSPasteboardTypePNG));
    if let Some(tiff) = NSBitmapImageRep::imageRepWithData(&png).and_then(|rep| rep.TIFFRepresentation()) {
        item.setData_forType(&tiff, constant!(NSPasteboardTypeTIFF));
    }
    let pasteboard = NSPasteboard::generalPasteboard();
    pasteboard.clearContents();
    pasteboard.writeObjects(&NSArray::from_retained_slice(&[ProtocolObject::from_retained(item)]));
}

/// Moves every CLI on until all wait for input or the time is up.
fn wait_ready(runs: Vec<Run>, restore: Restore, root: PathBuf) {
    let runs = Rc::new(RefCell::new(runs));
    let stepping = Rc::clone(&runs);
    let all_ready = move || {
        let mut runs = stepping.borrow_mut();
        runs.iter_mut().for_each(step);
        runs.iter().all(|run| run.phase == Phase::Ready)
    };
    helpers::wait_within(START, all_ready, move |_| {
        let runs = runs.take();
        let (ready, stuck): (Vec<Run>, Vec<Run>) = runs.into_iter().partition(|run| run.phase == Phase::Ready);
        for run in &stuck {
            check(
                "every installed CLI starts and waits for input",
                false,
                format!("{}: {}", run.name, run.screen.join(" ⏎ ")),
            );
        }
        paste_next(ready, stuck, restore, root);
    });
}

fn step(run: &mut Run) {
    let Some(view) = inspect::view(run.id) else { return };
    match run.phase {
        Phase::Shell if view.prompts() > 0 => {
            view.type_text(&format!("{}\r", run.name));
            run.phase = Phase::Starting;
            run.since = Instant::now();
        }
        Phase::Starting => {
            let screen: Vec<String> = view
                .visible_lines()
                .into_iter()
                .filter(|line| !line.trim().is_empty())
                .collect();
            if screen != run.screen {
                run.screen = screen;
                run.since = Instant::now();
                return;
            }
            if run.since.elapsed() < SETTLE || !matches!(view.foreground(), Foreground::Agent(_)) {
                return;
            }
            let asks_trust = run.screen.iter().any(|line| line.to_lowercase().contains("trust"));
            if !asks_trust || run.trusted {
                run.phase = Phase::Ready;
                return;
            }
            match answer(&run.screen) {
                Answer::Confirm => {
                    view.press(0x24, "\r", "\r", NSEventModifierFlags::empty());
                    run.trusted = true;
                }
                Answer::Down => view.press(0x7d, "\u{f701}", "\u{f701}", NSEventModifierFlags::empty()),
            }
            run.since = Instant::now();
        }
        Phase::Shell | Phase::Ready => {}
    }
}

/// What to press in a folder-trust question.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Answer {
    Confirm,
    Down,
}

/// Enter once the chosen option says yes, else ↓ towards the option that does; Enter when there is none.
fn answer(screen: &[String]) -> Answer {
    let chosen = |line: &str| {
        ["❯", "›", "▸", ">"]
            .iter()
            .any(|mark| line.trim_start().starts_with(mark))
    };
    let yes = |line: &str| line.to_lowercase().contains("yes");
    let options_say_yes = screen.iter().any(|line| yes(line));
    match screen.iter().find(|line| chosen(line)) {
        Some(line) if options_say_yes && !yes(line) => Answer::Down,
        _ => Answer::Confirm,
    }
}

/// Whether `line` looks like an image attachment in a CLI's input.
fn shows_image(line: &str) -> bool {
    let line = line.to_lowercase();
    ["image", "[img", ".png", "screenshot"]
        .iter()
        .any(|mark| line.contains(mark))
}

/// Pastes into the first of `runs`, then the rest; then puts the pasteboard back, closes the tabs and ends.
fn paste_next(mut runs: Vec<Run>, mut done: Vec<Run>, restore: Restore, root: PathBuf) {
    if runs.is_empty() {
        check("the user's pasteboard is back as it was", restore.finish(), "");
        close(&done, &root);
        return;
    }
    let run = runs.remove(0);
    let Some(view) = inspect::view(run.id) else {
        return paste_next(runs, done, restore, root);
    };
    let before = view.visible_lines();
    put_picture();
    let action = view.paste_image_from(&NSPasteboard::generalPasteboard());
    let id = run.id;
    let name = run.name;
    attach(id, before, move |shown| {
        restore.put_back();
        // Without a marker, the screen tells what the CLI showed instead.
        let seen = if shown.is_empty() {
            screen(id)
        } else {
            shown.join(" ⏎ ")
        };
        check(
            check_name(name),
            !shown.is_empty() && action == Some(PasteAction::SendCtrlV),
            format!("{action:?}: {seen}"),
        );
        done.push(run);
        paste_next(runs, done, restore, root);
    });
}

fn check_name(name: &str) -> &'static str {
    match name {
        "claude" => "claude shows a pasted image as an attachment",
        "codex" => "codex shows a pasted image as an attachment",
        _ => "agy shows a pasted image as an attachment",
    }
}

/// The lines of the terminal `id` that hold text.
fn screen(id: u64) -> String {
    inspect::view(id)
        .map(|view| view.visible_lines())
        .unwrap_or_default()
        .into_iter()
        .filter(|line| !line.trim().is_empty())
        .collect::<Vec<_>>()
        .join(" ⏎ ")
}

/// Calls `then` with the new lines of the terminal `id` that show an image, once there are any or the time is up;
/// halfway it pastes once more, for a CLI that was still busy starting when the first paste came.
fn attach(id: u64, before: Vec<String>, then: impl FnOnce(Vec<String>) + 'static) {
    let started = Instant::now();
    let repeated = Cell::new(false);
    let shown = move || {
        let shown: Vec<String> = inspect::view(id)
            .map(|view| view.visible_lines())
            .unwrap_or_default()
            .into_iter()
            .filter(|line| shows_image(line) && !before.contains(line))
            .map(|line| line.trim().to_string())
            .collect();
        if shown.is_empty()
            && !repeated.get()
            && started.elapsed() >= ATTACH / 2
            && let Some(view) = inspect::view(id)
        {
            repeated.set(true);
            put_picture();
            view.paste_image_from(&NSPasteboard::generalPasteboard());
        }
        (!shown.is_empty()).then_some(shown)
    };
    helpers::wait_for_within(ATTACH, shown, move |shown| then(shown.unwrap_or_default()));
}

/// Closes the CLIs' tabs, which ends them with their input unsent, and ends the scenario.
fn close(runs: &[Run], root: &Path) {
    let mut pids = Vec::new();
    if let Some(controller) = controller_of(root) {
        for run in runs {
            let Some(view) = inspect::view(run.id) else { continue };
            pids.extend(view.shell());
            if let Some(tab) = view.tab() {
                controller.close_tab(tab);
            }
        }
    }
    forget_when_gone(pids, root.to_path_buf());
}

/// Once the CLIs ended, which write their settings as they quit, takes the scenario's folders out of those settings.
fn forget_when_gone(pids: Vec<u32>, root: PathBuf) {
    let alive = move |pids: &[u32]| {
        let running = ide_process::Supervisor::global().running();
        pids.iter().any(|pid| running.iter().any(|process| process.pid == *pid))
    };
    let watched = pids.clone();
    helpers::wait_within(GONE, move || !alive(&watched), move |gone| cleaned(pids, gone, root));
}

/// Takes the scenario's folders out of the CLIs' settings once they are `gone`.
fn cleaned(pids: Vec<u32>, gone: bool, root: PathBuf) {
    let alive = !gone;
    let home = crate::environment::get().and_then(|env| {
        env.iter()
            .find(|(name, _)| name == "HOME")
            .map(|(_, value)| PathBuf::from(value))
    });
    let Some(home) = home.filter(|_| !alive) else {
        check(
            "the CLIs end before their settings are cleaned",
            false,
            format!("{pids:?}"),
        );
        return finish(NAME);
    };
    forget_again(home, root, Vec::new(), 0);
}

/// Cleans the settings now and twice more a few seconds apart, as another running CLI may write back an entry it read
/// before; then checks that none is left.
fn forget_again(home: PathBuf, root: PathBuf, mut removed: Vec<String>, round: u32) {
    let doomed = scenario_folders(&root);
    removed.extend(
        forget::forget(&home, &doomed)
            .into_iter()
            .map(|line| format!("{line} (round {round})")),
    );
    if round < 2 {
        helpers::after(REWRITE_GAP, move || forget_again(home, root, removed, round + 1));
        return;
    }
    let names_left = forget::named(&home, &doomed);
    check(
        "the CLIs' settings no longer name the scenario's folders",
        names_left.is_empty(),
        format!("removed: {}; left: {}", removed.join(", "), names_left.join(", ")),
    );
    finish(NAME);
}

/// Whether a path names this or an earlier run's folder of the scenario: `<temp>/<stem><number>/paste-cli-project`.
fn scenario_folders(root: &Path) -> impl Fn(&str) -> bool + use<> {
    let folder = root
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default();
    let run = root.parent();
    let stem = run
        .and_then(Path::file_name)
        .map(|name| {
            name.to_string_lossy()
                .trim_end_matches(|c: char| c.is_ascii_digit())
                .to_string()
        })
        .unwrap_or_default();
    let temp = run
        .and_then(Path::parent)
        .map(|dir| dir.to_string_lossy().into_owned())
        .unwrap_or_default();
    move |path: &str| {
        let Some(rest) = path.strip_prefix(&format!("{temp}/{stem}")) else {
            return false;
        };
        let Some(number) = rest.strip_suffix(&format!("/{folder}")) else {
            return false;
        };
        !stem.is_empty() && !number.is_empty() && number.chars().all(|c| c.is_ascii_digit())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn attachment_markers_of_the_clis_are_recognised() {
        assert!(shows_image("> [Image #1] "));
        assert!(shows_image("│ [image 1: 2x2 PNG]"));
        assert!(shows_image("@clipboard/pasted-17.png"));
        assert!(!shows_image("> describe this"));
    }

    #[test]
    fn only_the_scenarios_own_folders_are_forgotten() {
        let doomed = scenario_folders(Path::new(
            "/private/var/T/local.agentide.selftest-paste-cli-77/paste-cli-project",
        ));
        assert!(doomed(
            "/private/var/T/local.agentide.selftest-paste-cli-51815/paste-cli-project"
        ));
        assert!(!doomed(
            "/private/var/T/local.agentide.selftest-paste-cli-51815/paste-cli-project/sub"
        ));
        assert!(!doomed(
            "/private/var/T/local.agentide.selftest-paste-cli-x/paste-cli-project"
        ));
        assert!(!doomed("/Users/Sven/Repos"));
    }

    #[test]
    fn a_trust_question_is_answered_with_yes() {
        let lines = |text: &str| text.lines().map(String::from).collect::<Vec<_>>();
        assert_eq!(answer(&lines("❯ No, exit\n  Yes, I trust this folder")), Answer::Down);
        assert_eq!(
            answer(&lines("  No, exit\n❯ Yes, I trust this folder")),
            Answer::Confirm
        );
        assert_eq!(answer(&lines("› 1. Yes, allow\n  2. No")), Answer::Confirm);
        assert_eq!(answer(&lines("Trust this folder? Press Enter")), Answer::Confirm);
    }
}

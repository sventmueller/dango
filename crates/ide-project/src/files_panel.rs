//! What the app's files panel decides without AppKit: what a click does, what the context menu offers and what a new name
//! for a file means; the controller carries out the effects.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use splat_i18n::{t, tf};

use crate::tree::Entry;

/// What the tree knows of a row besides its path.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Row {
    /// Whether it is a folder that can open.
    pub expandable: bool,
    /// Ignored by git or hidden, drawn dimmer.
    pub ignored: bool,
}

/// What a row stands for on disk, as far as the model needs to know.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// A folder.
    Folder,
    /// Anything else.
    File,
}

/// A command of the context menu and the panel's title row.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Command {
    /// Open the file in the other half of the editor area.
    OpenBeside,
    /// Open an SVG file's drawing.
    OpenAsPreview,
    /// Open an HTML page in a browser tab, served by the app (E131).
    OpenInBrowserTab,
    /// Make a new file.
    NewFile,
    /// Make a new folder.
    NewFolder,
    /// Show the row in the Finder.
    Reveal,
    /// Copy the full path.
    CopyPath,
    /// Copy the path below the folder.
    CopyRelativePath,
    /// Give the row a new name.
    Rename,
    /// Move the row to the Trash.
    Trash,
}

/// A line of the context menu.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MenuEntry {
    /// A dividing line.
    Separator,
    /// An entry that runs a command.
    Item {
        /// What the entry says.
        title: &'static str,
        /// What it runs.
        command: Command,
        /// Whether it can be chosen.
        enabled: bool,
    },
}

/// What the user wants a name for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Naming {
    /// A new file or folder in `parent`.
    Create {
        /// The folder it goes into.
        parent: PathBuf,
        /// Whether it is a folder.
        folder: bool,
    },
    /// A new name for `path`.
    Rename {
        /// What is renamed.
        path: PathBuf,
    },
}

/// Something the controller does for the model.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Effect {
    /// Open the folder of the clicked row, or close it.
    Toggle,
    /// Open the file in the preview tab.
    Preview(PathBuf),
    /// Open the file in a tab of its own.
    Open(PathBuf),
    /// Open the file in the other half of the editor area.
    OpenBeside(PathBuf),
    /// Open the drawing of an SVG file.
    OpenAsPreview(PathBuf),
    /// Open the page in a browser tab, served by the app.
    OpenInBrowserTab(PathBuf),
    /// Show the path in the Finder.
    RevealInFinder(PathBuf),
    /// Put the text on the pasteboard.
    Copy(String),
    /// Move the path to the Trash.
    MoveToTrash(PathBuf),
    /// Ask for a name, prefilled with `current`, under `title`.
    AskName {
        /// What the name is for.
        naming: Naming,
        /// The question.
        title: &'static str,
        /// The text the field starts with.
        current: String,
    },
    /// Create the file or folder `path`.
    Create {
        /// Where it goes.
        path: PathBuf,
        /// Whether it is a folder.
        folder: bool,
    },
    /// Rename `from` to `to`.
    Rename {
        /// The present path.
        from: PathBuf,
        /// The new path.
        to: PathBuf,
    },
    /// Tell the user that something did not work.
    Fail {
        /// What went wrong.
        message: String,
        /// Why, as the system said it.
        detail: String,
    },
}

/// The state of the files panel of one window.
#[derive(Debug)]
pub struct Model {
    root: PathBuf,
    rows: HashMap<PathBuf, Row>,
    /// The path the context menu was opened on.
    clicked: Option<PathBuf>,
}

impl Model {
    /// The panel of the folder `root`, with nothing read yet.
    pub fn new(root: PathBuf) -> Model {
        Model {
            root,
            rows: HashMap::new(),
            clicked: None,
        }
    }

    /// What the tree last read of `path`.
    pub fn row(&self, path: &Path) -> Option<Row> {
        self.rows.get(path).copied()
    }

    /// Records what the tree last read of `path`.
    pub fn note(&mut self, path: &Path, row: Row) {
        self.rows.insert(path.to_path_buf(), row);
    }

    /// The row the context menu was opened on.
    pub fn clicked(&self) -> Option<&Path> {
        self.clicked.as_deref()
    }

    /// A single click opens a folder or closes it, as in VS Code, and opens a file, in the preview tab if the user
    /// turned those on.
    pub fn click(path: &Path, kind: Kind) -> Effect {
        match kind {
            Kind::Folder => Effect::Toggle,
            Kind::File => Effect::Preview(path.to_path_buf()),
        }
    }

    /// A double click opens a file in a tab of its own; a folder's first click already opened or closed it.
    pub fn double_click(path: &Path, kind: Kind) -> Option<Effect> {
        (kind == Kind::File).then(|| Effect::Open(path.to_path_buf()))
    }

    /// Remembers the row the context menu opens on, the root for the empty space below the rows, and says what the
    /// menu offers; `kind` is what `path` is on disk, if it still is.
    pub fn open_menu(&mut self, path: Option<&Path>, kind: Option<Kind>) -> Vec<MenuEntry> {
        self.clicked = Some(path.map_or_else(|| self.root.clone(), Path::to_path_buf));
        let file = kind == Some(Kind::File);
        let row = path.is_some();
        let item = |title, command, enabled| MenuEntry::Item {
            title,
            command,
            enabled,
        };
        let mut entries = vec![
            item(t("Daneben öffnen"), Command::OpenBeside, file),
            MenuEntry::Separator,
            item(t("Neue Datei …"), Command::NewFile, true),
            item(t("Neuer Ordner …"), Command::NewFolder, true),
            MenuEntry::Separator,
            item(t("Im Finder zeigen"), Command::Reveal, true),
            item(t("Pfad kopieren"), Command::CopyPath, true),
            item(t("Relativen Pfad kopieren"), Command::CopyRelativePath, true),
            MenuEntry::Separator,
            item(t("Umbenennen …"), Command::Rename, row),
            item(t("In den Papierkorb legen"), Command::Trash, row),
        ];
        if file && path.is_some_and(is_page) {
            entries.insert(1, item(t("Im Browser-Tab öffnen"), Command::OpenInBrowserTab, true));
        }
        if file && path.is_some_and(is_svg) {
            entries.insert(1, item(t("Als Vorschau öffnen"), Command::OpenAsPreview, true));
        }
        entries
    }

    /// What `command` of the context menu does to the clicked row; `kind` is what that row is on disk.
    pub fn command(&self, command: Command, kind: Option<Kind>) -> Option<Effect> {
        let path = self.clicked.clone()?;
        Some(match command {
            Command::OpenBeside => Effect::OpenBeside(path),
            Command::OpenAsPreview => Effect::OpenAsPreview(path),
            Command::OpenInBrowserTab => Effect::OpenInBrowserTab(path),
            Command::NewFile | Command::NewFolder => {
                return Some(create_in(&path, kind, command == Command::NewFolder));
            }
            Command::Reveal => Effect::RevealInFinder(path),
            Command::CopyPath => Effect::Copy(path.to_string_lossy().into_owned()),
            Command::CopyRelativePath => Effect::Copy(
                path.strip_prefix(&self.root)
                    .unwrap_or(&path)
                    .to_string_lossy()
                    .into_owned(),
            ),
            Command::Rename => Effect::AskName {
                current: file_name(&path),
                naming: Naming::Rename { path },
                title: t("Umbenennen"),
            },
            Command::Trash => Effect::MoveToTrash(path),
        })
    }

    /// The folder the title row's buttons create in: the selected row's, the one a selected file is in, or the root
    /// without a selection.
    pub fn create_from_header(&self, selected: Option<&Path>, kind: Option<Kind>, folder: bool) -> Effect {
        match selected {
            Some(path) => create_in(path, kind, folder),
            None => create_in(&self.root, Some(Kind::Folder), folder),
        }
    }

    /// What follows the name the user typed for `naming`; `exists` says whether something has that name already.
    pub fn named(naming: &Naming, name: &str, exists: impl Fn(&Path) -> bool) -> Option<Effect> {
        match naming {
            Naming::Create { parent, folder } => Some(Effect::Create {
                path: parent.join(name),
                folder: *folder,
            }),
            Naming::Rename { path } => {
                let current = file_name(path);
                if name == current {
                    return None;
                }
                let target = path.with_file_name(name);
                if exists(&target) && !same_file_ignoring_case(path, &target) {
                    return Some(Effect::Fail {
                        message: tf("„{}“ gibt es schon.", &[name]),
                        detail: String::new(),
                    });
                }
                Some(Effect::Rename {
                    from: path.clone(),
                    to: target,
                })
            }
        }
    }

    /// What follows creating `path`: a new file opens, a failure is said.
    pub fn created(path: &Path, folder: bool, result: Result<(), String>) -> Option<Effect> {
        match result {
            Ok(()) => (!folder).then(|| Effect::Open(path.to_path_buf())),
            Err(detail) => Some(Effect::Fail {
                message: tf("„{}“ wurde nicht angelegt.", &[&file_name(path)]),
                detail,
            }),
        }
    }

    /// What follows renaming `from`: only a failure is said.
    pub fn renamed(from: &Path, result: Result<(), String>) -> Option<Effect> {
        result.err().map(|detail| Effect::Fail {
            message: tf("„{}“ wurde nicht umbenannt.", &[&file_name(from)]),
            detail,
        })
    }
}

/// The name the user typed into the field, or `None` if it is empty or holds a slash.
pub fn valid_name(typed: &str) -> Option<String> {
    let name = typed.trim();
    (!name.is_empty() && !name.contains('/')).then(|| name.to_string())
}

/// Asks for the name of a new file or folder in `at`, or beside it if it is a file.
fn create_in(at: &Path, kind: Option<Kind>, folder: bool) -> Effect {
    let parent = if kind == Some(Kind::Folder) {
        at.to_path_buf()
    } else {
        at.parent().map_or_else(|| at.to_path_buf(), Path::to_path_buf)
    };
    Effect::AskName {
        naming: Naming::Create { parent, folder },
        title: if folder { t("Neuer Ordner") } else { t("Neue Datei") },
        current: String::new(),
    }
}

fn file_name(path: &Path) -> String {
    path.file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default()
}

/// Whether `a` and `b` name one file on a volume that ignores case, as a rename that only changes case does.
fn same_file_ignoring_case(a: &Path, b: &Path) -> bool {
    a.to_string_lossy().to_lowercase() == b.to_string_lossy().to_lowercase()
}

/// Whether `path` is an HTML page.
fn is_page(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| e.eq_ignore_ascii_case("html") || e.eq_ignore_ascii_case("htm"))
}

/// Whether `path` names an SVG drawing, which opens as a picture as well as as text.
pub fn is_svg(path: &Path) -> bool {
    path.extension().is_some_and(|e| e.eq_ignore_ascii_case("svg"))
}

impl Row {
    /// What the tree shows of `entry` besides its path.
    pub fn of(entry: &Entry) -> Row {
        Row {
            expandable: entry.can_expand(),
            ignored: entry.is_ignored || entry.is_hidden,
        }
    }
}

/// How the rows of a folder change from one reading to the next, so the tree moves only those.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Changes {
    /// The positions in the old list of the rows that go, removed first.
    pub removed: Vec<usize>,
    /// The positions in the new list of the rows that come, inserted after the removals.
    pub inserted: Vec<usize>,
    /// The positions in the new list of the rows that stay but look different.
    pub redrawn: Vec<usize>,
}

/// The changes from the rows `old` to `new`; `None` where removing and inserting cannot get there, such as when a
/// row turns from a file into a folder or rows swap places, and the folder's rows are shown again as a whole.
pub fn changes(old: &[Entry], new: &[Entry]) -> Option<Changes> {
    let before: HashMap<&Path, usize> = old.iter().enumerate().map(|(i, e)| (e.path.as_path(), i)).collect();
    let after: HashMap<&Path, usize> = new.iter().enumerate().map(|(i, e)| (e.path.as_path(), i)).collect();
    let mut changes = Changes {
        removed: (0..old.len())
            .filter(|i| !after.contains_key(old[*i].path.as_path()))
            .collect(),
        ..Changes::default()
    };
    let mut last = None;
    for (i, entry) in new.iter().enumerate() {
        let Some(&was) = before.get(entry.path.as_path()) else {
            changes.inserted.push(i);
            continue;
        };
        if last.is_some_and(|last| was < last) || Row::of(&old[was]).expandable != Row::of(entry).expandable {
            return None;
        }
        last = Some(was);
        if Row::of(&old[was]) != Row::of(entry) {
            changes.redrawn.push(i);
        }
    }
    Some(changes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tree::Kind as EntryKind;

    fn entry(name: &str, kind: EntryKind, is_ignored: bool) -> Entry {
        Entry {
            name: name.into(),
            path: PathBuf::from("/p").join(name),
            kind,
            link: None,
            size: 0,
            modified: None,
            is_ignored,
            is_hidden: false,
        }
    }

    #[test]
    fn a_folder_read_again_moves_only_the_rows_that_came_went_or_look_different() {
        let old = [
            entry("a", EntryKind::Dir, false),
            entry("b.rs", EntryKind::File, false),
            entry("c.rs", EntryKind::File, false),
        ];
        let new = [
            entry("a", EntryKind::Dir, false),
            entry("a.rs", EntryKind::File, false),
            entry("c.rs", EntryKind::File, true),
            entry("d.rs", EntryKind::File, false),
        ];
        assert_eq!(
            changes(&old, &new),
            Some(Changes {
                removed: vec![1],
                inserted: vec![1, 3],
                redrawn: vec![2],
            })
        );
        assert_eq!(changes(&new, &new), Some(Changes::default()));
    }

    #[test]
    fn rows_that_swap_places_or_a_file_that_turns_into_a_folder_show_the_folder_again() {
        let a = entry("a", EntryKind::File, false);
        let b = entry("b", EntryKind::File, false);
        assert_eq!(changes(&[a.clone(), b.clone()], &[b, a.clone()]), None);
        assert_eq!(changes(&[a], &[entry("a", EntryKind::Dir, false)]), None);
    }

    fn model() -> Model {
        Model::new(PathBuf::from("/p"))
    }

    fn enabled(entries: &[MenuEntry], wanted: Command) -> Option<bool> {
        entries.iter().find_map(|entry| match entry {
            MenuEntry::Item { command, enabled, .. } if *command == wanted => Some(*enabled),
            _ => None,
        })
    }

    #[test]
    fn a_click_opens_a_folder_or_previews_a_file_and_a_double_click_opens_only_files() {
        let path = Path::new("/p/a");
        assert_eq!(Model::click(path, Kind::Folder), Effect::Toggle);
        assert_eq!(Model::click(path, Kind::File), Effect::Preview(path.into()));
        assert_eq!(Model::double_click(path, Kind::Folder), None);
        assert_eq!(Model::double_click(path, Kind::File), Some(Effect::Open(path.into())));
    }

    #[test]
    fn the_menu_on_empty_space_acts_on_the_root_and_cannot_rename_it() {
        let mut model = model();
        let entries = model.open_menu(None, None);
        assert_eq!(model.clicked(), Some(Path::new("/p")));
        assert_eq!(enabled(&entries, Command::Rename), Some(false));
        assert_eq!(enabled(&entries, Command::Trash), Some(false));
        assert_eq!(enabled(&entries, Command::OpenBeside), Some(false));
        assert_eq!(enabled(&entries, Command::NewFile), Some(true));
        assert_eq!(enabled(&entries, Command::OpenAsPreview), None);
    }

    #[test]
    fn only_an_svg_file_offers_its_drawing() {
        let mut model = model();
        let entries = model.open_menu(Some(Path::new("/p/logo.SVG")), Some(Kind::File));
        assert_eq!(enabled(&entries, Command::OpenAsPreview), Some(true));
        assert_eq!(enabled(&entries, Command::OpenBeside), Some(true));
        let entries = model.open_menu(Some(Path::new("/p/x.svg")), Some(Kind::Folder));
        assert_eq!(enabled(&entries, Command::OpenAsPreview), None);
    }

    #[test]
    fn a_new_file_goes_into_a_clicked_folder_or_beside_a_clicked_file() {
        let mut model = model();
        model.open_menu(Some(Path::new("/p/src")), Some(Kind::Folder));
        let Some(Effect::AskName { naming, .. }) = model.command(Command::NewFile, Some(Kind::Folder)) else {
            panic!("no question for the name");
        };
        assert_eq!(
            naming,
            Naming::Create {
                parent: "/p/src".into(),
                folder: false
            }
        );
        model.open_menu(Some(Path::new("/p/src/main.rs")), Some(Kind::File));
        let Some(Effect::AskName { naming, .. }) = model.command(Command::NewFolder, Some(Kind::File)) else {
            panic!("no question for the name");
        };
        assert_eq!(
            naming,
            Naming::Create {
                parent: "/p/src".into(),
                folder: true
            }
        );
        assert_eq!(
            Model::named(&naming, "lib", |_| false),
            Some(Effect::Create {
                path: "/p/src/lib".into(),
                folder: true
            })
        );
    }

    #[test]
    fn the_header_creates_in_the_root_without_a_selection() {
        let Effect::AskName { naming, .. } = model().create_from_header(None, None, false) else {
            panic!("no question for the name");
        };
        assert_eq!(
            naming,
            Naming::Create {
                parent: "/p".into(),
                folder: false
            }
        );
    }

    #[test]
    fn copying_the_relative_path_leaves_out_the_root() {
        let mut model = model();
        model.open_menu(Some(Path::new("/p/src/main.rs")), Some(Kind::File));
        assert_eq!(
            model.command(Command::CopyRelativePath, Some(Kind::File)),
            Some(Effect::Copy("src/main.rs".into()))
        );
        assert_eq!(
            model.command(Command::CopyPath, Some(Kind::File)),
            Some(Effect::Copy("/p/src/main.rs".into()))
        );
    }

    #[test]
    fn a_rename_keeps_out_of_the_way_of_another_file_but_may_change_case() {
        let naming = Naming::Rename {
            path: "/p/Readme.md".into(),
        };
        assert_eq!(Model::named(&naming, "Readme.md", |_| true), None);
        assert!(matches!(
            Model::named(&naming, "other.md", |_| true),
            Some(Effect::Fail { .. })
        ));
        assert_eq!(
            Model::named(&naming, "README.md", |_| true),
            Some(Effect::Rename {
                from: "/p/Readme.md".into(),
                to: "/p/README.md".into()
            })
        );
    }

    #[test]
    fn a_created_file_opens_and_a_failure_is_said() {
        let path = Path::new("/p/a.rs");
        assert_eq!(Model::created(path, false, Ok(())), Some(Effect::Open(path.into())));
        assert_eq!(Model::created(path, true, Ok(())), None);
        assert!(matches!(
            Model::created(path, false, Err("exists".into())),
            Some(Effect::Fail { detail, .. }) if detail == "exists"
        ));
        assert_eq!(Model::renamed(path, Ok(())), None);
    }

    #[test]
    fn a_name_is_trimmed_and_may_hold_no_slash() {
        assert_eq!(valid_name("  a.rs "), Some("a.rs".into()));
        assert_eq!(valid_name("   "), None);
        assert_eq!(valid_name("a/b"), None);
    }

    #[test]
    fn svg_files_are_known_by_extension_in_any_case() {
        assert!(is_svg(Path::new("logo.Svg")));
        assert!(!is_svg(Path::new("logo.svg.txt")));
    }
}

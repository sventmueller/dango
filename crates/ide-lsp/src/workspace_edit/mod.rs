//! A server's workspace edit, as code actions, renaming and `workspace/applyEdit` bring it (Review C1, R21): its
//! changes keep the order the server gave; every path, also those of file operations, must lie inside the folder once
//! symlinks are resolved, and every version must match the open document's revision (E24), all checked before
//! anything changes. Open files change as one undo step each, closed ones are written as saving writes them; a step
//! that fails ends the edit and the message names the files already changed.

use std::cmp::Reverse;
use std::collections::HashMap;
use std::ops::Range;
use std::path::{Path, PathBuf};

use ide_core::explain::Explain;
use ide_document::{Document, Edit, EditError, SaveError};
use lsp_types::{DocumentChangeOperation, DocumentChanges, OneOf, ResourceOp, TextEdit, Uri, WorkspaceEdit};
use splat_i18n::{t, tf};

/// Why a workspace edit was not applied in full.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Failure {
    /// For the user, naming the files already changed.
    pub message: String,
    /// The change that failed, counted in the order the server gave.
    pub index: Option<usize>,
}

impl Failure {
    fn at(index: usize, message: String) -> Failure {
        Failure {
            message,
            index: Some(index),
        }
    }
}

/// Why one change of a workspace edit failed.
#[derive(Debug)]
pub enum StepError {
    /// The editor that shows the file did not take the change, as a read-only one does not.
    Refused(PathBuf),
    /// The file system refused.
    Io {
        /// The file or folder concerned.
        path: PathBuf,
        /// What the system answered.
        error: std::io::Error,
    },
    /// The change named no place in the file's text, or the file may not be changed.
    Edit {
        /// The file concerned.
        path: PathBuf,
        /// Why the change did not fit.
        error: EditError,
    },
    /// The changed file could not be written.
    Save {
        /// The file concerned.
        path: PathBuf,
        /// Why it was not written.
        error: SaveError,
    },
    /// The Trash did not take the file.
    Trash {
        /// The file concerned.
        path: PathBuf,
        /// The system's words for why, already in the user's language.
        why: String,
    },
}

impl Explain for StepError {
    fn explain(&self) -> String {
        let (path, why) = match self {
            StepError::Refused(path) => return tf("{} lässt sich nicht ändern", &[&path.display().to_string()]),
            StepError::Io { path, error } => (path, error.explain()),
            StepError::Edit { path, error } => (path, error.explain()),
            StepError::Save { path, error } => (path, error.explain()),
            StepError::Trash { path, why } => (path, why.clone()),
        };
        format!("{}: {why}", path.display())
    }
}

/// The open editors a workspace edit reaches.
pub trait Editors {
    /// The version the server knows of the document an editor shows for `path`; `None` when none shows it.
    fn version(&self, path: &Path) -> Option<i32>;
    /// Makes `edits` in the editor that shows `path` as one undo step; `None` when none shows it.
    fn edit(&mut self, path: &Path, edits: &[TextEdit]) -> Option<Result<(), StepError>>;
    /// The file at `from` is at `to` now; its editors follow.
    fn moved(&mut self, from: &Path, to: &Path);
    /// Moves `path` to the Trash.
    fn trash(&mut self, path: &Path) -> Result<(), StepError>;
}

/// One change of a workspace edit.
#[derive(Debug, Clone, PartialEq)]
enum Step {
    Edit {
        path: PathBuf,
        version: Option<i32>,
        edits: Vec<TextEdit>,
    },
    Create {
        path: PathBuf,
        overwrite: bool,
        ignore: bool,
    },
    Rename {
        from: PathBuf,
        to: PathBuf,
        overwrite: bool,
        ignore: bool,
    },
    Delete {
        path: PathBuf,
        ignore: bool,
    },
}

/// Applies `edit` to the files under `folder` and to the editors that show them.
pub fn apply(folder: &Path, edit: &WorkspaceEdit, editors: &mut impl Editors) -> Result<(), Failure> {
    let folder = folder.canonicalize().unwrap_or_else(|_| folder.to_path_buf());
    let steps = steps(edit)?;
    let planned = check(&folder, steps, editors)?;
    execute(&folder, planned, editors)
}

fn path_of(uri: &Uri, index: usize) -> Result<PathBuf, Failure> {
    crate::uri::to_path(uri).ok_or_else(|| Failure::at(index, format!("{}: {}", t("Keine Datei"), uri.as_str())))
}

/// The changes of `edit` in order: `documentChanges` when the server sent them, which the protocol prefers, else
/// `changes` by path.
fn steps(edit: &WorkspaceEdit) -> Result<Vec<Step>, Failure> {
    let text_edit = |index: usize, document: &lsp_types::TextDocumentEdit| {
        Ok(Step::Edit {
            path: path_of(&document.text_document.uri, index)?,
            version: document.text_document.version,
            edits: document
                .edits
                .iter()
                .map(|one| match one {
                    OneOf::Left(edit) => edit.clone(),
                    OneOf::Right(annotated) => annotated.text_edit.clone(),
                })
                .collect(),
        })
    };
    match &edit.document_changes {
        Some(DocumentChanges::Edits(documents)) => documents
            .iter()
            .enumerate()
            .map(|(index, document)| text_edit(index, document))
            .collect(),
        Some(DocumentChanges::Operations(operations)) => operations
            .iter()
            .enumerate()
            .map(|(index, operation)| match operation {
                DocumentChangeOperation::Edit(document) => text_edit(index, document),
                DocumentChangeOperation::Op(op) => operation_step(index, op),
            })
            .collect(),
        None => {
            let mut steps = edit
                .changes
                .iter()
                .flatten()
                .enumerate()
                .map(|(index, (uri, edits))| {
                    Ok(Step::Edit {
                        path: path_of(uri, index)?,
                        version: None,
                        edits: edits.clone(),
                    })
                })
                .collect::<Result<Vec<Step>, Failure>>()?;
            steps.sort_by(|a, b| first_path(a).cmp(first_path(b)));
            Ok(steps)
        }
    }
}

fn operation_step(index: usize, op: &ResourceOp) -> Result<Step, Failure> {
    Ok(match op {
        ResourceOp::Create(create) => {
            let options = create.options.as_ref();
            Step::Create {
                path: path_of(&create.uri, index)?,
                overwrite: options.and_then(|o| o.overwrite).unwrap_or(false),
                ignore: options.and_then(|o| o.ignore_if_exists).unwrap_or(false),
            }
        }
        ResourceOp::Rename(rename) => {
            let options = rename.options.as_ref();
            Step::Rename {
                from: path_of(&rename.old_uri, index)?,
                to: path_of(&rename.new_uri, index)?,
                overwrite: options.and_then(|o| o.overwrite).unwrap_or(false),
                ignore: options.and_then(|o| o.ignore_if_exists).unwrap_or(false),
            }
        }
        ResourceOp::Delete(delete) => Step::Delete {
            path: path_of(&delete.uri, index)?,
            ignore: delete
                .options
                .as_ref()
                .and_then(|o| o.ignore_if_not_exists)
                .unwrap_or(false),
        },
    })
}

fn first_path(step: &Step) -> &Path {
    match step {
        Step::Edit { path, .. } | Step::Create { path, .. } | Step::Delete { path, .. } => path,
        Step::Rename { from, .. } => from,
    }
}

/// `path` with every symlink resolved; of a file that does not exist yet, the folder it would be in. `None` for a
/// link that leads nowhere and for `..` in the part that does not exist, which could not be checked.
fn resolve(path: &Path) -> Option<PathBuf> {
    let mut existing = path;
    let mut rest = vec![];
    loop {
        if let Ok(found) = existing.canonicalize() {
            return Some(rest.iter().rev().fold(found, |path, part| path.join(part)));
        }
        if existing.symlink_metadata().is_ok() {
            return None;
        }
        rest.push(existing.file_name()?.to_owned());
        existing = existing.parent()?;
    }
}

/// What the checks know of the files while they go through the steps, before any of them runs.
#[derive(Default)]
struct Plan {
    /// Whether a path exists after the steps so far, where they changed it.
    exists: HashMap<PathBuf, bool>,
    /// Where the file now at a path was before the steps renamed it, for its editor's version.
    origin: HashMap<PathBuf, PathBuf>,
    /// Files the steps so far created.
    created: Vec<PathBuf>,
}

impl Plan {
    fn exists(&self, path: &Path) -> bool {
        self.exists
            .get(path)
            .copied()
            .unwrap_or_else(|| path.symlink_metadata().is_ok())
    }

    fn origin(&self, path: &Path) -> PathBuf {
        self.origin.get(path).cloned().unwrap_or_else(|| path.to_path_buf())
    }
}

fn inside(folder: &Path, path: &Path, index: usize) -> Result<PathBuf, Failure> {
    resolve(path)
        .filter(|resolved| resolved.starts_with(folder))
        .ok_or_else(|| {
            Failure::at(
                index,
                tf("{} liegt außerhalb des Ordners", &[&path.display().to_string()]),
            )
        })
}

fn missing(path: &Path, index: usize) -> Failure {
    Failure::at(index, tf("{} gibt es nicht", &[&path.display().to_string()]))
}

fn taken(path: &Path, index: usize) -> Failure {
    Failure::at(index, tf("{} gibt es schon", &[&path.display().to_string()]))
}

/// The steps with resolved paths, each with its index, and without those the server asked to skip; or why the
/// whole edit must not run.
fn check(folder: &Path, steps: Vec<Step>, editors: &impl Editors) -> Result<Vec<(usize, Step)>, Failure> {
    let mut plan = Plan::default();
    let mut planned = vec![];
    for (index, step) in steps.into_iter().enumerate() {
        let step = match step {
            Step::Edit { path, version, edits } => {
                let path = inside(folder, &path, index)?;
                let open = editors.version(&plan.origin(&path));
                let fresh = match (version, open) {
                    (Some(wanted), Some(open)) => wanted == open,
                    (Some(_), None) => plan.created.contains(&path),
                    (None, _) => true,
                };
                if !fresh {
                    let message = format!("{}\n{}", t("Die Datei hat sich inzwischen geändert"), path.display());
                    return Err(Failure::at(index, message));
                }
                if open.is_none() && !plan.exists(&path) {
                    return Err(missing(&path, index));
                }
                Step::Edit { path, version, edits }
            }
            Step::Create {
                path,
                overwrite,
                ignore,
            } => {
                let path = inside(folder, &path, index)?;
                if plan.exists(&path) && !overwrite {
                    if ignore {
                        continue;
                    }
                    return Err(taken(&path, index));
                }
                plan.exists.insert(path.clone(), true);
                plan.created.push(path.clone());
                Step::Create {
                    path,
                    overwrite,
                    ignore,
                }
            }
            Step::Rename {
                from,
                to,
                overwrite,
                ignore,
            } => {
                let (from, to) = (inside(folder, &from, index)?, inside(folder, &to, index)?);
                if !plan.exists(&from) {
                    return Err(missing(&from, index));
                }
                if plan.exists(&to) && !overwrite {
                    if ignore {
                        continue;
                    }
                    return Err(taken(&to, index));
                }
                let origin = plan.origin(&from);
                plan.exists.insert(from.clone(), false);
                plan.exists.insert(to.clone(), true);
                plan.origin.insert(to.clone(), origin);
                Step::Rename {
                    from,
                    to,
                    overwrite,
                    ignore,
                }
            }
            Step::Delete { path, ignore } => {
                let path = inside(folder, &path, index)?;
                if !plan.exists(&path) {
                    if ignore {
                        continue;
                    }
                    return Err(missing(&path, index));
                }
                plan.exists.insert(path.clone(), false);
                Step::Delete { path, ignore }
            }
        };
        planned.push((index, step));
    }
    Ok(planned)
}

/// A file changed so far, and whether an editor shows it, where undo takes the change back.
struct Changed {
    path: PathBuf,
    open: bool,
}

fn execute(folder: &Path, planned: Vec<(usize, Step)>, editors: &mut impl Editors) -> Result<(), Failure> {
    let mut changed: Vec<Changed> = vec![];
    for (index, step) in planned {
        if let Err(error) = run(&step, editors, &mut changed) {
            return Err(Failure::at(index, partial(folder, &error.explain(), &changed)));
        }
    }
    Ok(())
}

fn note(changed: &mut Vec<Changed>, path: &Path, open: bool) {
    match changed.iter_mut().find(|c| c.path == path) {
        Some(known) => known.open |= open,
        None => changed.push(Changed {
            path: path.to_path_buf(),
            open,
        }),
    }
}

fn run(step: &Step, editors: &mut impl Editors, changed: &mut Vec<Changed>) -> Result<(), StepError> {
    match step {
        Step::Edit { path, edits, .. } => {
            let open = match editors.edit(path, edits) {
                Some(result) => result.map(|()| true),
                None => edit_file(path, edits).map(|()| false),
            }?;
            note(changed, path, open);
        }
        Step::Create { path, .. } => {
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent).map_err(|error| io(parent, error))?;
            }
            std::fs::write(path, "").map_err(|error| io(path, error))?;
            note(changed, path, false);
        }
        Step::Rename { from, to, .. } => {
            if let Some(parent) = to.parent() {
                std::fs::create_dir_all(parent).map_err(|error| io(parent, error))?;
            }
            std::fs::rename(from, to).map_err(|error| io(from, error))?;
            editors.moved(from, to);
            match changed.iter_mut().find(|c| &c.path == from) {
                Some(known) => to.clone_into(&mut known.path),
                None => note(changed, to, false),
            }
        }
        Step::Delete { path, .. } => {
            editors.trash(path)?;
            note(changed, path, false);
        }
    }
    Ok(())
}

fn io(path: &Path, error: std::io::Error) -> StepError {
    StepError::Io {
        path: path.to_path_buf(),
        error,
    }
}

/// The message for `error` after `changed` changed already.
fn partial(folder: &Path, error: &str, changed: &[Changed]) -> String {
    if changed.is_empty() {
        return error.to_string();
    }
    let names: Vec<String> = changed
        .iter()
        .map(|c| c.path.strip_prefix(folder).unwrap_or(&c.path).display().to_string())
        .collect();
    let mut message = format!("{error}\n{}", tf("Schon geändert: {}", &[&names.join(", ")]));
    if changed.iter().any(|c| c.open) {
        message.push('\n');
        message.push_str(t("Änderungen an geöffneten Dateien lassen sich je Datei widerrufen."));
    }
    message
}

/// `edits`, UTF-16 ranges of one text that do not overlap, in the order to make them: from the end, and of two at
/// one place the later one first, so they end up in the order the server gave.
pub fn back_to_front(edits: Vec<(Range<usize>, String)>) -> Vec<(Range<usize>, String)> {
    let mut numbered: Vec<(usize, (Range<usize>, String))> = edits.into_iter().enumerate().collect();
    numbered.sort_by_key(|(index, (range, _))| Reverse((range.start, *index)));
    numbered.into_iter().map(|(_, edit)| edit).collect()
}

/// Edits a file no editor shows through the document model, which keeps its encoding and line breaks, and saves it.
fn edit_file(path: &Path, edits: &[TextEdit]) -> Result<(), StepError> {
    let mut document = Document::open(path).map_err(|error| io(path, error))?;
    let ranges = edits
        .iter()
        .map(|e| (crate::offsets::range(document.text(), e.range), e.new_text.clone()))
        .collect();
    for (range, text) in back_to_front(ranges) {
        document
            .apply(Edit {
                range_utf16: range,
                text,
            })
            .map_err(|error| StepError::Edit {
                path: path.to_path_buf(),
                error,
            })?;
    }
    document.save().map_err(|error| StepError::Save {
        path: path.to_path_buf(),
        error,
    })
}

#[cfg(test)]
mod tests;

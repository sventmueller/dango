//! Writing one changed value back into the workflow file's text, with comments, order and formatting of everything
//! else kept (E183); the dialog "Leitstand für <Ordner>" saves through it.

use toml_edit::{Array, DocumentMut, Item, Table, Value};

use super::WorkflowError;

/// A value to write.
#[derive(Debug, Clone, PartialEq)]
pub enum EditValue {
    /// `true` or `false`.
    Switch(bool),
    /// A whole number.
    Count(u64),
    /// A number.
    Number(f64),
    /// A string.
    Text(String),
    /// A list of strings.
    Texts(Vec<String>),
}

impl EditValue {
    fn into_value(self) -> Value {
        match self {
            EditValue::Switch(b) => Value::from(b),
            EditValue::Count(n) => Value::from(i64::try_from(n).unwrap_or(i64::MAX)),
            EditValue::Number(n) => Value::from(n),
            EditValue::Text(s) => Value::from(s),
            EditValue::Texts(list) => Value::Array(list.into_iter().collect::<Array>()),
        }
    }
}

/// Why an edit could not be made.
#[derive(Debug, Clone, PartialEq)]
pub enum EditError {
    /// The text is not valid TOML, so it cannot be edited safely.
    Invalid(Vec<WorkflowError>),
    /// A part of the key path holds a value, not a table.
    NotATable {
        /// The dotted path up to that part.
        key: String,
    },
    /// The key path is empty.
    EmptyKey,
}

fn document(text: &str) -> Result<DocumentMut, EditError> {
    text.parse::<DocumentMut>().map_err(|error| {
        EditError::Invalid(vec![WorkflowError {
            key: String::new(),
            line: None,
            column: None,
            kind: super::WorkflowErrorKind::Syntax {
                message: error.message().to_string(),
            },
        }])
    })
}

/// The table at `path` within `root`, made where missing.
fn table_at<'d>(root: &'d mut Table, path: &[&str]) -> Result<&'d mut dyn toml_edit::TableLike, EditError> {
    let mut table: &mut dyn toml_edit::TableLike = root;
    for (depth, key) in path.iter().enumerate() {
        let item = table.entry(key).or_insert_with(|| {
            let mut new = Table::new();
            new.set_implicit(true);
            Item::Table(new)
        });
        table = item.as_table_like_mut().ok_or_else(|| EditError::NotATable {
            key: path[..=depth].join("."),
        })?;
    }
    Ok(table)
}

/// `text` with the value at `key` (such as `["limits", "turn_minutes"]`) set to `value`; a value that was there keeps
/// its comments.
pub fn set(text: &str, key: &[&str], value: EditValue) -> Result<String, EditError> {
    let (last, parents) = key.split_last().ok_or(EditError::EmptyKey)?;
    let mut doc = document(text)?;
    let table = table_at(doc.as_table_mut(), parents)?;
    let mut new = value.into_value();
    match table.get_mut(last) {
        Some(Item::Value(old)) => {
            *new.decor_mut() = old.decor().clone();
            *old = new;
        }
        Some(item) => *item = Item::Value(new),
        None => {
            table.insert(last, Item::Value(new));
        }
    }
    Ok(doc.to_string())
}

/// `text` without the value at `key`, so the default applies again ("zurücksetzen", E183).
pub fn remove(text: &str, key: &[&str]) -> Result<String, EditError> {
    let (last, parents) = key.split_last().ok_or(EditError::EmptyKey)?;
    let mut doc = document(text)?;
    let mut table: &mut dyn toml_edit::TableLike = doc.as_table_mut();
    for part in parents {
        match table.get_mut(part).and_then(Item::as_table_like_mut) {
            Some(inner) => table = inner,
            None => return Ok(text.to_string()),
        }
    }
    table.remove(last);
    Ok(doc.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    const FILE: &str = "\
# Gates for this repository.
version = 1

[limits]
# Long builds.
turn_minutes = 45 # keep under an hour
attempts = 2

[gates]
network = [\"crates.io\"]
";

    #[test]
    fn a_changed_value_keeps_comments_and_order() {
        let out = set(FILE, &["limits", "turn_minutes"], EditValue::Count(30)).unwrap();
        assert_eq!(out, FILE.replace("turn_minutes = 45", "turn_minutes = 30"));
    }

    #[test]
    fn a_new_value_goes_into_its_table_or_a_new_one() {
        let out = set(FILE, &["limits", "review_rounds"], EditValue::Count(2)).unwrap();
        assert!(out.contains("attempts = 2\nreview_rounds = 2\n"), "{out}");
        let out = set(FILE, &["review", "rule"], EditValue::Text("preferred".into())).unwrap();
        assert!(out.starts_with("# Gates for this repository."), "{out}");
        assert!(out.ends_with("[review]\nrule = \"preferred\"\n"), "{out}");
        assert!(super::super::parse(&out).is_ok());
    }

    #[test]
    fn lists_and_removal() {
        let out = set(FILE, &["gates", "network"], EditValue::Texts(vec!["a.org".into(), "b.org".into()])).unwrap();
        assert!(out.contains("network = [\"a.org\", \"b.org\"]"), "{out}");
        let out = remove(FILE, &["limits", "attempts"]).unwrap();
        assert!(!out.contains("attempts") && out.contains("# keep under an hour"), "{out}");
        assert_eq!(remove(FILE, &["nothing", "here"]).unwrap(), FILE);
    }

    #[test]
    fn a_value_in_the_way_of_a_table_is_refused() {
        assert_eq!(
            set(FILE, &["version", "x"], EditValue::Switch(true)),
            Err(EditError::NotATable { key: "version".into() })
        );
        assert!(matches!(set("a = [", &["a"], EditValue::Count(1)), Err(EditError::Invalid(_))));
    }
}

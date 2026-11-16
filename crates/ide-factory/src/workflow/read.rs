//! Reading a parsed TOML document key by key, so every error names its key and place and none is a bare message of
//! the parser (E141).

use toml_edit::{Item, TableLike, Value};

use super::{WorkflowError, WorkflowErrorKind};

/// What a key holds, for an error about the wrong kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Expected {
    /// A string.
    Text,
    /// A whole number of at least zero.
    Count,
    /// A number.
    Number,
    /// `true` or `false`.
    Switch,
    /// A list of strings.
    Texts,
    /// A table.
    Table,
    /// A list of tables.
    Tables,
    /// One of the words given.
    OneOf(&'static [&'static str]),
}

pub(super) struct Reader<'t> {
    text: &'t str,
    pub(super) errors: Vec<WorkflowError>,
}

impl<'t> Reader<'t> {
    pub(super) fn new(text: &'t str) -> Reader<'t> {
        Reader {
            text,
            errors: Vec::new(),
        }
    }

    /// Line and column, both from one, of byte `offset`.
    pub(super) fn place(&self, offset: usize) -> (usize, usize) {
        let before = &self.text[..offset.min(self.text.len())];
        let line = before.matches('\n').count() + 1;
        let column = before.rsplit('\n').next().map_or(0, |l| l.chars().count()) + 1;
        (line, column)
    }

    pub(super) fn error(&mut self, key: &str, span: Option<std::ops::Range<usize>>, kind: WorkflowErrorKind) {
        let place = span.map(|s| self.place(s.start));
        self.errors.push(WorkflowError {
            key: key.to_string(),
            line: place.map(|p| p.0),
            column: place.map(|p| p.1),
            kind,
        });
    }

    fn wrong(&mut self, key: &str, item: &Item, expected: Expected) {
        self.error(key, item.span(), WorkflowErrorKind::WrongKind { expected });
    }

    /// Reports every key of `table` that `known` does not list.
    pub(super) fn known(&mut self, table: &dyn TableLike, path: &str, known: &[&str]) {
        for (key, item) in table.iter() {
            if !known.contains(&key) {
                let span = table.get_key_value(key).and_then(|(k, _)| k.span()).or_else(|| item.span());
                self.error(&join(path, key), span, WorkflowErrorKind::UnknownKey);
            }
        }
    }

    pub(super) fn table<'d>(&mut self, parent: &'d dyn TableLike, path: &str, key: &str) -> Option<&'d dyn TableLike> {
        let item = parent.get(key)?;
        let table = item.as_table_like();
        if table.is_none() {
            self.wrong(&join(path, key), item, Expected::Table);
        }
        table
    }

    pub(super) fn text(&mut self, parent: &dyn TableLike, path: &str, key: &str) -> Option<String> {
        let item = parent.get(key)?;
        let text = item.as_str().map(str::to_string);
        if text.is_none() {
            self.wrong(&join(path, key), item, Expected::Text);
        }
        text
    }

    pub(super) fn texts(&mut self, parent: &dyn TableLike, path: &str, key: &str) -> Vec<String> {
        let Some(item) = parent.get(key) else {
            return Vec::new();
        };
        let texts: Option<Vec<String>> = item
            .as_array()
            .and_then(|array| array.iter().map(|v| v.as_str().map(str::to_string)).collect());
        if texts.is_none() {
            self.wrong(&join(path, key), item, Expected::Texts);
        }
        texts.unwrap_or_default()
    }

    pub(super) fn count(&mut self, parent: &dyn TableLike, path: &str, key: &str) -> Option<u64> {
        let item = parent.get(key)?;
        let count = item.as_integer().and_then(|n| u64::try_from(n).ok());
        if count.is_none() {
            self.wrong(&join(path, key), item, Expected::Count);
        }
        count
    }

    pub(super) fn number(&mut self, parent: &dyn TableLike, path: &str, key: &str) -> Option<f64> {
        let item = parent.get(key)?;
        #[expect(clippy::cast_precision_loss, reason = "budgets are far below 2^52")]
        let number = item
            .as_float()
            .or_else(|| item.as_integer().map(|n| n as f64))
            .filter(|n| *n >= 0.0);
        if number.is_none() {
            self.wrong(&join(path, key), item, Expected::Number);
        }
        number
    }

    pub(super) fn switch(&mut self, parent: &dyn TableLike, path: &str, key: &str) -> Option<bool> {
        let item = parent.get(key)?;
        let switch = item.as_bool();
        if switch.is_none() {
            self.wrong(&join(path, key), item, Expected::Switch);
        }
        switch
    }

    /// The word at `key`, which must be one of `words`.
    pub(super) fn word(
        &mut self,
        parent: &dyn TableLike,
        path: &str,
        key: &str,
        words: &'static [&'static str],
    ) -> Option<&'static str> {
        let item = parent.get(key)?;
        let word = item.as_str().and_then(|s| words.iter().copied().find(|w| *w == s));
        if word.is_none() {
            self.wrong(&join(path, key), item, Expected::OneOf(words));
        }
        word
    }

    /// The tables of the list at `key`: an array of tables or an array of inline tables.
    pub(super) fn tables(&mut self, parent: &dyn TableLike, path: &str, key: &str) -> Vec<Item> {
        let Some(item) = parent.get(key) else {
            return Vec::new();
        };
        if let Some(tables) = item.as_array_of_tables() {
            return tables.iter().map(|t| Item::Table(t.clone())).collect();
        }
        let inline: Option<Vec<Item>> = item.as_array().and_then(|array| {
            array
                .iter()
                .map(|v| matches!(v, Value::InlineTable(_)).then(|| Item::Value(v.clone())))
                .collect()
        });
        if inline.is_none() {
            self.wrong(&join(path, key), item, Expected::Tables);
        }
        inline.unwrap_or_default()
    }
}

/// `path.key`, or `key` at the top.
pub(super) fn join(path: &str, key: &str) -> String {
    if path.is_empty() {
        key.to_string()
    } else {
        format!("{path}.{key}")
    }
}

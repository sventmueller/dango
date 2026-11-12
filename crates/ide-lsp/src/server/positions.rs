//! Positions between the editor's UTF-16 and the unit the server counts in.

use std::collections::HashMap;
use std::sync::{Arc, Weak};

use lsp_types::{Position, Uri};

use super::{Inner, State};
use crate::convert::Positions;
use crate::encoding::Encoding;
use crate::error::Error;
use crate::text::Text;
use crate::uri;

/// The largest file read from disk to recount positions in a document that is not open.
const LARGEST_UNOPENED: u64 = 32 * 1024 * 1024;

impl State {
    /// `position` in `uri`, from UTF-16 to the server's unit.
    pub(crate) fn to_server(&self, uri: &Uri, position: Position) -> Result<Position, Error> {
        Ok(self.doc(uri)?.text.convert(position, Encoding::Utf16, self.encoding))
    }

    /// Converts every position in `value` from UTF-16 to the server's unit; documents not open stay as they are.
    pub(crate) fn positions_to_server<T: Positions>(&self, uri: &Uri, mut value: T) -> T {
        if self.encoding != Encoding::Utf16 {
            let encoding = self.encoding;
            value.each(uri, &mut |u, p| {
                if let Some(doc) = self.documents.get(&uri::normalize(u)) {
                    *p = doc.text.convert(*p, Encoding::Utf16, encoding);
                }
            });
        }
        value
    }
}

impl Inner {
    /// A converter of positions in answers from the server's unit to UTF-16.
    pub(crate) fn mapper(self: &Arc<Self>) -> Mapper {
        Mapper {
            encoding: self.lock().encoding,
            inner: Arc::downgrade(self),
            texts: HashMap::new(),
        }
    }
}

/// Recounts positions in answers from the server's unit to UTF-16, reading documents that are not open from disk.
pub(crate) struct Mapper {
    encoding: Encoding,
    inner: Weak<Inner>,
    texts: HashMap<Uri, Option<Arc<Text>>>,
}

impl Mapper {
    pub(crate) fn recount<T: Positions>(&mut self, uri: &Uri, value: &mut T) {
        if self.encoding == Encoding::Utf16 {
            return;
        }
        let encoding = self.encoding;
        let (inner, texts) = (&self.inner, &mut self.texts);
        value.each(uri, &mut |u, p| {
            let text = texts.entry(u.clone()).or_insert_with(|| load(inner, u));
            if let Some(text) = text {
                *p = text.convert(*p, encoding, Encoding::Utf16);
            }
        });
    }
}

fn load(inner: &Weak<Inner>, uri: &Uri) -> Option<Arc<Text>> {
    let key = uri::normalize(uri);
    if let Some(inner) = inner.upgrade()
        && let Some(doc) = inner.lock().documents.get(&key)
    {
        return Some(doc.text.clone());
    }
    let path = uri::to_path(&key)?;
    if std::fs::metadata(&path).ok()?.len() > LARGEST_UNOPENED {
        return None;
    }
    let bytes = std::fs::read(path).ok()?;
    Some(Arc::new(Text::new(&String::from_utf8_lossy(&bytes))))
}

//! The widths of insets, measured once per text and font size, and the stretch of the text whose insets changed, found
//! in one pass over the old and the new ones.

use std::collections::HashMap;
use std::ops::Range;

use objc2::runtime::AnyObject;
use objc2_foundation::{NSDictionary, NSString};

use super::{Inset, MARGIN, PADDING, ghost_attributes, shown_text};
use crate::util::ns;
use objc2_app_kit::NSStringDrawing;

/// More texts than an editor shows at once; past it the widths are measured anew.
const KEPT: usize = 4096;

/// The widths of the texts insets showed, for the font size they were measured at.
#[derive(Default)]
pub(super) struct Widths {
    font_size: f64,
    /// By whether the text is ghost text and the text shown.
    texts: HashMap<(bool, String), f64>,
}

impl Widths {
    /// The width of `inset` on screen with its margins, from the cache where it was measured before.
    pub(super) fn of(&mut self, inset: &Inset, attributes: &NSDictionary<NSString, AnyObject>, font_size: f64) -> f64 {
        if self.font_size.to_bits() != font_size.to_bits() || self.texts.len() > KEPT {
            self.texts.clear();
            self.font_size = font_size;
        }
        let shown = if inset.ghost {
            inset.text.clone()
        } else {
            shown_text(inset)
        };
        let text = *self
            .texts
            .entry((inset.ghost, shown))
            .or_insert_with_key(|(ghost, shown)| {
                let attributes = if *ghost {
                    ghost_attributes(font_size)
                } else {
                    objc2::Message::retain(attributes)
                };
                // SAFETY: The attributes hold a font and a colour, as the method expects.
                unsafe { ns(shown).sizeWithAttributes(Some(&attributes)) }.width
            });
        if inset.ghost {
            return text.ceil();
        }
        let pads = f64::from(u8::from(inset.pad_left) + u8::from(inset.pad_right)) * PADDING;
        (text + 2.0 * MARGIN + pads).ceil()
    }
}

/// The offsets from the first to the last inset that differs between `old` and `new`, both ordered by offset.
pub(super) fn changed_span(old: &[Inset], new: &[Inset]) -> Option<Range<usize>> {
    let same = |a: &Inset, b: &Inset| a.at == b.at && a.text == b.text && a.place == b.place && a.ghost == b.ghost;
    let mut span: Option<Range<usize>> = None;
    let (mut i, mut j) = (0, 0);
    loop {
        let at = match (old.get(i), new.get(j)) {
            (Some(a), Some(b)) => a.at.min(b.at),
            (Some(a), None) => a.at,
            (None, Some(b)) => b.at,
            (None, None) => return span,
        };
        let a = &old[i..i + old[i..].iter().take_while(|x| x.at == at).count()];
        let b = &new[j..j + new[j..].iter().take_while(|x| x.at == at).count()];
        let differs =
            a.iter().any(|x| !b.iter().any(|y| same(x, y))) || b.iter().any(|y| !a.iter().any(|x| same(x, y)));
        if differs {
            span = Some(span.map_or(at..at + 1, |s| s.start.min(at)..s.end.max(at + 1)));
        }
        i += a.len();
        j += b.len();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::editor::insets::Place;

    fn inset(at: usize, text: &str) -> Inset {
        Inset::new(at, text, Place::Inline)
    }

    #[test]
    fn the_changed_stretch_spans_the_insets_that_came_went_or_changed() {
        let old = [inset(1, "a"), inset(5, "b"), inset(5, "c"), inset(9, "d")];
        assert_eq!(changed_span(&old, &old), None);
        let changed = [inset(1, "a"), inset(5, "c"), inset(5, "x"), inset(9, "d")];
        assert_eq!(changed_span(&old, &changed), Some(5..6));
        let moved = [inset(1, "a"), inset(5, "b"), inset(5, "c"), inset(12, "d")];
        assert_eq!(changed_span(&old, &moved), Some(9..13));
        assert_eq!(changed_span(&old, &[]), Some(1..10));
        assert_eq!(changed_span(&[], &old[..1]), Some(1..2));
        let reordered = [inset(1, "a"), inset(5, "c"), inset(5, "b"), inset(9, "d")];
        assert_eq!(changed_span(&old, &reordered), None, "the same insets at one place");
    }
}

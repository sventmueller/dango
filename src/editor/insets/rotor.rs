//! The VoiceOver rotor of an editor's insets: every inset as an entry with its line, stepped through both ways and
//! filtered by what the user types.

use objc2::rc::{Retained, Weak};
use objc2::runtime::ProtocolObject;
use objc2::{AnyThread, DefinedClass, MainThreadOnly, define_class, msg_send};
use objc2_app_kit::{
    NSAccessibility, NSAccessibilityCustomRotor, NSAccessibilityCustomRotorItemResult,
    NSAccessibilityCustomRotorItemSearchDelegate, NSAccessibilityCustomRotorSearchDirection,
    NSAccessibilityCustomRotorSearchParameters, NSView,
};
use objc2_foundation::{NSArray, NSObject, NSObjectProtocol, NSRange, NSString};
use splat_i18n::{t, tf};

use crate::editor::EditorTextView;
use crate::util::{guarded, ns};

pub(crate) struct RotorIvars {
    editor: Weak<EditorTextView>,
}

define_class!(
    // SAFETY: NSObject has no subclassing requirements and `Rotor` does not implement Drop.
    #[unsafe(super(NSObject))]
    #[thread_kind = MainThreadOnly]
    #[name = "IDEInsetRotor"]
    #[ivars = RotorIvars]
    pub(crate) struct Rotor;

    unsafe impl NSObjectProtocol for Rotor {}

    unsafe impl NSAccessibilityCustomRotorItemSearchDelegate for Rotor {
        #[unsafe(method_id(rotor:resultForSearchParameters:))]
        fn result(
            &self,
            _rotor: &NSAccessibilityCustomRotor,
            parameters: &NSAccessibilityCustomRotorSearchParameters,
        ) -> Option<Retained<NSAccessibilityCustomRotorItemResult>> {
            guarded(None, || self.search(parameters))
        }
    }
);

/// One entry of the rotor: where it is, and what VoiceOver reads.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct RotorItem {
    pub(crate) at: usize,
    pub(crate) label: String,
}

/// The rotor's entries for `view`: every inset, those at one place together, with their line.
pub(crate) fn rotor_items(view: &EditorTextView) -> Vec<RotorItem> {
    let Ok(state) = view.ivars().insets.try_borrow() else {
        return Vec::new();
    };
    let spoken: Vec<(usize, String)> = state
        .layers
        .iter()
        .flat_map(|(_, insets)| {
            insets.iter().map(|i| match i.severity {
                Some(severity) => (i.at, format!("{}: {}", severity.name(), i.text)),
                None => (i.at, i.text.clone()),
            })
        })
        .collect();
    let mut all: Vec<(usize, &str)> = spoken.iter().map(|(at, text)| (*at, text.as_str())).collect();
    all.sort_by_key(|(at, _)| *at);
    let document = view.document();
    let text = document.text();
    let mut items: Vec<RotorItem> = Vec::new();
    let mut texts: Vec<(usize, Vec<&str>)> = Vec::new();
    for (at, label) in all {
        match texts.last_mut() {
            Some((last, labels)) if *last == at => labels.push(label),
            _ => texts.push((at, vec![label])),
        }
    }
    for (at, labels) in texts {
        let line = text
            .utf16_to_byte(at.min(text.len_utf16()))
            .map_or(0, |byte| text.line_of(byte));
        items.push(RotorItem {
            at,
            label: tf("{}, Zeile {}", &[&labels.join(" "), &(line + 1).to_string()]),
        });
    }
    items
}

/// The entry after or before `current` in `items`, among those whose label holds `filter`.
fn next_item(items: &[RotorItem], current: Option<usize>, forward: bool, filter: &str) -> Option<RotorItem> {
    let filter = filter.to_lowercase();
    let matching = items
        .iter()
        .filter(|item| filter.is_empty() || item.label.to_lowercase().contains(&filter));
    let found = if forward {
        matching.into_iter().find(|item| current.is_none_or(|c| item.at > c))
    } else {
        matching
            .into_iter()
            .rev()
            .find(|item| current.is_none_or(|c| item.at < c))
    };
    found.cloned()
}

impl Rotor {
    fn search(
        &self,
        parameters: &NSAccessibilityCustomRotorSearchParameters,
    ) -> Option<Retained<NSAccessibilityCustomRotorItemResult>> {
        let editor = self.ivars().editor.load()?;
        let current = parameters
            .currentItem()
            .map(|item| item.targetRange().location)
            .filter(|&at| at != objc2_foundation::NSNotFound.cast_unsigned());
        let forward = parameters.searchDirection() == NSAccessibilityCustomRotorSearchDirection::Next;
        // SAFETY: The getter takes no arguments; AppKit answers nil without a filter, which the generated binding does
        // not allow for.
        let filter: Option<Retained<NSString>> = unsafe { msg_send![parameters, filterString] };
        let filter = filter.map(|f| f.to_string()).unwrap_or_default();
        let item = next_item(&rotor_items(&editor), current, forward, &filter)?;
        let element: &NSView = &editor;
        let result = NSAccessibilityCustomRotorItemResult::initWithTargetElement(
            NSAccessibilityCustomRotorItemResult::alloc(),
            ProtocolObject::from_ref(element),
        );
        result.setTargetRange(NSRange::new(item.at, 0));
        result.setCustomLabel(Some(&ns(&item.label)));
        Some(result)
    }
}

/// Gives `view` the rotor "Hinweise", beside the rotors it has.
pub(crate) fn attach(view: &EditorTextView) {
    let delegate = Rotor::alloc(view.mtm()).set_ivars(RotorIvars {
        editor: Weak::from(view),
    });
    // SAFETY: NSObject's initializer, called once on the newly allocated object.
    let delegate: Retained<Rotor> = unsafe { msg_send![super(delegate), init] };
    let rotor = NSAccessibilityCustomRotor::initWithLabel_itemSearchDelegate(
        NSAccessibilityCustomRotor::alloc(),
        &ns(t("Hinweise")),
        ProtocolObject::from_ref(&*delegate),
    );
    // SAFETY: The getter takes no arguments; NSTextView answers nil where it has no rotors, which the generated
    // binding does not allow for.
    let own: Option<Retained<NSArray<NSAccessibilityCustomRotor>>> =
        unsafe { msg_send![view, accessibilityCustomRotors] };
    let mut rotors: Vec<Retained<NSAccessibilityCustomRotor>> = own.map(|r| r.to_vec()).unwrap_or_default();
    rotors.push(rotor);
    view.setAccessibilityCustomRotors(&NSArray::from_retained_slice(&rotors));
    view.ivars().insets.borrow_mut().rotor = Some(delegate);
    let layout = crate::editor::inset_fragment::attach(view);
    view.ivars().insets.borrow_mut().layout = layout;
    if let Some(manager) = view.textLayoutManager() {
        // Fragments laid out before the delegate came are plain ones.
        manager.invalidateLayoutForRange(&objc2_app_kit::NSTextSelectionDataSource::documentRange(&*manager));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_rotor_steps_through_insets_both_ways_and_filters() {
        let items: Vec<RotorItem> = [
            (3, "a: i32, Zeile 1"),
            (9, "b: u8, Zeile 2"),
            (20, "c: String, Zeile 4"),
        ]
        .iter()
        .map(|(at, label)| RotorItem {
            at: *at,
            label: (*label).into(),
        })
        .collect();
        assert_eq!(next_item(&items, None, true, "").map(|i| i.at), Some(3));
        assert_eq!(next_item(&items, Some(3), true, "").map(|i| i.at), Some(9));
        assert_eq!(next_item(&items, Some(20), true, ""), None);
        assert_eq!(next_item(&items, None, false, "").map(|i| i.at), Some(20));
        assert_eq!(next_item(&items, Some(9), false, "").map(|i| i.at), Some(3));
        assert_eq!(next_item(&items, None, true, "STRING").map(|i| i.at), Some(20));
    }
}

#[cfg(feature = "selftest")]
mod selftest_probe;
#[cfg(feature = "selftest")]
pub(crate) use selftest_probe::*;

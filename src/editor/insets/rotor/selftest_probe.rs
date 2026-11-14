//! The self-test's hooks of `rotor.rs`.

use super::*;

/// The rotor "Hinweise" of `view` and the entry it finds from `current` onward, as VoiceOver asks, for the self-test.
pub fn rotor_step(view: &EditorTextView, current: Option<usize>, forward: bool) -> (Option<String>, Option<RotorItem>) {
    // SAFETY: The getter takes no arguments and may answer nil, which the `Option` result type allows for.
    let rotors: Option<Retained<NSArray<NSAccessibilityCustomRotor>>> =
        unsafe { msg_send![view, accessibilityCustomRotors] };
    let ours = rotors
        .iter()
        .flat_map(|r| r.to_vec())
        .find(|r| r.label().to_string() == t("Hinweise"));
    let label = ours.as_ref().map(|r| r.label().to_string());
    let Some(rotor) = ours else {
        return (None, None);
    };
    let parameters = NSAccessibilityCustomRotorSearchParameters::new();
    parameters.setSearchDirection(if forward {
        NSAccessibilityCustomRotorSearchDirection::Next
    } else {
        NSAccessibilityCustomRotorSearchDirection::Previous
    });
    if let Some(at) = current {
        let element: &NSView = view;
        let item = NSAccessibilityCustomRotorItemResult::initWithTargetElement(
            NSAccessibilityCustomRotorItemResult::alloc(),
            ProtocolObject::from_ref(element),
        );
        item.setTargetRange(NSRange::new(at, 0));
        parameters.setCurrentItem(Some(&item));
    }
    let found = rotor
        .itemSearchDelegate()
        .and_then(|delegate| delegate.rotor_resultForSearchParameters(&rotor, &parameters));
    let item = found.map(|result| RotorItem {
        at: result.targetRange().location,
        label: result.customLabel().map(|l| l.to_string()).unwrap_or_default(),
    });
    (label, item)
}

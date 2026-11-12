//! The VoiceOver rotor "Haltepunkte" of an editor (E101): its file's breakpoints and the paused line, each with its
//! line, stepped through both ways.

use std::cell::RefCell;

use objc2::rc::{Retained, Weak};
use objc2::runtime::ProtocolObject;
use objc2::{AnyThread, DefinedClass, MainThreadOnly, define_class, msg_send};
use objc2_app_kit::{
    NSAccessibility, NSAccessibilityCustomRotor, NSAccessibilityCustomRotorItemResult,
    NSAccessibilityCustomRotorItemSearchDelegate, NSAccessibilityCustomRotorSearchDirection,
    NSAccessibilityCustomRotorSearchParameters, NSView,
};
use objc2_foundation::{NSArray, NSObject, NSObjectProtocol, NSRange};
use splat_i18n::t;

use crate::editor::EditorTextView;
use crate::util::{guarded, ns};

pub struct RotorIvars {
    editor: Weak<EditorTextView>,
}

define_class!(
    // SAFETY: NSObject has no subclassing requirements and `BreakpointRotor` does not implement Drop.
    #[unsafe(super(NSObject))]
    #[thread_kind = MainThreadOnly]
    #[name = "IDEBreakpointRotor"]
    #[ivars = RotorIvars]
    pub struct BreakpointRotor;

    unsafe impl NSObjectProtocol for BreakpointRotor {}

    unsafe impl NSAccessibilityCustomRotorItemSearchDelegate for BreakpointRotor {
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

/// The rotors' delegates, which the rotors hold only weakly, until their editor is gone.
#[derive(Default)]
pub struct Rotors {
    delegates: RefCell<Vec<Retained<BreakpointRotor>>>,
}

/// The marked lines of `editor`, each with the UTF-16 offset of its start and what VoiceOver reads.
pub fn items(editor: &EditorTextView) -> Vec<(usize, String)> {
    let lines = editor.document().text().line_count();
    let mut marked: Vec<usize> = Vec::new();
    if let Some((panel, path)) = super::editor::panel_and_path(editor) {
        let model = panel.read();
        marked.extend(model.breakpoints.of(&path).map(|b| b.line));
        if let Some(frame) = model.paused_at().filter(|f| f.path.as_deref() == Some(&path)) {
            marked.push(frame.line);
        }
    }
    marked.sort_unstable();
    marked.dedup();
    marked
        .into_iter()
        .filter(|line| *line < lines)
        .filter_map(|line| {
            let (look, paused) = super::gutter::mark(editor, line);
            let words = super::gutter::words(look, paused, line)?;
            let document = editor.document();
            let byte = document.text().offset(line, 0, ide_document::Unit::Utf16).ok()?;
            Some((document.text().byte_to_utf16(byte).ok()?, words))
        })
        .collect()
}

impl BreakpointRotor {
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
        let all = items(&editor);
        let found = if forward {
            all.into_iter().find(|(at, _)| current.is_none_or(|c| *at > c))
        } else {
            all.into_iter().rev().find(|(at, _)| current.is_none_or(|c| *at < c))
        }?;
        let element: &NSView = &editor;
        let result = NSAccessibilityCustomRotorItemResult::initWithTargetElement(
            NSAccessibilityCustomRotorItemResult::alloc(),
            ProtocolObject::from_ref(element),
        );
        result.setTargetRange(NSRange::new(found.0, 0));
        result.setCustomLabel(Some(&ns(&found.1)));
        Some(result)
    }
}

/// Gives `editor` the rotor "Haltepunkte", beside the rotors it has.
pub fn attach(editor: &EditorTextView) {
    let delegate = BreakpointRotor::alloc(editor.mtm()).set_ivars(RotorIvars {
        editor: Weak::from(editor),
    });
    // SAFETY: NSObject's initializer, called once on the newly allocated object.
    let delegate: Retained<BreakpointRotor> = unsafe { msg_send![super(delegate), init] };
    let rotor = NSAccessibilityCustomRotor::initWithLabel_itemSearchDelegate(
        NSAccessibilityCustomRotor::alloc(),
        &ns(t("Haltepunkte")),
        ProtocolObject::from_ref(&*delegate),
    );
    // SAFETY: The getter takes no arguments; NSTextView answers nil where it has no rotors, which the generated binding
    // does not allow for.
    let own: Option<Retained<NSArray<NSAccessibilityCustomRotor>>> =
        unsafe { msg_send![editor, accessibilityCustomRotors] };
    let mut rotors: Vec<Retained<NSAccessibilityCustomRotor>> = own.map(|r| r.to_vec()).unwrap_or_default();
    rotors.push(rotor);
    editor.setAccessibilityCustomRotors(&NSArray::from_retained_slice(&rotors));
    let state = &crate::context::app().debug.rotors;
    let mut delegates = state.delegates.borrow_mut();
    delegates.retain(|d| d.ivars().editor.load().is_some());
    delegates.push(delegate);
}

/// The labels of `editor`'s rotors, for the self-test.
#[cfg(feature = "selftest")]
pub fn labels(editor: &EditorTextView) -> Vec<String> {
    // SAFETY: As in `attach`.
    let rotors: Option<Retained<NSArray<NSAccessibilityCustomRotor>>> =
        unsafe { msg_send![editor, accessibilityCustomRotors] };
    rotors
        .iter()
        .flat_map(|r| r.to_vec())
        .map(|r| r.label().to_string())
        .collect()
}

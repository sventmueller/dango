//! The editor's text view: TextKit 2, with the document model following every edit it makes.

use std::cell::{Cell, RefCell};

use ide_document::recovery::RecoveryId;
use ide_document::{Change, Document, Edit, Unit};
use objc2::rc::{Retained, Weak};
use objc2::runtime::{AnyObject, ProtocolObject, Sel};
use objc2::{AnyThread, DefinedClass, MainThreadMarker, MainThreadOnly, define_class, msg_send};
use objc2_app_kit::{
    NSEvent, NSSelectionAffinity, NSTextContainer, NSTextContentManager, NSTextContentManagerDelegate,
    NSTextContentManagerEnumerationOptions, NSTextContentStorage, NSTextContentStorageDelegate, NSTextElement,
    NSTextStorage, NSTextStorageDelegate, NSTextStorageEditActions, NSTextView, NSTrackingArea, NSTrackingAreaOptions,
};
use objc2_foundation::{NSArray, NSObjectProtocol, NSPoint, NSRange, NSRect, NSSize, NSValue};

use crate::util::{guarded, ns};
use crate::window::WindowController;

pub struct ViewIvars {
    pub(super) document: RefCell<Document>,
    /// Keeps the TextKit 2 text system alive; the view holds it only weakly.
    _content: RefCell<Option<Retained<NSTextContentStorage>>>,
    /// Set while the model changes the view's text, so the change is not fed back into the model.
    pub(super) applying: Cell<bool>,
    pub(super) controller: RefCell<Weak<WindowController>>,
    pub(super) tab: Cell<isize>,
    pub(super) font_size: Cell<f64>,
    pub(crate) indent: RefCell<String>,
    /// The dirty state the tab shows, to update it only on a change.
    pub(super) shown_dirty: Cell<bool>,
    /// Edits whose lines wait for colours.
    pub(super) recolor_from: Cell<Option<usize>>,
    /// Markdown's font traits were painted into the storage, which another language takes out again.
    pub(super) styled: Cell<bool>,
    /// The line number column, redrawn when lines change.
    pub(super) gutter: RefCell<Option<Weak<super::ruler::LineNumbers>>>,
    /// Its language server's diagnostics, colours, popovers and status.
    pub(crate) lsp: RefCell<crate::lsp::ViewState>,
    /// Names the copy of the unsaved text against a crash, the one a restored view came from.
    pub(super) recovery: RecoveryId,
    /// Where the text differs from its file's last commit.
    pub(super) marks: RefCell<super::changes::Marks>,
    /// How it indents and what a save tidies.
    pub(super) spacing: RefCell<super::indent::State>,
    /// The view that shows its whitespace.
    pub(super) whitespace: RefCell<Option<Weak<super::whitespace::Overlay>>>,
    /// The minimap beside it (E107).
    pub(super) minimap: RefCell<Option<Weak<super::minimap::Minimap>>>,
    /// The header lines held at its top (E108).
    pub(super) sticky: RefCell<Option<Weak<super::sticky::Sticky>>>,
    /// The lines edited lately, which wait before showing a diagnostic at their end (E130).
    pub(super) line_edits: RefCell<super::line_diagnostics::Edits>,
    /// Vim's state while the Vim mode is on (E105).
    pub(super) vim: RefCell<super::vim::State>,
    /// The ranges that fold and those folded.
    pub(super) folds: RefCell<super::fold::Folds>,
    /// The carets beyond the first.
    pub(super) carets: RefCell<super::multi::Carets>,
    /// What is drawn over the text: fold placeholders and the further carets.
    pub(super) overlay: RefCell<Option<Weak<super::overlay::Overlay>>>,
    /// What the bar above the editor is about: a change of the file on disk, conflict markers or a gone file.
    pub(super) conflict: RefCell<Option<super::conflict::State>>,
    /// The bar above the editor.
    pub(super) bar: RefCell<Option<super::conflict::Bar>>,
    /// The match the search panel revealed, in UTF-16 offsets, marked in the find colour until the selection moves.
    found: RefCell<Option<std::ops::Range<usize>>>,
    /// Text shown that is not in the file: inlay hints and blame (E81).
    pub(super) insets: RefCell<super::insets::Insets>,
    /// Whether a save writes its file on the disk lane.
    pub(super) writing: Cell<super::saving::Writing>,
    /// Who changed which line last (E86).
    pub(super) blame: RefCell<super::blame::State>,
    /// What edit prediction proposes and asks (E92).
    pub(super) predict: RefCell<super::predict::State>,
    /// The notifications the editor follows, its scroll view's size and scrolling, which end with it.
    pub(crate) watches: RefCell<Vec<crate::util::Observation>>,
}

define_class!(
    // SAFETY: NSTextView has no subclassing requirements and `EditorTextView` does not implement Drop.
    #[unsafe(super(NSTextView))]
    #[thread_kind = MainThreadOnly]
    #[name = "IDEEditorTextView"]
    #[ivars = ViewIvars]
    pub struct EditorTextView;

    unsafe impl NSObjectProtocol for EditorTextView {}

    unsafe impl NSTextContentManagerDelegate for EditorTextView {
        #[unsafe(method(textContentManager:shouldEnumerateTextElement:options:))]
        fn should_enumerate(
            &self,
            manager: &NSTextContentManager,
            element: &NSTextElement,
            _options: NSTextContentManagerEnumerationOptions,
        ) -> bool {
            guarded(true, || super::fold::shows(self, manager, element))
        }
    }

    unsafe impl NSTextContentStorageDelegate for EditorTextView {}

    unsafe impl NSTextStorageDelegate for EditorTextView {
        #[unsafe(method(textStorage:didProcessEditing:range:changeInLength:))]
        fn storage_edited(&self, storage: &NSTextStorage, mask: NSTextStorageEditActions, range: NSRange, delta: isize) {
            if mask.contains(NSTextStorageEditActions::EditedCharacters) && !self.ivars().applying.get() {
                guarded((), || self.follow_edit(storage, range, delta));
            }
        }
    }

    impl EditorTextView {
        #[unsafe(method(insertNewline:))]
        fn insert_newline(&self, _sender: Option<&AnyObject>) {
            guarded((), || super::editing::newline(self));
        }

        #[unsafe(method(insertTab:))]
        fn insert_tab(&self, _sender: Option<&AnyObject>) {
            guarded((), || super::editing::tab(self));
        }

        #[unsafe(method(insertText:replacementRange:))]
        fn insert_text(&self, text: &AnyObject, range: NSRange) {
            let _interval = ide_process::signpost::begin(ide_process::signpost::name!("keystroke"));
            if guarded(false, || super::vim::insert_text(self, text)) {
                return;
            }
            guarded((), || super::predict::before_edit(self));
            if guarded(false, || super::multi::insert(self, text)) {
                return;
            }
            let handled = guarded(false, || super::editing::typed(self, text, range));
            if !handled {
                // SAFETY: The overridden method with arguments of its types, which objc2 checks in debug builds.
                let _: () = unsafe { msg_send![super(self), insertText: text, replacementRange: range] };
            }
            if !super::multi::replaying(self) {
                guarded((), || crate::lsp::typed(self, text));
            }
        }

        #[unsafe(method(setSelectedRanges:affinity:stillSelecting:))]
        fn set_selected_ranges(&self, ranges: &NSArray<NSValue>, affinity: NSSelectionAffinity, still: bool) {
            // SAFETY: The overridden method with arguments of its types, which objc2 checks in debug builds.
            let _: () = unsafe { msg_send![super(self), setSelectedRanges: ranges, affinity: affinity, stillSelecting: still] };
            if !still {
                guarded((), || {
                    super::insets::strip_typing(self);
                    super::blame::cursor_moved(self);
                    self.clear_found();
                    super::multi::selection_changed(self);
                    super::predict::moved(self);
                    crate::lsp::selection_moved(self);
                    crate::navigation::moved(self, false);
                });
            }
        }

        #[unsafe(method(keyDown:))]
        fn key_down(&self, event: &NSEvent) {
            if !guarded(false, || super::vim::control_key(self, event)) {
                // SAFETY: The overridden method with arguments of its types, which objc2 checks in debug builds.
                let _: () = unsafe { msg_send![super(self), keyDown: event] };
            }
        }

        #[unsafe(method(performKeyEquivalent:))]
        fn perform_key_equivalent(&self, event: &NSEvent) -> bool {
            if guarded(false, || super::vim::control_key(self, event)) {
                true
            } else {
                // SAFETY: The overridden method with arguments of its types, which objc2 checks in debug builds.
                unsafe { msg_send![super(self), performKeyEquivalent: event] }
            }
        }

        #[unsafe(method(resignFirstResponder))]
        fn resign_first_responder(&self) -> bool {
            guarded((), || super::vim::focus_lost(self));
            // SAFETY: The overridden method with arguments of its types, which objc2 checks in debug builds.
            unsafe { msg_send![super(self), resignFirstResponder] }
        }

        #[unsafe(method(becomeFirstResponder))]
        fn become_first_responder(&self) -> bool {
            // SAFETY: The overridden method with arguments of its types, which objc2 checks in debug builds.
            let took: bool = unsafe { msg_send![super(self), becomeFirstResponder] };
            if took {
                guarded((), || crate::navigation::moved(self, true));
            }
            took
        }

        #[unsafe(method(paste:))]
        fn paste(&self, sender: Option<&AnyObject>) {
            if !guarded(false, || super::multi::paste(self) || crate::markdown::paste(self)) {
                // SAFETY: The overridden method with arguments of its types, which objc2 checks in debug builds.
                let _: () = unsafe { msg_send![super(self), paste: sender] };
            }
        }

        #[unsafe(method(copy:))]
        fn copy(&self, sender: Option<&AnyObject>) {
            if !guarded(false, || super::multi::copy(self)) {
                // SAFETY: The overridden method with arguments of its types, which objc2 checks in debug builds.
                let _: () = unsafe { msg_send![super(self), copy: sender] };
            }
        }

        #[unsafe(method(cut:))]
        fn cut(&self, sender: Option<&AnyObject>) {
            if !guarded(false, || super::multi::cut(self)) {
                // SAFETY: The overridden method with arguments of its types, which objc2 checks in debug builds.
                let _: () = unsafe { msg_send![super(self), cut: sender] };
            }
        }

        #[unsafe(method(ideFold:))]
        fn fold_action(&self, _sender: Option<&AnyObject>) {
            guarded((), || super::fold::fold_at_cursor(self));
        }

        #[unsafe(method(ideUnfold:))]
        fn unfold_action(&self, _sender: Option<&AnyObject>) {
            guarded((), || super::fold::unfold_at_cursor(self));
        }

        #[unsafe(method(ideFoldAll:))]
        fn fold_all_action(&self, _sender: Option<&AnyObject>) {
            guarded((), || super::fold::fold_all(self));
        }

        #[unsafe(method(ideUnfoldAll:))]
        fn unfold_all_action(&self, _sender: Option<&AnyObject>) {
            guarded((), || super::fold::unfold_all(self));
        }

        #[unsafe(method(ideAddCaretAbove:))]
        fn add_caret_above(&self, _sender: Option<&AnyObject>) {
            guarded((), || super::multi::add_vertical(self, true));
        }

        #[unsafe(method(ideAddCaretBelow:))]
        fn add_caret_below(&self, _sender: Option<&AnyObject>) {
            guarded((), || super::multi::add_vertical(self, false));
        }

        #[unsafe(method(ideAddNextOccurrence:))]
        fn add_next_occurrence(&self, _sender: Option<&AnyObject>) {
            guarded((), || super::multi::add_next_occurrence(self));
        }

        #[unsafe(method(ideRelayoutFolds))]
        fn relayout_folds(&self) {
            guarded((), || super::fold::relayout(self));
        }

        #[unsafe(method(ideBreadcrumbs))]
        fn breadcrumbs(&self) {
            guarded((), || crate::lsp::status::refresh(self));
        }

        #[unsafe(method(doCommandBySelector:))]
        fn do_command(&self, command: Sel) {
            #[cfg(feature = "dictation")]
            if (command == objc2::sel!(cancelOperation:) || command == objc2::sel!(complete:))
                && guarded(false, || ide_ui_dictation::cancel_in(self))
            {
                return;
            }
            if guarded(false, || {
                crate::lsp::command(self, command)
                    || super::predict::command(self, command)
                    || super::multi::command(self, command)
                    || super::vim::command(self, command)
            }) {
                return;
            }
            guarded((), || super::predict::before_command(self, command));
            // SAFETY: The overridden method with arguments of its types, which objc2 checks in debug builds.
            let _: () = unsafe { msg_send![super(self), doCommandBySelector: command] };
            guarded((), || crate::lsp::after_command(self, command));
        }

        #[unsafe(method(complete:))]
        fn complete(&self, sender: Option<&AnyObject>) {
            if !guarded(false, || crate::lsp::complete(self)) {
                // SAFETY: The overridden method with arguments of its types, which objc2 checks in debug builds.
                let _: () = unsafe { msg_send![super(self), complete: sender] };
            }
        }

        #[unsafe(method(textViewportLayoutControllerDidLayout:))]
        fn viewport_did_layout(&self, controller: &AnyObject) {
            // SAFETY: The overridden method with arguments of its types, which objc2 checks in debug builds.
            let _: () = unsafe { msg_send![super(self), textViewportLayoutControllerDidLayout: controller] };
            guarded((), || super::overlay::laid_out(self));
        }

        #[unsafe(method(mouseMoved:))]
        fn mouse_moved(&self, event: &NSEvent) {
            // SAFETY: The overridden method with arguments of its types, which objc2 checks in debug builds.
            let _: () = unsafe { msg_send![super(self), mouseMoved: event] };
            guarded((), || {
                crate::lsp::mouse_moved(self, event);
                super::blame::mouse_moved(self, event);
                // The text view has just set its I-beam; over a hint that leads somewhere the hand wins.
                let point = self.convertPoint_fromView(event.locationInWindow(), None);
                if super::insets::hits(self, point) {
                    objc2_app_kit::NSCursor::pointingHandCursor().set();
                }
            });
        }

        #[unsafe(method(ideBlameHover))]
        fn blame_hover(&self) {
            guarded((), || super::blame::hover_rest(self));
        }

        #[unsafe(method_id(menuForEvent:))]
        fn menu_for_event(&self, event: &NSEvent) -> Option<Retained<objc2_app_kit::NSMenu>> {
            // SAFETY: The overridden method with arguments of its types, which objc2 checks in debug builds.
            let menu: Option<Retained<objc2_app_kit::NSMenu>> = unsafe { msg_send![super(self), menuForEvent: event] };
            if let Some(menu) = &menu {
                guarded((), || super::context_menu::extend(self, menu));
            }
            menu
        }

        #[unsafe(method(mouseDown:))]
        fn mouse_down(&self, event: &NSEvent) {
            let handled = guarded(false, || {
                let point = self.convertPoint_fromView(event.locationInWindow(), None);
                super::fold::click(self, point)
                    || super::multi::option_mouse(self, event)
                    || super::insets::click_gap(self, event)
                    || crate::lsp::command_click(self, event)
            });
            if handled {
                return;
            }
            let moved = guarded(None, || super::insets::retarget(self, event));
            let event = moved.as_deref().unwrap_or(event);
            // SAFETY: The overridden method with arguments of its types, which objc2 checks in debug builds.
            let _: () = unsafe { msg_send![super(self), mouseDown: event] };
        }

        #[unsafe(method(setTypingAttributes:))]
        fn set_typing_attributes(&self, attributes: &objc2_foundation::NSDictionary<objc2_foundation::NSString, AnyObject>) {
            let clean = guarded(None, || super::insets::without_kern(attributes));
            let attributes = clean.as_deref().unwrap_or(attributes);
            // SAFETY: The overridden method with arguments of its types, which objc2 checks in debug builds.
            let _: () = unsafe { msg_send![super(self), setTypingAttributes: attributes] };
        }

        #[unsafe(method(ideInsets))]
        fn insets(&self) {
            guarded((), || super::insets::settle(self));
        }

        #[unsafe(method(ideBlameRest))]
        fn blame_rest(&self) {
            guarded((), || super::blame::rest(self));
        }

        #[unsafe(method(ideToggleBlameAtLineEnd:))]
        fn toggle_blame_at_line_end(&self, _sender: Option<&AnyObject>) {
            guarded((), super::blame::toggle_line_end);
        }

        #[unsafe(method(ideToggleBlameColumn:))]
        fn toggle_blame_column(&self, _sender: Option<&AnyObject>) {
            guarded((), || super::blame::toggle_column(self));
        }

        #[unsafe(method(ideShowLineBlame:))]
        fn show_line_blame(&self, _sender: Option<&AnyObject>) {
            guarded((), || super::blame::show_for_cursor(self));
        }

        #[unsafe(method(ideBlameCopyHash:))]
        fn blame_copy_hash(&self, _sender: Option<&AnyObject>) {
            guarded((), || super::blame::copy_hash(self));
        }

        #[unsafe(method(ideBlameShowInGraph:))]
        fn blame_show_in_graph(&self, _sender: Option<&AnyObject>) {
            guarded((), || super::blame::show_in_graph(self));
        }

        #[unsafe(method(ideBlameShowDiff:))]
        fn blame_show_diff(&self, _sender: Option<&AnyObject>) {
            guarded((), || super::blame::show_diff(self));
        }

        #[unsafe(method(idePredict))]
        fn predict(&self) {
            guarded((), || super::predict::request(self));
        }

        #[unsafe(method(idePrediction:))]
        fn prediction(&self, sender: Option<&AnyObject>) {
            guarded((), || super::predict::perform(self, sender));
        }

        #[unsafe(method(ideInlayHints))]
        fn inlay_hints(&self) {
            guarded((), || crate::lsp::inlay::request(self));
        }

        #[unsafe(method(ideToggleInlayHints:))]
        fn toggle_inlay_hints(&self, _sender: Option<&AnyObject>) {
            guarded((), crate::lsp::inlay::toggle);
        }

        #[unsafe(method(ideToggleInlayHintsLanguage:))]
        fn toggle_inlay_hints_language(&self, _sender: Option<&AnyObject>) {
            guarded((), || {
                if let Some(language) = crate::lsp::inlay::language(self) {
                    crate::lsp::inlay::toggle_language(&language);
                }
            });
        }

        #[unsafe(method(updateTrackingAreas))]
        fn update_tracking_areas(&self) {
            // SAFETY: The overridden method with arguments of its types, which objc2 checks in debug builds.
            let _: () = unsafe { msg_send![super(self), updateTrackingAreas] };
            guarded((), || self.track_mouse());
        }

        #[unsafe(method(validateMenuItem:))]
        fn validate_menu_item(&self, item: &objc2_app_kit::NSMenuItem) -> bool {
            let answer = guarded(None, || {
                super::whitespace::validate(item)
                    .or_else(|| super::minimap::validate(item))
                    .or_else(|| super::sticky::validate(item))
                    .or_else(|| super::line_diagnostics::validate(item))
                    .or_else(|| super::vim::validate(item))
                    .or_else(|| {
                        (item.action() == Some(objc2::sel!(ideOpenPageInBrowserTab:))).then(|| {
                            self.document().path().is_some_and(crate::site_preview::is_page)
                        })
                    })
                    .or_else(|| super::predict::validate(self, item))
                    .or_else(|| crate::lsp::inlay::validate(self, item))
                    .or_else(|| super::blame::validate(self, item))
                    .or_else(|| crate::preview::validate(self, item.action()))
                    .or_else(|| crate::markdown::validate(self, item))
                    .or_else(|| crate::lsp::validate(self, item.action()))
            });
            match answer {
                Some(enabled) => enabled,
                // SAFETY: The overridden method with arguments of its types, which objc2 checks in debug builds.
                None => unsafe { msg_send![super(self), validateMenuItem: item] },
            }
        }

        #[unsafe(method(ideSave:))]
        fn save_action(&self, _sender: Option<&AnyObject>) {
            guarded((), || crate::lsp::save(self));
        }

        #[unsafe(method(ideComplete:))]
        fn complete_action(&self, _sender: Option<&AnyObject>) {
            guarded((), || {
                crate::lsp::complete(self);
            });
        }

        #[unsafe(method(ideHover:))]
        fn hover_action(&self, _sender: Option<&AnyObject>) {
            guarded((), || crate::lsp::hover_at_cursor(self));
        }

        #[unsafe(method(ideDefinition:))]
        fn definition_action(&self, _sender: Option<&AnyObject>) {
            guarded((), || crate::lsp::definition(self));
        }

        #[unsafe(method(ideReferences:))]
        fn references_action(&self, _sender: Option<&AnyObject>) {
            guarded((), || crate::lsp::references(self));
        }

        #[unsafe(method(ideImplementation:))]
        fn implementation_action(&self, _sender: Option<&AnyObject>) {
            guarded((), || crate::lsp::go_to(self, crate::lsp::Goto::Implementation));
        }

        #[unsafe(method(ideTypeDefinition:))]
        fn type_definition_action(&self, _sender: Option<&AnyObject>) {
            guarded((), || crate::lsp::go_to(self, crate::lsp::Goto::TypeDefinition));
        }

        #[unsafe(method(ideDeclaration:))]
        fn declaration_action(&self, _sender: Option<&AnyObject>) {
            guarded((), || crate::lsp::go_to(self, crate::lsp::Goto::Declaration));
        }

        #[unsafe(method(ideCallHierarchy:))]
        fn call_hierarchy_action(&self, _sender: Option<&AnyObject>) {
            guarded((), || crate::lsp::call_hierarchy(self));
        }

        #[unsafe(method(ideTypeHierarchy:))]
        fn type_hierarchy_action(&self, _sender: Option<&AnyObject>) {
            guarded((), || crate::lsp::type_hierarchy(self));
        }

        #[unsafe(method(ideSymbols:))]
        fn symbols_action(&self, _sender: Option<&AnyObject>) {
            guarded((), || crate::lsp::symbols(self));
        }

        #[unsafe(method(ideRename:))]
        fn rename_action(&self, _sender: Option<&AnyObject>) {
            guarded((), || crate::lsp::rename(self));
        }

        #[unsafe(method(ideFormat:))]
        fn format_action(&self, _sender: Option<&AnyObject>) {
            guarded((), || crate::lsp::format(self, None));
        }

        #[unsafe(method(ideQuickFix:))]
        fn quick_fix_action(&self, _sender: Option<&AnyObject>) {
            guarded((), || crate::lsp::code_actions(self, crate::lsp::ActionKind::Fixes));
        }

        #[unsafe(method(ideRefactor:))]
        fn refactor_action(&self, _sender: Option<&AnyObject>) {
            guarded((), || crate::lsp::code_actions(self, crate::lsp::ActionKind::Refactor));
        }

        #[unsafe(method(ideSourceActions:))]
        fn source_actions(&self, _sender: Option<&AnyObject>) {
            guarded((), || crate::lsp::code_actions(self, crate::lsp::ActionKind::Source));
        }

        #[unsafe(method(ideSignatureHelp:))]
        fn signature_help_action(&self, _sender: Option<&AnyObject>) {
            guarded((), || crate::lsp::signature_help(self));
        }

        #[unsafe(method(ideNextProblem:))]
        fn next_problem_action(&self, _sender: Option<&AnyObject>) {
            guarded((), || crate::lsp::next_problem(self, true));
        }

        #[unsafe(method(idePreviousProblem:))]
        fn previous_problem_action(&self, _sender: Option<&AnyObject>) {
            guarded((), || crate::lsp::next_problem(self, false));
        }

        #[unsafe(method(ideLspAction:))]
        fn lsp_action(&self, _sender: Option<&AnyObject>) {
            guarded((), || crate::lsp::status_action(self));
        }

        #[unsafe(method(ideJumpToOffset:))]
        fn jump_to_offset(&self, sender: Option<&AnyObject>) {
            guarded((), || {
                let tag = sender
                    .and_then(|s| s.downcast_ref::<objc2_app_kit::NSMenuItem>())
                    .map_or(0, |item| item.tag());
                crate::lsp::jump_to(self, usize::try_from(tag).unwrap_or(0));
            });
        }

        #[unsafe(method(ideHoverRest))]
        fn hover_rest(&self) {
            guarded((), || crate::lsp::hover_rest(self));
        }

        #[unsafe(method(ideDecorate))]
        fn decorate(&self) {
            guarded((), || crate::lsp::paint(self));
        }

        #[unsafe(method(ideActionsMoved))]
        fn actions_moved(&self) {
            guarded((), || crate::lsp::actions_moved(self));
        }

        #[unsafe(method(ideLspRefresh))]
        fn lsp_refresh(&self) {
            guarded((), || crate::lsp::refresh(self));
        }

        #[unsafe(method(ideSaveAs:))]
        fn save_as_action(&self, _sender: Option<&AnyObject>) {
            guarded((), || super::save_soon(self, true));
        }

        #[unsafe(method(ideToggleComment:))]
        fn toggle_comment(&self, _sender: Option<&AnyObject>) {
            guarded((), || super::editing::toggle_comment(self));
        }

        #[unsafe(method(ideGoToLine:))]
        fn go_to_line(&self, _sender: Option<&AnyObject>) {
            guarded((), || super::editing::go_to_line(self));
        }

        #[unsafe(method(ideToggleWrap:))]
        fn toggle_wrap(&self, _sender: Option<&AnyObject>) {
            guarded((), || super::toggle_wrap(self));
        }

        #[unsafe(method(ideMarkdown:))]
        fn markdown(&self, sender: Option<&AnyObject>) {
            let tag = sender
                .and_then(|s| s.downcast_ref::<objc2_app_kit::NSMenuItem>())
                .map_or(0, |item| item.tag());
            guarded((), || crate::markdown::perform(self, tag));
        }

        #[unsafe(method(ideShowPreview:))]
        fn show_preview(&self, _sender: Option<&AnyObject>) {
            guarded((), || crate::preview::toggle_beside(self));
        }

        #[unsafe(method(ideConflictDiff:))]
        fn conflict_diff(&self, _sender: Option<&AnyObject>) {
            guarded((), || super::conflict::show_differences(self));
        }

        #[unsafe(method(ideConflictMerge:))]
        fn conflict_merge(&self, _sender: Option<&AnyObject>) {
            guarded((), || super::conflict::merge(self));
        }

        #[unsafe(method(ideConflictLoad:))]
        fn conflict_load(&self, _sender: Option<&AnyObject>) {
            guarded((), || super::conflict::load(self));
        }

        #[unsafe(method(ideConflictKeep:))]
        fn conflict_keep(&self, _sender: Option<&AnyObject>) {
            guarded((), || super::conflict::keep(self));
        }

        #[unsafe(method(ideConflictSave:))]
        fn conflict_save(&self, _sender: Option<&AnyObject>) {
            guarded((), || super::conflict::save(self));
        }

        #[unsafe(method(ideConflictClose:))]
        fn conflict_close(&self, _sender: Option<&AnyObject>) {
            guarded((), || super::conflict::close(self));
        }

        #[unsafe(method(ideConflictNext:))]
        fn conflict_next(&self, _sender: Option<&AnyObject>) {
            guarded((), || super::conflict::go_to_conflict(self, true));
        }

        #[unsafe(method(ideConflictPrevious:))]
        fn conflict_previous(&self, _sender: Option<&AnyObject>) {
            guarded((), || super::conflict::go_to_conflict(self, false));
        }

        #[unsafe(method(ideConflictRecount))]
        fn conflict_recount(&self) {
            guarded((), || super::conflict::recount(self));
        }

        #[unsafe(method(ideColorStep))]
        fn color_step(&self) {
            guarded((), || super::highlight::step(self));
        }

        #[unsafe(method(ideGitMarks))]
        fn git_marks(&self) {
            guarded((), || super::changes::compare(self));
        }

        #[unsafe(method(ideGitHead))]
        fn git_head(&self) {
            guarded((), || super::changes::reload(self, false));
        }

        #[unsafe(method(ideRevertHunk:))]
        fn revert_hunk(&self, _sender: Option<&AnyObject>) {
            guarded((), || super::changes::revert(self));
        }

        #[unsafe(method(ideOpenPageInBrowserTab:))]
        fn open_page_in_browser_tab(&self, _sender: Option<&AnyObject>) {
            guarded((), || {
                let page = self.document().path().map(std::path::Path::to_path_buf);
                if let (Some(page), Some(controller)) = (page, self.controller()) {
                    crate::site_preview::open(&controller, &page);
                }
            });
        }

        #[unsafe(method(ideTestRunAtLine:))]
        fn test_run_at_line(&self, sender: Option<&AnyObject>) {
            guarded((), || {
                let line = sender.and_then(|s| s.downcast_ref::<objc2_app_kit::NSMenuItem>()).map_or(-1, |i| i.tag());
                if let Ok(line) = usize::try_from(line) {
                    crate::tests_panel::gutter_click(self, line, false);
                }
            });
        }

        #[unsafe(method(ideTestDebugAtLine:))]
        fn test_debug_at_line(&self, sender: Option<&AnyObject>) {
            guarded((), || {
                let line = sender.and_then(|s| s.downcast_ref::<objc2_app_kit::NSMenuItem>()).map_or(-1, |i| i.tag());
                if let Ok(line) = usize::try_from(line) {
                    crate::tests_panel::gutter_click(self, line, true);
                }
            });
        }

        #[unsafe(method(ideToggleVimMode:))]
        fn toggle_vim(&self, _sender: Option<&AnyObject>) {
            guarded((), super::vim::toggle);
        }

        #[unsafe(method(ideToggleDiagnosticsAtLineEnd:))]
        fn toggle_line_diagnostics(&self, _sender: Option<&AnyObject>) {
            guarded((), super::line_diagnostics::toggle);
        }

        #[unsafe(method(ideStickyRefresh))]
        fn sticky_refresh(&self) {
            guarded((), || super::sticky::refresh(self));
        }

        #[unsafe(method(ideDiagnosticsRested))]
        fn diagnostics_rested(&self) {
            guarded((), || super::line_diagnostics::refresh(self));
        }

        #[unsafe(method(ideToggleStickyScroll:))]
        fn toggle_sticky(&self, _sender: Option<&AnyObject>) {
            guarded((), super::sticky::toggle);
        }

        #[unsafe(method(scrollRangeToVisible:))]
        fn scroll_range_to_visible(&self, range: NSRange) {
            // SAFETY: The overridden method with arguments of its types, which objc2 checks in debug builds.
            let () = unsafe { msg_send![super(self), scrollRangeToVisible: range] };
            guarded((), || super::sticky::keep_clear(self, range));
        }

        // TextKit 2's own `textContainerOrigin` lays out the whole text on every call, and the caret asks for it at
        // every keystroke; for this view it is always the inset.
        #[unsafe(method(textContainerOrigin))]
        fn text_container_origin(&self) -> NSPoint {
            let inset = self.textContainerInset();
            NSPoint::new(inset.width, inset.height)
        }

        #[unsafe(method(ideFitMinimap))]
        fn fit_minimap(&self) {
            guarded((), || super::minimap::fit(self));
        }

        #[unsafe(method(ideToggleMinimap:))]
        fn toggle_minimap(&self, _sender: Option<&AnyObject>) {
            guarded((), super::minimap::toggle);
        }

        #[unsafe(method(ideToggleWhitespace:))]
        fn toggle_whitespace(&self, _sender: Option<&AnyObject>) {
            guarded((), super::whitespace::toggle);
        }

        #[unsafe(method(ideIndentMenu:))]
        fn indent_menu(&self, sender: Option<&AnyObject>) {
            guarded((), || super::indent::show_menu(self, sender));
        }

        #[unsafe(method(ideSetIndent:))]
        fn set_indent(&self, sender: Option<&AnyObject>) {
            guarded((), || {
                let tag = sender
                    .and_then(|s| s.downcast_ref::<objc2_app_kit::NSMenuItem>())
                    .map_or(0, |item| item.tag());
                super::indent::choose(self, tag);
            });
        }
    }
);

impl EditorTextView {
    /// A text view over `document` with a TextKit 2 text system of its own, whose copy against a crash is `recovery`.
    pub fn create(
        mtm: MainThreadMarker,
        document: Document,
        controller: &WindowController,
        recovery: RecoveryId,
    ) -> Retained<Self> {
        let content = NSTextContentStorage::new();
        let layout = super::insets::GapLayoutManager::new();
        content.addTextLayoutManager(&layout);
        let container = NSTextContainer::initWithSize(NSTextContainer::alloc(), NSSize::new(1.0e7, 1.0e7));
        container.setWidthTracksTextView(false);
        layout.setTextContainer(Some(&container));
        let ivars = ViewIvars {
            document: RefCell::new(document),
            _content: RefCell::new(Some(content)),
            applying: Cell::new(false),
            controller: RefCell::new(Weak::from(controller)),
            tab: Cell::new(-1),
            font_size: Cell::new(13.0),
            indent: RefCell::new("    ".into()),
            shown_dirty: Cell::new(false),
            recolor_from: Cell::new(None),
            styled: Cell::new(false),
            gutter: RefCell::new(None),
            lsp: RefCell::new(crate::lsp::ViewState::new()),
            recovery,
            marks: RefCell::new(super::changes::Marks::default()),
            spacing: RefCell::new(super::indent::State::default()),
            whitespace: RefCell::new(None),
            minimap: RefCell::new(None),
            sticky: RefCell::new(None),
            line_edits: RefCell::new(Default::default()),
            vim: RefCell::new(Default::default()),
            folds: RefCell::new(super::fold::Folds::default()),
            carets: RefCell::new(super::multi::Carets::default()),
            overlay: RefCell::new(None),
            conflict: RefCell::new(None),
            bar: RefCell::new(None),
            found: RefCell::new(None),
            insets: RefCell::new(super::insets::Insets::default()),
            writing: Cell::new(super::saving::Writing::Idle),
            blame: RefCell::new(super::blame::State::default()),
            predict: RefCell::new(super::predict::State::default()),
            watches: RefCell::new(Vec::new()),
        };
        let this = Self::alloc(mtm).set_ivars(ivars);
        let frame = NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(600.0, 400.0));
        // SAFETY: NSTextView's designated initializer with a container of its own text system.
        let this: Retained<Self> = unsafe { msg_send![super(this), initWithFrame: frame, textContainer: &*container] };
        if let Some(content) = this.ivars()._content.borrow().as_ref() {
            // SAFETY: The storage holds its delegate weakly, and the view owns the storage, so the delegate lives as long.
            unsafe { content.setDelegate(Some(ProtocolObject::from_ref(&*this))) };
        }
        this
    }

    pub fn storage(&self) -> Option<Retained<NSTextStorage>> {
        // SAFETY: The view keeps its text system alive through `content`; asking for the storage keeps TextKit 2.
        unsafe { self.textStorage() }
    }

    /// Starts following the storage, after the initial text is in.
    pub fn follow_storage(&self) {
        if let Some(storage) = self.storage() {
            storage.setDelegate(Some(ProtocolObject::from_ref(self)));
        }
    }

    fn follow_edit(&self, storage: &NSTextStorage, range: NSRange, delta: isize) {
        let old_length = usize::try_from(isize::try_from(range.length).unwrap_or(0) - delta).unwrap_or(0);
        let text = storage.string().substringWithRange(range).to_string();
        let edit = Edit {
            range_utf16: range.location..range.location + old_length,
            text,
        };
        let change = self.ivars().document.borrow_mut().apply(edit);
        match change {
            Ok(change) => self.changed(&change),
            Err(error) => {
                crate::diag::report("editor out of step with its document", format!("{error:?}"));
                self.resync();
            }
        }
    }

    /// What follows an edit of the document, from the view or from the model.
    pub(super) fn changed(&self, change: &Change) {
        let line = change.start.line;
        let from = self
            .ivars()
            .recolor_from
            .get()
            .map_or(line, |earlier| earlier.min(line));
        self.ivars().recolor_from.set(Some(from));
        super::highlight::schedule(self);
        super::recovery::changed(self);
        super::conflict::text_changed(self);
        super::update_dirty(self);
        super::fold::shift(self, change);
        super::multi::shift(self, change);
        super::insets::shift(self, change);
        super::notify_change(self, change);
        super::changes::schedule(self);
        super::whitespace::redraw(self);
        super::minimap::redraw(self);
        // It reads the layout, which TextKit forbids while the edit is still under way.
        self.later(objc2::sel!(ideStickyRefresh), 0.0);
        super::line_diagnostics::edited(self, change);
        super::vim::changed(self);
        if (change.start.line != change.new_end.line || change.start.line != change.old_end.line)
            && let Some(gutter) = self.ivars().gutter.borrow().as_ref().and_then(Weak::load)
        {
            gutter.fit();
            gutter.setNeedsDisplay(true);
        }
    }

    /// Puts the document's whole text into the view after the two drifted apart.
    pub fn resync(&self) {
        let text = self.ivars().document.borrow().text().to_string();
        self.ivars().applying.set(true);
        self.setString(&ns(&text));
        self.ivars().applying.set(false);
        super::highlight::recolor_all(self);
        super::insets::reset(self);
    }

    /// Makes `changes` of the model in the view, as one undoable step.
    pub fn apply_model_changes(&self, changes: &[Change]) {
        super::vim::before_model_change(self);
        self.apply_changes(changes);
    }

    /// Makes a change Vim made in the view, as an undo step of its own or of the insert pass that is open.
    pub fn apply_vim_change(&self, change: &Change) {
        self.apply_changes(std::slice::from_ref(change));
    }

    fn apply_changes(&self, changes: &[Change]) {
        let Some(storage) = self.storage() else { return };
        self.ivars().applying.set(true);
        super::undo_step(self, || {
            for change in changes {
                let range = NSRange::new(change.utf16.start, change.utf16.end - change.utf16.start);
                if self.shouldChangeTextInRange_replacementString(range, Some(&ns(&change.text))) {
                    storage.replaceCharactersInRange_withString(range, &ns(&change.text));
                    self.didChangeText();
                }
            }
        });
        self.ivars().applying.set(false);
        for change in changes {
            self.changed(change);
        }
    }

    pub fn document(&self) -> std::cell::Ref<'_, Document> {
        self.ivars().document.borrow()
    }

    pub fn document_mut(&self) -> std::cell::RefMut<'_, Document> {
        self.ivars().document.borrow_mut()
    }

    pub fn controller(&self) -> Option<Retained<WindowController>> {
        self.ivars().controller.borrow().load()
    }
}

mod methods;

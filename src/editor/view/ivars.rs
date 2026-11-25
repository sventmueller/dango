//! What the editor's text view holds, and how one is made over a document.

use std::cell::{Cell, RefCell};

use ide_document::Document;
use ide_document::recovery::RecoveryId;
use objc2::rc::{Retained, Weak};
use objc2::runtime::ProtocolObject;
use objc2::{AnyThread, DefinedClass, MainThreadMarker, MainThreadOnly, msg_send};
use objc2_app_kit::{NSTextContainer, NSTextContentStorage, NSTextStorage};
use objc2_foundation::{NSPoint, NSRect, NSSize};

use super::EditorTextView;
use crate::window::{TabId, WindowController};

pub(crate) struct ViewIvars {
    pub(in crate::editor) document: RefCell<Document>,
    /// Keeps the TextKit 2 text system alive; the view holds it only weakly.
    pub(super) _content: RefCell<Option<Retained<NSTextContentStorage>>>,
    /// Set while the model changes the view's text, so the change is not fed back into the model.
    pub(in crate::editor) applying: Cell<bool>,
    /// Whether a panic in one of its callbacks left the editor to be put back in step (Review B9).
    pub(super) recovering: crate::editor::resync::Recovery,
    pub(in crate::editor) controller: RefCell<Weak<WindowController>>,
    pub(in crate::editor) tab: Cell<Option<TabId>>,
    pub(in crate::editor) font_size: Cell<f64>,
    pub(crate) indent: RefCell<String>,
    /// How it indents and what a save tidies.
    pub(in crate::editor) spacing: RefCell<crate::editor::indent::State>,
    /// The lines that wait to be coloured.
    pub(in crate::editor) paint: crate::editor::highlight::Paint,
    /// The views around and over the text.
    pub(in crate::editor) around: crate::editor::container::Around,
    /// What it knows of its file beside the document.
    pub(in crate::editor) file: crate::editor::file::File,
    /// Its language server's diagnostics, colours, popovers and status.
    pub(crate) lsp: RefCell<ide_ui_lsp::ViewState>,
    /// Tells editors apart in callbacks from other threads, which cannot hold a view.
    pub(crate) serial: ide_ui_lsp::EditorSerial,
    /// Where the text differs from its file's last commit.
    pub(in crate::editor) marks: RefCell<crate::editor::changes::Marks>,
    /// The lines edited lately, which wait before showing a diagnostic at their end (E130).
    pub(in crate::editor) line_edits: RefCell<crate::editor::line_diagnostics::Edits>,
    /// Vim's state while the Vim mode is on (E105).
    pub(in crate::editor) vim: RefCell<crate::editor::vim::State>,
    /// The ranges that fold and those folded.
    pub(in crate::editor) folds: RefCell<crate::editor::fold::Folds>,
    /// The carets beyond the first.
    pub(in crate::editor) carets: RefCell<crate::editor::multi::Carets>,
    /// The match the search panel revealed, in UTF-16 offsets, marked in the find colour until the selection moves.
    pub(super) found: RefCell<Option<std::ops::Range<usize>>>,
    /// Text shown that is not in the file: inlay hints and blame (E81).
    pub(in crate::editor) insets: RefCell<crate::editor::insets::Insets>,
    /// Who changed which line last (E86).
    pub(in crate::editor) blame: RefCell<crate::editor::blame::State>,
    /// What edit prediction proposes and asks (E92).
    pub(in crate::editor) predict: RefCell<crate::editor::predict::State>,
    /// The notifications the editor follows, its scroll view's size and scrolling, which end with it.
    pub(crate) watches: RefCell<Vec<crate::util::Observation>>,
    /// The revision and frame size at which TextKit last laid out the whole text.
    pub(in crate::editor) laid_out: Cell<Option<(ide_document::Revision, NSSize)>>,
    /// The fragments on screen as TextKit last laid them out (SN-S0).
    pub(in crate::editor) viewport: crate::editor::viewport::Cache,
    /// Whether what follows a move of the selection after the frame waits to run.
    pub(super) selection_due: Cell<bool>,
    /// Whether the view waits for its next layout pass to take its scroll view's new width (SN-S3).
    pub(in crate::editor) width_due: Cell<bool>,
}

impl EditorTextView {
    /// A text view over `document` with a TextKit 2 text system of its own, whose copy against a crash is `recovery`.
    pub(crate) fn create(
        mtm: MainThreadMarker,
        document: Document,
        controller: &WindowController,
        recovery: RecoveryId,
    ) -> Retained<Self> {
        let content = NSTextContentStorage::new();
        let layout = crate::editor::insets::GapLayoutManager::new();
        content.addTextLayoutManager(&layout);
        let container = NSTextContainer::initWithSize(NSTextContainer::alloc(), NSSize::new(1.0e7, 1.0e7));
        container.setWidthTracksTextView(false);
        layout.setTextContainer(Some(&container));
        let serial = ide_ui_lsp::new_serial();
        let ivars = ViewIvars {
            document: RefCell::new(document),
            _content: RefCell::new(Some(content)),
            applying: Cell::new(false),
            recovering: crate::editor::resync::Recovery::default(),
            controller: RefCell::new(Weak::from(controller)),
            tab: Cell::new(None),
            font_size: Cell::new(13.0),
            indent: RefCell::new("    ".into()),
            spacing: RefCell::new(crate::editor::indent::State::default()),
            paint: crate::editor::highlight::Paint::default(),
            around: crate::editor::container::Around::default(),
            file: crate::editor::file::File::new(recovery),
            lsp: RefCell::new(ide_ui_lsp::ViewState::new(serial)),
            serial,
            marks: RefCell::new(crate::editor::changes::Marks::default()),
            line_edits: RefCell::new(Default::default()),
            vim: RefCell::new(Default::default()),
            folds: RefCell::new(crate::editor::fold::Folds::default()),
            carets: RefCell::new(crate::editor::multi::Carets::default()),
            found: RefCell::new(None),
            insets: RefCell::new(crate::editor::insets::Insets::default()),
            blame: RefCell::new(crate::editor::blame::State::default()),
            predict: RefCell::new(crate::editor::predict::State::default()),
            watches: RefCell::new(Vec::new()),
            laid_out: Cell::new(None),
            viewport: crate::editor::viewport::Cache::default(),
            selection_due: Cell::new(false),
            width_due: Cell::new(false),
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

    pub(crate) fn storage(&self) -> Option<Retained<NSTextStorage>> {
        // SAFETY: The view keeps its text system alive through `content`; asking for the storage keeps TextKit 2.
        unsafe { self.textStorage() }
    }

    /// Starts following the storage, after the initial text is in.
    pub(crate) fn follow_storage(&self) {
        if let Some(storage) = self.storage() {
            storage.setDelegate(Some(ProtocolObject::from_ref(self)));
        }
        crate::editor::long_lines::opened(self);
    }

    /// The size of its text in points.
    pub(crate) fn font_size(&self) -> f64 {
        self.ivars().font_size.get()
    }

    pub(crate) fn document(&self) -> std::cell::Ref<'_, Document> {
        self.ivars().document.borrow()
    }

    pub(crate) fn document_mut(&self) -> std::cell::RefMut<'_, Document> {
        self.ivars().document.borrow_mut()
    }

    pub(crate) fn controller(&self) -> Option<Retained<WindowController>> {
        self.ivars().controller.borrow().load()
    }
}

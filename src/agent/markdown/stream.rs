//! Incremental parsing and rendered-block reuse for the answer that is still arriving.

use objc2::MainThreadMarker;
use objc2::rc::Retained;
use objc2_foundation::{NSAttributedString, NSMutableAttributedString};
use splat_markdown::doc::Doc;
use splat_preview::layout::{Layout, layout, relayout, reusable};
use splat_render::js;

use super::{Renderer, Tone, render_paragraphs};

struct Block {
    key: u64,
    last: bool,
    text: Retained<NSAttributedString>,
}

/// The parsed answer and its rendered blocks; owned only by its visible text view.
pub struct Streaming {
    doc: Doc,
    laid: Layout,
    blocks: Vec<Block>,
    dark: bool,
    macros: std::sync::Arc<str>,
    width: f64,
}

impl Streaming {
    pub fn new(markdown: &str) -> Streaming {
        Streaming {
            doc: splat_markdown::reader::parse(markdown),
            laid: Layout::default(),
            blocks: Vec::new(),
            dark: false,
            macros: "".into(),
            width: 0.0,
        }
    }

    /// Updates only affected Markdown blocks; definitions and other global changes fall back to a full parse.
    pub fn render(
        &mut self,
        mtm: MainThreadMarker,
        markdown: &str,
        width: f64,
    ) -> Option<(Retained<NSAttributedString>, usize)> {
        let dark = super::super::figures::is_dark(mtm);
        let changed = markdown != self.doc.text();
        if !changed && self.width == width && self.dark == dark {
            return None;
        }
        let splice = if changed {
            match splat_markdown::incremental::reparse(&mut self.doc, markdown) {
                Some(splice) => Some(splice),
                None => {
                    self.doc = splat_markdown::reader::parse(markdown);
                    None
                }
            }
        } else {
            None
        };
        let wanted = js::wanted(&self.doc, dark);
        super::super::figures::want(wanted.keys);
        let same_style = self.dark == dark && self.macros == wanted.macros && self.width == width;
        let ready = js::Ready {
            dark,
            macros: wanted.macros.clone(),
        };
        self.laid = match splice.filter(|_| same_style) {
            Some(splice) => relayout(
                &self.doc,
                &ready,
                None,
                std::mem::take(&mut self.laid),
                &reusable(&splice, &self.doc, None),
            ),
            None => layout(&self.doc, &ready, None),
        };
        if !same_style {
            self.blocks.clear();
        }
        self.dark = dark;
        self.macros = wanted.macros;
        self.width = width;
        Some(self.render_blocks(mtm))
    }

    fn render_blocks(&mut self, mtm: MainThreadMarker) -> (Retained<NSAttributedString>, usize) {
        let renderer = Renderer {
            mtm,
            size: super::super::style::BODY,
            tone: Tone::Primary,
            dark: self.dark,
            macros: self.macros.clone(),
            width: self.width,
        };
        let old = std::mem::take(&mut self.blocks);
        let out = NSMutableAttributedString::new();
        let mut prefix = 0;
        let mut changed = false;
        let last_nonempty = self.laid.blocks.iter().rposition(|block| !block.paras.is_empty());
        for (i, block) in self.laid.blocks.iter().enumerate() {
            let last = Some(i) == last_nonempty;
            let cached = old
                .get(i)
                .filter(|cached| cached.key == block.key && cached.last == last);
            changed |= cached.is_none();
            let text = cached.map(|cached| cached.text.clone()).unwrap_or_else(|| {
                let paras: Vec<_> = block.paras.iter().map(|p| (0, p)).collect();
                render_paragraphs(&renderer, &paras, last)
            });
            if changed {
                out.appendAttributedString(&text);
            } else {
                prefix += text.length();
            }
            self.blocks.push(Block {
                key: block.key,
                last,
                text,
            });
        }
        (Retained::into_super(out), prefix)
    }
}

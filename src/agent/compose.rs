//! What the composer does with what the user types: `@` mentions with completion, the editor's selection offered
//! as context, the chips of attachments, and sending or queueing the message.

use ide_thread::{Attachment, Status, UserMessage};
use objc2::runtime::Sel;
use objc2::sel;
use objc2_app_kit::{NSColor, NSTextInputClient};
use objc2_foundation::NSRange;
use splat_i18n::{t, tf};

use super::mention::{self, Mention};
use super::suggest::Item;
use super::tab::AgentTab;
use super::{context, format, style, ui};
use crate::util::{main_soon, ns};

/// The byte offset in `text` of the UTF-16 offset `units`.
fn byte_offset(text: &str, units: usize) -> usize {
    let mut counted = 0;
    for (at, c) in text.char_indices() {
        if counted >= units {
            return at;
        }
        counted += c.len_utf16();
    }
    text.len()
}

/// The UTF-16 range of the bytes `range` of `text`.
fn utf16_range(text: &str, range: std::ops::Range<usize>) -> NSRange {
    let start = text[..range.start].encode_utf16().count();
    let length = text[range].encode_utf16().count();
    NSRange::new(start, length)
}

impl AgentTab {
    /// Return in the composer: takes a completion, opens the task overview for `/tasks`, else sends what was typed
    /// or queues it while a turn runs.
    pub fn submit(&self) {
        if self.accept_suggestion() {
            return;
        }
        let typed = self.composer.text().trim_end().to_string();
        if typed.trim() == "/tasks" {
            self.composer.clear();
            self.show_tasks(None);
            return;
        }
        let (text, resolved) = mention::resolve_files(&typed, &self.cwd);
        let attached = std::mem::take(&mut self.state.borrow_mut().attachments);
        let mut attachments: Vec<Attachment> = resolved
            .into_iter()
            .chain(attached)
            .map(|a| super::outside::classify(a, &self.cwd))
            .collect();
        if text.trim().is_empty() && attachments.is_empty() {
            return;
        }
        self.composer.clear();
        self.refresh_chips();
        let controller = self.controller();
        if let Some(controller) = &controller
            && mention::mentions(&text, &Mention::Terminal)
        {
            attachments.extend(context::terminal(controller));
        }
        let message = UserMessage {
            text,
            attachments,
            message_id: None,
        };
        match controller.filter(|_| mention::mentions(&message.text, &Mention::Browser)) {
            Some(controller) => {
                let key = self.key;
                context::browser(&controller, move |page| {
                    if let Some(tab) = super::get(key) {
                        let mut message = message;
                        message.attachments.extend(page);
                        tab.dispatch(message);
                    }
                });
            }
            None => self.dispatch(message),
        }
    }

    /// Sends `message` now, or queues it while a turn runs or the session is not there yet.
    fn dispatch(&self, message: UserMessage) {
        let busy = {
            let state = self.state.borrow();
            state.status != Status::Idle || state.sending || !state.source.is_live()
        };
        if busy {
            self.state.borrow_mut().queue.push(message);
            self.refresh_queue();
            return;
        }
        self.send_message(message);
    }

    pub fn composer_changed(&self) {
        self.composer.fit();
        self.refresh_composer();
        self.update_suggestions();
    }

    /// Commands of the composer's text view: arrows and Tab steer an open completion list, Esc closes it or stops
    /// the turn.
    pub fn composer_command(&self, command: Sel) -> bool {
        let open = self.suggestions.is_shown();
        if open && command == sel!(moveUp:) {
            self.suggestions.step(-1);
            return true;
        }
        if open && command == sel!(moveDown:) {
            self.suggestions.step(1);
            return true;
        }
        if open && command == sel!(insertTab:) {
            return self.accept_suggestion();
        }
        // A text view turns Esc into `complete:` unless the key bindings say `cancelOperation:`.
        if command == sel!(cancelOperation:) || command == sel!(complete:) {
            #[cfg(feature = "dictation")]
            if crate::dictation::cancel_in(&self.composer.text) {
                return true;
            }
            if open {
                self.suggestions.hide();
            } else {
                self.stop_turn();
            }
            return true;
        }
        false
    }

    /// Offers completions for the mention at the caret: the terminal, the browser and the project's files.
    fn update_suggestions(&self) {
        let text = self.composer.text();
        let caret = byte_offset(&text, self.composer.text.selectedRange().location);
        let Some((range, query)) = mention::typing(&text, caret) else {
            self.suggestions.hide();
            return;
        };
        let lower = query.to_lowercase();
        let mut special = Vec::new();
        for (word, symbol, detail) in [
            (mention::TERMINAL, "terminal", t("Ausgabe des Terminals")),
            (mention::BROWSER, "globe", t("Seite des Browser-Tabs")),
        ] {
            if word.starts_with(&lower) {
                special.push(Item {
                    label: format!("@{word}"),
                    detail: detail.to_string(),
                    insert: format!("@{word}"),
                    symbol,
                });
            }
        }
        self.suggestions.show(special.clone(), range.clone());
        let Some(controller) = self.controller() else { return };
        if query.is_empty() {
            return;
        }
        let asked = self.suggestions.ask();
        let key = self.key;
        controller.query_files(&query, super::suggest::SHOWN, move |hits| {
            let files: Vec<(String, String)> = hits
                .into_iter()
                .map(|hit| {
                    let name = hit.path.rsplit('/').next().unwrap_or(&hit.path).to_string();
                    (name, hit.path)
                })
                .collect();
            main_soon(move || {
                let Some(tab) = super::get(key) else { return };
                if !tab.suggestions.is_current(asked) {
                    return;
                }
                let mut items = special;
                items.extend(files.into_iter().map(|(name, path)| Item {
                    label: name,
                    insert: mention::written(&path),
                    detail: path,
                    symbol: "doc.text",
                }));
                tab.suggestions.show(items, range);
            });
        });
    }

    /// Replaces the typed mention with the chosen completion; whether one was open.
    pub(super) fn accept_suggestion(&self) -> bool {
        if !self.suggestions.is_shown() {
            return false;
        }
        let Some((item, range)) = self.suggestions.chosen() else {
            self.suggestions.hide();
            return false;
        };
        self.suggestions.hide();
        let text = self.composer.text();
        if range.end > text.len() {
            return true;
        }
        let replaced = utf16_range(&text, range);
        let insert = ns(&format!("{} ", item.insert));
        // SAFETY: The replacement is a string and the range lies inside the text, as `insertText` asks.
        unsafe { self.composer.text.insertText_replacementRange(&insert, replaced) };
        true
    }

    /// The completion clicked in the list.
    pub fn pick_suggestion(&self, index: usize) {
        self.suggestions.choose(index);
        self.accept_suggestion();
        self.focus_composer();
    }

    /// Offers the selection of the editor as context, as the composer gets the focus.
    pub(super) fn offer_selection(&self) {
        let offer = self.controller().and_then(|c| context::editor_selection(&c));
        let changed = {
            let mut state = self.state.borrow_mut();
            let already = offer.as_ref().is_some_and(|o| state.attachments.contains(o));
            let offer = offer.filter(|_| !already);
            let changed = state.offer != offer;
            state.offer = offer;
            changed
        };
        if changed {
            self.refresh_chips();
        }
    }

    /// Takes the offered selection as an attachment.
    pub fn use_selection(&self) {
        {
            let mut state = self.state.borrow_mut();
            if let Some(offer) = state.offer.take() {
                state.attachments.push(super::outside::classify(offer, &self.cwd));
            }
        }
        self.refresh_chips();
    }

    /// The attachments above the text, each with a button that drops it, and the offered selection.
    pub(super) fn refresh_chips(&self) {
        let chips = &self.composer.chips;
        for view in chips.arrangedSubviews().iter() {
            chips.removeArrangedSubview(&view);
            view.removeFromSuperview();
        }
        let (names, offer) = {
            let state = self.state.borrow();
            let names: Vec<((String, &'static str), bool)> = state
                .attachments
                .iter()
                .map(|a| (self.chip_name(a), matches!(a, Attachment::External { .. })))
                .collect();
            (names, state.offer.as_ref().map(|o| self.chip_name(o).0))
        };
        for (n, ((name, symbol), outside)) in names.iter().enumerate() {
            let remove = ui::symbol_button(
                self.mtm,
                "xmark",
                &tf("{} entfernen", &[name]),
                8.0,
                &self.target,
                sel!(ideAgentRemoveAttachment:),
                isize::try_from(n).unwrap_or(-1),
            );
            chips.addArrangedSubview(&self.chip(symbol, name, &remove, (false, *outside)));
        }
        if let Some(offer) = &offer {
            let take = ui::symbol_button(
                self.mtm,
                "plus.circle",
                t("Auswahl anhängen"),
                10.0,
                &self.target,
                sel!(ideAgentUseSelection:),
                0,
            );
            let label = tf("Auswahl: {}", &[offer]);
            chips.addArrangedSubview(&self.chip("text.cursor", &label, &take, (true, false)));
        }
        chips.addArrangedSubview(&ui::spacer(self.mtm));
        chips.setHidden(names.is_empty() && offer.is_none());
        self.refresh_composer();
    }

    fn chip_name(&self, attachment: &Attachment) -> (String, &'static str) {
        match attachment {
            Attachment::Image { name, .. } => (name.clone().unwrap_or_else(|| t("Bild").to_string()), "photo"),
            Attachment::File { path, lines } => {
                let mut name = format::short_path(&path.to_string_lossy(), &self.cwd);
                if let Some((a, b)) = lines {
                    name.push_str(&format!(":{a}–{b}"));
                }
                (name, "doc.text")
            }
            Attachment::Selection { path, lines, .. } => (
                format!(
                    "{}:{}–{}",
                    format::short_path(&path.to_string_lossy(), &self.cwd),
                    lines.0,
                    lines.1
                ),
                "text.cursor",
            ),
            Attachment::External { label, .. } => (label.clone(), "globe"),
        }
    }

    /// A chip: offered rather than attached, or with the mark of outside text.
    fn chip(
        &self,
        symbol: &str,
        name: &str,
        button: &objc2_app_kit::NSButton,
        (offered, outside): (bool, bool),
    ) -> objc2::rc::Retained<objc2_app_kit::NSBox> {
        let mtm = self.mtm;
        let tone = if offered {
            NSColor::secondaryLabelColor()
        } else {
            NSColor::labelColor()
        };
        let icon = ui::icon(mtm, symbol, name, 10.0, &NSColor::secondaryLabelColor());
        let label = ui::label(mtm, name, &style::small(), &tone);
        let line = if outside {
            ui::row(mtm, 3.0, &[&icon, &label, &super::outside::marker(mtm), button])
        } else {
            ui::row(mtm, 3.0, &[&icon, &label, button])
        };
        let border = if offered {
            NSColor::controlAccentColor().colorWithAlphaComponent(0.6)
        } else if outside {
            NSColor::systemOrangeColor().colorWithAlphaComponent(0.6)
        } else {
            NSColor::separatorColor()
        };
        ui::framed(mtm, &line, &border, None, 5.0, ui::insets(2.0, 5.0, 2.0, 4.0))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn offsets_count_utf16_units_as_text_views_do() {
        let text = "ä @src";
        assert_eq!(byte_offset(text, 2), 3);
        assert_eq!(utf16_range(text, 3..7), NSRange::new(2, 4));
    }
}

//! The model families whose fill-in-the-middle marks are known, for LM Studio, which takes the prompt as written.

/// A family of code models trained for fill-in-the-middle, with its own marks around prefix and suffix.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Family {
    /// Qwen2.5 Coder and Qwen3 Coder.
    QwenCoder,
    /// StarCoder and StarCoder2.
    StarCoder,
    /// DeepSeek Coder, also V2.
    DeepSeekCoder,
    /// Code Llama.
    CodeLlama,
    /// Mistral's Codestral.
    Codestral,
    /// Google's CodeGemma.
    CodeGemma,
}

impl Family {
    /// Every supported family, in the order the settings name them.
    #[cfg(test)]
    pub(crate) const ALL: [Family; 6] = [
        Family::QwenCoder,
        Family::StarCoder,
        Family::DeepSeekCoder,
        Family::CodeLlama,
        Family::Codestral,
        Family::CodeGemma,
    ];

    /// The family of the model LM Studio names `model`, such as `qwen2.5-coder-7b-instruct`, if it is supported.
    ///
    /// ```
    /// use ide_predict::fim::Family;
    ///
    /// assert_eq!(Family::of("lmstudio-community/Qwen2.5-Coder-7B-GGUF"), Some(Family::QwenCoder));
    /// assert_eq!(Family::of("llama-3.2-3b-instruct"), None);
    /// ```
    pub fn of(model: &str) -> Option<Family> {
        let id = model.to_ascii_lowercase().replace(['_', ' ', '.'], "-");
        let has = |part: &str| id.contains(part);
        Some(if has("codegemma") {
            Family::CodeGemma
        } else if has("codestral") {
            Family::Codestral
        } else if has("starcoder") {
            Family::StarCoder
        } else if has("deepseek") && has("coder") {
            Family::DeepSeekCoder
        } else if has("codellama") || has("code-llama") {
            Family::CodeLlama
        } else if has("qwen") && has("coder") {
            Family::QwenCoder
        } else {
            return None;
        })
    }

    /// The prompt that asks for the text between `prefix` and `suffix`.
    pub(crate) fn prompt(self, prefix: &str, suffix: &str) -> String {
        match self {
            Family::QwenCoder | Family::CodeGemma => {
                format!("<|fim_prefix|>{prefix}<|fim_suffix|>{suffix}<|fim_middle|>")
            }
            Family::StarCoder => format!("<fim_prefix>{prefix}<fim_suffix>{suffix}<fim_middle>"),
            Family::DeepSeekCoder => format!("<｜fim▁begin｜>{prefix}<｜fim▁hole｜>{suffix}<｜fim▁end｜>"),
            Family::CodeLlama => format!("<PRE> {prefix} <SUF>{suffix} <MID>"),
            Family::Codestral => format!("[SUFFIX]{suffix}[PREFIX]{prefix}"),
        }
    }

    /// Where the model's answer ends: its end-of-text marks and its own marks, which it writes when it starts over.
    pub(crate) fn stops(self) -> &'static [&'static str] {
        match self {
            Family::QwenCoder => &[
                "<|endoftext|>",
                "<|fim_prefix|>",
                "<|fim_suffix|>",
                "<|fim_middle|>",
                "<|fim_pad|>",
                "<|repo_name|>",
                "<|file_sep|>",
                "<|im_end|>",
            ],
            Family::StarCoder => &[
                "<|endoftext|>",
                "<fim_prefix>",
                "<fim_suffix>",
                "<fim_middle>",
                "<file_sep>",
            ],
            Family::DeepSeekCoder => &[
                "<｜end▁of▁sentence｜>",
                "<｜fim▁begin｜>",
                "<｜fim▁hole｜>",
                "<｜fim▁end｜>",
                "<|EOT|>",
            ],
            Family::CodeLlama => &["<EOT>", "<PRE>", "<SUF>", "<MID>"],
            Family::Codestral => &["</s>", "[PREFIX]", "[SUFFIX]", "[INST]"],
            Family::CodeGemma => &[
                "<|file_separator|>",
                "<|fim_prefix|>",
                "<|fim_suffix|>",
                "<|fim_middle|>",
                "<eos>",
            ],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn model_ids_as_lm_studio_and_hugging_face_write_them_find_their_family() {
        let cases = [
            ("qwen2.5-coder-1.5b", Some(Family::QwenCoder)),
            ("Qwen/Qwen3-Coder-30B-A3B-Instruct-GGUF", Some(Family::QwenCoder)),
            ("second-state/StarCoder2-7B-GGUF", Some(Family::StarCoder)),
            ("deepseek-coder-6.7b-base", Some(Family::DeepSeekCoder)),
            ("DeepSeek-Coder-V2-Lite-Base", Some(Family::DeepSeekCoder)),
            ("TheBloke/CodeLlama-7B-GGUF", Some(Family::CodeLlama)),
            ("codestral-22b-v0.1", Some(Family::Codestral)),
            ("google/codegemma-2b", Some(Family::CodeGemma)),
            ("qwen2.5-7b-instruct", None),
            ("deepseek-r1-distill", None),
            ("gemma-3-4b", None),
        ];
        for (model, family) in cases {
            assert_eq!(Family::of(model), family, "{model}");
        }
    }

    #[test]
    fn each_family_writes_prefix_and_suffix_in_its_own_order_and_stops_at_its_marks() {
        assert_eq!(
            Family::QwenCoder.prompt("a", "b"),
            "<|fim_prefix|>a<|fim_suffix|>b<|fim_middle|>"
        );
        assert_eq!(Family::Codestral.prompt("a", "b"), "[SUFFIX]b[PREFIX]a");
        assert_eq!(Family::CodeLlama.prompt("a", "b"), "<PRE> a <SUF>b <MID>");
        for family in Family::ALL {
            assert!(!family.stops().is_empty());
            let prompt = family.prompt("PREFIX_TEXT", "SUFFIX_TEXT");
            assert!(prompt.contains("PREFIX_TEXT") && prompt.contains("SUFFIX_TEXT"));
        }
    }
}

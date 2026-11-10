//! What tokens cost: Anthropic's list prices, the prices OpenRouter lists with its models, nothing for models on the
//! user's own hardware, and prices the user enters for the rest.
//!
//! OpenAI's and Google's prices are not built in: the public figures disagree across sources and change with each
//! model generation, and a wrong price shown as a cost is worse than none. The user can enter them per model.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::chat::TurnUsage;
use crate::provider::ProviderKind;

/// Dollars per million tokens.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Price {
    /// Input tokens.
    pub input: f64,
    /// Output tokens, reasoning included.
    pub output: f64,
    /// Input tokens read from the prompt cache.
    pub cache_read: f64,
    /// Input tokens written to the prompt cache.
    pub cache_write: f64,
    /// Higher prices for prompts above a size, where the model has them.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub long: Option<LongPrompt>,
}

/// The prices of prompts above a size.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct LongPrompt {
    /// Prompts above this many tokens cost these prices.
    pub above: u64,
    /// Input tokens.
    pub input: f64,
    /// Output tokens.
    pub output: f64,
}

impl Price {
    /// A price whose cache reads cost a tenth of input and cache writes a quarter more, as Anthropic's do for the
    /// five-minute cache.
    pub fn new(input: f64, output: f64) -> Price {
        Price {
            input,
            output,
            cache_read: input * 0.1,
            cache_write: input * 1.25,
            long: None,
        }
    }

    /// Nothing, for models on the user's own hardware.
    pub fn free() -> Price {
        Price {
            input: 0.0,
            output: 0.0,
            cache_read: 0.0,
            cache_write: 0.0,
            long: None,
        }
    }

    fn cache_read_at(mut self, price: f64) -> Price {
        self.cache_read = price;
        self
    }

    fn long_above(mut self, above: u64, input: f64, output: f64) -> Price {
        self.long = Some(LongPrompt { above, input, output });
        self
    }

    /// What `usage` costs, in US dollars.
    #[expect(clippy::cast_precision_loss, reason = "token counts far below 2^52")]
    pub fn cost(&self, usage: &TurnUsage) -> f64 {
        let (input, output, read, write) = match self.long {
            Some(long) if usage.prompt() > long.above => (
                long.input,
                long.output,
                long.input * self.cache_read / self.input.max(f64::MIN_POSITIVE),
                long.input * self.cache_write / self.input.max(f64::MIN_POSITIVE),
            ),
            _ => (self.input, self.output, self.cache_read, self.cache_write),
        };
        (usage.input as f64 * input
            + usage.output as f64 * output
            + usage.cache_read as f64 * read
            + usage.cache_write as f64 * write)
            / 1_000_000.0
    }
}

/// Anthropic's list prices, as published on 2026-10-06; a dated model id takes the price of its family.
pub fn anthropic(model: &str) -> Option<Price> {
    let table: [(&str, Price); 13] = [
        ("claude-fable-5-1", Price::new(10.0, 50.0).cache_read_at(0.25)),
        ("claude-mythos-5-1", Price::new(10.0, 50.0).cache_read_at(0.25)),
        ("claude-fable-5", Price::new(10.0, 50.0)),
        ("claude-opus-5-5", Price::new(4.0, 20.0).cache_read_at(0.20)),
        ("claude-opus-5", Price::new(5.0, 25.0)),
        ("claude-opus-4-8", Price::new(5.0, 25.0)),
        ("claude-opus-4-7", Price::new(5.0, 25.0)),
        ("claude-opus-4-6", Price::new(5.0, 25.0)),
        ("claude-sonnet-5-5", Price::new(2.0, 10.0).cache_read_at(0.20)),
        ("claude-sonnet-5", Price::new(2.0, 10.0)),
        ("claude-sonnet-4-6", Price::new(3.0, 15.0)),
        (
            "claude-haiku-5-5",
            Price::new(0.10, 0.50).long_above(100_000, 0.50, 2.50),
        ),
        ("claude-haiku-4-5", Price::new(1.0, 5.0)),
    ];
    table
        .iter()
        .filter(|(id, _)| model == *id || model.strip_prefix(id).is_some_and(|rest| rest.starts_with('-')))
        .max_by_key(|(id, _)| id.len())
        .map(|(_, price)| *price)
}

/// The prices the user entered, by model id, which win over every other source.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct PriceTable {
    /// The user's prices.
    #[serde(default)]
    pub models: HashMap<String, Price>,
}

impl PriceTable {
    /// The price of `model` at a provider of `kind`: the user's, else the one its list gives, else Anthropic's list
    /// price at Anthropic, else nothing for a model on the user's hardware.
    pub fn price(&self, kind: ProviderKind, model: &str, listed: Option<Price>) -> Option<Price> {
        if let Some(price) = self.models.get(model) {
            return Some(*price);
        }
        if listed.is_some() {
            return listed;
        }
        match kind {
            ProviderKind::Anthropic => anthropic(model),
            kind if kind.is_local() => Some(Price::free()),
            _ => None,
        }
    }
}

/// What a request cost: what the provider said, else what the price makes of its tokens.
pub fn cost(usage: &TurnUsage, price: Option<&Price>) -> Option<f64> {
    usage.cost_usd.or_else(|| price.map(|p| p.cost(usage)))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn usage(input: u64, output: u64, read: u64, write: u64) -> TurnUsage {
        TurnUsage {
            input,
            output,
            cache_read: read,
            cache_write: write,
            cost_usd: None,
        }
    }

    #[test]
    fn a_dated_id_takes_its_familys_price_and_the_longest_name_wins() {
        assert_eq!(anthropic("claude-haiku-4-5-20251001").unwrap().input, 1.0);
        assert_eq!(anthropic("claude-opus-5-5").unwrap().input, 4.0);
        assert_eq!(anthropic("claude-opus-5").unwrap().input, 5.0);
        assert_eq!(anthropic("claude-opus-5-20260101").unwrap().input, 5.0);
        assert_eq!(anthropic("claude-opus-50"), None);
        assert_eq!(anthropic("gpt-x"), None);
    }

    #[test]
    fn cost_adds_input_output_and_cache_at_their_prices() {
        let price = anthropic("claude-opus-5-5").unwrap();
        let cost = price.cost(&usage(1_000_000, 100_000, 2_000_000, 1_000_000));
        assert!((cost - (4.0 + 2.0 + 0.4 + 5.0)).abs() < 1e-9, "{cost}");
    }

    #[test]
    fn a_long_prompt_costs_the_higher_prices() {
        let price = anthropic("claude-haiku-5-5").unwrap();
        let short = price.cost(&usage(100_000, 1_000_000, 0, 0));
        let long = price.cost(&usage(200_000, 1_000_000, 0, 0));
        assert!((short - (0.01 + 0.5)).abs() < 1e-9, "{short}");
        assert!((long - (0.1 + 2.5)).abs() < 1e-9, "{long}");
    }

    #[test]
    fn the_users_price_wins_and_local_models_cost_nothing() {
        let mut table = PriceTable::default();
        assert_eq!(table.price(ProviderKind::OpenAi, "gpt-x", None), None);
        assert_eq!(table.price(ProviderKind::Ollama, "qwen", None), Some(Price::free()));
        table.models.insert("gpt-x".into(), Price::new(1.0, 2.0));
        assert_eq!(table.price(ProviderKind::OpenAi, "gpt-x", None).unwrap().output, 2.0);
        let reported = TurnUsage {
            cost_usd: Some(0.5),
            ..usage(1, 1, 0, 0)
        };
        assert_eq!(cost(&reported, Some(&Price::new(100.0, 100.0))), Some(0.5));
    }
}

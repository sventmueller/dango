//! The models an endpoint offers, as far as its API lists them.

use crate::prices::Price;

/// A model an endpoint offers.
#[derive(Debug, Clone, PartialEq)]
pub struct ModelInfo {
    /// The id requests name.
    pub id: String,
    /// The name the provider shows, where it gives one.
    pub name: Option<String>,
    /// How many tokens the context window holds.
    pub context: Option<u64>,
    /// How many tokens one answer may take.
    pub max_output: Option<u64>,
    /// What it costs, where the list or the built-in prices say.
    pub price: Option<Price>,
}

impl ModelInfo {
    /// A model of which only the id is known.
    pub fn id(id: &str) -> ModelInfo {
        ModelInfo {
            id: id.to_string(),
            name: None,
            context: None,
            max_output: None,
            price: None,
        }
    }
}

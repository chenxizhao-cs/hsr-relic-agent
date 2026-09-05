//! Demo v0.1: scanner input is converted at the boundary; decisions use our own models.
pub mod decision;
pub mod evaluator;
pub mod import;
pub mod model;

pub use decision::DecisionEngine;
pub use evaluator::{Evaluation, Evaluator, MockEvaluator};
pub use import::load_scanner_v4;
pub use model::*;

pub const DEMO_ACCOUNT: &str = include_str!("../../fixtures/scanner-v4-demo.json");

#[derive(Debug, Clone, PartialEq)]
pub struct Error(pub String);

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for Error {}

pub type Result<T> = std::result::Result<T, Error>;

#[cfg(test)]
mod tests;

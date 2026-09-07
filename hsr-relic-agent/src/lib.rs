//! Demo v0.2: scanner and evaluator protocols stay outside decision logic.
pub mod decision;
pub mod evaluator;
pub mod fribbels;
pub mod import;
pub mod model;
pub mod recommendations;

pub use decision::{DecisionEngine, RelicOperationError, RelicOperationResult, RelicSelection};
pub use evaluator::{Evaluation, Evaluator, MockEvaluator};
pub use fribbels::{
    BuildMetrics, BuildPanel, CombatConditions, DamageModel, EvaluationDetails, EvaluationProgress,
    FRIBBELS_COMMIT, FribbelsConfig, FribbelsEvaluator, ReferenceBuild, RelicMetrics,
};
pub use import::{
    AccountImportError, AccountImportErrorCode, AccountImportSummary, ImportedAccount,
    load_reliquary_v4, load_scanner_v4,
};
pub use model::*;
pub use recommendations::{
    CharacterRelicDatabase, CharacterRelicProfile, RecommendationMatch, RecommendationSource,
    RecommendedMainStats, load_character_relic_database,
};

pub const DEMO_ACCOUNT: &str = include_str!("../../fixtures/scanner-v4-demo.json");
pub const RELIQUARY_DEMO_ACCOUNT: &str = include_str!("../../fixtures/reliquary-v4-demo.json");

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

use crate::ModelConfig;
use serde::{Deserialize, Serialize};
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct TokenUsage {
    pub input_tokens: u64,
    pub output_tokens: u64,
}

impl TokenUsage {
    pub fn total(self) -> u64 {
        self.input_tokens.saturating_add(self.output_tokens)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct UsageRecord {
    pub sequence: u64,
    pub response_id: String,
    pub model: String,
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub total_tokens: u64,
    pub cost: f64,
    pub recorded_at_unix_ms: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct UsageSummary {
    pub calls: u64,
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub total_tokens: u64,
    pub total_cost: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct UsageLedger {
    records: Vec<UsageRecord>,
}

impl UsageLedger {
    pub fn record(
        &mut self,
        config: &ModelConfig,
        response_id: String,
        usage: TokenUsage,
    ) -> UsageRecord {
        let cost = usage.input_tokens as f64 / 1_000_000.0 * config.input_price_per_million()
            + usage.output_tokens as f64 / 1_000_000.0 * config.output_price_per_million();
        let record = UsageRecord {
            sequence: self.records.len() as u64 + 1,
            response_id,
            model: config.model().into(),
            input_tokens: usage.input_tokens,
            output_tokens: usage.output_tokens,
            total_tokens: usage.total(),
            cost,
            recorded_at_unix_ms: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis() as u64,
        };
        self.records.push(record.clone());
        record
    }

    pub fn summary(&self) -> UsageSummary {
        self.records
            .iter()
            .fold(UsageSummary::default(), |mut total, call| {
                total.calls += 1;
                total.input_tokens = total.input_tokens.saturating_add(call.input_tokens);
                total.output_tokens = total.output_tokens.saturating_add(call.output_tokens);
                total.total_tokens = total.total_tokens.saturating_add(call.total_tokens);
                total.total_cost += call.cost;
                total
            })
    }

    pub fn records(&self) -> &[UsageRecord] {
        &self.records
    }
    pub fn can_start_request(&self, config: &ModelConfig) -> bool {
        self.summary().total_tokens < config.token_budget()
    }

    pub fn validate(&self) -> Result<(), String> {
        for (index, record) in self.records.iter().enumerate() {
            if record.sequence != index as u64 + 1
                || record.total_tokens != record.input_tokens.saturating_add(record.output_tokens)
                || !record.cost.is_finite()
                || record.cost < 0.0
                || record.response_id.is_empty()
                || record.model.is_empty()
            {
                return Err("Usage 历史包含无效记录".into());
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ModelConfigPatch, ReasoningMode};

    #[test]
    fn cost_uses_configured_prices_and_real_usage() {
        let mut config = ModelConfig::default();
        config
            .apply(ModelConfigPatch {
                endpoint: "http://localhost:1/v1".into(),
                api_key: None,
                clear_api_key: false,
                model: "m".into(),
                context_length: 1024,
                reasoning_mode: ReasoningMode::Disabled,
                input_price_per_million: 2.0,
                output_price_per_million: 8.0,
                token_budget: 1000,
            })
            .unwrap();
        let mut ledger = UsageLedger::default();
        let call = ledger.record(
            &config,
            "r1".into(),
            TokenUsage {
                input_tokens: 100,
                output_tokens: 25,
            },
        );
        assert!((call.cost - 0.0004).abs() < 1e-12);
        assert_eq!(ledger.summary().total_tokens, 125);
    }
}

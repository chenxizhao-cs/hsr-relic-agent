use serde::{Deserialize, Serialize};
use std::{env, fmt};
use url::Url;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum ReasoningMode {
    #[default]
    Disabled,
    Minimal,
    Low,
    Medium,
    High,
}

impl ReasoningMode {
    pub(crate) fn as_api_value(self) -> Option<&'static str> {
        match self {
            Self::Disabled => None,
            Self::Minimal => Some("minimal"),
            Self::Low => Some("low"),
            Self::Medium => Some("medium"),
            Self::High => Some("high"),
        }
    }
}

#[derive(Clone, PartialEq, Eq, Default)]
struct Secret(String);

impl fmt::Debug for Secret {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(if self.0.is_empty() {
            "Secret(empty)"
        } else {
            "Secret([REDACTED])"
        })
    }
}

#[derive(Clone, PartialEq)]
pub struct ModelConfig {
    endpoint: String,
    api_key: Secret,
    model: String,
    context_length: u32,
    reasoning_mode: ReasoningMode,
    input_price_per_million: f64,
    output_price_per_million: f64,
    token_budget: u64,
}

impl fmt::Debug for ModelConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ModelConfig")
            .field("endpoint", &self.endpoint)
            .field("api_key", &self.api_key)
            .field("model", &self.model)
            .field("context_length", &self.context_length)
            .field("reasoning_mode", &self.reasoning_mode)
            .field("input_price_per_million", &self.input_price_per_million)
            .field("output_price_per_million", &self.output_price_per_million)
            .field("token_budget", &self.token_budget)
            .finish()
    }
}

impl Default for ModelConfig {
    fn default() -> Self {
        Self {
            endpoint: "https://api.openai.com/v1".into(),
            api_key: Secret::default(),
            model: "gpt-4o-mini".into(),
            context_length: 4096,
            reasoning_mode: ReasoningMode::Disabled,
            input_price_per_million: 0.0,
            output_price_per_million: 0.0,
            token_budget: 20_000,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ModelConfigView {
    pub endpoint: String,
    pub api_key_configured: bool,
    pub model: String,
    pub context_length: u32,
    pub reasoning_mode: ReasoningMode,
    pub input_price_per_million: f64,
    pub output_price_per_million: f64,
    pub token_budget: u64,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct ModelConfigPatch {
    pub endpoint: String,
    #[serde(default)]
    pub api_key: Option<String>,
    #[serde(default)]
    pub clear_api_key: bool,
    pub model: String,
    pub context_length: u32,
    pub reasoning_mode: ReasoningMode,
    pub input_price_per_million: f64,
    pub output_price_per_million: f64,
    pub token_budget: u64,
}

impl ModelConfig {
    pub fn from_env() -> Result<Self, String> {
        let mut value = Self::default();
        if let Ok(v) = env::var("HSR_LLM_API_ENDPOINT") {
            value.endpoint = v;
        }
        if let Ok(v) = env::var("HSR_LLM_API_KEY") {
            value.api_key = Secret(v);
        }
        if let Ok(v) = env::var("HSR_LLM_MODEL") {
            value.model = v;
        }
        if let Ok(v) = env::var("HSR_LLM_CONTEXT_LENGTH") {
            value.context_length = parse_env("HSR_LLM_CONTEXT_LENGTH", &v)?;
        }
        if let Ok(v) = env::var("HSR_LLM_REASONING_MODE") {
            value.reasoning_mode = match v.as_str() {
                "disabled" => ReasoningMode::Disabled,
                "minimal" => ReasoningMode::Minimal,
                "low" => ReasoningMode::Low,
                "medium" => ReasoningMode::Medium,
                "high" => ReasoningMode::High,
                _ => {
                    return Err(
                        "HSR_LLM_REASONING_MODE 必须是 disabled/minimal/low/medium/high".into(),
                    );
                }
            };
        }
        if let Ok(v) = env::var("HSR_LLM_INPUT_PRICE_PER_MILLION") {
            value.input_price_per_million = parse_env("HSR_LLM_INPUT_PRICE_PER_MILLION", &v)?;
        }
        if let Ok(v) = env::var("HSR_LLM_OUTPUT_PRICE_PER_MILLION") {
            value.output_price_per_million = parse_env("HSR_LLM_OUTPUT_PRICE_PER_MILLION", &v)?;
        }
        if let Ok(v) = env::var("HSR_LLM_TOKEN_BUDGET") {
            value.token_budget = parse_env("HSR_LLM_TOKEN_BUDGET", &v)?;
        }
        value.validate()?;
        Ok(value)
    }

    pub fn apply(&mut self, patch: ModelConfigPatch) -> Result<(), String> {
        let api_key = if patch.clear_api_key {
            Secret::default()
        } else {
            patch
                .api_key
                .filter(|v| !v.is_empty())
                .map(Secret)
                .unwrap_or_else(|| self.api_key.clone())
        };
        let next = Self {
            endpoint: patch.endpoint,
            api_key,
            model: patch.model,
            context_length: patch.context_length,
            reasoning_mode: patch.reasoning_mode,
            input_price_per_million: patch.input_price_per_million,
            output_price_per_million: patch.output_price_per_million,
            token_budget: patch.token_budget,
        };
        next.validate()?;
        *self = next;
        Ok(())
    }

    pub fn view(&self) -> ModelConfigView {
        ModelConfigView {
            endpoint: self.endpoint.clone(),
            api_key_configured: !self.api_key.0.is_empty(),
            model: self.model.clone(),
            context_length: self.context_length,
            reasoning_mode: self.reasoning_mode,
            input_price_per_million: self.input_price_per_million,
            output_price_per_million: self.output_price_per_million,
            token_budget: self.token_budget,
        }
    }

    fn validate(&self) -> Result<(), String> {
        let endpoint = Url::parse(self.endpoint.trim()).map_err(|_| "API Endpoint 不是有效 URL")?;
        if !matches!(endpoint.scheme(), "http" | "https") {
            return Err("API Endpoint 只支持 http 或 https".into());
        }
        if !endpoint.username().is_empty() || endpoint.password().is_some() {
            return Err("API Endpoint 不能在 URL 中包含用户名或密码".into());
        }
        if self.model.trim().is_empty() {
            return Err("Model 不能为空".into());
        }
        if !(256..=2_000_000).contains(&self.context_length) {
            return Err("Context Length 必须在 256 到 2000000 之间".into());
        }
        if self.token_budget == 0 {
            return Err("Token Budget 必须大于 0".into());
        }
        for (label, price) in [
            ("Input Token Price", self.input_price_per_million),
            ("Output Token Price", self.output_price_per_million),
        ] {
            if !price.is_finite() || price < 0.0 {
                return Err(format!("{label} 必须是非负有限数值"));
            }
        }
        Ok(())
    }

    pub fn endpoint(&self) -> &str {
        &self.endpoint
    }
    pub fn api_key(&self) -> &str {
        &self.api_key.0
    }
    pub fn model(&self) -> &str {
        &self.model
    }
    pub fn context_length(&self) -> u32 {
        self.context_length
    }
    pub fn reasoning_mode(&self) -> ReasoningMode {
        self.reasoning_mode
    }
    pub fn input_price_per_million(&self) -> f64 {
        self.input_price_per_million
    }
    pub fn output_price_per_million(&self) -> f64 {
        self.output_price_per_million
    }
    pub fn token_budget(&self) -> u64 {
        self.token_budget
    }
}

fn parse_env<T: std::str::FromStr>(name: &str, value: &str) -> Result<T, String> {
    value.parse().map_err(|_| format!("{name} 格式无效"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn debug_and_public_view_never_expose_key() {
        let mut config = ModelConfig::default();
        config
            .apply(ModelConfigPatch {
                endpoint: "http://localhost:1234/v1".into(),
                api_key: Some("very-secret".into()),
                clear_api_key: false,
                model: "local".into(),
                context_length: 2048,
                reasoning_mode: ReasoningMode::Low,
                input_price_per_million: 1.0,
                output_price_per_million: 2.0,
                token_budget: 100,
            })
            .unwrap();
        assert!(!format!("{config:?}").contains("very-secret"));
        let public = serde_json::to_string(&config.view()).unwrap();
        assert!(!public.contains("very-secret"));
        assert!(config.view().api_key_configured);
    }

    #[test]
    fn endpoint_rejects_embedded_credentials() {
        let mut config = ModelConfig::default();
        let error = config
            .apply(ModelConfigPatch {
                endpoint: "https://user:password@example.com/v1".into(),
                api_key: None,
                clear_api_key: false,
                model: "model".into(),
                context_length: 2048,
                reasoning_mode: ReasoningMode::Disabled,
                input_price_per_million: 0.0,
                output_price_per_million: 0.0,
                token_budget: 100,
            })
            .unwrap_err();
        assert!(error.contains("用户名或密码"));
    }
}

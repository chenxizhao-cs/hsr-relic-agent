//! Minimal v0.3 agent runtime. Model protocols, tools, and accounting stay
//! outside the deterministic relic decision engine.
mod config;
mod provider;
mod runtime;
mod tools;
mod usage;

pub use config::{ModelConfig, ModelConfigPatch, ModelConfigView, ReasoningMode};
pub use provider::{
    ChatMessage, ModelProvider, ModelResponse, ModelToolCall, OpenAiCompatibleProvider,
    ProviderError, ProviderRequest, ToolDefinition,
};
pub use runtime::{AgentEvent, AgentRun, AgentRunStatus, AgentRuntime, RuntimeError};
pub use tools::{CoreTools, TOOL_NAMES, tool_definitions};
pub use usage::{TokenUsage, UsageLedger, UsageRecord, UsageSummary};

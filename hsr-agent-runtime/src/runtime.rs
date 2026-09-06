use crate::{
    ChatMessage, CoreTools, ModelConfig, ModelProvider, ProviderError, ProviderRequest,
    ToolDefinition, UsageLedger, UsageRecord, tool_definitions,
};
use hsr_relic_agent::{DecisionEngine, Evaluator};
use serde::Serialize;
use serde_json::Value;
use std::{
    fmt,
    sync::atomic::{AtomicBool, Ordering},
};

const SYSTEM_PROMPT: &str = r#"你是《崩坏：星穹铁道》单目标角色遗器强化助手。
用户已经知道要培养哪个角色。你负责理解目标、调用工具、解释确定性结果。
必须通过工具读取账号状态和推荐；不要自行计算、编造或覆盖遗器评分、候选排序、Build 数值、Continue/Hold/Stop。
如果用户要求推荐：先确认或设置目标角色，再调用 get_next_relic_recommendation。材料紧张只能作为解释偏好，不能改变 Rust 规则或虚构资源成本。
最终用简洁中文回答，指出推荐遗器 ID、部位、当前分、平均满级潜力和 Rust 返回的推荐原因；说明最终排序来自 Rust Decision Engine，数值来自 Evaluator。"#;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentRunStatus {
    Completed,
    BudgetReached,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum AgentEvent {
    RunStarted { user_input: String },
    ModelRequestStarted { call_index: u32 },
    UsageRecorded { call: UsageRecord },
    ToolRequested { name: String, arguments: Value },
    ToolFinished { name: String, result: Value },
    AssistantReply { content: String },
    BudgetBlocked { used_tokens: u64, token_budget: u64 },
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct AgentRun {
    pub status: AgentRunStatus,
    pub user_input: String,
    pub reply: String,
    pub events: Vec<AgentEvent>,
}

#[derive(Debug)]
pub enum RuntimeError {
    InvalidInput(String),
    ModelNotConfigured,
    BudgetReached { used_tokens: u64, token_budget: u64 },
    Cancelled,
    Provider(ProviderError),
    ToolLoopLimit,
    MissingToolCall,
    MissingReply,
}

impl fmt::Display for RuntimeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidInput(m) => f.write_str(m),
            Self::ModelNotConfigured => {
                f.write_str("请先配置 API Endpoint、API Key（本地服务可留空）和 Model")
            }
            Self::BudgetReached {
                used_tokens,
                token_budget,
            } => write!(
                f,
                "Token Budget 已达到：已用 {used_tokens} / 上限 {token_budget}，未发起新的模型请求"
            ),
            Self::Cancelled => f.write_str("Agent 运行已取消，账号保持本次运行前的状态"),
            Self::Provider(error) => error.fmt(f),
            Self::ToolLoopLimit => f.write_str("Agent 工具调用轮数超过上限，已停止"),
            Self::MissingToolCall => f.write_str("模型没有调用任何 Agent Tool，本次结果不接受"),
            Self::MissingReply => f.write_str("模型既没有回复文本，也没有调用工具"),
        }
    }
}

impl std::error::Error for RuntimeError {}
impl From<ProviderError> for RuntimeError {
    fn from(value: ProviderError) -> Self {
        Self::Provider(value)
    }
}

pub struct AgentRuntime<P> {
    provider: P,
    tools: Vec<ToolDefinition>,
    max_model_calls: u32,
}

impl<P: ModelProvider> AgentRuntime<P> {
    pub fn new(provider: P) -> Self {
        Self {
            provider,
            tools: tool_definitions(),
            max_model_calls: 6,
        }
    }

    pub fn run<E: Evaluator + Clone>(
        &self,
        input: &str,
        config: &ModelConfig,
        usage: &mut UsageLedger,
        engine: &mut DecisionEngine<E>,
        cancelled: &AtomicBool,
    ) -> Result<AgentRun, RuntimeError> {
        let input = input.trim();
        if input.is_empty() || input.chars().count() > 4000 {
            return Err(RuntimeError::InvalidInput(
                "自然语言输入必须为 1 到 4000 个字符".into(),
            ));
        }
        if config.model().trim().is_empty() || config.endpoint().trim().is_empty() {
            return Err(RuntimeError::ModelNotConfigured);
        }
        check_budget(config, usage)?;
        let mut staged = engine.clone();
        let mut messages = vec![
            ChatMessage::System(SYSTEM_PROMPT.into()),
            ChatMessage::User(input.into()),
        ];
        let mut events = vec![AgentEvent::RunStarted {
            user_input: input.into(),
        }];
        let mut used_tool = false;

        for call_index in 1..=self.max_model_calls {
            if cancelled.load(Ordering::SeqCst) {
                return Err(RuntimeError::Cancelled);
            }
            check_budget(config, usage)?;
            events.push(AgentEvent::ModelRequestStarted { call_index });
            let response = self.provider.complete(
                config,
                &ProviderRequest {
                    messages: messages.clone(),
                    tools: self.tools.clone(),
                    require_tool: !used_tool,
                },
            )?;
            let record = usage.record(config, response.response_id, response.usage);
            events.push(AgentEvent::UsageRecorded { call: record });
            if cancelled.load(Ordering::SeqCst) {
                return Err(RuntimeError::Cancelled);
            }

            if response.tool_calls.is_empty() {
                if !used_tool {
                    return Err(RuntimeError::MissingToolCall);
                }
                let reply = response.content.ok_or(RuntimeError::MissingReply)?;
                events.push(AgentEvent::AssistantReply {
                    content: reply.clone(),
                });
                *engine = staged;
                return Ok(AgentRun {
                    status: AgentRunStatus::Completed,
                    user_input: input.into(),
                    reply,
                    events,
                });
            }
            used_tool = true;
            messages.push(ChatMessage::Assistant {
                content: response.content,
                tool_calls: response.tool_calls.clone(),
            });
            let mut tools = CoreTools::new(&mut staged);
            for call in response.tool_calls {
                events.push(AgentEvent::ToolRequested {
                    name: call.name.clone(),
                    arguments: call.arguments.clone(),
                });
                let result = match tools.execute(&call.name, &call.arguments) {
                    Ok(value) => serde_json::json!({"ok":true,"result":value}),
                    Err(message) => serde_json::json!({"ok":false,"error":message}),
                };
                events.push(AgentEvent::ToolFinished {
                    name: call.name.clone(),
                    result: result.clone(),
                });
                messages.push(ChatMessage::Tool {
                    call_id: call.id,
                    name: call.name,
                    content: result.to_string(),
                });
            }
            if !usage.can_start_request(config) {
                let used_tokens = usage.summary().total_tokens;
                events.push(AgentEvent::BudgetBlocked {
                    used_tokens,
                    token_budget: config.token_budget(),
                });
                let reply = format!(
                    "模型已完成工具调用，但 Token Budget 已达到（{used_tokens} / {}），因此没有继续发起模型请求。请查看工具轨迹中的确定性结果。",
                    config.token_budget()
                );
                events.push(AgentEvent::AssistantReply {
                    content: reply.clone(),
                });
                *engine = staged;
                return Ok(AgentRun {
                    status: AgentRunStatus::BudgetReached,
                    user_input: input.into(),
                    reply,
                    events,
                });
            }
        }
        Err(RuntimeError::ToolLoopLimit)
    }
}

fn check_budget(config: &ModelConfig, usage: &UsageLedger) -> Result<(), RuntimeError> {
    let used_tokens = usage.summary().total_tokens;
    if used_tokens >= config.token_budget() {
        Err(RuntimeError::BudgetReached {
            used_tokens,
            token_budget: config.token_budget(),
        })
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ModelResponse, ModelToolCall, TokenUsage};
    use hsr_relic_agent::{DEMO_ACCOUNT, MockEvaluator, load_scanner_v4};
    use serde_json::json;
    use std::{collections::VecDeque, sync::Mutex};

    struct ScriptedProvider {
        responses: Mutex<VecDeque<ModelResponse>>,
        calls: Mutex<u32>,
    }
    impl ModelProvider for ScriptedProvider {
        fn complete(
            &self,
            _: &ModelConfig,
            _: &ProviderRequest,
        ) -> Result<ModelResponse, ProviderError> {
            *self.calls.lock().unwrap() += 1;
            self.responses
                .lock()
                .unwrap()
                .pop_front()
                .ok_or_else(|| ProviderError::InvalidResponse("没有测试响应".into()))
        }
    }
    fn call(id: &str, name: &str, arguments: Value) -> ModelToolCall {
        ModelToolCall {
            id: id.into(),
            name: name.into(),
            raw_arguments: arguments.to_string(),
            arguments,
        }
    }
    fn response(
        id: &str,
        content: Option<&str>,
        calls: Vec<ModelToolCall>,
        input: u64,
        output: u64,
    ) -> ModelResponse {
        ModelResponse {
            response_id: id.into(),
            content: content.map(str::to_string),
            tool_calls: calls,
            usage: TokenUsage {
                input_tokens: input,
                output_tokens: output,
            },
        }
    }

    #[test]
    fn agent_calls_tools_commits_core_and_records_exact_usage() {
        let provider = ScriptedProvider {
            responses: Mutex::new(VecDeque::from([
                response(
                    "r1",
                    None,
                    vec![call(
                        "c1",
                        "set_target_character",
                        json!({"character":"Blade"}),
                    )],
                    10,
                    2,
                ),
                response(
                    "r2",
                    None,
                    vec![call("c2", "get_next_relic_recommendation", json!({}))],
                    20,
                    3,
                ),
                response("r3", Some("推荐 9100002；分数来自工具。"), vec![], 30, 4),
            ])),
            calls: Mutex::new(0),
        };
        let runtime = AgentRuntime::new(provider);
        let mut engine =
            DecisionEngine::new(load_scanner_v4(DEMO_ACCOUNT, 8).unwrap(), MockEvaluator);
        let mut usage = UsageLedger::default();
        let run = runtime
            .run(
                "我想养刃，推荐一件",
                &ModelConfig::default(),
                &mut usage,
                &mut engine,
                &AtomicBool::new(false),
            )
            .unwrap();
        assert_eq!(run.status, AgentRunStatus::Completed);
        assert_eq!(engine.goal().unwrap().character_id, "1205");
        assert_eq!(engine.selected().unwrap().id, "9100002");
        assert_eq!(usage.summary().input_tokens, 60);
        assert_eq!(usage.summary().output_tokens, 9);
        assert!(run.events.iter().any(|e| matches!(e,AgentEvent::ToolFinished { name,.. } if name=="get_next_relic_recommendation")));
    }

    #[test]
    fn reached_budget_blocks_without_calling_provider() {
        let provider = ScriptedProvider {
            responses: Mutex::new(VecDeque::new()),
            calls: Mutex::new(0),
        };
        let runtime = AgentRuntime::new(provider);
        let mut config = ModelConfig::default();
        let mut usage = UsageLedger::default();
        usage.record(
            &config,
            "old".into(),
            TokenUsage {
                input_tokens: 20_000,
                output_tokens: 0,
            },
        );
        let mut engine =
            DecisionEngine::new(load_scanner_v4(DEMO_ACCOUNT, 8).unwrap(), MockEvaluator);
        let error = runtime
            .run(
                "推荐刃",
                &config,
                &mut usage,
                &mut engine,
                &AtomicBool::new(false),
            )
            .unwrap_err();
        assert!(matches!(error, RuntimeError::BudgetReached { .. }));
        assert!(engine.goal().is_none());
        config = ModelConfig::default();
        assert_eq!(config.token_budget(), 20_000);
    }
}

use crate::{
    ChatMessage, CoreTools, ModelConfig, ModelProvider, ProviderError, ProviderRequest,
    ToolDefinition, UsageLedger, UsageRecord, tool_definitions,
};
use hsr_relic_agent::{CultivationGoal, CultivationStrategy, DecisionEngine, Evaluator};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{
    fmt,
    sync::atomic::{AtomicBool, AtomicU64, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

const SYSTEM_PROMPT: &str = r#"你是《崩坏：星穹铁道》单目标角色遗器强化助手。
用户已经知道要培养哪个角色。你负责把自然语言中的目标、材料压力、风险倾向和培养目标转换为受控的结构化意图，调用工具，并解释确定性结果。
推荐前必须调用 set_cultivation_intent：材料宽松/一般/紧张映射为 relaxed/normal/tight；风险保守/均衡/愿意赌映射为 conservative/balanced/aggressive；快速提升当前战力/平衡/追求极品满级上限映射为 immediate_power/balanced/max_potential。用户没有表达某项偏好时分别使用 normal、balanced、balanced。
如果目标角色缺失，或用户表达的关键偏好互相冲突而无法可靠归类，先调用 get_current_state 读取已有状态，再用一个简短问题追问，不要猜测。
设置意图后调用 get_next_relic_recommendation。Rust 会校验意图并选择有限策略；不得添加评分权重、阈值、priority、decision 等工具参数。
用户报告已经发生的强化时，使用 record_upgrade_result。relic_id 可省略并使用当前选中遗器；expected_level 和 resulting_level 分别是强化前后等级，必须恰好相差 3。只有明确知道副属性类别和精确增加量时才能记录：生命、攻击、防御必须区分固定值和百分比，不能根据“歪了”“出了一次”等措辞猜数值。信息不足时先调用 get_current_state，再追问缺失字段，绝不能重复记录同一次观察。
record_upgrade_result 的 Continue/Hold/Stop、reason、预算、历史和 next_recommendation 均来自 Rust。Continue 时解释后等待下一次强化；Hold 时可调用 get_relic_candidates 查看其他候选；Stop 时根据返回的下一候选继续；预算耗尽时停止请求强化。若用户没有报告新强化、只是明确说换下一件或先不升当前遗器，调用 get_next_relic_recommendation 并设 exclude_selected=true；不要伪造强化或三态。
用户中途明确修改培养偏好时，可以再次调用 set_cultivation_intent；否则沿用 get_current_state 中已有的 CultivationGoal。
必须通过工具读取账号状态和推荐；不要自行计算、编造或覆盖遗器评分、候选排序、Build 数值、Continue/Hold/Stop。
最终用简洁中文回答。推荐任务说明 Rust 策略、推荐遗器和工具理由；强化反馈说明 Rust 返回的 Continue/Hold/Stop、预算变化和下一步。数值来自 Evaluator，排序、状态更新和三态决策来自 Rust Decision Engine。"#;

static RUN_SEQUENCE: AtomicU64 = AtomicU64::new(1);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentRunStatus {
    Running,
    Completed,
    BudgetReached,
    Failed,
    Cancelled,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolProgressStage {
    ReadingState,
    EvaluatingRelics,
    RecordingUpgrade,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum AgentEvent {
    RunStarted {
        user_input: String,
    },
    ModelRequestStarted {
        call_index: u32,
        model: String,
    },
    ModelResponseReceived {
        call_index: u32,
        response_id: String,
        tool_call_count: usize,
        has_text: bool,
    },
    UsageRecorded {
        call: UsageRecord,
    },
    ToolRequested {
        call_id: String,
        name: String,
        arguments: Value,
    },
    ToolProgress {
        call_id: String,
        name: String,
        stage: ToolProgressStage,
    },
    ToolFinished {
        call_id: String,
        name: String,
        result: Value,
    },
    CultivationIntentResolved {
        intent: CultivationGoal,
        strategy: CultivationStrategy,
    },
    DecisionRecorded {
        tool_name: String,
        result: Value,
    },
    AssistantReply {
        content: String,
    },
    BudgetBlocked {
        used_tokens: u64,
        token_budget: u64,
    },
    RunFinished {
        status: AgentRunStatus,
    },
    Error {
        code: String,
        message: String,
    },
    Cancelled {
        message: String,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TraceEvent {
    pub run_id: String,
    pub sequence: u64,
    pub recorded_at_unix_ms: u64,
    #[serde(flatten)]
    pub event: AgentEvent,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AgentRun {
    pub id: String,
    pub status: AgentRunStatus,
    pub started_at_unix_ms: u64,
    pub finished_at_unix_ms: Option<u64>,
    pub user_input: String,
    pub reply: Option<String>,
    pub error: Option<String>,
    pub events: Vec<TraceEvent>,
}

#[derive(Debug)]
pub struct AgentRunFailure {
    pub error: RuntimeError,
    pub run: Box<AgentRun>,
}

pub struct AgentRunContext<'a, E> {
    pub usage: &'a mut UsageLedger,
    pub engine: &'a mut DecisionEngine<E>,
    pub history: &'a mut Vec<ChatMessage>,
    pub cancelled: &'a AtomicBool,
    pub on_update: &'a mut dyn FnMut(&AgentRun),
}

#[derive(Debug)]
pub enum RuntimeError {
    InvalidInput(String),
    InvalidHistory(String),
    ModelNotConfigured,
    BudgetReached { used_tokens: u64, token_budget: u64 },
    Cancelled,
    Provider(ProviderError),
    ToolLoopLimit,
    MissingToolCall,
    MissingReply,
}

impl RuntimeError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::InvalidInput(_) => "invalid_agent_input",
            Self::InvalidHistory(_) => "invalid_agent_history",
            Self::ModelNotConfigured => "model_not_configured",
            Self::BudgetReached { .. } => "token_budget_reached",
            Self::Cancelled => "cancelled",
            Self::Provider(_) => "model_provider_failed",
            Self::ToolLoopLimit => "tool_loop_limit",
            Self::MissingToolCall => "tool_call_required",
            Self::MissingReply => "model_reply_missing",
        }
    }
}

impl fmt::Display for RuntimeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidInput(m) | Self::InvalidHistory(m) => f.write_str(m),
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
            Self::Cancelled => f.write_str("Agent 运行已取消；已完成的事件和工具状态已保留"),
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
        if value == ProviderError::Cancelled {
            Self::Cancelled
        } else {
            Self::Provider(value)
        }
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
        let mut history = Vec::new();
        let mut on_update = |_: &AgentRun| {};
        self.run_with_context(
            input,
            config,
            AgentRunContext {
                usage,
                engine,
                history: &mut history,
                cancelled,
                on_update: &mut on_update,
            },
        )
        .map_err(|failure| failure.error)
    }

    pub fn run_with_context<E>(
        &self,
        input: &str,
        config: &ModelConfig,
        context: AgentRunContext<'_, E>,
    ) -> Result<AgentRun, AgentRunFailure>
    where
        E: Evaluator + Clone,
    {
        let AgentRunContext {
            usage,
            engine,
            history,
            cancelled,
            on_update,
        } = context;
        let input = input.trim();
        let mut trace = RunRecorder::new(input, on_update);
        trace.event(AgentEvent::RunStarted {
            user_input: input.into(),
        });
        if input.is_empty() || input.chars().count() > 4000 {
            return Err(trace.fail(RuntimeError::InvalidInput(
                "自然语言输入必须为 1 到 4000 个字符".into(),
            )));
        }
        if config.model().trim().is_empty() || config.endpoint().trim().is_empty() {
            return Err(trace.fail(RuntimeError::ModelNotConfigured));
        }
        if history.is_empty() {
            history.push(ChatMessage::System {
                content: SYSTEM_PROMPT.into(),
            });
        } else {
            match history.first_mut() {
                Some(ChatMessage::System { content }) => *content = SYSTEM_PROMPT.into(),
                _ => {
                    return Err(trace.fail(RuntimeError::InvalidHistory(
                        "Agent 历史缺少 system message".into(),
                    )));
                }
            }
        }
        history.push(ChatMessage::User {
            content: input.into(),
        });
        if let Err(error) = check_budget(config, usage) {
            return Err(trace.fail(error));
        }

        let mut staged = engine.clone();
        let mut used_tool = false;
        for call_index in 1..=self.max_model_calls {
            if cancelled.load(Ordering::SeqCst) {
                *engine = staged;
                return Err(trace.fail(RuntimeError::Cancelled));
            }
            if let Err(error) = check_budget(config, usage) {
                return Err(trace.fail(error));
            }
            trace.event(AgentEvent::ModelRequestStarted {
                call_index,
                model: config.model().into(),
            });
            let response = match self.provider.complete(
                config,
                &ProviderRequest {
                    messages: history.clone(),
                    tools: self.tools.clone(),
                    require_tool: !used_tool,
                },
                cancelled,
            ) {
                Ok(response) => response,
                Err(ProviderError::Cancelled) => {
                    *engine = staged;
                    return Err(trace.fail(RuntimeError::Cancelled));
                }
                Err(error) => return Err(trace.fail(RuntimeError::Provider(error))),
            };
            trace.event(AgentEvent::ModelResponseReceived {
                call_index,
                response_id: response.response_id.clone(),
                tool_call_count: response.tool_calls.len(),
                has_text: response.content.is_some(),
            });
            let record = usage.record(config, response.response_id, response.usage);
            trace.event(AgentEvent::UsageRecorded { call: record });
            if cancelled.load(Ordering::SeqCst) {
                *engine = staged;
                return Err(trace.fail(RuntimeError::Cancelled));
            }

            if response.tool_calls.is_empty() {
                history.push(ChatMessage::Assistant {
                    content: response.content.clone(),
                    tool_calls: vec![],
                });
                if !used_tool {
                    return Err(trace.fail(RuntimeError::MissingToolCall));
                }
                let reply = match response.content {
                    Some(reply) => reply,
                    None => return Err(trace.fail(RuntimeError::MissingReply)),
                };
                trace.event(AgentEvent::AssistantReply {
                    content: reply.clone(),
                });
                *engine = staged;
                return Ok(trace.finish(AgentRunStatus::Completed, Some(reply)));
            }

            used_tool = true;
            history.push(ChatMessage::Assistant {
                content: response.content,
                tool_calls: response.tool_calls.clone(),
            });
            for call in response.tool_calls {
                trace.event(AgentEvent::ToolRequested {
                    call_id: call.id.clone(),
                    name: call.name.clone(),
                    arguments: call.arguments.clone(),
                });
                trace.event(AgentEvent::ToolProgress {
                    call_id: call.id.clone(),
                    name: call.name.clone(),
                    stage: tool_stage(&call.name),
                });
                let execution = CoreTools::new(&mut staged).execute(&call.name, &call.arguments);
                let tool_succeeded = execution.is_ok();
                let result = match execution {
                    Ok(value) => serde_json::json!({"ok":true,"result":value}),
                    Err(error) => serde_json::json!({"ok":false,"error":error}),
                };
                trace.event(AgentEvent::ToolFinished {
                    call_id: call.id.clone(),
                    name: call.name.clone(),
                    result: result.clone(),
                });
                if matches!(
                    call.name.as_str(),
                    "set_cultivation_intent" | "set_target_character"
                ) && result["ok"] == Value::Bool(true)
                    && let Some(intent) = staged.goal().cloned()
                {
                    trace.event(AgentEvent::CultivationIntentResolved {
                        strategy: intent.strategy(),
                        intent,
                    });
                }
                if matches!(
                    call.name.as_str(),
                    "get_relic_candidates"
                        | "get_next_relic_recommendation"
                        | "record_upgrade_result"
                ) {
                    trace.event(AgentEvent::DecisionRecorded {
                        tool_name: call.name.clone(),
                        result: result.clone(),
                    });
                }
                if tool_succeeded && tool_commits_state(&call.name) {
                    *engine = staged.clone();
                }
                history.push(ChatMessage::Tool {
                    call_id: call.id,
                    name: call.name,
                    content: result.to_string(),
                });
                if cancelled.load(Ordering::SeqCst) {
                    *engine = staged;
                    return Err(trace.fail(RuntimeError::Cancelled));
                }
            }
            if !usage.can_start_request(config) {
                let used_tokens = usage.summary().total_tokens;
                trace.event(AgentEvent::BudgetBlocked {
                    used_tokens,
                    token_budget: config.token_budget(),
                });
                let reply = format!(
                    "模型已完成工具调用，但 Token Budget 已达到（{used_tokens} / {}），因此没有继续发起模型请求。请查看工具轨迹中的确定性结果。",
                    config.token_budget()
                );
                history.push(ChatMessage::Assistant {
                    content: Some(reply.clone()),
                    tool_calls: vec![],
                });
                trace.event(AgentEvent::AssistantReply {
                    content: reply.clone(),
                });
                *engine = staged;
                return Ok(trace.finish(AgentRunStatus::BudgetReached, Some(reply)));
            }
        }
        Err(trace.fail(RuntimeError::ToolLoopLimit))
    }
}

fn tool_stage(name: &str) -> ToolProgressStage {
    match name {
        "get_current_state" | "get_upgrade_history" => ToolProgressStage::ReadingState,
        "record_upgrade_result" => ToolProgressStage::RecordingUpgrade,
        _ => ToolProgressStage::EvaluatingRelics,
    }
}

fn tool_commits_state(name: &str) -> bool {
    matches!(
        name,
        "set_cultivation_intent"
            | "set_target_character"
            | "get_next_relic_recommendation"
            | "record_upgrade_result"
    )
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

struct RunRecorder<'a> {
    run: AgentRun,
    on_update: &'a mut dyn FnMut(&AgentRun),
}

impl<'a> RunRecorder<'a> {
    fn new(input: &str, on_update: &'a mut dyn FnMut(&AgentRun)) -> Self {
        let now = now_ms();
        Self {
            run: AgentRun {
                id: format!("run-{now}-{}", RUN_SEQUENCE.fetch_add(1, Ordering::Relaxed)),
                status: AgentRunStatus::Running,
                started_at_unix_ms: now,
                finished_at_unix_ms: None,
                user_input: input.into(),
                reply: None,
                error: None,
                events: vec![],
            },
            on_update,
        }
    }

    fn event(&mut self, event: AgentEvent) {
        self.run.events.push(TraceEvent {
            run_id: self.run.id.clone(),
            sequence: self.run.events.len() as u64 + 1,
            recorded_at_unix_ms: now_ms(),
            event,
        });
        (self.on_update)(&self.run);
    }

    fn finish(mut self, status: AgentRunStatus, reply: Option<String>) -> AgentRun {
        self.run.status = status;
        self.run.reply = reply;
        self.run.finished_at_unix_ms = Some(now_ms());
        self.event(AgentEvent::RunFinished { status });
        self.run
    }

    fn fail(mut self, error: RuntimeError) -> AgentRunFailure {
        let status = if matches!(error, RuntimeError::Cancelled) {
            AgentRunStatus::Cancelled
        } else {
            AgentRunStatus::Failed
        };
        let message = error.to_string();
        self.run.status = status;
        self.run.error = Some(message.clone());
        self.run.finished_at_unix_ms = Some(now_ms());
        if status == AgentRunStatus::Cancelled {
            self.event(AgentEvent::Cancelled { message });
        } else {
            self.event(AgentEvent::Error {
                code: error.code().into(),
                message,
            });
        }
        AgentRunFailure {
            error,
            run: Box::new(self.run),
        }
    }
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ModelResponse, ModelToolCall, TokenUsage};
    use hsr_relic_agent::{DEMO_ACCOUNT, MockEvaluator, UpgradeDecision, load_scanner_v4};
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
            _: &AtomicBool,
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
    fn agent_calls_tools_commits_core_records_usage_and_keeps_context() {
        let provider = ScriptedProvider {
            responses: Mutex::new(VecDeque::from([
                response(
                    "r1",
                    None,
                    vec![call(
                        "c1",
                        "set_cultivation_intent",
                        json!({"character":"Blade","material_pressure":"tight",
                            "risk_tolerance":"conservative","objective":"immediate_power"}),
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
                response(
                    "r3",
                    Some("即使这段文本声称推荐 9100001，也不能覆盖 Rust 状态。"),
                    vec![],
                    30,
                    4,
                ),
            ])),
            calls: Mutex::new(0),
        };
        let runtime = AgentRuntime::new(provider);
        let mut engine =
            DecisionEngine::new(load_scanner_v4(DEMO_ACCOUNT, 8).unwrap(), MockEvaluator);
        let mut usage = UsageLedger::default();
        let mut history = vec![];
        let mut live = vec![];
        let mut on_update = |run: &AgentRun| live.push(run.events.last().unwrap().clone());
        let run = runtime
            .run_with_context(
                "我想养刃，推荐一件",
                &ModelConfig::default(),
                AgentRunContext {
                    usage: &mut usage,
                    engine: &mut engine,
                    history: &mut history,
                    cancelled: &AtomicBool::new(false),
                    on_update: &mut on_update,
                },
            )
            .unwrap();
        assert_eq!(run.status, AgentRunStatus::Completed);
        assert_eq!(engine.goal().unwrap().character_id, "1205");
        assert_eq!(
            engine.goal().unwrap().strategy(),
            CultivationStrategy::Conservative
        );
        assert_eq!(engine.selected().unwrap().id, "9100002");
        assert_eq!(usage.summary().input_tokens, 60);
        assert_eq!(usage.summary().output_tokens, 9);
        assert_eq!(history.len(), 7);
        assert_eq!(live, run.events);
        assert!(run.events.iter().any(|e| matches!(&e.event, AgentEvent::DecisionRecorded { tool_name,.. } if tool_name=="get_next_relic_recommendation")));
        assert!(run.events.iter().any(|e| matches!(&e.event,
            AgentEvent::CultivationIntentResolved { intent, strategy }
                if intent.preferences.material_pressure == hsr_relic_agent::MaterialPressure::Tight
                    && *strategy == CultivationStrategy::Conservative)));
    }

    #[test]
    fn full_multi_turn_agent_loop_records_an_observation_and_continues() {
        let provider = ScriptedProvider {
            responses: Mutex::new(VecDeque::from([
                response(
                    "goal",
                    None,
                    vec![call(
                        "goal-call",
                        "set_cultivation_intent",
                        json!({"character":"Blade","material_pressure":"normal",
                            "risk_tolerance":"balanced","objective":"balanced"}),
                    )],
                    10,
                    2,
                ),
                response(
                    "recommend",
                    None,
                    vec![call(
                        "recommend-call",
                        "get_next_relic_recommendation",
                        json!({}),
                    )],
                    12,
                    2,
                ),
                response("recommend-reply", Some("先强化 9100002。"), vec![], 14, 3),
                response(
                    "observe",
                    None,
                    vec![call(
                        "observe-call",
                        "record_upgrade_result",
                        json!({"expected_level":3,"resulting_level":6,
                            "stat":"crit_rate","increase":3.24}),
                    )],
                    16,
                    2,
                ),
                response(
                    "observe-reply",
                    Some("Rust 判断为 Continue，请继续观察下一次强化。"),
                    vec![],
                    18,
                    3,
                ),
            ])),
            calls: Mutex::new(0),
        };
        let runtime = AgentRuntime::new(provider);
        let mut engine =
            DecisionEngine::new(load_scanner_v4(DEMO_ACCOUNT, 8).unwrap(), MockEvaluator);
        let mut usage = UsageLedger::default();
        let mut history = vec![];
        let mut on_update = |_: &AgentRun| {};
        runtime
            .run_with_context(
                "我想培养 Blade，按均衡方式推荐一件",
                &ModelConfig::default(),
                AgentRunContext {
                    usage: &mut usage,
                    engine: &mut engine,
                    history: &mut history,
                    cancelled: &AtomicBool::new(false),
                    on_update: &mut on_update,
                },
            )
            .unwrap();
        assert_eq!(engine.selected().unwrap().id, "9100002");

        let run = runtime
            .run_with_context(
                "刚才那件从 +3 升到 +6，暴击率增加了 3.24",
                &ModelConfig::default(),
                AgentRunContext {
                    usage: &mut usage,
                    engine: &mut engine,
                    history: &mut history,
                    cancelled: &AtomicBool::new(false),
                    on_update: &mut on_update,
                },
            )
            .unwrap();
        let record = engine.account().history.last().unwrap();
        assert_eq!(record.after.id, "9100002");
        assert_eq!(record.before.level, 3);
        assert_eq!(record.after.level, 6);
        assert_eq!(record.decision, UpgradeDecision::Continue);
        assert_eq!(engine.account().upgrade_steps, 7);
        assert_eq!(engine.selected().unwrap().id, "9100002");
        let decision_event = run.events.iter().find(|event| {
            matches!(&event.event, AgentEvent::DecisionRecorded { tool_name, result }
                if tool_name == "record_upgrade_result"
                    && result["result"]["decision"] == "Continue")
        });
        assert!(decision_event.is_some());
    }

    #[test]
    fn ambiguous_upgrade_feedback_is_clarified_without_mutation() {
        let provider = ScriptedProvider {
            responses: Mutex::new(VecDeque::from([
                response(
                    "inspect",
                    None,
                    vec![call("state", "get_current_state", json!({}))],
                    4,
                    1,
                ),
                response(
                    "question",
                    Some("是固定防御还是防御百分比？精确增加了多少？"),
                    vec![],
                    5,
                    2,
                ),
            ])),
            calls: Mutex::new(0),
        };
        let runtime = AgentRuntime::new(provider);
        let mut engine =
            DecisionEngine::new(load_scanner_v4(DEMO_ACCOUNT, 8).unwrap(), MockEvaluator);
        engine.set_goal("1205").unwrap();
        engine.recommend_next().unwrap();
        let before = engine.account().clone();
        let mut usage = UsageLedger::default();
        let mut history = vec![];
        let mut on_update = |_: &AgentRun| {};
        let run = runtime
            .run_with_context(
                "刚才那件 +3 出了防御",
                &ModelConfig::default(),
                AgentRunContext {
                    usage: &mut usage,
                    engine: &mut engine,
                    history: &mut history,
                    cancelled: &AtomicBool::new(false),
                    on_update: &mut on_update,
                },
            )
            .unwrap();
        assert_eq!(*engine.account(), before);
        assert!(run.reply.unwrap().contains("精确增加"));
        assert!(!run.events.iter().any(|event| {
            matches!(&event.event, AgentEvent::ToolRequested { name, .. }
                if name == "record_upgrade_result")
        }));
    }

    #[test]
    fn hold_observation_is_followed_by_a_real_candidate_query() {
        let provider = ScriptedProvider {
            responses: Mutex::new(VecDeque::from([
                response(
                    "hold",
                    None,
                    vec![call(
                        "hold-call",
                        "record_upgrade_result",
                        json!({"expected_level":0,"resulting_level":3,
                            "stat":"def_percent","increase":5.4}),
                    )],
                    5,
                    1,
                ),
                response(
                    "compare",
                    None,
                    vec![call(
                        "candidate-call",
                        "get_relic_candidates",
                        json!({"limit":3}),
                    )],
                    6,
                    1,
                ),
                response(
                    "reply",
                    Some("Rust 已 Hold 当前遗器，我比较了其他候选。"),
                    vec![],
                    7,
                    2,
                ),
            ])),
            calls: Mutex::new(0),
        };
        let runtime = AgentRuntime::new(provider);
        let mut engine =
            DecisionEngine::new(load_scanner_v4(DEMO_ACCOUNT, 8).unwrap(), MockEvaluator);
        engine.set_goal("1205").unwrap();
        engine.select_relic("9100001").unwrap();
        let mut usage = UsageLedger::default();
        let mut history = vec![];
        let mut on_update = |_: &AgentRun| {};
        let run = runtime
            .run_with_context(
                "9100001 从 +0 到 +3，防御百分比增加 5.4",
                &ModelConfig::default(),
                AgentRunContext {
                    usage: &mut usage,
                    engine: &mut engine,
                    history: &mut history,
                    cancelled: &AtomicBool::new(false),
                    on_update: &mut on_update,
                },
            )
            .unwrap();
        assert_eq!(
            engine.account().history.last().unwrap().decision,
            UpgradeDecision::Hold
        );
        let hold_index = run
            .events
            .iter()
            .position(|event| {
                matches!(&event.event,
                AgentEvent::DecisionRecorded { tool_name, result }
                    if tool_name == "record_upgrade_result"
                        && result["result"]["decision"] == "Hold")
            })
            .unwrap();
        let candidates_index = run
            .events
            .iter()
            .position(|event| {
                matches!(&event.event,
                AgentEvent::ToolRequested { name, .. }
                    if name == "get_relic_candidates")
            })
            .unwrap();
        assert!(hold_index < candidates_index);
    }

    #[test]
    fn stop_observation_is_followed_by_a_real_replan_tool_call() {
        let provider = ScriptedProvider {
            responses: Mutex::new(VecDeque::from([
                response(
                    "stop",
                    None,
                    vec![call(
                        "stop-call",
                        "record_upgrade_result",
                        json!({"expected_level":6,"resulting_level":9,
                            "stat":"def_percent","increase":5.4}),
                    )],
                    5,
                    1,
                ),
                response(
                    "replan",
                    None,
                    vec![call(
                        "replan-call",
                        "get_next_relic_recommendation",
                        json!({}),
                    )],
                    6,
                    1,
                ),
                response(
                    "reply",
                    Some("Rust 已 Stop 当前遗器，并切换到下一候选。"),
                    vec![],
                    7,
                    2,
                ),
            ])),
            calls: Mutex::new(0),
        };
        let runtime = AgentRuntime::new(provider);
        let mut engine =
            DecisionEngine::new(load_scanner_v4(DEMO_ACCOUNT, 8).unwrap(), MockEvaluator);
        engine.set_goal("1205").unwrap();
        engine.select_relic("9100003").unwrap();
        let mut usage = UsageLedger::default();
        let mut history = vec![];
        let mut on_update = |_: &AgentRun| {};
        let run = runtime
            .run_with_context(
                "9100003 从 +6 到 +9，防御百分比增加 5.4",
                &ModelConfig::default(),
                AgentRunContext {
                    usage: &mut usage,
                    engine: &mut engine,
                    history: &mut history,
                    cancelled: &AtomicBool::new(false),
                    on_update: &mut on_update,
                },
            )
            .unwrap();
        assert_eq!(
            engine.account().history.last().unwrap().decision,
            UpgradeDecision::Stop
        );
        assert_ne!(engine.selected().unwrap().id, "9100003");
        let record_index = run
            .events
            .iter()
            .position(|event| {
                matches!(&event.event,
                AgentEvent::DecisionRecorded { tool_name, .. }
                    if tool_name == "record_upgrade_result")
            })
            .unwrap();
        let replan_index = run
            .events
            .iter()
            .position(|event| {
                matches!(&event.event,
                AgentEvent::ToolRequested { name, .. }
                    if name == "get_next_relic_recommendation")
            })
            .unwrap();
        assert!(record_index < replan_index);
    }

    #[test]
    fn completed_upgrade_tool_is_a_state_checkpoint_if_later_model_call_fails() {
        struct FailsAfterObservation(AtomicU64);
        impl ModelProvider for FailsAfterObservation {
            fn complete(
                &self,
                _: &ModelConfig,
                _: &ProviderRequest,
                _: &AtomicBool,
            ) -> Result<ModelResponse, ProviderError> {
                if self.0.fetch_add(1, Ordering::SeqCst) == 0 {
                    Ok(response(
                        "observed",
                        None,
                        vec![call(
                            "observation",
                            "record_upgrade_result",
                            json!({"expected_level":3,"resulting_level":6,
                                "stat":"crit_rate","increase":3.24}),
                        )],
                        4,
                        1,
                    ))
                } else {
                    Err(ProviderError::RequestFailed("测试中的后续解释失败".into()))
                }
            }
        }
        let runtime = AgentRuntime::new(FailsAfterObservation(AtomicU64::new(0)));
        let mut engine =
            DecisionEngine::new(load_scanner_v4(DEMO_ACCOUNT, 8).unwrap(), MockEvaluator);
        engine.set_goal("1205").unwrap();
        engine.recommend_next().unwrap();
        let mut usage = UsageLedger::default();
        let mut history = vec![];
        let mut on_update = |_: &AgentRun| {};
        let failure = runtime
            .run_with_context(
                "刚才那件暴击率增加 3.24",
                &ModelConfig::default(),
                AgentRunContext {
                    usage: &mut usage,
                    engine: &mut engine,
                    history: &mut history,
                    cancelled: &AtomicBool::new(false),
                    on_update: &mut on_update,
                },
            )
            .unwrap_err();
        assert!(matches!(failure.error, RuntimeError::Provider(_)));
        assert_eq!(engine.account().history.len(), 1);
        assert_eq!(engine.account().relics["9100002"].level, 6);
        assert_eq!(engine.account().upgrade_steps, 7);
    }

    #[test]
    fn second_run_extends_previous_context() {
        let provider = ScriptedProvider {
            responses: Mutex::new(VecDeque::from([
                response(
                    "a1",
                    None,
                    vec![call("c1", "get_current_state", json!({}))],
                    1,
                    1,
                ),
                response("a2", Some("第一次完成"), vec![], 1, 1),
                response(
                    "b1",
                    None,
                    vec![call("c2", "get_current_state", json!({}))],
                    1,
                    1,
                ),
                response("b2", Some("第二次完成"), vec![], 1, 1),
            ])),
            calls: Mutex::new(0),
        };
        let runtime = AgentRuntime::new(provider);
        let mut engine =
            DecisionEngine::new(load_scanner_v4(DEMO_ACCOUNT, 8).unwrap(), MockEvaluator);
        let mut usage = UsageLedger::default();
        let mut history = vec![];
        for input in ["先看状态", "再看一次"] {
            let mut on_update = |_: &AgentRun| {};
            runtime
                .run_with_context(
                    input,
                    &ModelConfig::default(),
                    AgentRunContext {
                        usage: &mut usage,
                        engine: &mut engine,
                        history: &mut history,
                        cancelled: &AtomicBool::new(false),
                        on_update: &mut on_update,
                    },
                )
                .unwrap();
        }
        assert_eq!(
            history
                .iter()
                .filter(|m| matches!(m, ChatMessage::User { .. }))
                .count(),
            2
        );
        assert_eq!(
            history
                .iter()
                .filter(|m| matches!(m, ChatMessage::Tool { .. }))
                .count(),
            2
        );
    }

    #[test]
    fn agent_can_inspect_state_and_ask_for_a_missing_target() {
        let provider = ScriptedProvider {
            responses: Mutex::new(VecDeque::from([
                response(
                    "clarify-1",
                    None,
                    vec![call("state", "get_current_state", json!({}))],
                    4,
                    1,
                ),
                response(
                    "clarify-2",
                    Some("你想培养哪位角色？也可以同时告诉我材料和风险偏好。"),
                    vec![],
                    5,
                    2,
                ),
            ])),
            calls: Mutex::new(0),
        };
        let runtime = AgentRuntime::new(provider);
        let mut engine =
            DecisionEngine::new(load_scanner_v4(DEMO_ACCOUNT, 8).unwrap(), MockEvaluator);
        let mut usage = UsageLedger::default();
        let mut history = vec![];
        let mut on_update = |_: &AgentRun| {};
        let run = runtime
            .run_with_context(
                "帮我看看遗器",
                &ModelConfig::default(),
                AgentRunContext {
                    usage: &mut usage,
                    engine: &mut engine,
                    history: &mut history,
                    cancelled: &AtomicBool::new(false),
                    on_update: &mut on_update,
                },
            )
            .unwrap();
        assert!(engine.goal().is_none());
        assert!(run.reply.unwrap().contains("哪位角色"));
    }

    #[test]
    fn cancellation_is_a_persistable_terminal_run() {
        let provider = ScriptedProvider {
            responses: Mutex::new(VecDeque::new()),
            calls: Mutex::new(0),
        };
        let runtime = AgentRuntime::new(provider);
        let mut engine =
            DecisionEngine::new(load_scanner_v4(DEMO_ACCOUNT, 8).unwrap(), MockEvaluator);
        let mut usage = UsageLedger::default();
        let mut history = vec![];
        let mut on_update = |_: &AgentRun| {};
        let failure = runtime
            .run_with_context(
                "推荐刃",
                &ModelConfig::default(),
                AgentRunContext {
                    usage: &mut usage,
                    engine: &mut engine,
                    history: &mut history,
                    cancelled: &AtomicBool::new(true),
                    on_update: &mut on_update,
                },
            )
            .unwrap_err();
        assert_eq!(failure.run.status, AgentRunStatus::Cancelled);
        assert!(matches!(
            failure.run.events.last().unwrap().event,
            AgentEvent::Cancelled { .. }
        ));
    }

    #[test]
    fn cancellation_after_a_tool_keeps_completed_tool_state_and_trace() {
        struct CancelAfterToolProvider(AtomicU64);
        impl ModelProvider for CancelAfterToolProvider {
            fn complete(
                &self,
                _: &ModelConfig,
                _: &ProviderRequest,
                _: &AtomicBool,
            ) -> Result<ModelResponse, ProviderError> {
                if self.0.fetch_add(1, Ordering::SeqCst) == 0 {
                    Ok(response(
                        "tool",
                        None,
                        vec![call(
                            "set-target",
                            "set_cultivation_intent",
                            json!({"character":"Blade","material_pressure":"normal",
                                "risk_tolerance":"balanced","objective":"balanced"}),
                        )],
                        2,
                        1,
                    ))
                } else {
                    Err(ProviderError::Cancelled)
                }
            }
        }
        let runtime = AgentRuntime::new(CancelAfterToolProvider(AtomicU64::new(0)));
        let mut engine =
            DecisionEngine::new(load_scanner_v4(DEMO_ACCOUNT, 8).unwrap(), MockEvaluator);
        let mut usage = UsageLedger::default();
        let mut history = vec![];
        let mut on_update = |_: &AgentRun| {};
        let failure = runtime
            .run_with_context(
                "培养 Blade",
                &ModelConfig::default(),
                AgentRunContext {
                    usage: &mut usage,
                    engine: &mut engine,
                    history: &mut history,
                    cancelled: &AtomicBool::new(false),
                    on_update: &mut on_update,
                },
            )
            .unwrap_err();
        assert_eq!(engine.goal().unwrap().character_id, "1205");
        assert!(failure.run.events.iter().any(|event| {
            matches!(&event.event, AgentEvent::ToolFinished { name, .. } if name == "set_cultivation_intent")
        }));
        assert_eq!(failure.run.status, AgentRunStatus::Cancelled);
    }

    #[test]
    fn reached_budget_blocks_without_calling_provider() {
        let provider = ScriptedProvider {
            responses: Mutex::new(VecDeque::new()),
            calls: Mutex::new(0),
        };
        let runtime = AgentRuntime::new(provider);
        let config = ModelConfig::default();
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
    }
}

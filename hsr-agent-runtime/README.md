# hsr-agent-runtime

Demo v0.3 的独立 Rust Agent Runtime。它把自然语言模型、Tool 编排和 token 记账放在确定性遗器 core 之外：

```text
AgentRuntime → ModelProvider → OpenAI-compatible Chat Completions
      ↓              ↓
  CoreTools     response usage → UsageLedger
      ↓
DecisionEngine → Evaluator → Fribbels Adapter
```

LLM 不获得重新实现评分或排序的职责。系统提示要求它先调用 Tool；Rust 只在至少成功执行过一次 Tool 后接受自然语言答案。Tool 的结构化结果由现有 `DecisionEngine` 产生。

## 模块

| 文件 | 职责 |
|---|---|
| `src/config.rs` | `ModelConfig`、安全的公开视图、环境变量和 Web patch 校验 |
| `src/provider.rs` | `ModelProvider` 抽象及可取消的 OpenAI-compatible Chat Completions 实现 |
| `src/tools.rs` | 六个 Agent Tool、结构化参数校验及对现有 Rust core 的薄封装 |
| `src/runtime.rs` | 最多六轮的 tool loop、多轮上下文、取消/预算边界和统一 `TraceEvent` |
| `src/usage.rs` | API 响应 usage 的逐次 ledger、累计与费用计算 |

## 模型配置

`ModelConfig` 支持：

- API Endpoint；base URL 或完整 `/chat/completions` 地址；
- API Key；可以为空以调用无鉴权本地服务；
- Model；
- Context Length；当前映射为请求 `max_completion_tokens`。完整多轮历史会保存并传给模型，但尚未实现自动摘要或按窗口裁剪；
- Reasoning Mode：disabled / minimal / low / medium / high；disabled 不发送该字段；
- Input / Output Token Price，均按 1M tokens；
- Token Budget，以 input + output 累计 tokens 计。

新 Web 会话从同名 `HSR_LLM_*` 环境变量初始化；之后可以通过 Web 设置独立修改。Key 在内部使用不可序列化的私密字段，`Debug` 只打印 `[REDACTED]`，公开视图只返回是否已配置。Session JSON 保存公开配置但不保存 Key。

Context Length 不是对远端模型真实上下文窗口的修改；服务端模型仍有自己的硬限制。不同供应商对 `max_completion_tokens` 与 `reasoning_effort` 的支持也可能不同。

## Tools

| Tool | 输入 | 输出与副作用 |
|---|---|---|
| `set_cultivation_intent` | 角色、材料压力、风险倾向、培养目标 | 用 Rust 闭集枚举校验意图，选择保守/均衡/高潜力策略 |
| `get_current_state` | 无 | 角色、当前目标/选择、剩余强化步数、历史数量 |
| `get_relic_candidates` | 可选 `limit` | Rust 排序后的候选及 Evaluator 结构化指标；不改变选择 |
| `get_next_relic_recommendation` | 可选 `exclude_selected` | 由 Rust 推荐并选中下一件；明确换件时可排除当前项一次 |
| `record_upgrade_result` | 可选遗器 ID、强化前后等级、属性、精确增量 | 校验必须恰好 +3 后调用 `DecisionEngine::apply_upgrade`，返回状态更新、Continue/Hold/Stop 和下一候选 |
| `get_upgrade_history` | 无 | 已接受的强化观察及 Continue / Hold / Stop |

`CoreTools` 不包含评分阈值、候选排序或三态决策算法。一次 Agent run 在 `DecisionEngine` 副本上工作；成功的 mutation Tool 形成状态检查点，之后即使解释模型失败，已经确认的真实强化观察也不会回滚。用户取消时同样保留取消前完成的 Tool；正在执行但未完成的 core 操作仍由原有事务边界回滚。

LLM 只提交受控意图，不能提交评分、priority、阈值或 `Continue / Hold / Stop`。Runtime 在 Tool 成功后从 Rust `DecisionEngine` 读取已校验的意图和实际策略，生成 `cultivation_intent_resolved` Trace 事件。缺少角色或关键偏好冲突时，模型可以先调用 `get_current_state`，再用自然语言追问；没有表达偏好时回退到均衡默认。

强化反馈必须包含可以确定副属性类型和精确增量的信息。`get_current_state` 返回当前选中遗器的等级、副属性和最近一次观察，帮助模型解析“刚才那件”；无法区分固定/百分比或缺少数值时只允许追问。强化 Tool Result 包含 observation、剩余预算、历史数量、Rust 三态结果和下一候选，模型据此继续、查询候选或停止，不自行重算。

## Usage、费用和预算

Provider 必须从每个成功响应中读取 `usage.prompt_tokens` 与 `usage.completion_tokens`。缺失 usage 时返回错误，不估算。每次记录包含模型、响应 ID、input/output/total tokens、费用和时间：

```text
cost = input_tokens / 1_000_000 × input_price
     + output_tokens / 1_000_000 × output_price
```

Runtime 在每次模型请求前比较累计 total tokens 与 budget；`used >= budget` 时拒绝发起新请求。单个远端请求仍可能让累计值越过预算，因为准确 token 数只有收到响应后才能得到；下一次请求会被阻止。

## 测试

```bash
cargo test --offline --manifest-path hsr-agent-runtime/Cargo.toml
cargo clippy --offline --manifest-path hsr-agent-runtime/Cargo.toml --all-targets -- -D warnings
```

测试覆盖配置密钥不泄露、协议请求与真实 usage 解析、费用、六个 Tool、自然语言多轮强化闭环、歧义追问、三态/预算/非法输入、Tool 检查点和请求前预算阻断。Provider 的本地 HTTP 测试需要操作系统允许绑定回环端口。

## R4 / R5 公共结构

- `TraceEvent` 带 run ID、事件序号和 Unix 毫秒时间；`AgentEvent` 统一模型、结构化意图、Tool、决策、usage、回复、错误和取消语义。
- `run_with_context` 同时接收历史 `ChatMessage` 和事件回调。每次事件只生成一次：Web SSE 实时发送它，任务结束后同一个 `AgentRun` 被保存。
- Provider 使用可取消的异步 HTTP future；Runtime 轮次与 Tool 边界检查同一取消标志，Fribbels Evaluator 继续使用该标志终止 Adapter 子进程。
- `ChatMessage`、`AgentRun`、`TraceEvent` 和 `UsageLedger` 均可序列化/反序列化，供 Web 的版本化 Session JSON 保存完整上下文。

持久化文件和 SSE 协议由 Web 层负责，Runtime 不依赖 Axum、浏览器或具体存储位置。

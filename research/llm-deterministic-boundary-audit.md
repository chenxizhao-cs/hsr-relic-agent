# LLM 与确定性逻辑边界审计报告

审计日期：2026-09-08

> 本文记录第一轮边界改进前的代码基线。后续实现状态以 `DESIGN.md` 的“结构化培养意图与有限策略”一节为准；保留本文用于说明问题来源和改进依据。

## 结论

当前项目的核心遗器能力不依赖 LLM：Fribbels 负责确定性数值评价，Rust `DecisionEngine` 负责候选过滤、排序、状态更新和 `Continue / Hold / Stop`。LLM 是可选的自然语言入口和 Tool 编排层。

“LLM 只是自然语言壳”的判断对当前核心闭环基本成立，但需要区分两个角度：

- 从产品可靠性看，这不是缺陷。评分、排序、状态与投入决策保持确定性是正确边界。
- 从课程 Agent 的说服力看，存在中高程度风险。LLM 确实进行真实 Tool Calling，但当前不是核心业务成功不可缺少的一环，也没有真正影响用户偏好、约束和强化策略。

## 1. 当前真实架构

实际代码存在三条调用路径：

```text
直接 Web 操作
web/app.js
  → POST /api/action
  → hsr-relic-web::execute
  → DecisionEngine
  → Evaluator
  → Fribbels Adapter

自然语言 Agent
web/app.js
  → POST /api/agent
  → AgentRuntime
  → OpenAI-compatible Provider
  → CoreTools
  → DecisionEngine
  → Evaluator
  → Fribbels Adapter

CLI
hsr-relic-agent::cli
  → DecisionEngine
  → Evaluator
  → Fribbels Adapter / MockEvaluator
```

依据：

- `hsr-relic-web/src/lib.rs::router` 将 `/api/action` 和 `/api/agent` 注册为两个并列入口。
- `hsr-relic-web/src/lib.rs::execute` 直接调用 `set_goal`、`select_relic`、`recommend_next` 和 `apply_upgrade`。
- `hsr-relic-web/src/lib.rs::execute_agent` 才会创建 `AgentRuntime<OpenAiCompatibleProvider>`。
- `web/api.js::action` 与 `web/api.js::agent` 对应两个不同 API。
- `hsr-relic-agent/Cargo.toml` 不依赖 `hsr-agent-runtime`；Web crate 同时依赖 core 和 runtime。

因此 Agent Runtime 是叠加在确定性产品之上的可选层，不是所有业务操作的必经入口。

## 2. 无 LLM 时系统还能做什么

完全不配置模型、API Endpoint 或 API Key，仍可以完成：

1. 加载内置账号或导入 Reliquary JSON；
2. 查看角色、遗器、光锥和装备关系摘要；
3. 选择目标角色；
4. 获取候选排序和下一件推荐；
5. 手动选择遗器；
6. 录入一次强化结果；
7. 更新同一件遗器、预算和强化历史；
8. 获得 `Continue / Hold / Stop`；
9. 在 Hold/Stop 后重新推荐其他遗器；
10. Reset、查看历史、保存和加载业务状态。

Web 页面中的角色、遗器和强化表单直接使用 `/api/action`。`hsr-relic-web/tests/api.rs::core_loop_updates_same_relic_and_resumes_hold_explicitly` 也直接验证了不经过模型的 `Continue → Hold → Stop`、同一遗器更新和重新推荐。

CLI 同样直接构造 `DecisionEngine`，可选择 `FribbelsEvaluator` 或 `MockEvaluator`。`hsr-relic-agent/tests/cli.rs::mock_demo_completes_and_shows_all_decisions` 验证了不使用 LLM 的完整三分支闭环。

不受 LLM 影响的核心能力包括：

- Reliquary 导入和内部 `AccountState`；
- 静态角色—套装适配过滤；
- Fribbels 当前评分、潜力、Build 面板和简化伤害指标；
- 候选排序；
- 装备保护、lock/discard、满级、预算等操作限制；
- 强化状态更新；
- `Continue / Hold / Stop`；
- 业务历史、会话状态以及 Fribbels 任务取消。

## 3. 当前 LLM 的真实作用

| 能力 | 当前实际情况 | 判断 |
| --- | --- | --- |
| 用户意图理解 | 从自然语言判断用户想设置角色、查询状态或获取推荐 | 真实参与，但范围窄 |
| 目标角色提取 | 生成 `set_target_character` 参数；Rust 再解析和校验 | 部分由 LLM 完成 |
| 约束/偏好提取 | 没有对应业务结构或 Tool；“材料紧张”不得改变 Rust 规则 | 基本未实现 |
| Tool selection | 模型真实决定 Tool、参数以及之后是否继续调用 | 真实参与 |
| 多步规划 | 可以读取 Tool Result 后再调用其他 Tool，最多六轮 | 浅层反应式编排 |
| 状态维护 | 账号、目标、选择、历史、预算和会话均由 Rust 保存 | 不由 LLM 负责 |
| 遗器决策 | Fribbels 算数值，Rust 排序并决定三态 | LLM 不负责 |
| 结果解释 | 模型生成最终中文回复 | 真实生成，但多为复述 Rust 理由 |

`hsr-agent-runtime/src/runtime.rs::SYSTEM_PROMPT` 明确禁止模型计算或覆盖遗器评分、排序、Build 数值和三态决策；推荐请求还被提示遵循“确认或设置目标 → 获取下一件推荐”的固定路径。提示词明确规定“材料紧张”只能作为解释偏好，不能改变 Rust 规则。

模型调用本身是真实的。`AgentRuntime::run_with_context` 首次请求强制 Tool Call，模型可以读取 Tool Result 后继续调用工具或回答，最多六次模型请求。因此当前不是伪造的 Agent 调用，但模型可发挥的自主性有限。

## 4. 哪些 Agent 行为仍是固定 Rust 流程

### 固定 Tool 集合

`hsr-agent-runtime/src/tools.rs::TOOL_NAMES` 只有：

- `set_target_character`
- `get_current_state`
- `get_relic_candidates`
- `get_next_relic_recommendation`
- `get_upgrade_history`

目前没有用于录入强化结果、设置资源/风险偏好、比较策略、确认计划或查询缺失信息的 Tool。因此自然语言 Agent 尚不能独立完成“推荐 → 录入强化 → 重新判断”的完整闭环。

### Tool 是 core 的薄包装

`CoreTools::candidates` 直接调用 `DecisionEngine::rank_candidates`，`CoreTools::next_recommendation` 直接调用 `DecisionEngine::recommend_next`。Runtime 产生的 `DecisionRecorded` 事件只是把确定性 Tool Result 记录到 Trace，并不表示模型做出了业务决策。

### 目标和策略结构很窄

`hsr-relic-agent/src/model.rs::CultivationGoal` 当前只有 `character_id`。材料压力、风险倾向、当前即战力、高潜力偏好等信息没有进入业务状态。

### 排序和三态规则完全确定

`DecisionEngine::recommendation` 固定计算：

```text
max(预计评分 - 同部位基线, 0)
÷ 剩余强化步数
× Build 伤害比
```

`DecisionEngine::rank_account` 负责资格过滤、静态套装过滤、阈值过滤和按优先级排序。

`DecisionEngine::apply_upgrade` 按固定顺序检查满级、低潜力、连续两次无效、预算耗尽、单次无效、不能超过基线、替代候选高出 25% 等条件，并决定 `Continue / Hold / Stop`。LLM 不参与这些分支。

### Fribbels 是确定性 Evaluator

`adapters/fribbels/adapter.ts::evaluate` 直接调用上游 `RelicScorer.scoreCurrentRelic`、`RelicScorer.scoreRelicPotential` 和 `simulateBuild`，没有 LLM 参与。

## 5. 如果删除 LLM

会真正失去：

- 自然语言 Agent 输入；
- 模型动态选择 Tool 和调用顺序；
- 模型可读取的多轮对话上下文；
- 模型生成的自由文本解释和有限追问；
- LLM API 配置、真实 Token usage、费用与预算功能；
- 模型请求、Tool Call 等 Agent Trace。

只属于交互体验下降的部分：

- 用户需要点击角色或输入明确命令；
- 查询状态、推荐和历史需要显式操作；
- 推荐原因直接展示 Rust 文本，而不是由模型重新组织；
- 改变目标需要通过表单或命令完成。

当前已实现范围内，没有一项遗器评价、排序或强化决策能力必须依赖 LLM。自由语言理解和开放式解释不能由固定 UI 完整等价替代，但它们不是核心遗器决策成立的前提。

## 6. 当前边界问题

当前系统本质上是：

> 确定性遗器决策程序 + 可选 LLM 自然语言控制层。

严重程度：

- 产品可靠性问题：低。精确逻辑保持确定性是合理设计。
- 当前功能定位偏差：中。文档中的“提取约束和偏好、制定和调整强化计划”尚没有完整代码承载。
- 课程 Agent 说服力风险：中高。Tool Calling、Trace、多轮和 usage 都是真实的，但核心用户目标不使用 LLM 也能完成。
- 黑盒风险：低。Tool Result、Rust 决策理由和 Trace 都可见。

最明确的问题是：用户说“材料比较紧”目前只可能改变回复措辞，不会改变预算、候选排序或三态判断。

## 7. 合理的职责划分

### 继续由确定性程序负责

- 外部 schema 校验和 `AccountState` 转换；
- 装备关系、lock/discard、套装合法性；
- 遗器评分、潜力、面板、伤害、概率和材料成本；
- 在一个明确策略下的候选排序；
- 强化状态更新和三态决策；
- 预算扣减、事务、并发、历史和 Tool 参数校验。

### 适合由 LLM / Agent 负责

- 把模糊描述转换成结构化培养意图；
- 识别材料压力、风险倾向、即战力或高上限偏好；
- 发现信息缺失或约束冲突并追问；
- 决定下一步查询状态、比较策略、请求用户观察还是结束；
- 用户中途改变目标或偏好后重新规划；
- 在 Rust 算出的多个可靠方案之间根据用户偏好权衡；
- 解释决策变化和反事实方案。

原则应是：LLM 选择问题、约束、策略和工具；Rust/Fribbels 计算每个策略下的答案。

## 8. 改进方案

| 方案 | LLM 负责 | Rust / Fribbels 负责 | 可靠性 | Agent 说服力 | 复杂度 |
| --- | --- | --- | --- | --- | --- |
| A. 最小改动 | 提取目标、材料压力、风险倾向；必要时追问；选择受控策略 | 校验意图并按预设策略确定性评价和决策 | 高 | 中 | 低 |
| B. 平衡方案（推荐） | 建立任务意图、处理缺失信息、比较策略、根据结果调整计划 | 计算保守/均衡/高潜力方案；拥有评分、排序和三态规则 | 高 | 高 | 中 |
| C. 更强 Agent 化 | 维护显式计划，动态查询、比较、追问、录入观察和重新规划 | 状态机、策略守卫、确定性工具与状态变更确认 | 较高 | 很高 | 高 |

### A. 最小改动

增加受控的结构化偏好，例如 `resource_pressure`、`risk_tolerance`、`objective` 和 `missing_information`。LLM 从语言提取，Rust 将其映射到有限、可测试的策略。至少一个偏好必须真实改变 Rust 使用的策略，否则仍然只是语言壳。

### B. 平衡方案

Rust 计算保守、均衡和高潜力等多个确定性方案。LLM 理解用户偏好、缺信息时追问、调用 Rust 比较方案，再根据用户明确偏好选择方案。三态判断仍由 Rust 在所选策略下完成。

这是当前最值得采用的方向，能够兼顾可靠性、课程展示和开发成本。

### C. 更强 Agent 化

增加分析账号、比较策略、查询资源、录入强化观察、重新规划和请求确认等原子 Tool。LLM 决定调用顺序，Rust 保证每一步合法、可回滚和可审计。说服力最强，但当前阶段容易过度设计。

## 9. 推荐的下一轮最小改动

先建立一个有因果意义的纵向闭环：

```text
自然语言目标
→ LLM 输出结构化 CultivationIntent
→ Rust 校验
→ 信息不足时 Agent 追问
→ Rust 按受控策略计算候选
→ LLM 解释结果和权衡
```

首批偏好只需要：

- 材料压力：宽松 / 一般 / 紧张；
- 风险倾向：保守 / 均衡 / 激进；
- 培养目标：当前即战力 / 平衡 / 满级上限。

验收重点不是回复更像聊天，而是同一账号下，“材料紧、保守”和“愿意赌高潜力”会被解析成不同的结构化意图，并由 Rust 在明确规则下产生可复现的不同策略结果。

完成后再增加 `record_upgrade_result` Tool，使 Agent 可以参与“推荐 → 录入结果 → 重新规划”的完整闭环。精确评分、概率、排序和三态规则仍不应交给 LLM。

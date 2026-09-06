# 项目设计

本文描述当前范围与后续方向；已冻结的 Demo v0.1、v0.1.1 修订、v0.2 评价链路、当前 v0.3 Agent Runtime 与后续计划分别说明。

## 1. 项目定位与范围

用户已经知道自己要培养哪个角色，并向系统指定该角色。系统结合账号遗器库存、该角色当前配装和本次培养的有限预算，持续回答：

1. 库存中下一件值得为该角色强化的遗器是什么？
2. 当前遗器获得一次强化结果后，应 Continue / Hold / Stop 吗？

一次决策只围绕一个指定角色，在该角色的候选遗器之间比较投入价值。预算是本次培养的输入，系统不决定哪些角色应获得资源。

用户可以主动更换目标，之后按新目标重新评价库存；这不表示系统负责推荐角色培养顺序。其他角色的装备信息可作为“不要拆用”等约束，不用于多角色资源竞争。

## 2. 评价与决策

区分两个层次：

- **遗器自身质量与潜力**：当前有效属性、评分和剩余强化空间，均结合指定角色的需求评价。
- **对指定角色的边际价值**：能否改善当前配装（Build）、补足属性缺口、超越已有替代品，以及收益是否值得消耗本次预算。

账号库存提供候选和比较基线，最终比较的是对指定角色的收益。

### 推荐下一件遗器

输入内部 `AccountState`、用户指定角色、培养约束和剩余预算；比较候选潜力、已有配装与替代品、强化成本，输出有理由的候选排序和下一件推荐。

### 每次强化后重新判断

录入所选遗器的一次强化结果，更新**同一遗器**的等级、属性、预算与历史，再重新评价：

- `Continue`：继续投入当前遗器。
- `Hold`：暂时保留并暂停投入，必要时可重新考虑。
- `Stop`：针对当前角色停止投入该遗器，不表示删除或分解它。

Hold / Stop 后可改推同一目标角色的另一件候选；预算耗尽或无合适候选时允许不推荐。重置类操作不纳入当前决策集合。

## 3. 强化闭环

```text
加载账号 → 用户指定角色、预算与约束
                    ↓
           筛选并评价候选遗器
                    ↓
             推荐下一件遗器
                    ↓
         玩家强化一次并录入结果
                    ↓
 更新同一遗器、预算与历史（AccountState）
                    ↓
       重新评价 → Continue / Hold / Stop
                    ↓
 继续当前遗器 / 改推其他遗器 / 暂停本次投入
```

业务闭环由 Rust core 执行。输入可来自 CLI、Web 表单，也可由 v0.3 LLM Agent 理解自然语言后通过 Tool 调用；LLM 只参与理解、编排与解释。

## 4. 工程结构原则

### Rust core / library

- 承担内部账号模型、数据校验、状态管理、候选排序、强化决策、工具编排与 API 调用流程。
- 对调用者提供结构化输入、结果和错误，业务逻辑能够脱离界面独立调用和测试。
- 不依赖 CLI 命令、终端读写、前端组件或具体 GUI 框架。

### 交互层

- CLI 是当前的一种交互层，负责输入解析、调用 core 和展示结果，不承载评分、排序或决策规则。
- 当前 Web/API 与 CLI 直接复用同一 core；服务层只负责会话访问、交互与传输适配。
- 前端与核心后端解耦，不能通过解析 CLI 展示文本复用业务能力。
- 本次 Web 单页选择原生 HTML/CSS/JavaScript 与 Rust Axum；不因此预设后续完整 GUI 的结构。

### Adapter / Tool 边界

- 第三方项目通过明确的 Adapter / Tool 接入。
- 导入层将外部数据转换为内部 `AccountState`；数值评价通过 `Evaluator` 抽象调用。
- Adapter 处理外部 schema、调用方式和结果转换。业务层不直接依赖第三方对象、网页 UI 或前端状态。
- 数值工具提供评价，Rust Decision Engine 结合目标、候选与预算给出投入决策。

### LLM Agent

v0.3 的 LLM 负责理解用户已指定角色的培养要求、提取当前请求、选择工具并解释结果。工具执行与状态更新流程由 Rust 承担。

例如：“给 Blade 选下一件值得强化的遗器，不要拆其他角色的装备；这件强化到 +9 后还值得继续吗？”

LLM 不决定优先培养哪个角色，不分配多角色资源，也不直接计算遗器评分、伤害或强化收益。

模型配置和 OpenAI-compatible 协议集中在独立 Provider 层；Agent Runtime 只依赖内部 `ModelProvider`、Tool 定义和 `DecisionEngine`。API Key 是服务端内存中的私密字段，公开 DTO 只暴露 `api_key_configured`。真实模型 usage 由 Provider 从响应读取后交给统一 ledger 记账，缺失 usage 的响应会被拒绝，不进行本地 token 估算。

## 5. 第三方项目定位

| 项目 | 接入定位与边界 |
|---|---|
| Fribbels HSR Optimizer | 通过薄 Adapter 提供确定性评价，优先接入遗器评分和强化潜力；目标角色面板、Build 与伤害评价按闭环需要逐步引入。 |
| HSR-Scanner | 已有 v4 fixture 是 Demo 输入；后续完善数据兼容，不集成 OCR 实现。 |
| Reliquary Archiver | 后续真实账号初始化与重新导入的数据源候选，经导入边界转换为内部账号状态。 |
| HSR_Nous | 保留已有调研作为参考，当前无集成计划。 |

Fribbels 已通过 Rust 子进程与薄 Node Adapter 交换自有版本化 JSON，接入 RelicScorer 与 simulateBuild；完整配装搜索不作为最小强化闭环的前置条件。v0.2 不带队友，不扩展为队伍培养规划或四人整体伤害优化。

账号导入后由本项目维护状态，强化结果可由用户录入。实时同步暂不排期。已有源码与接口资料见 [上游索引](research/upstream-index.md) 和 [Fribbels 集成实验](research/fribbels-integration-spike.md)。

## 6. Demo v0.1（已实现并冻结）

Checkpoint：`9d0501c`；旧 Mock 逻辑与测试保留为回归基线。

当前具备：

- 独立 Rust library 与薄 CLI，加载 `fixtures/scanner-v4-demo.json` 并转换为内部模型。
- 用户指定 Blade / Seele 后，MockEvaluator 和 Decision Engine 给出不同的候选排序。
- 一次录入更新同一遗器；根据新状态判断 Continue / Hold / Stop，必要时重新推荐。
- 强化步数预算、内存历史和错误时不提交状态变化的处理。
- 自动演示、交互模式，以及可脱离 CLI 验证的核心测试。

Mock 规则用角色属性偏好估算潜力，按“预计同部位替换收益 / 剩余强化步数”排序；连续无效、低潜力、预算和替代候选影响后续判断。具体规则见 [Demo 说明](hsr-relic-agent/README.md)。

v0.1 没有真实 LLM、Fribbels 调用、真实资源成本或完整 Build 计算。评分与步数是演示代理，不代表游戏中的真实收益；历史暂不保存到磁盘。两个角色用于验证用户更换目标后的排序变化，不涉及角色之间的资源规划。Mock fixture 与测试继续保留作为回归基线。

### v0.1.1：不可操作状态

选择遗器和录入强化结果时，core 返回可匹配的结构化原因，包括遗弃、锁定、满级、预算耗尽、装备于其他角色、当前目标下 Stop，以及 Hold 需要显式恢复等状态。输入结果不匹配当前选择或等级、词条与数值不合法时也分别返回具体原因。CLI 将这些状态转换成操作提示；未来 Web/GUI 可以直接匹配同一类型，无需解析 CLI 文本。

本修订不改变候选排序或 Continue / Hold / Stop 决策逻辑，也不接入新的外部工具。

## 7. Demo v0.2：已接入真实评价

- `FribbelsEvaluator` 与 `MockEvaluator` 共享现有 Evaluator 抽象。Rust 管理参考 Build、候选比较、缓存、超时/取消和状态提交；Adapter 只转换字段并调用确定性计算。
- 自有 JSON v1 传角色/光锥、遗器实际等级与词条、六件 Build IDs 和敌人条件，返回评分/潜力、基础面板与有限的伤害数值。`EvaluationDetails` 保留条件与参考配装，未来界面不需解析 CLI 文本。
- fixture 每个角色仅装备球/绳；缺失部位按可用库存 ID 补齐，并明确标为参考假设，不假装真实现有配装、不自动装备、不搜索最优 Build。
- 排序使用 `max(平均满级潜力 - 参考同部位当前分, 0) / 剩余 +3 步数 × 当前候选 Build 简化普攻伤害 / 当前参考 Build 简化普攻伤害`。原始当前分另行展示，决策用与 potential 同口径的当前分。
- Fribbels 模式低潜力阈值为 20 分；仍按满级、低潜力、连续两次无有效评分增长、预算、单次无效、替代候选高出 25% 等顺序判断。Mock 阈值 4 和原有逻辑不变。这些阈值是 Rust Demo 策略，不是上游推荐或已证明最优的游戏规则。
- 每次观察真正更新同一遗器，再调用 Fribbels；无效响应/超时/取消不提交状态，不自动降级成 Mock。

重要边界：当前旧版 `1205 / 1102` 源码只实现普通攻击与击破，`BASIC` 为 100% ATK 简化普攻；**不是 Blade 生命缩放强化普攻、完整角色输出或 DPS**。它在排序中只是有限的 Build 结构参考，不能把评分潜力推断成未来伤害。角色/光锥只接受 80 级，完整行迹与默认条件为显式假设；不擅自采用 `b1` 角色版本。详细协议、敌人条件和来源见 [Adapter 文档](adapters/fribbels/README.md)。

真实资源成本、游戏 roll 合法性完整验证、持久化、LLM、Reliquary 均未接入。

### Web Demo：复用 v0.2 闭环

`web/` 展示与输入 → `hsr-relic-web/` 薄 API → 现有 Rust DecisionEngine → Evaluator → Fribbels Adapter。无新增排序或决策算法，CLI 不受影响。API 在独立会话中维护模拟账号，以版本号拒绝过期/重复提交；成功生成评价快照后才提交操作，失败/取消保留先前状态。进度显示任务等待秒数，不虚构内部计算百分比。

卡片与页面独立实现；构建时调用上游 `src/lib/rendering/assets.ts` 的 Assets 方法，读取当前 fixture 所需资源映射。前端通过独立 AssetProvider 访问本地图片，不依赖 Fribbels React 组件或 store。上游固定版本、来源文件和图片权利说明见 [Web 文档](web/README.md) 与 [资源致谢](web/credits.html)。未修改第三方源码。

课堂试用暂用内存会话、固定 8 步预算与当前 fixture，没有公网部署、认证或数据库；服务器重启会丢失历史。Web 中的简化普攻口径与 v0.2 一致，不代表完整角色输出。

## 8. 后续版本规划

后续版本均保持“用户已指定单个目标角色”的范围。

### 评价与输入的后续完善

- 优先由输入明确角色技能版本，补齐可靠参考配装，再验证有代表性的单角色伤害动作和未来 Build 收益。
- 使用对照样例校准潜力/伤害加权、Continue / Hold / Stop 阈值，不把 v0.2 启发式当作最终结论。
- 更广泛 scanner 导入、真实强化成本与 Reliquary 按后续任务范围接入。

### v0.3：Minimal Agent Runtime（已实现）

```text
Web UI
  ↓
Rust Web API
  ↓
Agent Runtime ─→ ModelProvider ─→ OpenAI-compatible API
  ↓                    ↓
Agent Tools       response usage → UsageLedger
  ↓
Existing DecisionEngine → Evaluator → Fribbels Adapter
```

- Agent 的首次模型请求强制要求 Tool call；只有至少完成一次 Tool 调用后才接受最终自然语言回复，最多六轮，避免无界循环。
- Tool 层提供设置目标、查询状态、候选排序、下一件推荐和强化历史；其职责只是参数转换和结构化返回。
- Agent 在 `DecisionEngine` 副本上执行，完整成功后才提交。模型或工具链错误不会留下半完成的账号状态；已经收到的真实 usage 仍然记账。
- `ModelConfig` 支持 Endpoint、API Key、Model、Context Length、Reasoning Mode、输入/输出价格和 Token Budget，可由环境变量初始化，也可在 Web 会话内修改。
- Provider 当前适配 OpenAI-compatible Chat Completions function tools。Context Length 在无历史的 v0.3 中作为请求的 `max_completion_tokens` 上限；真正的历史截断策略留给 R5。
- `UsageLedger` 逐次记录响应 ID、模型、input/output/total tokens、时间和按配置价格计算的费用。累计 tokens 达到 budget 后，在下一次模型请求发出前终止；已经完成的确定性工具结果仍可返回。
- Web 展示自然语言输入、最终回复、可展开的 Agent → Tool 事件、模型设置以及累计 usage/cost/budget。模型配置和遗器账号状态属于同一个内存会话。

当前 `AgentEvent` 已表达开始、模型请求、usage、Tool 请求/完成、回复和预算阻断，可作为 R4/R5 的公共事件语义；本轮仍一次性返回 JSON，不是 SSE/WebSocket。`AgentRun` 保留本轮输入、回复和事件，但只保存最近一轮且不落盘。

### R4 / R5 后续

- R4：将 `AgentEvent` 改为实时事件通道，并使取消句柄能够中止正在等待的 HTTP 模型请求；现有轮询进度和请求边界取消不视为 R4 完整完成。
- R5：持久化完整多轮 messages、每轮 `AgentRun`、Tool 输入输出与 `UsageLedger`，支持列出、保存和加载会话；当前仅内存保存遗器历史与最近一次 Agent 轨迹，不视为 R5 完整完成。
- 课程硬性要求继续按 [AGENTS.md](AGENTS.md) 执行，业务范围收缩不取消这些要求。

### 按需考虑，暂不排期

- 在现有 Web Demo 基础上完善 GUI 与持久化。
- 更精细的单角色配装反馈与强化概率模型。
- 账号状态实时同步。

## 9. 当前非目标

- 决定培养哪个角色、推荐角色培养优先级或自动切换培养角色。
- 多角色之间的资源分配、角色资源竞争与全账号培养规划。
- 全账号库存清理建议、重置资源决策。
- HSR_Nous 集成、完整队伍模拟或四人整体伤害优化。
- 自行实现 OCR、自动操作游戏或重写 Fribbels。
- 自行实现完整战斗模拟器、一开始就覆盖所有角色机制、复杂 Multi-Agent 系统。

这些能力不作为当前版本的交付要求；若以后重新纳入，先重新界定范围。

## 10. 下一阶段需确定的问题

1. 如何明确角色技能版本、补全实际配装与行迹，使 Build 伤害指标更有代表性。
2. 如何用对照数据校准真实评分、Build 改善与成本在排序和决策中的作用。
3. 接入真实账号后，为可靠更新状态还需哪些输入与校验。
4. R5 多轮上下文采用何种持久化格式，以及不同模型的上下文窗口如何映射到统一裁剪策略。

自有 JSON v1、结构化评价结果及外部失败回滚已在 v0.2 实现；v0.3 在其上增加独立 Provider、Tool、Agent Event 与 usage ledger，后续继续以现有单目标闭环为基础迭代。

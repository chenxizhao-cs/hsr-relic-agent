# HSR Relic Agent

面向《崩坏：星穹铁道》的遗器管理与强化决策 Agent。

用户已经知道自己要培养哪个角色。系统围绕这个用户指定的目标，结合账号遗器库存、该角色已有配装和本次培养的有限预算，回答两个核心问题：

1. **库存中下一件值得为该角色强化的遗器是什么？**
2. **当前遗器强化到这一阶段后，还值得继续投入吗？**

项目为清华大学 Rust 课程 AI Agent 大作业。

一次只处理一个目标角色的遗器强化。用户可以主动更换目标；系统暂不决定培养哪个角色，也不分配多个角色之间的资源。

## 当前状态

项目目前已实现 **Demo v0.1.1：Rust + MockEvaluator 的最小强化决策闭环**。

已经完成：

- 上游开源项目调研；
- Fribbels 可调用性验证；
- HSR-Scanner v4 模拟账号数据构造与解析验证；
- 初步系统设计；
- 强化决策 Demo 数据准备；
- 独立 Rust library、CLI 演示与核心逻辑测试。
- 可供 CLI 和未来 GUI 复用的结构化遗器不可操作原因。

目前不接 LLM / Fribbels / 真实账号采集，评分和预算仅为可解释的演示规则，不代表真实战斗收益。

```bash
cd hsr-relic-agent
cargo test
cargo run
cargo run -- --interactive
```

操作方法、模块和规则见 [`hsr-relic-agent/README.md`](./hsr-relic-agent/README.md)。

## 核心思路

项目将现有开源工具作为独立能力使用，而不是基于其源码进行增量开发。

Rust core / library 承担账号状态、候选排序、强化事件与决策、工具编排等业务逻辑。CLI 是当前的输入与展示层；后续 Web/API/GUI 应复用同一 core，前端与核心后端保持解耦。

```text
用户指定角色、预算与约束
   ↓ 当前 CLI；后续可由 Web/API/GUI 输入
Rust core：AccountState → 候选排序 → 推荐遗器
   ↑                               ↓
更新同一遗器 ← 录入一次强化结果 ← 玩家强化
   ↓
重新评估 → Continue / Hold / Stop → 必要时改推另一件遗器
```

边界原则：

- core 使用自己的内部模型和结构化结果，可脱离界面独立调用和测试。
- 数值评价通过 `Evaluator` 抽象调用；v0.1 使用 MockEvaluator，Fribbels 后续通过 Adapter / Tool 接入。
- Reliquary / HSR-Scanner 等外部数据通过导入边界转换为 `AccountState`，业务层不直接依赖第三方 schema 或网页状态。
- 后续 LLM 理解已指定角色的培养约束、编排工具并解释结果；确定性计算和状态更新由 Rust core 及其工具完成。
- 当前仅明确复用与解耦原则，具体 GUI 技术栈、HTTP API 和新增目录结构待实际需要时确定。

详细设计见 [`DESIGN.md`](./DESIGN.md)。

## Workspace

```text
hsr-relic-agent-workspace/
├── AGENTS.md
├── DESIGN.md
├── README.md
├── hsr-relic-agent/       # 独立 Rust package：library + CLI + tests
│
├── fixtures/
│   ├── README.md
│   ├── scanner-v4-minimal.json
│   └── scanner-v4-demo.json
│
├── research/
│   ├── upstream-index.md
│   ├── fribbels-integration-spike.md
│   └── spikes/
│
└── upstream/
    ├── hsr-optimizer/
    ├── reliquary-archiver/
    ├── HSR-Scanner/
    └── HSR_Nous/
```

`upstream/` 中均为第三方开源项目，不修改其业务源码。

## 模拟数据

`fixtures/` 中目前提供两套 HSR-Scanner v4 数据：

### `scanner-v4-minimal.json`

用于验证数据格式和 importer。

包含：

- 1 个角色；
- 1 件光锥；
- 3 件遗器。

### `scanner-v4-demo.json`

用于当前强化决策 Demo。

包含：

- Blade；
- Seele；
- 2 件光锥；
- 12 件处于不同强化阶段、具有不同培养价值的遗器。

Blade / Seele 用于验证不同用户指定目标会改变候选排序，不代表系统会推荐角色培养顺序。

两份数据均已经通过当前 Fribbels `KelzFormatParser` 实际解析验证。

详细说明见 [`fixtures/README.md`](./fixtures/README.md)。

## 上游项目

| 项目 | 在本项目中的定位 |
|---|---|
| Fribbels HSR Optimizer | 后续通过 Evaluator Adapter 提供遗器评分、潜力和目标角色 Build 评价 |
| Reliquary Archiver | 真实账号数据导入候选 |
| HSR-Scanner | v4 JSON 数据格式及备用数据导入 |
| HSR_Nous | 保留已有调研作参考，当前无集成计划 |

源码与接口索引见：

[`research/upstream-index.md`](./research/upstream-index.md)

Fribbels 集成实验见：

[`research/fribbels-integration-spike.md`](./research/fribbels-integration-spike.md)

## 下一步

Demo v0.1.1 已使用 Mock 数据和抽象 Evaluator 实现：

```text
加载账号
→ 用户指定目标角色
→ 推荐候选遗器
→ 输入一次强化结果
→ 更新 AccountState
→ Continue / Hold / Stop
→ 必要时改推另一件遗器
```

后续迭代保持单目标角色范围：

1. v0.2：接入 Fribbels Evaluator、完善账号导入，逐步引入目标角色的 Build 收益与真实资源成本，并重新校准决策规则。
2. v0.3：接入该角色强化任务的 LLM 交互与工具编排，完善历史、配置、进度/打断及 Token/费用管理等课程要求。
3. 后续按需增加复用 core 的 Web/API/GUI 交互层。

角色培养优先级、多角色资源分配和全账号库存清理不在当前计划内；重置资源决策、完整队伍模拟和复杂概率模型也暂不排期。

## 开源与课程要求

第三方源码、算法和数据的使用遵循课程 Honor Code。

开发规则与课程约束见 [`AGENTS.md`](./AGENTS.md)。

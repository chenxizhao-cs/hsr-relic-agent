# HSR Relic Agent

面向《崩坏：星穹铁道》的遗器管理与强化决策 Agent。

项目目标是结合玩家当前账号、目标角色、已有配装和有限强化资源，回答两个核心问题：

1. **下一件最值得强化的遗器是什么？**
2. **当前遗器强化到这一阶段后，还值得继续投入吗？**

项目为清华大学 Rust 课程 AI Agent 大作业。

## 当前状态

项目目前已实现 **Demo v0.1：Rust + MockEvaluator 的最小强化决策闭环**。

已经完成：

- 上游开源项目调研；
- Fribbels 可调用性验证；
- HSR-Scanner v4 模拟账号数据构造与解析验证；
- 初步系统设计；
- 强化决策 Demo 数据准备；
- 独立 Rust library、CLI 演示与核心逻辑测试。

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

以下为目标设计；v0.1 中培养目标由 CLI 指定，数值评价暂用 MockEvaluator。

```text
账号数据
   ↓
AccountState
   ↓
用户培养目标
   ↓
LLM Agent
   ↓
Rust Decision Engine
   ↓
Fribbels Evaluator
   ├─ 遗器评分
   ├─ 强化潜力
   ├─ Build 面板
   └─ 目标角色伤害
   ↓
强化决策
   ├─ 选择下一件遗器
   └─ Continue / Hold / Stop
```

其中：

- **LLM Agent**：理解培养目标、约束和偏好，编排工具并解释结果；
- **Rust**：维护账号状态、资源预算和强化历史，并执行核心决策逻辑；
- **Fribbels**：作为确定性数值 Evaluator；
- **Reliquary / HSR-Scanner**：作为账号数据来源。

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

两份数据均已经通过当前 Fribbels `KelzFormatParser` 实际解析验证。

详细说明见 [`fixtures/README.md`](./fixtures/README.md)。

## 上游项目

| 项目 | 当前用途 |
|---|---|
| Fribbels HSR Optimizer | 遗器评分、强化潜力、Build 与伤害评价 |
| Reliquary Archiver | 真实账号数据导入候选 |
| HSR-Scanner | v4 JSON 数据格式及备用数据导入 |
| HSR_Nous | 战斗模拟与 Agent 架构参考 |

源码与接口索引见：

[`research/upstream-index.md`](./research/upstream-index.md)

Fribbels 集成实验见：

[`research/fribbels-integration-spike.md`](./research/fribbels-integration-spike.md)

## 下一步

Demo v0.1 已使用 Mock 数据和抽象 Evaluator 实现：

```text
加载账号
→ 指定培养目标
→ 推荐候选遗器
→ 输入一次强化结果
→ 更新 AccountState
→ Continue / Hold / Stop
→ 重新规划
```

之后再逐步接入：

1. Fribbels Evaluator；
2. Reliquary / HSR-Scanner 真实数据；
3. 库存清理和多角色资源规划；
4. 更完整的战斗收益评价。

## 开源与课程要求

第三方源码、算法和数据的使用遵循课程 Honor Code。

开发规则与课程约束见 [`AGENTS.md`](./AGENTS.md)。

# HSR Relic Agent

面向《崩坏：星穹铁道》的遗器管理与强化决策 Agent。

用户已经知道自己要培养哪个角色。系统围绕这个用户指定的目标，结合账号遗器库存、该角色已有配装和本次培养的有限预算，回答两个核心问题：

1. **库存中下一件值得为该角色强化的遗器是什么？**
2. **当前遗器强化到这一阶段后，还值得继续投入吗？**

项目为清华大学 Rust 课程 AI Agent 大作业。

一次只处理一个目标角色的遗器强化。用户可以主动更换目标；系统暂不决定培养哪个角色，也不分配多个角色之间的资源。

## 当前状态

项目目前已实现 **Demo v0.2：Rust + Fribbels Evaluator 的强化决策闭环**，保留 v0.1 MockEvaluator 作对照。

已经完成：

- 上游开源项目调研；
- Fribbels 可调用性验证；
- HSR-Scanner v4 模拟账号数据构造与解析验证；
- 初步系统设计；
- 强化决策 Demo 数据准备；
- 独立 Rust library、CLI 演示与核心逻辑测试。
- 可供 CLI 和未来 GUI 复用的结构化遗器不可操作原因。
- 真实遗器评分、满级潜力、六件参考 Build 面板与简化普攻指标；Rust 负责最终排序和决策。
- 可供课堂试用的单页 Web Demo：与 CLI 共用 core，支持选择、强化观察、三种判断、预算、历史与重置。

目前不接 LLM / 真实账号采集。评分和潜力来自固定 Fribbels 源码，预算与决策仍为演示启发式。**fixture 对应的旧版 Blade / Seele 在当前上游只有简化普通攻击实现，不能把该伤害指标当作完整实战收益。**

```bash
node adapters/fribbels/build.mjs
cd hsr-relic-agent
cargo test
cargo test --features fribbels-integration
cargo run
cargo run -- --interactive
cargo run -- --mock
```

首次缺少上游依赖时先运行 `npm ci --prefix upstream/hsr-optimizer`。默认模式不会在 Adapter 失败时悄悄退回 Mock。构建、协议与限制见 [Adapter 说明](adapters/fribbels/README.md)。

操作方法、模块和规则见 [`hsr-relic-agent/README.md`](./hsr-relic-agent/README.md)。

### Web 试用

```bash
node web/prepare.mjs
cargo run --manifest-path hsr-relic-web/Cargo.toml
```

打开 <http://127.0.0.1:3000>。同一局域网试用可添加 `-- --lan`，同学使用主机局域网 IP 访问。每个独立会话使用模拟账号；不面向公网部署。启动、观察样例和资源来源见 [Web Demo 说明](web/README.md)。

## 核心思路

项目将现有开源工具作为独立能力使用，而不是基于其源码进行增量开发。

Rust core / library 承担账号状态、候选排序、强化事件与决策、工具编排等业务逻辑。CLI 和当前 Web Demo 共用这一 core，前端与核心后端保持解耦。

```text
用户指定角色、预算与约束
   ↓ CLI 或 Web/API 输入
Rust core：AccountState → 候选排序 → 推荐遗器
   ↑                               ↓
更新同一遗器 ← 录入一次强化结果 ← 玩家强化
   ↓
重新评估 → Continue / Hold / Stop → 必要时改推另一件遗器
```

边界原则：

- core 使用自己的内部模型和结构化结果，可脱离界面独立调用和测试。
- 数值评价通过 `Evaluator` 抽象调用；v0.2 的 FribbelsEvaluator 经独立 Node Adapter 接入，MockEvaluator 保留为显式对照。
- Reliquary / HSR-Scanner 等外部数据通过导入边界转换为 `AccountState`，业务层不直接依赖第三方 schema 或网页状态。
- 后续 LLM 理解已指定角色的培养约束、编排工具并解释结果；确定性计算和状态更新由 Rust core 及其工具完成。
- 当前 Web 采用独立原生前端和薄 Rust API；不预设后续完整 GUI 或 Agent 的具体结构。

详细设计见 [`DESIGN.md`](./DESIGN.md)。

## Workspace

```text
hsr-relic-agent-workspace/
├── AGENTS.md
├── DESIGN.md
├── README.md
├── hsr-relic-agent/       # 独立 Rust package：library + CLI + tests
├── hsr-relic-web/         # Rust Web/API：会话、DTO、传输适配
├── web/                   # 独立单页与 Fribbels 视觉 AssetProvider
├── adapters/fribbels/    # 我们的 JSON ↔ Fribbels 薄 Adapter
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
| Fribbels HSR Optimizer | 已通过 Evaluator Adapter 提供遗器评分、潜力、参考 Build 面板及有限的伤害指标 |
| Reliquary Archiver | 真实账号数据导入候选 |
| HSR-Scanner | v4 JSON 数据格式及备用数据导入 |
| HSR_Nous | 保留已有调研作参考，当前无集成计划 |

源码与接口索引见：

[`research/upstream-index.md`](./research/upstream-index.md)

Fribbels 集成实验见：

[`research/fribbels-integration-spike.md`](./research/fribbels-integration-spike.md)

## 下一步

Demo v0.2 已在同一模拟账号上接入真实 Evaluator，保持：

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

1. 评价迭代：优先明确角色技能版本与完整参考配装，验证更有代表性的单角色伤害指标，再校准当前启发式；真实资源成本与更广泛导入仍待后续。
2. v0.3：接入该角色强化任务的 LLM 交互与工具编排，完善历史、配置、进度/打断及 Token/费用管理等课程要求。
3. 在现有 Web Demo 上按需完善交互与持久化，继续复用同一 core。

角色培养优先级、多角色资源分配和全账号库存清理不在当前计划内；重置资源决策、完整队伍模拟和复杂概率模型也暂不排期。

## 开源与课程要求

第三方源码、算法和数据的使用遵循课程 Honor Code。

开发规则与课程约束见 [`AGENTS.md`](./AGENTS.md)。

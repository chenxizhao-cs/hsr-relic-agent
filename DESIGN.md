# Design v0.2

## 1. 项目定位

本项目是一个面向《崩坏：星穹铁道》的遗器管理与强化决策 Agent。

核心问题不是：

> “这件遗器多少分？”

而是：

> **结合玩家当前账号、目标角色、已有配装和有限强化资源，下一份资源最值得投入到哪件遗器，以及当前正在强化的遗器是否还值得继续投入？**

Agent 应随着每次强化结果持续更新判断，而不是一次性给出静态评分。

---

## 2. 核心思路

系统区分两种价值。

### Intrinsic Relic Value

遗器本身的质量和潜力，例如：

- 当前评分；
- 有效词条；
- 强化潜力；
- 最佳 / 平均 / 最差强化结果；
- 剩余可强化次数。

### Account Marginal Value

这件遗器对于玩家当前账号的实际边际价值，例如：

- 是否能改善目标角色当前 Build；
- 是否解决暴击率、速度等属性缺口；
- 是否已经存在更好的同类遗器；
- 强化成功后是否有机会进入目标 Build；
- 相对于材料成本，预期收益是否值得；
- 与其他待强化遗器相比，当前继续投资它是否仍然最优。

本项目主要关注第二层。

---

## 3. 两类核心决策

本项目需要连续解决两个不同的问题。

### 3.1 强化对象选择

> **下一件应该开始强化哪件遗器？**

系统从库存中筛选候选遗器，结合：

- 遗器自身潜力；
- 当前 Build 缺口；
- 强化后进入目标 Build 的可能性；
- 账号已有替代品；
- 强化资源预算；

给出强化优先级。

### 3.2 强化过程决策

> **当前遗器强化到这个阶段后，还值得继续吗？**

每次 +3 / +6 / +9 / +12 等强化结果出现后重新计算。

可能的决策包括：

- `Continue`：继续强化当前遗器；
- `Hold`：暂时保留，但停止投入资源；
- `Stop`：当前已经不值得继续投入；
- `Reset`：使用重置类资源重新培养（后续功能）。

例如：

```text
推荐遗器 A
    ↓
强化到 +3
    ↓
命中有效词条
    ↓
Continue
    ↓
强化到 +6
    ↓
再次命中有效词条
    ↓
Continue
    ↓
强化到 +9
    ↓
命中无效词条
    ↓
重新比较：
继续强化 A
vs
转而强化 B
    ↓
Stop A / Continue B
```

因此，强化不是一次性决策，而是一个持续更新的序贯决策过程。

---

## 4. 总体流程

```text
账号数据
   ↓
AccountState
   ↓
用户输入培养目标
   ↓
LLM Agent 理解目标和约束
   ↓
分析当前 Build 缺口
   ↓
筛选候选遗器
   ↓
Fribbels Evaluator
   ├─ 当前遗器评分
   ├─ 强化潜力
   ├─ Build 面板
   ├─ 目标角色伤害
   └─ 候选配装
   ↓
Rust Decision Engine
   ├─ Account Marginal Value
   ├─ 强化成本
   ├─ 替代品比较
   └─ 资源预算
   ↓
推荐下一件强化遗器
   ↓
玩家强化一次
   ↓
输入强化结果
   ↓
更新 AccountState
   ↓
重新评价当前遗器
   ↓
Continue / Hold / Stop
   ↓
必要时选择新的强化目标
   ↓
循环
```

---

## 5. 外部项目的定位

### Reliquary Archiver

主要作为账号初始化数据源。

首次导入：

- 遗器；
- 角色；
- 光锥；
- 材料；
- 装备关系等。

导入后由本项目维护自己的 `AccountState`。

遗器强化产生的小规模状态变化可以由用户在本项目中直接录入。

必要时允许重新导入 Reliquary 数据进行同步。

实时 WebSocket 暂时不作为 MVP 必需功能。

### Fribbels HSR Optimizer

作为独立的确定性 `Evaluator Tool`，而不是本项目的开发基础。

计划在外部包装一层薄 Adapter：

```text
Rust
 ↓ JSON
Fribbels Adapter
 ↓
Fribbels calculation modules
 ↓ JSON
Rust
```

主要使用：

- Relic Scorer；
- Current / Future / Potential 等评分；
- Build optimization；
- 角色面板；
- 指定队伍条件下的目标角色伤害。

Fribbels 负责提供确定性的数值评价。

但：

> **“应该继续强化还是止损”由本项目自己的 Rust Decision Engine 决定。**

### HSR-Scanner

作为 Reliquary 的备用数据导入方案。

主要兼容其 v4 JSON，不计划集成 OCR 实现。

### HSR_Nous

暂不作为 MVP 依赖。

未来如果需要：

> 四人完整行动轴 / 战斗总伤害 / 回合收益评价

可以考虑把它作为更高级的 `TeamEvaluator`。

---

## 6. Agent、Rust 与外部工具的边界

### LLM Agent

负责：

- 理解自然语言培养目标；
- 提取硬约束和软偏好；
- 选择需要调用的工具；
- 管理多轮培养计划；
- 根据强化结果调整计划；
- 解释推荐和止损原因。

例如：

> “先养 A，不要拆 B 的装备，材料有限，这件 +9 已经歪两次了，还值得继续吗？”

### Rust

负责：

- `AccountState`；
- 数据校验；
- 候选遗器筛选；
- Build 缺口分析；
- 资源成本；
- 强化事件处理；
- `Account Marginal Value`；
- `Upgrade Decision`；
- 不同候选遗器之间的收益比较；
- Agent Tool orchestration；
- 历史、预算和 API 统计。

### Fribbels 等外部 Tool

负责确定性数值计算。

LLM 不直接计算伤害、遗器分数或强化收益。

---

## 7. 核心 Decision Engine

第一版 Decision Engine 需要支持两个输出。

### Rank Upgrade Candidates

输入：

```text
AccountState
+ Target Character
+ Build Goal
+ Resource Budget
```

输出：

```text
候选遗器强化优先级
```

### Evaluate Current Upgrade

输入：

```text
当前遗器
+ 当前强化等级
+ 当前强化结果
+ 剩余强化潜力
+ 目标 Build
+ 账号其他候选遗器
+ 剩余资源
```

输出：

```text
Continue / Hold / Stop
```

第一版不要求建立完美的概率模型。

可以先综合：

- 当前遗器价值；
- 剩余潜力；
- 强化成功后的 Build 提升；
- 剩余强化成本；
- 其他候选遗器的预期价值；

形成简单、可解释的决策规则。

---

## 8. MVP（Demo v0.1）

第一版不依赖真实游戏。

使用 Mock Account 数据，跑通：

```text
加载模拟账号
→ 用户指定培养目标
→ Agent 分析 Build
→ 筛选候选遗器
→ Evaluator 给出评分 / 潜力
→ 推荐强化遗器 A
→ 用户输入 +3 强化结果
→ 重新评价
→ Continue
→ 用户输入 +6 强化结果
→ 重新评价
→ Stop
→ 自动切换推荐遗器 B
```

第一版最重要的是证明两个闭环：

### 闭环一：强化对象选择

```text
库存
→ 分析
→ 推荐下一件遗器
```

### 闭环二：强化过程止损

```text
强化
→ 获取随机结果
→ 重新评价
→ Continue / Hold / Stop
→ 调整培养计划
```

Mock 数据之后继续保留为测试和答辩 Demo fixture。

---

## 9. 后续迭代

### v0.2

- Reliquary / HSR-Scanner JSON 导入；
- Fribbels Adapter；
- 真实遗器评分；
- Build evaluation；
- 更真实的强化收益计算。

### v0.3

- 库存清理建议；
- 多角色资源竞争；
- 重置资源决策；
- 更复杂的强化概率模型；
- 更完整的配装反馈。

### 后续可选

- Reliquary WebSocket 实时同步；
- HSR_Nous / Team DPS Simulator；
- 更复杂的战斗收益评价；
- 自动根据游戏状态更新强化结果。

---

## 10. 当前暂不做

- 自己实现 OCR；
- 自动操作游戏；
- 完整重写 Fribbels；
- 自己实现完整战斗模拟器；
- 一开始就支持所有角色机制；
- 复杂 Multi-Agent 系统。

---

## 11. 当前最需要继续确定的问题

1. `Account Marginal Value` 第一版具体怎么算；
2. `Continue / Hold / Stop` 的第一版决策规则；
3. MVP 第一批 Rust Tools 有哪些；
4. Fribbels Adapter 的稳定输入输出 schema；
5. 强化后用户需要输入哪些最少信息；
6. Demo 应该选择什么典型账号和强化场景。
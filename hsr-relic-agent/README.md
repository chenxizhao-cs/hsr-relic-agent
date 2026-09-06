# 遗器强化决策 Demo v0.2

独立 Rust package：library 负责导入、评价调用、排序、状态修改与决策，CLI 只负责输入输出。Rust 依赖仍仅为 `serde` / `serde_json`，默认经 Node 子进程调用 Fribbels；`--mock` 保留 v0.1 对照。不接 LLM。

## 运行

从 workspace 根目录进入：

```bash
node adapters/fribbels/build.mjs
cd hsr-relic-agent
cargo test
cargo test --features fribbels-integration
cargo run
cargo run -- --interactive
cargo run -- --mock
```

首次缺少上游依赖时，先在 workspace 根目录执行 `npm ci --prefix upstream/hsr-optimizer`。默认测试不要求 Node；开启 `fribbels-integration` 后额外验证真实工具与故障注入，必须先构建 Adapter。默认运行失败不会隐式退回 Mock。

`cargo run` 自动走完固定演示并退出：比较 Seele / Blade 排序 → 手动选择 `9100002` 强化得到 Continue → 选择 `9100001`，两次真实更新得到 Hold / Stop → 重新推荐其他遗器 → 用户切换 Seele 并观察 `9200002`。这些手动选择用于演示分支，不伪称它们是当时排名第一的候选。

每次运行以编译时嵌入的 [`scanner-v4-demo.json`](../fixtures/scanner-v4-demo.json) 初始化内存账号（2 角色 / 12 遗器），预算 8 步。修改 fixture 后重新 `cargo run` 会重新编译嵌入内容；不会写回 fixture，也不保存本次会话到磁盘。

交互模式输入以下命令可复现全部分支：

```text
target Blade
choose 9100002
upgrade cr 3.24
choose 9100001
upgrade def_pct 5.4
choose 9100001
upgrade def_pct 5.4
target Seele
choose 9200002
upgrade cr 3.24
history
quit
```

其他命令：`rank` 查看排序；`next` 重新推荐并选中；`show` 显示当前遗器与预算；`help` 查看帮助。`target` 接受 Blade / Seele 或 `1205` / `1102`。

`upgrade STAT DELTA` 将**当前选中的同一遗器**升 3 级：三副属性时录入新增第四条，四副属性时录入某个已有属性的增量。`DELTA` 不是最终总值；百分比使用百分点（`cr 3.24` 表示增加 3.24 个百分点，不是 `0.0324`）。可用属性：

| 缩写 | 内部属性 |
|---|---|
| hp / atk / def | 固定生命 / 攻击 / 防御 |
| hp_pct / atk_pct / def_pct | 生命 / 攻击 / 防御百分比 |
| spd / cr / cd | 速度 / 暴击率 / 暴击伤害 |
| ehr / res / be | 效果命中 / 效果抵抗 / 击破特攻 |

Continue 保持当前选择；Hold / Stop 自动选择下一候选（若有）。Hold 可用 `choose ID` 恢复；Stop 在本次会话中对当前目标排除，**不会删除遗器**。切换目标不重置属性、预算或历史。

无法选择或强化时，core 返回 `RelicOperationError`，区分未选目标/遗器、预算耗尽、遗弃、锁定、满级、装备在其他角色身上、当前目标下 Stop、Hold 尚未显式恢复，以及强化结果与当前选择、等级、词条规则或数值不匹配等原因。`select_relic` 返回 `RelicSelection`，CLI 因而能明确显示普通选择或从 Hold 恢复。未来界面可以直接匹配这些类型，不需要解析中文错误文本。

## 模块

```text
src/
├── lib.rs        公共 API、错误类型、fixture 入口
├── model.rs      内部账号、角色、遗器、培养目标、结果、决策与历史
├── import.rs     私有 scanner v4 DTO → AccountState
├── evaluator.rs  Evaluator 抽象与 MockEvaluator
├── fribbels.rs   自有 JSON DTO、真实数值、参考 Build、子进程/缓存/超时/取消
├── decision.rs   候选排序、选中、强化状态更新与 Continue/Hold/Stop
├── tests.rs      不依赖 CLI 的核心单元测试
├── cli.rs        自动演示、命令解析与显示
└── main.rs       薄启动入口
tests/cli.rs      Mock 自动与交互流程回归
tests/evaluator_boundary.rs  无 Node 的边界测试
tests/fribbels.rs  显式 feature 开启的真实工具与故障测试
```

Node Adapter 源码位于 workspace 的 `adapters/fribbels/`，不在第三方仓库内部。

业务 API：`load_scanner_v4(json, steps)` → `DecisionEngine::new(account, evaluator)` → `set_goal(id)` → `recommend_next()` → `apply_upgrade(UpgradeResult)`。调用者可以直接构造内部模型或实现 `Evaluator`，无需启动 CLI。`UpgradeResult.expected_level` 用于拒绝过期/重复结果；CLI 根据当前选中遗器填入。

## v0.2 真实评价与规则

Fribbels 提供：当前原始评分与评级、同口径 current / average / best / worst 强化潜力、参考和候选六件 Build 基础面板及简化普攻指标。Rust 的 `EvaluationDetails` 保留全部这些指标、来源 SHA、敌人条件、参考六件与自动补齐部位。

参考 Build 先用已装备遗器，再按库存 ID 补缺失部位（排除锁定/弃置/已装备他人的补位件）；它不是最优 Build，不改变账号装备。fixture 的 Head / Hands / Body / Feet 都属于补齐假设。缺少可用部位时直接报错。

真实模式的排序为：

`max(average - 参考同部位 current, 0) / 剩余步数 × 候选当前 Build 简化普攻伤害 / 参考当前 Build 简化普攻伤害`

低潜力阈值改为 20（Mock 仍为 4）。下述判定顺序不变，评分增量换为 Fribbels potential 的 current 增量；Build 伤害比参与当前/替代候选的比较。阈值 20、连续两次无效、替代候选高 25% 仍是人工 Demo 策略，未经过最优性校准。平均潜力不是未来真实伤害，当前伤害比也不是完整收益预测。

**伤害限制**：当前 fixture 的未强化版 Blade / Seele 在固定上游仅实现 100% ATK 普通攻击和击破。此处展示/比较 `actionDamage.BASIC` 的暴击期望，尤其不代表 Blade 的生命缩放强化普攻；不是完整角色输出、DPS 或队伍总伤害。默认无队友、95 级单体、有属性弱点且未击破；角色/光锥 80 级、完整行迹和固定上游开关。协议以 `legacy_atk_basic_v1` 标明这个限制，未擅自改用 buffed 版本。

Rust API 使用 `DecisionEngine::new(account, FribbelsEvaluator::new(FribbelsConfig::default()))` 即可，无需 CLI。配置提供敌人条件、Node/bundle 路径、20 秒默认超时、进度回调和取消标记；等待时每秒通知，CLI 可 Ctrl+C。每次批量计算，缓存按完整输入区分，升级后的同 ID 必须重新计算；失败不提交状态。

协议、完整口径与来源见 [Adapter 文档](../adapters/fribbels/README.md)。

## 保留的 v0.1 Mock 对照规则

### Mock 评分

- Blade：HP%、暴击率、暴伤、速度权重 1；Seele：ATK%、暴击率、暴伤、速度权重 1；其他副属性权重 0。
- 归一化单位：暴击率 3、暴伤 6、速度 2.5，其他属性 4。这些是人为选定的 **Demo 单位**，不代表真实 roll 档位。
- Blade 偏好套装 ID `113` / `306`，Seele 偏好 `108` / `309`，单件匹配加 2 分。不计算实际套装效果或件数。
- 主属性匹配时系数 1，否则 0.25。躯干偏好双暴；鞋偏好速度或目标的 HP% / ATK%；球偏好风/量子伤害或对应 HP% / ATK%；绳偏好对应 HP% / ATK%。头/手固定主属性视为匹配。
- `当前分 = (Σ 副属性值 / 单位 × 权重 + 套装加分) × 主属性系数`。
- `剩余步数 = (15 - 等级) / 3`；`平均权重 = (已有副属性权重之和 + 缺失词条数 × 0.5) / 4`。
- `预计满级分 = 当前分 + 剩余步数 × 平均权重 × 主属性系数`。缺失词条的 0.5 是固定启发式，不是概率或实际观察，也不读取 fixture 的 `preview_substats`。

### 排序

只考虑未满级、未锁定、未标记丢弃、未装备于其他角色的遗器；跳过当前目标的 Hold / Stop。预计满级分须至少 4，且能超越目标角色已装备的同部位遗器当前分（无装备时基线为 0）。

`优先级 = max(预计满级分 - 同部位基线, 0) / 剩余步数`，由高到低排序，同分按遗器 ID 排序。锁定装备仍参与基线比较。预算为 0 时没有推荐。

这只是单部位替换收益代理，不构造完整 Build，不自动装备推荐遗器，也不判断能否在剩余预算内升满级。

### 强化后判断（按以下顺序）

1. 已满级：Hold，保留但不再投入。
2. 预计满级分低于 4：Stop。
3. 对当前目标、同一遗器连续两次当前分未增加：Stop；中间一次有效强化会清零连续计数。
4. 预算耗尽：Hold。
5. 本次分数未增加：Hold，可手动恢复观察。
6. 不能超越同部位基线，或另一候选的优先级超过当前的 1.25 倍：Hold。
7. 否则 Continue。

每次有效录入扣 1 个演示步数，按 ID 原位替换遗器状态，并追加包含 before / after、目标、增量、判断与原因的历史。先在暂存账号上验证与重评估，全部成功才提交；非法输入或评估失败不改变账号、预算、历史和选择。

## 仍然保留的边界与后续替换点

- 当前导入层支持现有 fixture 所用的 v4 子集：五星遗器、0/3/6/9/12/15 检查点，显式 `ability_version: 0`。不是完整通用 scanner importer。未知副属性、重复 ID、无效装备关联等返回错误。头/手主属性固定为 HP/ATK，与既有 fixture 约定一致。
- 字段映射只在 `import.rs`，业务层只使用内部 `Stat` / `Slot` / `AccountState`。保留角色/光锥基础信息、套装 ID、装备关联、锁定/丢弃标记；忽略 metadata、gacha、materials、ascension、预览/重掷字段等未消费数据。没有完整行迹、库存材料或主属性数值模型。
- `Evaluator` 已接入确定性评分，Mock 显式保留；后续最值得先明确角色技能版本、完整参考配装和有代表性的伤害动作，再校准策略。
- 步数预算仍是模拟资源，不验证剩余预算能否升满；正增量仍只验证形状、有限值与词条规则，不验证所有游戏 roll 档位/数值上限。真实模式会由上游修正词条精度，并按等级重算主属性，但不把修正后的值反写账号。
- 历史仅在内存；持久化、LLM、真实账号采集和完整课程 R1–R6 外围功能仍未实现。真实模式没有 Mock 评分参与，但输入、成本与策略假设仍是 Demo。

输入格式与素材来源见 [`fixtures/README.md`](../fixtures/README.md)；设计边界见 [`DESIGN.md`](../DESIGN.md)。本轮未复制第三方业务源码，未修改 `upstream/`；Mock 公式是本项目演示规则，不是 Fribbels 算法。

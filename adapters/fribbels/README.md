# Fribbels Adapter v1（Demo v0.2）

只包装 `RelicScorer` 与 `simulateBuild`，不挂载网页、不使用 Optimizer 门面、不保存账号，也不决定推荐或 Continue / Hold / Stop。源码是本项目的协议转换；评分、主属性成长、套装与数值计算由上游完成。

## 构建与验证

在 workspace 根目录执行：

```bash
# 仅首次缺少依赖时需要；保留上游 package-lock，不自动升级或 audit fix
npm ci --prefix upstream/hsr-optimizer
node adapters/fribbels/build.mjs
node upstream/hsr-optimizer/node_modules/typescript/bin/tsc --project adapters/fribbels/tsconfig.json
cd hsr-relic-agent
cargo test
cargo test --features fribbels-integration
cargo run
```

本机验证环境：Node 26.0.0、Cargo 1.97.1。未验证其他 Node 版本；构建目标语法为 Node 22。依赖沿用上游 lockfile，本轮未重新安装/升级。既有依赖安全风险记录见 [Spike](../../research/fribbels-integration-spike.md)，尚未重新审计。

2026-09-06 验收：`cargo test` 24 项通过；启用 `fribbels-integration` 共 37 项通过（含真实数值/闭环及故障注入）；Adapter 构建、TypeScript 检查、Rust clippy 通过。`cargo run` 自动 Demo 与真实交互模式均完成 Continue / Hold / Stop、同件状态更新和改推，并验证锁定/Stop 拒绝操作。并非完整游戏模拟正确性认证。

构建脚本要求上游为干净的 `df630a0488a64eeb740e4e0c14f265d96b9f6f8f`，生成被 Git 忽略的 `dist/adapter.mjs` 和 Fribbels MIT license。运行时只需 Node 与 bundle，不需要网页或网络。分发 bundle 时还需随附其中依赖的适用声明，当前未制作对外分发包。

## JSON 协议：我们自己的 schema_version 1

一次子进程读取一个完整 stdin JSON，EOF 后计算，stdout 返回一个 JSON；日志走 stderr。没有 scanner / Fribbels 对象直通 Rust 业务层。Rust 会验证协议版本、固定 SHA、退出状态、结果完整性和数值。

输入字段：

| 字段 | 内容 |
|---|---|
| `schema_version` | 必须为 `1` |
| `character` | `id`、`level`、`eidolon`、`light_cone: {id, level, superimposition}`；当前只支持未强化版 `1205` / `1102`、角色与光锥均 80 级、同命途光锥 |
| `relics[]` | `id, slot, set_id, rarity, level, main_stat, substats`；五星、+3 检查点，`substats` 是属性到数值的对象 |
| `builds[]` | `{id, relic_ids}`；每个 Build 必须六个不同 ID，六部位齐全；Rust 构造参考 Build 和逐件同部位替换 Build，Adapter 不选择配装 |
| `conditions` | `preset: "solo-default-v1"`、`enemy_level`（1..100）、`enemy_resistance_pct`（0..100）、`elemental_weakness`、`weakness_broken` |

部位：`head, hands, body, feet, sphere, rope`。

属性：`hp, atk, def, hp_percent, atk_percent, def_percent, speed, crit_rate, crit_damage, effect_hit, effect_res, break_effect, energy_regen, healing, physical_damage, fire_damage, ice_damage, lightning_damage, wind_damage, quantum_damage, imaginary_damage`。只有前 12 项可作副属性；主属性需与部位匹配。

所有百分比输入是**百分点**，如 `crit_rate: 3.24`。主属性数值不传，由 Fribbels 按真实等级补齐；不调用主属性满级放大过滤器。不读取或构造预览词条，也不把 potential 当作已发生的强化。

成功响应为 `{schema_version, upstream_commit, ok: true, result: {scores, builds}}`：

- `scores[]`：`id, raw_current_score, rating, current, average, best, worst`。后四项是 `scoreRelicPotential` 的 `currentPct / averagePct / bestPct / worstPct`，即同一量纲的当前与满级潜力。原始当前评分不含相同的主属性惩罚，故单独保留，不能和 potential 混用。百分制分数不是概率，也不保证封顶 100。
- `builds[]`：`id, panel: {hp, atk, def, speed, crit_rate_pct, crit_damage_pct}, damage_model: "legacy_atk_basic_v1", basic_damage`。面板来自基础属性数组，不是行动内加成后的面板；暴击/暴伤已转为百分点。

失败响应为 `{schema_version, upstream_commit, ok: false, error: {code, message}}`，退出码 1。当前 code 为 `evaluation_failed`；无支持能力时明确失败，不回退 Mock。

Rust 对外另提供可序列化的 `EvaluationDetails`，包含上述数值、参考六件、自动补齐部位、同部位基线与敌人条件，供 CLI / 后续 GUI 直接使用。

## 伤害口径与重要限制

`basic_damage` 是上游 `simulateBuild(...).actionDamage.BASIC`，含上游暴击期望和敌人/光锥/套装计算，但**不是完整角色伤害、DPS、四人队伍总伤害或满级潜力对应的未来伤害**。

当前固定源码中 `Blade.ts` / `Seele.ts` 的旧版 ID 只定义 BASIC（100% ATK）和 BREAK，缺少完整技能实现；因此本版尤其不能据此评价 Blade 的生命缩放强化普攻。协议明确标识 `legacy_atk_basic_v1`。本轮尊重 fixture 的 `ability_version: 0`，没有偷偷改用 `1205b1 / 1102b1`。

`solo-default-v1`：无队友、单个敌人、默认 95 级、有属性弱点、未击破、韧性 360、效果抵抗 30%；角色、光锥、套装使用固定上游默认开关，额外战斗 buff 为零。配置抗性默认 20%，但上游在有属性弱点时有效伤害抗性为 0；无弱点时才使用该值。

`generateContext` 直接读取满级基础属性和默认完整行迹，不能诚实支持低等级/部分行迹，因此当前拒绝非 80 级角色/光锥，行迹为明确假设而不是 fixture 观察。星魂与叠影会传入上游，但旧角色缺少的星魂技能机制不会因此补齐。

## Rust 侧规则与可靠性

- 参考 Build 优先使用目标角色已装备遗器；缺失部位按库存 ID 取第一件未装备、未锁定、未弃置的遗器。这不是优化结果，不写回装备关系。缺少某个可用部位则报错，不造假遗器。Hold/Stop 不等于弃置，仍可在参考配装中作保留件。
- 一次批量评价覆盖库存与所有单部位替换；完整输入作为缓存键（最多 4 个快照），避免同 ID 升级后沿用旧分数。上游静态 scorer 每次新建评分缓存，Build 之间隔离可变对象。
- 默认 20 秒超时，1 秒一次等待进度回调；`FribbelsConfig.cancelled` 支持其他交互层打断，CLI 终端支持 Ctrl+C。超时/取消会终止并回收当前子进程。输入输出都有大小界限。
- 子进程、JSON、版本或指标错误均上报；强化状态先暂存，重评估全部成功后才提交账号、预算、选择和历史。

依赖来源：[Fribbels HSR Optimizer](https://github.com/fribbels/hsr-optimizer)，固定 SHA 见上文，MIT，版权 Fribbels 2024。入口索引沿用已有 research；本轮未修改第三方源码。

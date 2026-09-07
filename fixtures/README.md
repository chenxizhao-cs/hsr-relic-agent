# Fribbels scanner v4 fixtures

这两份数据按当前 `upstream/hsr-optimizer` 的以下源码构造：

- `src/lib/importer/kelzFormatParser.tsx`
- `src/lib/importer/importConfig.ts`
- `src/lib/constants/constants.ts`
- `src/data/game_data.json`

固定验证版本：`df630a0488a64eeb740e4e0c14f265d96b9f6f8f`。

## 文件

- `scanner-v4-minimal.json`：保持当前 `ScannerParserJson` v4 类型的完整顶层形状，包含 Blade、其光锥和 3 件遗器。
- `scanner-v4-demo.json`：包含 Blade、Seele、两件光锥和 12 件不同等级/培养价值的候选遗器。
- `reliquary-v4-demo.json`：由真实 Reliquary Archiver v4 导出重复清洗得到的长期 Web Demo，保留 64 个角色、3001 件遗器、391 个光锥和装备/锁定/弃置状态。玩家 UID 和开拓者选择已删除，抽卡资源归零、材料移除，遗器和光锥 `_uid` 已替换。生成规则见 [`adapters/reliquary/README.md`](../adapters/reliquary/README.md)。

Rust Reliquary importer 会从这 3001 件源遗器中接收 2971 件五星、+3 检查点遗器，另 30 件低稀有度或中间强化等级遗器保持在 v4 文件中并计入跳过摘要。Web 内置 Demo 和用户上传原始 JSON 使用同一个 importer。

## Demo 使用顺序

1. 目标设为 Blade（`1205`），在未满级候选中比较强化潜力。
2. `9100001` 是 `+0` 三副属性候选，`preview_substats` 提供当前 parser 支持的第四副属性预览，可用于“推荐强化”步骤。
3. `9100002` 是 `+3` 的正向结果示例；当前实测平均潜力约 `95.38`，适合演示 Continue。
4. `9100003` 是 `+6` 的低价值结果示例；当前实测平均潜力约 `1.99`，适合演示 Stop。
5. 将目标切换到 Seele（`1102`）。`9200002` 是正向结果示例，`9200003` 是停止示例。当前实测平均潜力约为 `92.68` 和 `0`。
6. 同时比较 `9100002` 与 `9200002`：Blade 目标下当前分为 `55.5 / 47.2`，Seele 目标下为 `45.7 / 54.2`，可演示“切换目标后推荐顺序改变”。

这些 ID 表示账号中不同遗器，不是同一遗器的历史快照。Demo 在展示一次实际强化时，应由自己的状态逻辑更新所选遗器的 `level` 和 `substats`；fixture 里的不同阶段遗器用于提供可复核的 Continue/Stop 分支样例。

## 当前 v4 类型中的必填与可省略字段

以下“必填”首先按 `ScannerParserJson`、`V4Parser*` 类型定义；运行时说明则按 `KelzFormatParser` 的实际读取行为。

### 顶层

- 类型必填：`source`、`build`、`version`、`metadata`、`gacha`、`materials`、`characters`、`light_cones`、`relics`。
- `source` 必须匹配 `HSR-Scanner`，`version` 必须为 `4`，否则 parser 明确抛错。
- `build` 在运行时可以为空，但会按 `v0.0.0` 处理并触发过期提示；fixture 使用当前 config 的 `v1.2.0`。
- `metadata` 类型要求 `uid` 和 `trailblazer`；parser 使用 `trailblazer`，当前不读取 `uid`。
- `metadata.current_trailblazer_path` 可省略；parser 默认 `Destruction`。
- `gacha` 类型要求 `stellar_jade` 和 `oneric_shards`。`gacha` 与 `materials` 当前 importer 都不读取，但它们是当前类型的必填顶层字段，因此 fixture 分别保留零值和空数组。

### 角色

- 类型必填：`id`、`name`、`path`、`level`、`ascension`、`eidolon`。
- parser 实际使用 `id`、`level` 和 `eidolon`；`level` 的假值默认 `80`，`eidolon` 的假值默认 `0`。
- `name`、`path`、`ascension` 当前不参与解析结果，但仍按 v4 类型保留。
- `ability_version` 可省略。不过当前游戏数据同时存在 `1205b1` 和 `1102b1`；省略时 parser 会自动采用 buffed 版本。本 fixture 显式设置 `0`，确保解析结果保持 `1205`/`1102`。

### 光锥

- 类型必填：`id`、`name`、`level`、`ascension`、`superimposition`、`location`、`lock`、`_uid`。
- parser 通过 `location === character.id` 关联光锥，实际读取 `id`、`level` 和 `superimposition`；找不到时角色的 `lightCone` 为 `null`。
- `name`、`ascension`、`lock`、`_uid` 当前不参与角色转换，但仍按 v4 类型保留。

### 遗器

- 类型必填：`set_id`、`name`、`slot`、`rarity`、`level`、`mainstat`、`substats`、`location`、`lock`、`discard`、`_uid`。
- `set_id` 必须存在于当前 `game_data.json`；未知套装会被跳过。
- `slot` 去除空格后必须映射到 `Head`、`Hands`、`Body`、`Feet`、`PlanarSphere` 或 `LinkRope`。
- `rarity` 和 `level` 会分别被限制到 `2..5` 与 `0..15`。
- `mainstat` 对 Body、Feet、Planar Sphere、Link Rope 必须命中当前主属性 lookup；Head 和 Hands 的主属性由 parser 固定为 HP/ATK，但当前类型仍要求该字段。
- `substats` 必须是数组；每项的 `key` 和 `value` 被实际读取。HSR-Scanner 的副属性 key 使用 `HP_`、`ATK_`、`CRIT Rate_`、`CRIT DMG_` 等 scanner 名称。
- `location` 为空表示未装备；匹配角色 ID 时会写入 `equippedBy`。
- `_uid` 成为 Fribbels relic ID，并被 `parseInt` 用于 `ageIndex`。
- `name`、`lock`、`discard` 当前普通 importer 不读取，但仍按 v4 类型保留。
- `reroll_substats`、`preview_substats` 可省略。

### 副属性和材料

- 副属性类型必填：`key`、`value`。
- `count`、`step` 可省略。当前 `KelzScannerConfig.speedVerified` 为 `false`，普通 HSR-Scanner 导入不会使用它们验证 roll；`preview_substats.step` 若提供，则用于构造预览 roll 档位。
- 材料类型要求 `id`、`name`、`count`，`expire_time` 可省略；当前 importer 不消费材料数组。

“parser 目前不读取”不表示上游协议永久允许删除该字段。对我们生成的兼容 fixture，仍保留当前 v4 类型声明为必填的字段，只省略源码明确标为可选的字段。

## 验证

外部验证位于 `research/spikes/scanner-v4-fixtures.test.ts`，直接实例化当前 `KelzFormatParser(KelzScannerConfig)`。验证结果：`1 test file / 2 tests passed`。

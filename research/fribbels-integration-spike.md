# Fribbels (`hsr-optimizer`) 独立工具调用 Spike

> v0.2 实现补充：以下历史实验仅证明接口可调用。核对同一 SHA 的角色源码发现，fixture 的旧版 `1205 / 1102` 只定义 BASIC（100% ATK）与 BREAK，不能代表完整角色战斗机制；`generateContext` 此路径也直接使用满级基础属性。当前实现没有切换到 `b1`，明确标注简化普攻、限制 80 级并披露满行迹假设。可复现的正式 Adapter 及边界见 [Adapter 文档](../adapters/fribbels/README.md)，原实验记录保留不变。

## 结论

在当前固定版本上，Fribbels 的**评分与单 Build 模拟能力可以被薄 Node/TypeScript Adapter 包装成 Rust Agent 的独立子进程 Tool**；完整 `Optimizer` 门面则不能直接作为无 UI library 调用。

推荐边界是：

```text
Rust Agent --stdin JSON--> 固定版本的 Node Adapter
           <--stdout JSON-- RelicScorer / simulateBuild / optimizerWorker + BufferPacker
```

- 可直接包装为 Tool：`RelicScorer`、`simulateBuild`，以及经过内部协议适配后的 `optimizerWorker` 计算内核。
- 不建议直接调用：`Optimizer.optimize()`。它返回 `Promise<void>`，候选结果通过 Zustand、表格控制器和 worker 回调写入 UI 状态。
- 适合作为独立 benchmark/evaluator：**有条件适合**。它可作为固定版本、确定性的评分/面板/伤害/Build 搜索基线；不应被描述为游戏真值或与模型假设无关的外部裁判。
- 不需要基于 Fribbels 网页本体二次开发；Adapter 可以完全位于我们自己的目录中。

## 固定版本与验证环境

| 项目 | 值 |
|---|---|
| Repository | <https://github.com/fribbels/hsr-optimizer.git> |
| Commit | `df630a0488a64eeb740e4e0c14f265d96b9f6f8f` |
| License | MIT，见 `upstream/hsr-optimizer/LICENSE.md` |
| Node / npm | Node `v26.0.0`；npm `11.12.1` |
| 实际安装 | `npm ci`，345 packages；Vitest 实装版本 `4.1.6` |
| 上游状态 | 验证前后 `git status --short` 均无 tracked/untracked 输出；仅有已忽略的 `node_modules/` 和既有 `.DS_Store` |

安装时 `patch-package` 报告：为 `i18next@26.0.8` 制作的 patch 被应用到了 lockfile 安装的 `i18next@26.1.0`；应用成功，但这是以后升级或做正式集成时需要单独回归的风险。`npm audit` 还报告 1 个 high severity dependency finding，本轮未执行自动修复，以免改变 lockfile。

## 实际验证结果

实验输入不是 Fribbels 页面或账号存档，而是我们自行构造并经 `JSON.parse` 得到的结构化数据：角色 Blade（`1205`）、光锥 `23009` 和六件遗器。每件遗器只包含 Adapter 需要的 `id`、部位、套装、主属性和副属性。

| 能力 | 实际结果 | 结构化输出 | 独立调用判断 |
|---|---|---|---|
| 自造 JSON 输入 | `RelicAugmenter.augment` 成功生成 6 个内部 `Relic`；随后可评分、压缩并模拟 | 内部 `Relic[]` / `SimulationRelicByPart` | 可行，但 Adapter 必须做字段校验、百分比单位转换和内部补全 |
| `RelicScorer` | 6 件全部成功；样例头部当前分 `88`、评级 `WTF+` | 当前分、评级、当前/最佳/平均/最差潜力、reroll 指标 | 可直接包装 |
| `simulateBuild` | 未挂载 React 页面即可同步运行 | 基础面板、`actionDamage`、`rotationDamage`、`primaryActionStats` | 可直接包装，但必须先生成完整 context |
| `optimizerWorker` | 用 1 个 permutation 同步调用成功，packed buffer 可解码 | 候选行的面板、伤害指标和 permutation id | 计算内核可包装，不是现成公共 API |
| `Optimizer.getFilteredRelics` | 在写入 relic store 后成功返回六个部位各 1 个候选 | 分部位候选数组 | 可用于内部预筛选，但依赖 Zustand store |
| `Optimizer.optimize()` | 用零 permutation 路径实际 `await` 后得到 `undefined` | 无返回结果；结果由 UI/store 副作用传递 | 不能直接当独立 Tool |
| Node JSON Adapter | Vite SSR bundle 后，stdin 输入 JSON、stdout 只输出 JSON，成功返回评分、面板和伤害 | 1,929-byte JSON（本次 fixture） | 可行 |
| Rust 子进程 | 最小 `std::process::Command` 冒烟测试成功，stdin/stdout 传输并检查预期 JSON keys | `status=success bytes=1929` | 可行 |

本次 `simulateBuild` 的代表性输出为：基础 HP `7158.7495`、ATK `1672.7067`、SPD `143.0320`、CR `1.05208`、CD `1.29704`；默认普攻伤害与 combo 均约 `2619.7567`。这里的百分比属性使用 Fribbels 内部小数单位，例如 CR `1.05208` 表示约 `105.208%`。这些数字只用于证明调用链闭合，不作为配装结论。

优化 worker 的同一 Build 解码结果包含 HP `7158.7495`、SPD `143.0320`、CR `1.05208`、CD `1.29704`、COMBO `2619.7566`。worker 使用 `Float32Array` 打包，所以与 `simulateBuild` 的双精度结果存在预期的尾数差异。

## 能力边界与源码入口

### 遗器评分

- `src/lib/relics/scoring/relicScorer.ts`
  - `RelicScorer.scoreCurrentRelic`
  - `RelicScorer.scoreRelicPotential`
  - `ScoringCache`
- `src/lib/state/metadataInitializer.ts`
  - 调用前需要 `Metadata.initialize()`，评分默认值来自当前版本游戏元数据和角色 scoring metadata。
- `src/lib/relics/relicAugmenter.ts`
  - `RelicAugmenter.augment` 将较小的调用方输入补成内部 `Relic`，修正主/副属性数值并计算 roll 信息。

原始 `scoreCurrentRelic` 结果还带有内部 `ScorerMetadata`，其中包含 `Map`，JSON round-trip 会丢失内容。因此 Adapter 应明确投影稳定字段，不能把整个内部对象直接 `JSON.stringify` 后当长期协议。

### Build 面板与伤害

- `src/lib/simulations/simulateBuild.ts`
  - `simulateBuild(relics, context, ...)` 是同步计算函数，本轮已在 jsdom 测试环境和 Node SSR bundle 中脱离页面运行。
- `src/lib/optimization/context/calculateContext.ts`
  - `generateContext(Form)` 生成模拟所需角色、敌人、条件、rotation 和 action context。
- `src/lib/simulations/utils/benchmarkForm.ts`
  - `generateFullDefaultForm` 可补齐角色/光锥默认条件，适合作为 Adapter 构造完整请求的当前入口。
- `src/lib/stores/optimizerForm/optimizerFormConversions.ts`
  - `normalizeForm` 补齐 filter 和单位等默认值。
- `src/lib/relics/relicFilters.ts`
  - `condenseRelicSubstatsForOptimizerSingle` 把普通遗器转成模拟器使用的 numeric stat rows。

`simulateBuild` 的原始返回值包含 `ComputedStatsContainer` 和 typed arrays，也不应直接暴露为 JSON。可稳定投影的字段包括基础面板、动作伤害、rotation 伤害和 primary action stats。

### Build 搜索

- `src/lib/optimization/optimizer.ts`
  - `Optimizer.getFilteredRelics` 可工作，但从全局 relic store 读输入。
  - `Optimizer.optimize` 直接连接 `useOptimizerDisplayStore`、`OptimizerTabController`、AG Grid、消息/弹窗、worker pool 和 WebGPU/CPU 选择；其 async 返回值不包含结果。
- `src/lib/worker/optimizerWorker.ts`
  - `optimizerWorker` 是实际 CPU 枚举内核。本轮在外部测试中提供 `MessageEvent`、`self.postMessage`、context、packed set-solution bitset 和 buffer 后成功执行。
- `src/lib/optimization/bufferPacker.ts`
  - `BufferPacker.createFloatBuffer`、`extractCharacter`、`extractArrayToResults` 定义 worker 的二进制输入/输出边界。
- `src/lib/tabs/tabOptimizer/optimizerTabController.ts`
  - `calculateRelicsFromId` / `calculateRelicIdsFromId` 将 permutation id 还原成六件遗器，但位于 UI controller 中。

因此候选 Build 搜索不是“导入 `Optimizer` 然后 await 返回 JSON”，而是“薄 Adapter 直接调用 CPU worker kernel、解码 buffer、在 Adapter 内维护本次候选数组并还原 permutation id”。这仍然可以很薄，但比评分/单 Build 模拟多一层版本相关的内部协议。

## 为什么不能直接作为 Node library 引入

该仓库的 `package.json` 标记为 `private: true`，没有 `exports`、library build 或 CLI script。源码还使用 `lib/*`、`types/*`、`data/*`、`style/*`、`icons/*` 等 Vite/tsconfig alias。

本轮用 Node 26 的 TypeScript stripping 直接导入 `src/lib/relics/scoring/relicScorer.ts`，实际失败为：

```text
ERR_MODULE_NOT_FOUND: Cannot find package 'lib' imported from .../relicScorer.ts
```

顶层 `Optimizer` 在外部 Vitest 配置中也先后暴露了 `style/*`、`icons/*` alias 依赖。补齐当前 Vite alias 后可以导入并验证门面行为，但这说明普通 `node import` 不是受支持的调用面。

## 推荐调用方式

建议维护一个属于本项目的、版本固定的 Node/TypeScript Adapter：

1. 构建时用 Vite SSR 将需要的 Fribbels 内部模块打成 Node bundle；不要修改上游业务源码。
2. 运行时使用 stdin/stdout JSON。stdout 只写协议数据，诊断信息写 stderr。
3. Adapter 只暴露少量稳定操作，例如 `score_relics`、`simulate_build`、`optimize_builds`；输入输出由我们自己的 schema 控制，不泄漏 `Map`、class、typed array 或 Zustand state。
4. Rust 使用 `std::process::Command` 启动并监管子进程。长搜索可用进程终止实现打断；进度流是否采用 JSON Lines 需在正式开发时再定，本轮未设计。
5. 在结果中记录 Adapter schema version、Fribbels commit、角色/光锥/敌人/条件参数，保证 benchmark 可重放。

本轮临时 SSR bundle 为未压缩约 `1.88 MB`，证明独立分发可行；没有测试冷启动成本、常驻进程模式或大 inventory 性能。

## 主要阻塞点

- 没有公共、稳定、semver 管理的 library/CLI API；所有入口都是内部模块。
- 直接 Node import 无法解析 Vite aliases，需要 bundle 或自定义 loader。
- 评分和模拟依赖 `Metadata.initialize()`、当前 `game_data.json`、角色 scoring presets 与大量完整 `Form` 默认值。
- 百分比在 scanner/普通遗器输入与优化器内部表示之间存在 `100` 倍单位边界，必须由 Adapter 统一。
- `Optimizer.optimize()` 无结构化返回，且与 UI/store/Worker/WebGPU 强耦合。
- worker kernel 的调用协议包含 `ArrayBuffer`、bit-packed set solutions、内部 context 和 permutation id；候选遗器 ID 还原逻辑当前位于 UI controller。
- 原始对象含 `Map`、class 和 typed arrays，不能直接承诺为 JSON schema。
- 当前 i18next patch 版本不完全匹配；上游升级后必须重跑 spike/golden tests。

## Benchmark / evaluator 判断

适合的用途：

- 以固定角色、敌人和条件对候选遗器给出确定性的 Fribbels 分数；
- 对固定 Build 计算面板、动作伤害和 combo 指标；
- 对小型 inventory 枚举候选 Build，作为我们 Rust 编排和决策逻辑的可重放 evaluator；
- 与我们以后自写的计算结果做差分/回归测试。

不应直接承担的含义：

- 它的遗器评分权重和模拟条件是 Fribbels 当前版本的模型与 presets，不是官方游戏“最佳配装”真值；
- 如果我们的 Agent 用同一 evaluator 指标优化，再用它报告效果，不能据此宣称对独立目标的泛化；
- 本轮没有验证所有角色、队友/光锥条件、复杂 rotation、WebGPU 路径或大规模 inventory。

综合判断：**适合作为版本固定的独立确定性 Tool 和项目内 benchmark/evaluator；不适合作为无需注明假设的唯一权威评价标准。**

## 可复核实验

实验文件全部位于我们自己的 `research/spikes/`，未修改 `upstream/hsr-optimizer`：

- `fribbels-integration.test.ts`：四项集成测试；结果为 `4 passed`。
- `fribbels.vitest.config.ts`：外部测试所需 alias 配置。
- `fribbels-adapter-input.json`：自造 JSON fixture。
- `fribbels-adapter-spike.ts` 与 `fribbels-adapter.vite.config.ts`：stdin/stdout Adapter 和临时 SSR 构建配置。
- `fribbels-rust-subprocess-smoke.rs`：Rust 子进程边界冒烟测试。

另外运行了上游现有的 `relicScorer.test.ts` 与 `actionTransform.test.ts`，结果为 `2 files / 14 tests passed`。没有运行完整测试套件、浏览器 E2E 或 WebGPU 测试。

仍待确认：大 inventory 的耗时/内存/进度粒度、worker kernel 的安全并发方式、复杂队伍条件的 JSON schema、上游更新兼容策略，以及是否需要长期常驻 Node 进程。

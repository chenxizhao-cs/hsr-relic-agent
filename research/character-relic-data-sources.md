# 角色—遗器套装数据源与 Fribbels 覆盖审计

> 调研快照：2026-09-07
> Fribbels：`upstream/hsr-optimizer`，commit `df630a0488a64eeb740e4e0c14f265d96b9f6f8f`
> 范围：只读检查当前固定源码，并检索米哈游/HoYoLAB 推荐功能及公开社区接口资料；本轮不修改推荐算法。

## 结论

- Fribbels 当前有 **108 个 `CharacterConfig`**，全部提供角色相关的主属性候选 `scoring.parts` 和副属性权重 `scoring.stats`。
- 其中 **63 / 108（58.3%）** 具有完整 `simulation()`，并同时声明 `relicSets` 和 `ornamentSets`；其余 **45 个**不能从当前 Fribbels 源码直接得到套装候选。
- `simulation.relicSets` 是 Fribbels benchmark 使用的候选组合，不是按数组顺序排列的攻略排名。可用于“允许哪些组合进入评价”，最终优先级仍应由实际 Build 数值决定。
- 游戏客户端静态配置中存在 `AvatarRelicRecommend.json`。当前公开游戏资源快照包含 **93 个不重复角色条目**，每条都有非空的外圈套装、位面套装、主属性和副属性候选。它比 Fribbels 的 simulation 覆盖更适合作为近期的角色—套装适配来源。
- 游戏协议还存在 `GetBigDataAllRecommend` / `GetBigDataRecommend`，可以按角色返回外圈套装、位面套装和四个可变主属性部位的玩家使用百分比。客户端类型明确包含套装 `Percent` 和主属性 `Percent`；因此在线使用率并非只能从截图 OCR。
- 但这些都是社区提取的游戏资源或逆向协议，**不是米哈游发布的公共 API**。本轮仍未找到稳定、可匿名调用的官方 HTTP 数据库；协议字段、命令号和访问条件会随游戏版本变化，公开数据仓库也没有声明许可证。
- 当前最稳妥的近期方案是：先把静态推荐表作为带版本和来源的适配候选；若需要玩家使用率，再由用户在自己的机器上授权本地采集、立刻去标识化，并只把规范化 JSON 交给本项目。官方使用率只说明“常用”，最终数值优劣仍由 Fribbels 评价。

## Fribbels 数据从哪里来

### 类型与初始化

- `src/types/metadata.ts::ScoringMetadata`
  - `parts`：躯干、脚部、位面球、连结绳的主属性候选；
  - `stats`：角色相关副属性权重；
  - `simulation`：可选的 Build benchmark 配置。
- `src/types/metadata.ts::SimulationMetadata`
  - `relicSets`：四件套或 2+2 组合；
  - `ornamentSets`：位面饰品候选；
  - 同时包含模拟主属性、副属性、连招和队友条件。
- `src/lib/state/metadataInitializer.ts::applyCharacterConfig`
  - 把每个 `CharacterConfig.scoring` 合并到运行时角色元数据。
- `src/data/game_data.json`
  - 角色、光锥、套装 ID、套装名称和套装效果等基础数据。

### 角色配置

角色配置位于：

```text
src/lib/conditionals/character/<分组>/<Character>.ts
```

例如：

- `1100/SeeleB1.ts::simulation`：学者四件套、量子四件套及位面候选；
- `1200/BladeB1.ts::simulation`：莳者四件套及位面候选；
- `src/lib/scoring/scoringConstants.ts`：通用条件型候选集合。

不能把 `scoring.presets` 当作套装推荐表。它主要用于自动启用适合的条件效果；旧角色即使有 `presets`，仍可能没有 `simulation.relicSets`。

## 米哈游/HoYoLAB 使用率数据调查

### 已确认

- 米哈游游戏内存在“遗器推荐”界面，会展示角色的常用套装和属性使用情况。HoYoLAB 的工具更新也公开说明加入了 Upgrade Recommendation、遗器详细属性与简要 Build 展示：[HoYoLAB 2.7 Tools Update](https://www.hoyolab.com/article/35366650)。
- 游戏界面的实际操作资料明确展示了“遗器套装使用率”和“属性使用率”这一数据形态：[遗器筛选功能简介](https://news.17173.com/z/xqtd/content/09072024/183616691.shtml)。这能证明数据产品存在，但不能证明存在可公开复用的批量接口。
- 游戏静态资源中的 [`AvatarRelicRecommend.json`](https://github.com/DimbreathBot/TurnBasedGameData/blob/8cdb905dc2f8e6fffa9be4eb07af3e34435d6091/ExcelOutput/AvatarRelicRecommend.json) 提供：
  - `AvatarID`；
  - `Set4IDList`：外圈套装候选；
  - `Set2IDList`：位面套装候选；
  - `PropertyList3` 至 `PropertyList6`：躯干、脚部、位面球和连结绳主属性候选；
  - `SubAffixPropertyList`：副属性候选；
  - `ScoreRankList`：含义尚未从当前源码确认。
  当前固定快照 commit 为 `8cdb905dc2f8e6fffa9be4eb07af3e34435d6091`；静态检查得到 93 个唯一 `AvatarID`，93 条均有非空 `Set4IDList` 与 `Set2IDList`。
- 在线玩家统计存在批量协议：
  - [`GetBigDataAllRecommendCsReq`](https://github.com/Mar7thLover/March7thHoney-OpenSource/blob/88de9bff0863b3239cfee8c005b0eda42d1af69d/Proto/ProtoFile/GetBigDataAllRecommendCsReq.proto) 只需指定 `BIG_DATA_RECOMMEND_TYPE_AVATAR_RELIC`；
  - [`GetBigDataAllRecommendScRsp`](https://github.com/Mar7thLover/March7thHoney-OpenSource/blob/88de9bff0863b3239cfee8c005b0eda42d1af69d/Proto/ProtoFile/GetBigDataAllRecommendScRsp.proto) 的 `avatar_relic` 返回多个角色记录；
  - [`BigDataAvatarRelicRecommend`](https://github.com/Mar7thLover/March7thHoney-OpenSource/blob/88de9bff0863b3239cfee8c005b0eda42d1af69d/Proto/ProtoFile/BigDataAvatarRelicRecommend.proto) 包含 `outer_set_list`、`inner_set_list`，以及球、绳、鞋、衣的主属性列表；
  - 当前客户端类型中的 [`RelicRecommendSuitData`](https://github.com/Z4ee/AnimeSDK/blob/3dfc8baeb886d7907c7771b5f70f9775727300d2/unitysdk/RPG/Client/RelicRecommendSuitData.h) 明确包含 `SetID1`、`SetID2`、`Percent`；[`RelicRecommendPropertyData`](https://github.com/Z4ee/AnimeSDK/blob/3dfc8baeb886d7907c7771b5f70f9775727300d2/unitysdk/RPG/Client/RelicRecommendPropertyData.h) 明确包含 `PropertyType`、`Percent`、`IsExcellent`。
- 上述证据足以确认“批量角色套装/主属性使用率”在游戏数据链路中存在，但并不等于我们已经拥有一个可直接调用的官方 Web API。
- 社区整理的米游社接口中，可以找到：
  - `game_record/app/hkrpg/api/avatar/basic`：账号已有角色；
  - `event/rpgcalc/avatar/list`：养成计算器角色列表；
  - `event/rpgcalc/avatar/detail`：某账号角色的等级、遗器、行迹和光锥详情。
  参考：[MiTool API Document](https://github.com/CainLuo/MiTool/blob/main/API-Document.md)。这些接口是账号/计算器数据，不是角色全服聚合使用率数据库。
- 相关接口通常依赖登录 Cookie、设备信息和签名字段。它们未被米哈游作为本项目可依赖的正式公共 API 文档发布，字段、鉴权和端点都可能变化。

### 尚未确认

- **待确认：** 当前正式服版本的协议字段号和 command ID。不同版本的社区 proto 已出现差异，不能直接硬编码某个旧值。
- **待确认：** 正式服是否允许客户端主动请求全量 `AVATAR_RELIC`，还是只在特定界面/登录阶段下发。
- **待确认：** 嵌套消息中当前版本各 protobuf 字段号到 `SetID1 / SetID2 / Percent` 和 `PropertyType / Percent / IsExcellent` 的准确映射；需要一份本机实际响应验证。
- **待确认：** 国服米游社与国际服 HoYoLAB 是否使用相同数据源、相同统计样本和相同字段。
- **待确认：** 统计的筛选条件，例如玩家等级、角色等级、开拓等级、版本窗口、是否只统计活跃玩家。
- **待确认：** 数据是否给出具体百分比、Top-N，还是只给出排序后的推荐项。
- **待确认：** 自动访问、缓存和再发布聚合数据的服务条款与课程展示边界。

### 接入优先级

1. **先用游戏静态推荐配置。** 它已经覆盖套装、位面、主属性和副属性候选；接入前保留固定 commit、游戏版本和来源说明。由于上游仓库未声明许可证，暂不把其完整原始文件复制到公开仓库，先确认课程展示中的再分发边界。
2. **再做用户授权的本地使用率采集。** 利用现有 Reliquary 的本地抓包能力观察 `GetBigDataAllRecommendScRsp`；新增解析代码应放在我们自己的 Adapter 中，不修改 `upstream/`。原始抓包只留在用户本机，Adapter 仅输出去标识化的统计数据。
3. **最后才用截图/OCR。** 它适合核对一个角色的人眼显示值，不适合维护全量数据库。

在没有完成上述确认前，不应把未文档化米游社端点接进默认 Demo，也不应让用户把 Cookie 交给公开 Web 服务。

### 用户账号辅助验证方案

用户账号只用于验证在线百分比，不用于收集账号私有角色或库存。建议先做一个角色的小样本：

1. 用户在本机启动抓包工具并登录游戏；
2. 打开一个角色的遗器“推荐”界面，触发推荐数据加载；
3. 本地 Adapter 从响应中只保留：游戏区域、游戏版本、采集时间、角色 ID、套装 ID 对、主属性类型、百分比；
4. 丢弃 UID、登录凭据、网络头、设备标识及其他无关封包；
5. 将规范化 JSON 与游戏 UI 显示的一个角色结果人工核对；
6. 只有字段映射确认无误后，才考虑采集全量响应。

不要把账号密码、Cookie、Token 或原始抓包文件发到聊天、提交到 Git，或上传给公开 Demo。当前 `reliquary-archiver` 能捕获和解码游戏数据并导出账号库存，但没有把 `GetBigDataAllRecommend` 写入现有 Fribbels 导出；因此还需要一个独立、很薄的本地解析 Adapter。

## 其他在线数据源

| 来源 | 可提供 | 不能提供 / 风险 | 当前建议 |
|---|---|---|---|
| [DimbreathBot/TurnBasedGameData](https://github.com/DimbreathBot/TurnBasedGameData) | 游戏静态 `AvatarRelicRecommend.json`；93 条角色套装/位面/主副属性推荐 | 社区提取而非官方 API；仓库未声明许可证；不含玩家使用率百分比 | 最有价值的近期适配来源；先固定版本并确认再分发边界 |
| [Kel-Z/HSR-Data](https://github.com/kel-z/HSR-Data) | 套装、部位、角色和光锥基础 JSON；Unlicense | 没有角色—套装适配关系 | 可作为基础 ID/名称来源 |
| [Mar-7th/StarRailRes](https://github.com/Mar-7th/StarRailRes) | 多语言角色、套装、词条和素材索引 | AGPL-3.0；不提供适配关系 | 当前没有必要额外引入 |
| [Mar-7th/StarRailScore](https://github.com/Mar-7th/StarRailScore) | `score.json` 中的角色主属性和副属性权重 | 没有套装候选；仓库页面未声明明确许可证 | 仅用于人工交叉检查 |
| Prydwen 角色攻略 | 人工整理的套装推荐与解释 | 本轮没有找到稳定公开 API 或可复用数据许可证 | 不自动抓取，不作为默认依赖 |

## 建议的数据边界

以后接入时，Rust core 不应直接依赖 Fribbels TypeScript 或米游社响应。建议先转换成我们自己的版本化结构：

```text
CharacterRelicProfile
├── character_id
├── character_variant
├── preferred_main_stats
├── substat_weights
├── relic_set_combinations
├── ornament_sets
├── coverage: complete | partial | unknown
└── provenance
    ├── source
    ├── source_revision
    ├── region
    └── observed_at
```

规则建议：

- `complete`：可用套装候选做强约束；
- `partial`：只做加分或提示，不因缺失而硬排除；
- `unknown`：保持现有评分行为，并在 UI 明确说明套装知识缺失；
- 官方使用率只能说明“常用”，不能直接等同于数学最优；如果同时有 Fribbels Build 计算，应保留两者各自的来源和数值。

## 当前 Demo 的关键缺口

当前 fixture 使用旧 ID：

- Seele：`1102`，只有主属性/副属性配置，没有套装/位面 simulation；
- Blade：`1205`，只有主属性/副属性配置，没有套装/位面 simulation。

Fribbels 中有完整套装配置的是 `1102b1` 与 `1205b1`，但不能未经确认把 B1 配置自动继承给旧角色。新的游戏静态推荐表恰好直接包含旧 ID：

- Seele `1102`：外圈候选 `108 / 122 / 102`，即当前基础数据中的 Genius of Brilliant Stars、Scholar Lost in Erudition、Musketeer of Wild Wheat；没有把 `113` Longevous Disciple 列入候选。
- Blade `1205`：外圈候选 `113 / 110 / 102`，即 Longevous Disciple、Eagle of Twilight Line、Musketeer of Wild Wheat。

因此 Demo 可直接按旧角色 ID 接静态推荐表，不需要借用 B1 配置。数组是否代表严格的优先级，以及未列入是否应做硬过滤，仍应先结合客户端行为和一次真实 UI 样本确认。

## Fribbels 逐角色覆盖清单

表中的路径相对于 `src/lib/conditionals/character/`。“主属性”指 `scoring.parts`，“副属性”指 `scoring.stats`；“套装/位面”只有在可静态确认 `simulation.relicSets/ornamentSets` 时才标记为“是”。

| ID | Symbol | 源文件 | 主属性 | 副属性 | Simulation | 套装 | 位面 |
|---|---|---|---:|---:|---:|---:|---:|
| `1015` | Archer | `1000/Archer.ts` | 是 | 是 | 是 | 是 | 是 |
| `1008` | Arlan | `1000/Arlan.ts` | 是 | 是 | 是 | 是 | 是 |
| `1009` | Asta | `1000/Asta.ts` | 是 | 是 | 否 | 否 | 否 |
| `1002` | DanHeng | `1000/DanHeng.ts` | 是 | 是 | 是 | 是 | 是 |
| `1013` | Herta | `1000/Herta.ts` | 是 | 是 | 是 | 是 | 是 |
| `1003` | Himeko | `1000/Himeko.ts` | 是 | 是 | 是 | 是 | 是 |
| `1005` | Kafka | `1000/Kafka.ts` | 是 | 是 | 否 | 否 | 否 |
| `1005b1` | KafkaB1 | `1000/KafkaB1.ts` | 是 | 是 | 是 | 是 | 是 |
| `1001` | March7th | `1000/March7th.ts` | 是 | 是 | 否 | 否 | 否 |
| `1014` | Saber | `1000/Saber.ts` | 是 | 是 | 是 | 是 | 是 |
| `1006` | SilverWolf | `1000/SilverWolf.ts` | 是 | 是 | 否 | 否 | 否 |
| `1006b1` | SilverWolfB1 | `1000/SilverWolfB1.ts` | 是 | 是 | 否 | 否 | 否 |
| `1004` | Welt | `1000/Welt.ts` | 是 | 是 | 否 | 否 | 否 |
| `1004b1` | WeltB1 | `1000/WeltB1.ts` | 是 | 是 | 是 | 是 | 是 |
| `1101` | Bronya | `1100/Bronya.ts` | 是 | 是 | 否 | 否 | 否 |
| `1107` | Clara | `1100/Clara.ts` | 是 | 是 | 是 | 是 | 是 |
| `1104` | Gepard | `1100/Gepard.ts` | 是 | 是 | 否 | 否 | 否 |
| `1109` | Hook | `1100/Hook.ts` | 是 | 是 | 是 | 是 | 是 |
| `1111` | Luka | `1100/Luka.ts` | 是 | 是 | 是 | 是 | 是 |
| `1110` | Lynx | `1100/Lynx.ts` | 是 | 是 | 否 | 否 | 否 |
| `1105` | Natasha | `1100/Natasha.ts` | 是 | 是 | 否 | 否 | 否 |
| `1106` | Pela | `1100/Pela.ts` | 是 | 是 | 否 | 否 | 否 |
| `1108` | Sampo | `1100/Sampo.ts` | 是 | 是 | 是 | 是 | 是 |
| `1102` | Seele | `1100/Seele.ts` | 是 | 是 | 否 | 否 | 否 |
| `1102b1` | SeeleB1 | `1100/SeeleB1.ts` | 是 | 是 | 是 | 是 | 是 |
| `1103` | Serval | `1100/Serval.ts` | 是 | 是 | 是 | 是 | 是 |
| `1112` | Topaz | `1100/Topaz.ts` | 是 | 是 | 是 | 是 | 是 |
| `1211` | Bailu | `1200/Bailu.ts` | 是 | 是 | 否 | 否 | 否 |
| `1205` | Blade | `1200/Blade.ts` | 是 | 是 | 否 | 否 | 否 |
| `1205b1` | BladeB1 | `1200/BladeB1.ts` | 是 | 是 | 是 | 是 | 是 |
| `1220` | Feixiao | `1200/Feixiao.ts` | 是 | 是 | 是 | 是 | 是 |
| `1208` | FuXuan | `1200/FuXuan.ts` | 是 | 是 | 否 | 否 | 否 |
| `1225` | Fugue | `1200/Fugue.ts` | 是 | 是 | 是 | 是 | 是 |
| `1210` | Guinaifen | `1200/Guinaifen.ts` | 是 | 是 | 否 | 否 | 否 |
| `1215` | Hanya | `1200/Hanya.ts` | 是 | 是 | 否 | 否 | 否 |
| `1217` | Huohuo | `1200/Huohuo.ts` | 是 | 是 | 否 | 否 | 否 |
| `1217b1` | HuohuoB1 | `1200/HuohuoB1.ts` | 是 | 是 | 否 | 否 | 否 |
| `1213` | ImbibitorLunae | `1200/ImbibitorLunae.ts` | 是 | 是 | 是 | 是 | 是 |
| `1218` | Jiaoqiu | `1200/Jiaoqiu.ts` | 是 | 是 | 否 | 否 | 否 |
| `1204` | JingYuan | `1200/JingYuan.ts` | 是 | 是 | 是 | 是 | 是 |
| `1212` | Jingliu | `1200/Jingliu.ts` | 是 | 是 | 否 | 否 | 否 |
| `1212b1` | JingliuB1 | `1200/JingliuB1.ts` | 是 | 是 | 是 | 是 | 是 |
| `1222` | Lingsha | `1200/Lingsha.ts` | 是 | 是 | 否 | 否 | 否 |
| `1203` | Luocha | `1200/Luocha.ts` | 是 | 是 | 否 | 否 | 否 |
| `1224` | March7thImaginary | `1200/March7thImaginary.ts` | 是 | 是 | 是 | 是 | 是 |
| `1223` | Moze | `1200/Moze.ts` | 是 | 是 | 是 | 是 | 是 |
| `1201` | Qingque | `1200/Qingque.ts` | 是 | 是 | 是 | 是 | 是 |
| `1206` | Sushang | `1200/Sushang.ts` | 是 | 是 | 是 | 是 | 是 |
| `1202` | Tingyun | `1200/Tingyun.ts` | 是 | 是 | 否 | 否 | 否 |
| `1214` | Xueyi | `1200/Xueyi.ts` | 是 | 是 | 是 | 是 | 是 |
| `1209` | Yanqing | `1200/Yanqing.ts` | 是 | 是 | 是 | 是 | 是 |
| `1207` | Yukong | `1200/Yukong.ts` | 是 | 是 | 否 | 否 | 否 |
| `1221` | Yunli | `1200/Yunli.ts` | 是 | 是 | 是 | 是 | 是 |
| `1308` | Acheron | `1300/Acheron.ts` | 是 | 是 | 是 | 是 | 是 |
| `1302` | Argenti | `1300/Argenti.ts` | 是 | 是 | 是 | 是 | 是 |
| `1304` | Aventurine | `1300/Aventurine.ts` | 是 | 是 | 否 | 否 | 否 |
| `1307` | BlackSwan | `1300/BlackSwan.ts` | 是 | 是 | 否 | 否 | 否 |
| `1307b1` | BlackSwanB1 | `1300/BlackSwanB1.ts` | 是 | 是 | 是 | 是 | 是 |
| `1315` | Boothill | `1300/Boothill.ts` | 是 | 是 | 是 | 是 | 是 |
| `1305` | DrRatio | `1300/DrRatio.ts` | 是 | 是 | 是 | 是 | 是 |
| `1310` | Firefly | `1300/Firefly.ts` | 是 | 是 | 否 | 否 | 否 |
| `1310b1` | FireflyB1 | `1300/FireflyB1.ts` | 是 | 是 | 是 | 是 | 是 |
| `1301` | Gallagher | `1300/Gallagher.ts` | 是 | 是 | 否 | 否 | 否 |
| `1314` | Jade | `1300/Jade.ts` | 是 | 是 | 是 | 是 | 是 |
| `1312` | Misha | `1300/Misha.ts` | 是 | 是 | 是 | 是 | 是 |
| `1317` | Rappa | `1300/Rappa.ts` | 是 | 是 | 是 | 是 | 是 |
| `1309` | Robin | `1300/Robin.ts` | 是 | 是 | 否 | 否 | 否 |
| `1303` | RuanMei | `1300/RuanMei.ts` | 是 | 是 | 否 | 否 | 否 |
| `1306` | Sparkle | `1300/Sparkle.ts` | 是 | 是 | 否 | 否 | 否 |
| `1306b1` | SparkleB1 | `1300/SparkleB1.ts` | 是 | 是 | 否 | 否 | 否 |
| `1313` | Sunday | `1300/Sunday.ts` | 是 | 是 | 否 | 否 | 否 |
| `1321` | TheDahlia | `1300/TheDahlia.ts` | 是 | 是 | 是 | 是 | 是 |
| `1402` | Aglaea | `1400/Aglaea.ts` | 是 | 是 | 是 | 是 | 是 |
| `1405` | Anaxa | `1400/Anaxa.ts` | 是 | 是 | 是 | 是 | 是 |
| `1407` | Castorice | `1400/Castorice.ts` | 是 | 是 | 是 | 是 | 是 |
| `1412` | Cerydra | `1400/Cerydra.ts` | 是 | 是 | 否 | 否 | 否 |
| `1406` | Cipher | `1400/Cipher.ts` | 是 | 是 | 是 | 是 | 是 |
| `1415` | Cyrene | `1400/Cyrene.ts` | 是 | 是 | 是 | 是 | 是 |
| `1413` | Evernight | `1400/Evernight.ts` | 是 | 是 | 是 | 是 | 是 |
| `1409` | Hyacine | `1400/Hyacine.ts` | 是 | 是 | 是 | 是 | 是 |
| `1410` | Hysilens | `1400/Hysilens.ts` | 是 | 是 | 是 | 是 | 是 |
| `1404` | Mydei | `1400/Mydei.ts` | 是 | 是 | 是 | 是 | 是 |
| `1414` | PermansorTerrae | `1400/PermansorTerrae.ts` | 是 | 是 | 否 | 否 | 否 |
| `1408` | Phainon | `1400/Phainon.ts` | 是 | 是 | 是 | 是 | 是 |
| `1401` | TheHerta | `1400/TheHerta.ts` | 是 | 是 | 是 | 是 | 是 |
| `1403` | Tribbie | `1400/Tribbie.ts` | 是 | 是 | 是 | 是 | 是 |
| `1504` | Ashveil | `1500/Ashveil.ts` | 是 | 是 | 是 | 是 | 是 |
| `1513` | AventurineWaveflair | `1500/AventurineWaveflair.ts` | 是 | 是 | 是 | 是 | 是 |
| `1505` | Evanescia | `1500/Evanescia.ts` | 是 | 是 | 是 | 是 | 是 |
| `1509` | Gilgamesh | `1500/Gilgamesh.ts` | 是 | 是 | 是 | 是 | 是 |
| `1510` | HimekoNova | `1500/HimekoNova.ts` | 是 | 是 | 是 | 是 | 是 |
| `1507` | MortenaxBlade | `1500/MortenaxBlade.ts` | 是 | 是 | 是 | 是 | 是 |
| `1503` | Pearl | `1500/Pearl.ts` | 是 | 是 | 否 | 否 | 否 |
| `1508` | RinTohsaka | `1500/RinTohsaka.ts` | 是 | 是 | 是 | 是 | 是 |
| `1512` | RobinSummeretto | `1500/RobinSummeretto.ts` | 是 | 是 | 是 | 是 | 是 |
| `1506` | SilverWolfLv999 | `1500/SilverWolfLv999.ts` | 是 | 是 | 是 | 是 | 是 |
| `1501` | Sparxie | `1500/Sparxie.ts` | 是 | 是 | 是 | 是 | 是 |
| `1502` | Yaoguang | `1500/Yaoguang.ts` | 是 | 是 | 是 | 是 | 是 |
| `8001` | TrailblazerDestructionCaelus | `8000/TrailblazerDestruction.ts` | 是 | 是 | 是 | 是 | 是 |
| `8002` | TrailblazerDestructionStelle | `8000/TrailblazerDestruction.ts` | 是 | 是 | 是 | 是 | 是 |
| `8009` | TrailblazerElationCaelus | `8000/TrailblazerElation.ts` | 是 | 是 | 否 | 否 | 否 |
| `8010` | TrailblazerElationStelle | `8000/TrailblazerElation.ts` | 是 | 是 | 否 | 否 | 否 |
| `8005` | TrailblazerHarmonyCaelus | `8000/TrailblazerHarmony.ts` | 是 | 是 | 否 | 否 | 否 |
| `8006` | TrailblazerHarmonyStelle | `8000/TrailblazerHarmony.ts` | 是 | 是 | 否 | 否 | 否 |
| `8003` | TrailblazerPreservationCaelus | `8000/TrailblazerPreservation.ts` | 是 | 是 | 否 | 否 | 否 |
| `8004` | TrailblazerPreservationStelle | `8000/TrailblazerPreservation.ts` | 是 | 是 | 否 | 否 | 否 |
| `8007` | TrailblazerRemembranceCaelus | `8000/TrailblazerRemembrance.ts` | 是 | 是 | 否 | 否 | 否 |
| `8008` | TrailblazerRemembranceStelle | `8000/TrailblazerRemembrance.ts` | 是 | 是 | 否 | 否 | 否 |

## 审计方法和限制

- 表格按当前 TypeScript AST 静态识别 `CharacterConfig`、`scoring()` 和 `simulation()`；没有运行每个角色的完整 benchmark。
- 表格只判断字段是否存在，不评价候选套装是否覆盖所有流派，也不把源码顺序解释为强度顺序。
- B1、不同命途和同角色新形态按不同 ID 分开统计。
- 官方使用率接口结论是“本轮未找到”，不是证明接口不存在；如果以后获得合法的客户端响应样本，应重新审计并记录响应版本。

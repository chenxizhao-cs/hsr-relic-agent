# 上游项目源码与接口索引

> 调研快照：2026-09-04
> 范围：`upstream/` 下当前四个独立 Git checkout。
> 方法：只读检查当前源码、清单文件、入口、数据类型和许可证；本轮未构建、未运行测试、未启动游戏、未调用远程接口。
> 用途：回答“以后需要某类能力时，先去哪个仓库、哪个文件、哪个 symbol 找”。本文不是完整架构设计，也不把实现存在等同于已完成运行验证。

## 阅读约定

- 路径均相对于对应上游仓库根目录。
- “可作为 library”分为两种：
  - **公开/明确入口**：清单或包入口明确导出，可直接作为依赖或模块调用；
  - **源码内可导入**：内部 symbol 技术上可复用，但没有稳定公共包/API 承诺。
- 对第三方协议、游戏网络协议、OCR 准确率、远程数据可用性和运行平台支持，只记录源码能确认的事实；其余标为“待确认”。
- 四个上游仓库在调研时均为干净工作树。上游依赖及其许可证没有在本轮逐一审计。

---

## 1. `reliquary-archiver`

### 一句话定位

从游戏网络数据包中恢复账号的遗器、光锥、角色、材料和抽卡资源状态，并输出 Fribbels/HSR-Scanner v4 风格 JSON；也能通过 WebSocket 推送增量更新，是本项目最直接的**高精度账号库存采集候选**。

### 仓库、版本与许可证

| 项目 | 当前值 |
|---|---|
| Repository | <https://github.com/IceDynamix/reliquary-archiver> |
| Commit | `cb109f17a4a15b7604cfe9d078a8735e7735cd25` |
| 包版本 | `0.18.0`（`Cargo.toml`） |
| License | MIT（`LICENSE`、`Cargo.toml`） |
| 工作树 | 干净 |

### 主要语言、技术栈与运行入口

- Rust 2024 edition；异步运行时为 Tokio。
- 命令行：Clap；序列化：Serde/serde_json；网络协议解析依赖 `reliquary` git tag `v23.0.0` 和 protobuf。
- 抓包后端：`pcap`，以及 Windows 可选的 `pktmon`；默认关注 UDP 端口 `23301-23302`。
- 实时服务：Axum WebSocket；GUI：可选 `raxis`，源码限制为 Windows。
- 程序入口：`src/main.rs::main`；常规构建/运行入口是 `cargo build` / `cargo run`。
- `build.rs::main` 在构建时从 Dimbreath 的 turn-based game data 下载并裁剪角色、遗器、光锥、物品和英文 TextMap 数据，再编入二进制。

### 输入、输出与外部接口

**输入**

- 实时抓取游戏 UDP 流量；`src/capture/` 把各抓包后端统一成 `Packet` 流。
- 离线抓包文件：CLI 的 `--pcap`；Windows `pktmon` feature 下还有 `--etl`。README 还声明 GUI 可上传 `.pcap`、`.pcapng`、`.etl`，各格式的完整兼容范围仍需运行验证。
- CLI 参数包括输出路径、超时、日志、WebSocket 端口和 headless 等；详见 `src/main.rs::Args`。
- 构建阶段依赖远程配置文件；运行阶段的协议解析依赖游戏当前网络协议与 `reliquary`。

**输出**

- 单次 JSON：`export::fribbels::models::Export`，`source = "reliquary_archiver"`、`version = 4`。
- 默认文件名形如 `archive_output-<timestamp>.json`。
- WebSocket：默认端口 `23313`，路由 `/ws`；服务端源码绑定 `0.0.0.0:<port>`。
- WebSocket 消息采用 Serde tagged enum：顶层是 `event` 与 `data`，事件见 `OptimizerEvent`：
  `InitialScan`、`UpdateRelics`、`DeleteRelics`、`UpdateLightCones`、`DeleteLightCones`、`UpdateCharacters`、`UpdateMaterials`、`UpdateGachaFunds`、`GachaResult`。

**抓取的数据包到业务数据的关键映射**

- `GetBagScRsp` → 遗器、光锥、材料全量状态。
- `GetAvatarDataScRsp` → 角色与多命途角色状态。
- `PlayerSyncScNotify` → 遗器/光锥/材料/角色/抽卡资源增量与删除事件。
- `PlayerGetTokenScRsp` → UID；`PlayerLoginScRsp` → 星琼/古老梦华；另有抽卡与角色强化版本相关响应。

### 可调用形式

| 形式 | 结论 | 定位 |
|---|---|---|
| GUI | 可用，但源码限定 Windows | `src/rgui/`，`gui` feature |
| CLI | **可直接调用** | `src/main.rs::Args`、`main` |
| WebSocket 服务 | **可直接调用**，启用 `stream` feature 后由 CLI/GUI 启动 | `src/websocket.rs::start_websocket_server`、`/ws` |
| Rust library | **有限可用**：存在 library target，但只公开 `export` 模块，且包 `publish = false` | `src/lib.rs`、`src/export/mod.rs` |
| HTTP REST | 未发现 | WebSocket 路由之外无 REST 业务端点 |

注意：WebSocket server 位于二进制 crate 的私有模块中，不是 `src/lib.rs` 当前公开 API；若以后希望直接嵌入其服务端，需要先评估维护内部 fork 的成本。`Exporter`、`OptimizerExporter` 和导出模型则由 library target 暴露。

### 重要数据类型 / schema

集中位置：`src/export/fribbels/models.rs`。

- `Export`
  - 元信息：`source`、`build`、`version`、`metadata`。
  - 账号数据：`gacha`、`materials`、`light_cones`、`relics`、`characters`。
- `Metadata`：可选 `uid`、可选 `trailblazer`。
- `Material`：字符串化 `id`、`name`、`count`、可选 `expire_time`。
- `Relic`
  - 标识与装备：字符串化 `set_id`、字符串化 `_uid`、`location`、`lock`、`discard`。
  - 数值：`slot`、`rarity`、`level`、`mainstat`、`substats`。
  - 新旧词条：可选 `reroll_substats`、可选 `preview_substats`。
- `Substat`：`key`、浮点 `value`，并保留协议侧 `count` 与 `step`；百分比 stat key 使用 `_` 后缀。
- `LightCone`：字符串化物品 ID/唯一 ID、等级、突破、叠影、装备角色与锁定状态。
- `Character`：ID、命途、等级、突破、星魂、技能、行迹、可选忆灵、`ability_version`。
- `GachaFunds` / `GachaResult` / `PityUpdate`：抽卡资源与本次抽卡带来的相对 pity 变化。

与本项目最相关的一点是：该项目不仅给出展示值，还为副词条保留 `count` / `step`，比纯 OCR 结果更适合做确定性校验；但其语义仍应以当前转换器和上游协议版本共同确认。

当前 `export_proto_material` 会把 `expire_time` 固定为 `None`；字段存在不代表过期时间已经完成采集。

### 源码地图

| 文件 / symbol | 职责；以后何时来找 |
|---|---|
| `src/main.rs::Args`、`CaptureMode`、`capture` | CLI 契约、运行模式和总体抓取流程；需要封装进程调用时先看这里 |
| `src/main.rs::capture_from_pcap`、`live_capture_wrapper`、`live_capture` | 离线/实时数据包如何进入 sniffer 与 exporter |
| `src/capture/mod.rs::{Packet, PacketCapture, CaptureDevice, CaptureBackend}` | 抓包后端抽象；需要替换/增加数据源时查看 |
| `src/capture/pcap.rs::{PcapBackend, PcapCapture}` | libpcap 实时抓包实现 |
| `src/capture/pktmon.rs::{PktmonBackend, PktmonCapture}` | Windows pktmon 抓包实现 |
| `src/export/mod.rs::Exporter` | `GameCommand` → 状态/导出/事件的公共 trait |
| `src/export/fribbels/exporter.rs::OptimizerExporter` | 全量状态容器、协议 command 分派、初始化完成条件、v4 导出 |
| `src/export/fribbels/handlers.rs::OptimizerExporter::*` | 背包、角色、同步、抽卡等具体协议消息处理 |
| `src/export/fribbels/converters.rs::export_proto_relic` | 协议遗器 → v4 遗器；套装、部位、主词条、锁定/弃置/装备关系 |
| `src/export/fribbels/converters.rs::export_substat` | `count` / `step` → 副词条精确值；核对强化 roll 语义时优先看 |
| `src/export/fribbels/converters.rs::{export_proto_material, export_proto_light_cone, export_proto_character, export_proto_multipath_character, export_skill_tree}` | 其余业务对象转换 |
| `src/export/fribbels/models.rs::{Export, Relic, Substat, Material, Character, OptimizerEvent}` | 文件和 WebSocket 的外部 schema 唯一集中定义 |
| `src/export/fribbels/utils.rs` | 游戏内部 stat/slot/path 名称到导出名称的映射 |
| `src/export/database.rs::{Database, get_database}` | 构建期资源在运行时如何加载与查名 |
| `src/websocket.rs::start_websocket_server`、`handle_socket` | `/ws` 生命周期、初始快照、账号选择与增量事件转发 |
| `src/worker.rs::{MultiAccountManager, AccountEvent, WorkerCommand, archiver_worker}` | GUI/多账号模式中的 worker 与账号状态管理 |
| `build.rs` | 构建期远程资源来源、下载文件清单和 TextMap 裁剪 |

### 待确认

- **待确认：** 当前游戏版本/各服务器区域下，`reliquary v23.0.0` 的协议覆盖率与长期稳定性；本轮没有真机抓包。
- **待确认：** 抓取游戏网络数据的课程合规性、游戏服务条款风险及分发边界，需要单独审查，不能仅凭 MIT license 判断。
- **待确认：** `.pcapng` 与 `.etl` 在各 feature / 平台组合中的实际可用性。
- **待确认：** WebSocket 没有在已读路由中看到鉴权；若对非 loopback 暴露，安全边界需单独验证。
- **待确认：** 材料覆盖的是哪些背包条目、强化材料是否全部具名并稳定转换。
- **待确认：** `publish = false` 且 WebSocket 不在 library API 中；直接作为 Rust 依赖时可承诺的 API 稳定性。
- **待确认：** 构建期远程数据下载的可复现性、离线构建方案和各下载源许可证。

---

## 2. `HSR-Scanner`

### 一句话定位

通过 Windows 桌面自动操作与截图 OCR 扫描游戏内遗器、光锥和角色，再导出 v4 JSON 或 SRO JSON；它与网络抓包无关，可作为本项目的**可见 UI 采集备选/校验来源**。

### 仓库、版本与许可证

| 项目 | 当前值 |
|---|---|
| Repository | <https://github.com/kel-z/HSR-Scanner> |
| Commit | `cad008e2509b6b086fd892b140c8b277424fcb03` |
| 导出 build | `v1.5.0`（当前 `scanner.py` 硬编码） |
| License | GNU GPL v3（`LICENSE`） |
| 工作树 | 干净 |

### 主要语言、技术栈与运行入口

- Python；GUI 为 PyQt6。
- 屏幕和图像：mss、Pillow、NumPy、OpenCV；OCR：随包 Tesseract 与 pytesseract 适配；模糊匹配：RapidFuzz/Levenshtein。
- 自动操作：PyAutoGUI、pynput、vgamepad、pywin32；依赖和 PyInstaller 配置明显面向 Windows。
- 源码入口：`src/main.py::main`；构建入口由 `HSR-Scanner.spec` 指向 `src/main.py`，并请求 UAC 管理员权限。
- `src/main.py::HSRScannerUI` 负责配置和生命周期；`ScannerThread` 在单独 Qt 线程中运行异步扫描。

### 输入、输出与外部接口

**输入**

- 游戏客户端可见 UI：源码/README 当前要求英文文本与 16:9 画面；通过键鼠/手柄导航，并对指定区域截图。
- 用户配置：是否扫描遗器/光锥/角色、等级/稀有度过滤、导航/扫描延迟、OCR 并发与批大小、输出目录、是否包含 UID、是否输出 SRO 等。
- 远程元数据：
  - `GAME_DATA_URL` → `kel-z/HSR-Data` v6 的 `game_data_with_icons.json`；
  - `SRO_MAPPINGS_URL` → 同仓库的 `sro_key_map.json`。

**输出**

- 默认 v4 JSON：`source = "HSR-Scanner"`、`build = "v1.5.0"`、`version = 4`，包含 `metadata`、`light_cones`、`relics`、`characters`。
- 可选 SRO v1 JSON：`format = "SRO"`，由 `utils/conversion.py::convert_to_sro` 生成。
- 调试模式可保存中间截图与日志；这不是稳定业务接口。
- 当前源码没有输出材料库存、抽卡资源或实时增量事件。

### 可调用形式

| 形式 | 结论 | 定位 |
|---|---|---|
| Windows GUI / 打包程序 | **主要调用方式** | `src/main.py::main`、`HSR-Scanner.spec` |
| CLI | 未发现面向扫描任务的 CLI 参数入口 | `main()` 直接启动 Qt GUI |
| Python library | **仅源码内可导入**；没有 `pyproject.toml`/`setup.py` 或稳定公共包入口 | `HSRScanner`、各 parser class |
| HTTP / WebSocket 服务 | 未发现 | 当前仓库没有服务端实现 |
| 文件接口 | **可直接消费** | v4 JSON / SRO JSON |

### 重要数据类型 / schema

当前项目以 Python dict 为外部 schema，没有单独的 JSON Schema 文件。

- 顶层 v4：`metadata.uid`、`metadata.trailblazer`、`light_cones[]`、`relics[]`、`characters[]`。
- 遗器：`set_id`、`name`、`slot`、`rarity`、`level`、`mainstat`、`substats`、`preview_substats`、`location`、`lock`、`discard`、扫描期分配的 `_uid`。
- 遗器副词条：`key`、`value`。与 archiver 不同，当前 OCR 导出没有 `count` / `step`。
- 光锥：`id`、`name`、`level`、`ascension`、`superimposition`、`location`、`lock`、扫描期 `_uid`。
- 角色：`id`、`name`、`path`、`level`、`ascension`、`eidolon`、`skills`、`traces`，按命途可有 `memosprite` 或 `skills.elation`。
- 中间截图类型：`src/type_defs/stats_dict.py::{RelicDict, LightConeDict}`；它们是 OCR 前后的内部状态，不是文件格式。
- key 常量集中在 `src/models/const.py`；以后核对字段拼写应先看这里和各 parser 的返回字典。

已知数据质量边界：README 和 parser 都表明 OCR 只能看到游戏显示值；特别是 SPD 副词条隐藏小数不能从显示文本恢复，不能把 OCR 结果当作精确 roll 序列。

### 源码地图

| 文件 / symbol | 职责；以后何时来找 |
|---|---|
| `src/main.py::HSRScannerUI` | GUI 配置、进度、结果保存、开始/停止动作 |
| `src/main.py::{ScannerThread, InterruptListener}` | 后台扫描线程与 Enter 中断；研究实时进度/打断交互时查看 |
| `src/services/scanner/scanner.py::HSRScanner` | 总体导航、截图、OCR 任务编排，最终组装 v4 顶层 JSON |
| `HSRScanner.start_scan`、`stop_scan` | 扫描生命周期和中断入口 |
| `HSRScanner.scan_inventory`、`scan_characters` | 遗器/光锥共享库存扫描流程与角色扫描流程 |
| `src/services/scanner/parsers/parse_strategy.py::BaseParseStrategy` | 光锥/遗器 parser 的公共过滤和解析接口 |
| `src/services/scanner/parsers/relic_strategy.py::RelicStrategy` | 遗器 OCR、元数据匹配、副词条与预览词条拆分、锁定/弃置/装备关系 |
| `src/services/scanner/parsers/light_cone_strategy.py::LightConeStrategy` | 光锥 OCR 与输出字典 |
| `src/services/scanner/parsers/character_parser.py::CharacterParser` | 角色、命途、等级、星魂、技能与行迹解析 |
| `src/models/game_data.py::GameData` | 下载 HSR-Data、名称模糊匹配、稀有度颜色、装备角色头像匹配、SRO 映射 |
| `src/models/const.py` | v4 字段名、配置 key、扫描 key 的集中定义 |
| `src/type_defs/stats_dict.py` | OCR 中间 dict 的 TypedDict |
| `src/utils/screenshot.py::Screenshot` | 截图区域、画面变化检测、抓取性能信息 |
| `src/utils/ocr.py`、`src/utils/ocr_batch.py` | OCR 预处理、单张/批量识别与并发 |
| `src/utils/navigation.py::Navigation` | 键鼠/虚拟手柄操作与画面比例处理 |
| `src/utils/conversion.py::convert_to_sro` | v4 风格 dict → SRO v1；跨工具格式对照时查看 |
| `src/utils/data.py::{save_to_json, resource_path, executable_path}` | 文件保存和 PyInstaller 资源路径 |
| `src/config/*.py` | 各分辨率/页面的截图区域与导航坐标；适配新 UI 时查看 |
| `HSR-Scanner.spec` | Windows 打包入口、随包二进制/图片、管理员权限 |

### 待确认

- **待确认：** 当前游戏 UI 版本下不同分辨率、缩放、窗口模式的识别率和完整扫描耗时；本轮没有真机验证。
- **待确认：** 除 Windows 外的平台可行性；当前依赖和打包配置不能证明跨平台支持。
- **待确认：** `HSR-Data` v6 两个远程 JSON 的版本兼容、可用性和许可证边界。
- **待确认：** README 引用了 `sample_output.json`，但当前 checkout 中未找到该文件；当前完整实例只能从 README 示例和 parser 源码交叉判断。
- **待确认：** v4 schema 没有机器可校验的正式 JSON Schema；字段可选性主要由代码路径决定。
- **待确认：** `_uid` 是扫描顺序生成值，不是游戏内稳定物品 ID；多次扫描间如何稳定匹配需依赖消费者策略验证。
- **待确认：** GPLv3 程序若被嵌入、修改或重新分发，对本项目交付方式的具体影响；应单独做许可证评审。

---

## 3. `hsr-optimizer`

### 一句话定位

Fribbels 的浏览器端配装优化、遗器评分与管理工具，直接消费 HSR-Scanner/reliquary v4 数据，并用 CPU Web Workers 或 WebGPU 枚举配装、计算面板/伤害/治疗/护盾等指标；它是本项目最重要的**数据格式消费者和确定性数值/搜索实现参考**。

### 仓库、版本与许可证

| 项目 | 当前值 |
|---|---|
| Repository | <https://github.com/fribbels/hsr-optimizer> |
| Commit | `df630a0488a64eeb740e4e0c14f265d96b9f6f8f` |
| package version | `1.0.0`，且 `private: true` |
| License | MIT（`LICENSE.md`） |
| 工作树 | 干净 |

### 主要语言、技术栈与运行入口

- TypeScript / TSX；React 19、Vite 8、Zustand、AG Grid、Mantine。
- 优化计算：浏览器 Web Workers、Float32/Float64 buffer、可选 WebGPU/WGSL。
- Node 要求：`>=26.0.0`；npm 要求 `>=11.0.0`。
- 页面入口：`src/index.tsx` → `App`；开发/生产构建入口为 `npm run start`、`npm run build`、`npm run start:prod`。
- 部署脚本把 Vite 静态产物发布到 GitHub Pages；当前仓库不是一个后端服务。

### 输入、输出与外部接口

**输入**

- HSR-Scanner / reliquary-archiver v4 JSON：`KelzFormatParser` 根据 `source` 和 `version` 选择配置并解析。
- reliquary WebSocket：默认连接 `ws://127.0.0.1:23313/ws`，消费前述 tagged events。
- 内部存档 JSON：`HsrOptimizerSaveFormat`，包含 `relics`、`characters`、评分覆盖、会话设置、扫描器设置等。
- 优化请求：`types/form.ts::Form`，主要包含目标角色/光锥、敌人参数、套装/主词条/副词条权重/面板范围过滤、队友与条件开关、连招及结果排序。
- 静态游戏元数据与词条表：`src/data/game_data.json`、`relic_main_affixes.json`、`relic_sub_affixes.json`。

**输出**

- UI 中的候选配装排序、遗器评分/潜力、角色面板和伤害/治疗/护盾/有效生命等结果。
- 内部结果结构：`lib/optimization/bufferPacker.ts::OptimizerDisplayData`；包含基础/战斗面板、套装索引、各技能/连招指标和压缩后的计算数组。
- 浏览器 `localStorage` 的 `state` 项，以及导入/导出的 optimizer save JSON；`SaveState.save()` 会从存档中移除可重算的 `augmentedStats`。
- WebSocket 侧是**客户端**，不会向本项目提供服务端 API。

与核心任务直接相关的外部网络接口主要是本地 scanner WebSocket。仓库还包含 showcase、leaderboard、图片等网络功能，但它们不是遗器库存/配装计算的核心接口，本文不展开。

### 可调用形式

| 形式 | 结论 | 定位 |
|---|---|---|
| Web 应用 | **主要调用方式**；Vite 构建后为静态站点 | `src/index.tsx`、`package.json` scripts |
| WebSocket | **仅客户端**；消费 reliquary `/ws` | `ScannerWebsocketClient.tsx` |
| TypeScript library | **源码内可导入，但不是稳定发布库**：`private: true`，无 package `exports` | `Optimizer`、`RelicScorer` 等内部 export |
| CLI | 没有面向最终用户的优化 CLI；npm scripts 是开发/构建/测试入口 | `package.json` |
| HTTP 服务 | 未发现核心优化 HTTP server | 计算在浏览器 worker/WebGPU 中完成 |
| 文件接口 | **可直接使用** | scanner v4 JSON、optimizer save JSON |

### 重要数据类型 / schema

**扫描器边界**（`src/lib/importer/kelzFormatParser.tsx`）

- `ScannerParserJson`：v4 顶层；在 archiver 情况下还包括 `gacha` 与 `materials`。
- `V4ParserRelic` / `V4ParserSubstat`：兼容 OCR 只有 `key/value`，也兼容 reliquary 的 `count/step`、reroll/preview 副词条。
- `V4ParserCharacter` / `V4ParserLightCone` / `V4ParserMaterial` / `V4ParserGachaFunds`。
- `importConfig.ts` 明确为 HSR-Scanner、Reliquary Archiver 和 YAS 分别配置 `sourceString` 与格式版本；当前两者都按 v4 解析。
- `V4ParserCharacter` 当前只声明并消费角色 ID、名称/命途、等级/突破/星魂与 `ability_version`；scanner/archiver 导出的技能和行迹没有进入 Fribbels 的内部角色 form。

**内部库存与配装**

- `types/relic.ts::Relic`
  - 稳定 ID、套装、部位、星级、强化等级、主副词条、预览词条、装备角色、校验标记；
  - `augmentedStats`、`condensedStats`、`weightScore` 是内部可重算/优化字段。
- `types/character.ts::Character`：角色 ID、当前六部位 `Build`、优化表单 `Form`、保存的 builds。
- `types/savedBuild.ts::{Build, SavedBuild}`：六部位 relic ID 映射；区分 Character 来源与 Optimizer 来源，后者保存队伍、条件、连招和套装条件。
- `types/store.ts::HsrOptimizerSaveFormat`：持久化总格式。
- `scannerStore.ts::ScannerEvent`：与 reliquary 的 `OptimizerEvent` 对齐的前端 union。

**优化输入与结果**

- `types/form.ts::Form`：优化约束与上下文。
- `types/optimizer.ts::OptimizerContext` / `OptimizerAction`：生成后的角色、敌人、技能、队友与条件计算上下文。
- `bufferPacker.ts::OptimizerDisplayData`：搜索结果行的数值 schema。

**materials 边界**

- `V4ParserMaterial` 和 scanner store 能接收/保存材料增量；当前已读核心路径主要把它用于抽卡资源/warp 相关同步，不能据此断言已经提供通用的遗器强化材料预算模型。

### 源码地图

| 文件 / symbol | 职责；以后何时来找 |
|---|---|
| `src/index.tsx` | 浏览器启动、元数据/存档初始化、调试导出对象 |
| `src/lib/importer/importConfig.ts::{ScannerSourceToParser, ReliquaryArchiverConfig, KelzScannerConfig}` | 扫描器 source/version 路由 |
| `src/lib/importer/kelzFormatParser.tsx::{KelzFormatParser, ScannerParserJson}` | v4 外部 JSON → 内部角色/遗器；跨项目格式契约首要入口 |
| `KelzFormatParser.parseRelic`、`readRelicStats` | 外部词条、roll 信息、装备关系 → `Relic` |
| `src/lib/tabs/tabImport/ScannerWebsocketClient.tsx::ScannerWebsocket` | `/ws` 连接、事件反序列化和事件分派 |
| `src/lib/tabs/tabImport/scannerStore.ts::{ScannerStore, ScannerEvent, initialScan}` | 实时全量/增量状态、材料与近期遗器缓存、是否导入 optimizer |
| `src/types/relic.ts::{Relic, RelicSubstatMetadata}` | 内部遗器 canonical type |
| `src/types/character.ts::Character`、`src/types/savedBuild.ts` | 角色当前配装与保存方案 |
| `src/types/form.ts::Form` | 优化器完整请求参数；以后设计我们自己的 tool input 时用于比对维度 |
| `src/lib/optimization/optimizer.ts::Optimizer` | 过滤、排列数、worker 分块、取消、结果收集的总编排 |
| `src/lib/worker/optimizerWorker.ts::optimizerWorker` | CPU worker 内六部位枚举、套装校验、面板/伤害计算与过滤 |
| `src/lib/gpu/webgpuOptimizer.ts`、`src/lib/gpu/wgsl/` | WebGPU 计算路径和 WGSL 生成/执行 |
| `src/lib/relics/relicFilters.ts::RelicFilters` | 装备、强化、星级、角色优先级、主词条、套装、权重等筛选 |
| `src/lib/optimization/relicSetSolver.ts` | 套装组合约束求解与有效排列数相关逻辑 |
| `src/lib/simulations/simulateBuild.ts::simulateBuild` | 给定六件遗器和上下文，执行单 build 的确定性计算 |
| `src/lib/optimization/engine/` | 当前计算容器、damage calculator、config key/tag 等底层数值核心 |
| `src/lib/optimization/calculateStats.ts`、`calculateDamage.ts` | 基础/战斗属性和伤害倍率计算主路径 |
| `src/lib/conditionals/character/`、`lightcone/`、`src/lib/sets/` | 角色、光锥、遗器/位面套装特殊条件实现；查某个机制时按实体找 |
| `src/lib/relics/scoring/relicScorer.ts::{RelicScorer, ScoringCache}` | 当前分、未来分、潜力分和最优分的统一入口 |
| `src/lib/relics/scoring/{currentScore,futureScore,potentialScore,optimalScore,substatScoring}.ts` | 各评分定义的具体计算 |
| `src/lib/relics/statCalculator.ts::StatCalculator` | 满级主词条/副词条值与空属性表 |
| `src/lib/relics/relicRollFixer.ts`、`relicAugmenter.ts` | 外部显示值修正 roll、生成内部可计算字段 |
| `src/lib/services/persistenceService.ts::{loadSaveData, mergeRelics, mergePartialRelics}` | 扫描结果与已有角色/遗器/配装的合并规则 |
| `src/lib/state/saveState.ts::SaveState` | localStorage 与 save JSON 的生成/恢复 |
| `src/data/game_data.json`、`relic_*_affixes.json` | 静态角色/光锥/套装/词条基础数据 |

### 待确认

- **待确认：** 哪些内部 TypeScript API 能在不带 React/Zustand/browser 全局状态的情况下独立调用；当前不是公开 npm library。
- **待确认：** 若从 Rust 调用，是复用网页、启动 JS runtime、抽取算法思想还是另行实现；这属于后续架构决策，本索引不预设。
- **待确认：** CPU worker 与 WebGPU 路径在目标设备上的性能、一致性和浏览器兼容性；本轮未构建/运行 benchmark。
- **待确认：** 扫描器 v4 的长期兼容策略。当前 `importConfig.ts` 中 scanner/archiver 的 `latestBuildVersion` 可能落后于两个本地 checkout，但版本过旧检查与 schema `version` 校验是不同逻辑，实际警告行为需运行确认。
- **待确认：** 静态 `game_data.json` 与各词条表的生成来源、更新流程、版本锁定和许可证链。
- **待确认：** 材料数据在 warp planner 之外的覆盖范围；尚未看到通用遗器强化材料成本/库存决策接口。
- **待确认：** MIT 主许可证之外，仓库内特别标注的移植/衍生算法与数据资产是否有额外 attribution 要求。

---

## 4. `HSR_Nous`

### 一句话定位

以 StarRailRes/Fandom/Hakushin 等数据为基础，提供角色/遗器静态数据加载、YAML 仿真 schema、战斗编译器/模拟器及 LangChain 多 Agent 编排；与本项目最相关的是**数据管线、战斗数值模拟和 Agent/编译器分层参考**，但当前并没有接入前述 v4 个人遗器库存。

### 仓库、版本与许可证

| 项目 | 当前值 |
|---|---|
| Repository | <https://github.com/pzc2004/HSR_Nous> |
| Commit | `efd7415f7e72190acbaefc705551ecdf4ccf7000` |
| 包版本 | `0.1.0`（`pyproject.toml`、`hsr_nous.__version__`） |
| License | MIT（`LICENSE`） |
| 工作树 | 干净 |

### 主要语言、技术栈与运行入口

- Python `>=3.10`，Hatchling 打包；核心依赖包括 LangChain、langchain-openai、httpx、tenacity、python-dotenv。
- 战斗配置/模板使用 YAML；大量核心数据类型为 dataclass 或对 dict 的 typed view。
- 已注册 CLI：`hsr-data-update = hsr_nous.pipeline.update:main`。
- 可直接作为 Python 包导入：`hsr_nous.pipeline`、`hsr_nous.sim`、`hsr_nous.sim.compile`、`hsr_nous.account`。
- `agent/main.py` 另有交互式 Agent CLI，但它位于包外并引用平行的 `agent/` 实现，没有注册为 project script。
- `python -m hsr_nous.pilot` 是自动战斗/dry-run CLI；与本项目的遗器管理核心关系较弱，而且源码明确提示真实模式有服务条款/封号风险，本文不把它列为候选集成主路径。

### 输入、输出与外部接口

**静态数据管线**

- `hsr-data-update` 从 `Mar-7th/StarRailRes` 拉取角色、技能、行迹、突破、星魂、光锥、遗器套装/单件、主副词条、属性、命途、元素等 JSON 到本地缓存。
- pipeline 还包含 Fandom 技能/敌人提取、Hakushin/其他关卡数据路径和红线过滤；这些是独立数据源，具体来源见 `pipeline/` 与 `docs/INTEGRATIONS.md`。
- `pipeline.loader` 的公共函数输入通常为字符串 ID/name、语言、可选 `data_dir`，输出以字符串 ID 为 key 的 dict 或组装后的 dict。

**战斗模拟**

- 高层输入：`build.yaml` + `stage.yaml` 文本/字典，以及模板根。
- `compile_encounter_yaml` 输出不可变 `CompiledEncounter`。
- `CombatEngine.from_compiled(...).run()` 输出 `BattleState`；核心结果包括总伤害、按角色伤害、回合/AV、行动历史和截断标记。
- `sim.montecarlo.run_distribution` 接收 engine factory，输出 `DistributionStats`（均值、标准差、分位数、最大最小值、截断局数）。

**账号接口**

- `account.client` 是 HoYoLAB/米游社非官方 HTTP thin wrapper，凭据来自 keyring 或 `HSR_NOUS_HOYO_*` 环境变量。
- 当前输出 `AccountSnapshot`：角色、开拓力、忘却之庭记录；`OwnedCharacter` 只有装备光锥和遗器套装 ID 列表，不含每件遗器的主副词条、锁定状态或材料库存。
- 端点、签名和字段稳定性在源码中明确被视为不稳定；不能替代 scanner/archiver 的完整 inventory。

**Agent 编排**

- `hsr_nous.api.orchestrator.Orchestrator.run(user_goal)` 顺序调用 Planner、Builder、Search、Evaluator、Explainer，输出文本报告并保留各阶段文本。
- `agents/tools/` 把数据查询和战斗模拟包装为 LangChain tools。
- LLM 工厂读取 `OPENAI_MODEL`、`OPENAI_API_BASE`、`OPENAI_API_KEY`。

### 可调用形式

| 形式 | 结论 | 定位 |
|---|---|---|
| Python library | **明确可导入** | `hsr_nous.pipeline`、`hsr_nous.sim`、`hsr_nous.account` |
| 数据更新 CLI | **已注册** | `hsr-data-update` |
| Agent CLI | 有源码脚本，但未注册为包命令，且走平行的 `agent/` 目录 | `agent/main.py` |
| Pilot CLI | 可用但不属于遗器管理主路径；默认 dry-run | `python -m hsr_nous.pilot` |
| HTTP client | 有，调用外部 HoYoLAB/米游社非官方端点 | `account/client.py` |
| HTTP / WebSocket server | 未发现 | 无 FastAPI/Flask/uvicorn/WebSocket 服务端入口 |
| YAML / 文件接口 | **明确存在** | StarRailRes JSON、本地模板、build/stage YAML |

### 重要数据类型 / schema

**静态原始数据**

- `raw_schema.character.Character`：`id/name/tag/rarity/path/element/max_sp/ranks/skills/skill_trees` 等。
- `raw_schema.relic::{RelicSet, Relic}`：套装描述/属性；单件的 `set_id/rarity/type/max_level/main_affix_id/sub_affix_id`。
- `raw_schema.relic_affix::{RelicMainAffix, RelicSubAffix}`：affix group 与各 property 的 base/step 数据。
- `raw_schema.character_promotion.CharacterPromotion`：各突破阶段属性值与 `materials`；这是培养需求数据，不是账号材料库存。

**账号快照**

- `account.models::OwnedCharacter`：账号角色等级、突破、星魂、装备光锥、`relic_set_ids` 和原始响应。
- `AccountSnapshot`：UID、昵称/开拓等级字段、开拓力、角色、忘却之庭记录、原始响应；当前 client 并未填满所有声明字段。

**仿真输入**

- `sim_schema.actor::StatBlock`：HP/ATK/DEF/SPD、暴击、击破、效果命中/抵抗、穿透/易伤、能量、治疗/护盾、属性增伤/抗性/弱点/韧性等。
- `sim_schema.actor::Actor`：单位身份、等级、属性、技能 ID/等级、召唤归属和命途。
- `sim_schema.action::Action`：行动类型、目标类型、倍率、能量、战技点、削韧、多段、自定义资源、modifier 等。
- `sim_schema.encounter::{Encounter, Cycle, TerminationConfig}`：队伍/敌人/策略、轮次 AV 与结束条件。
- `build.yaml` 的 relic 表示按六个部位记录 `set_id`、`main` 和 `subs` 的 roll 数；这不是具体账号 inventory schema，也没有单件稳定 ID/锁定/弃置/装备状态。

**编译产物与输出**

- `CompiledPolicyRule` / `CompiledPolicy`：预编译条件和目标规则。
- `CompiledStage`：敌人、波次、终止模式和敌人行动。
- `CompiledEncounter`：队伍、全行动表、stage、policy、modifier、状态配置、hooks、自定义资源与表达式编译器。
- `sim.state::{ActorState, BattleState, Modifier, ShieldInstance}`：可序列化运行时状态。

### 源码地图

| 文件 / symbol | 职责；以后何时来找 |
|---|---|
| `src/hsr_nous/pipeline/__init__.py` | 数据层公开 export 清单；找“是否已有查询函数”先看这里 |
| `src/hsr_nous/pipeline/update.py::{CORE_FILES, run_update, main}` | StarRailRes 下载清单、缓存布局和 `hsr-data-update` CLI |
| `src/hsr_nous/pipeline/loader.py::{load_*, get_*, list_*}` | 本地 JSON 加载、按 ID/name 查询、完整角色组装 |
| `pipeline.loader::{calc_character_stats, calc_light_cone_stats}` | 角色/光锥等级属性确定性计算 |
| `pipeline.loader::{calc_relic_main_affix_values, calc_relic_sub_affix_values}` | 从 StarRailRes affix 表计算主词条满级值与副词条高档 roll |
| `src/hsr_nous/raw_schema/` | StarRailRes 各实体的轻量 typed view；核对字段语义时查对应文件 |
| `src/hsr_nous/account/models.py::{OwnedCharacter, AccountSnapshot}` | 账号级数据的当前边界 |
| `src/hsr_nous/account/client.py::{AccountClient, get_account_snapshot}` | HoYoLAB/米游社凭据、端点、容错与响应转换 |
| `src/hsr_nous/adapters/template_generator.py::{generate_character_template, generate_light_cone_template, generate_relic_set_template, generate_enemy_template}` | pipeline 数据 → per-entity YAML 模板 |
| `src/hsr_nous/adapters/template_verifier.py` | 生成模板回读/数值对照 |
| `src/hsr_nous/adapters/character_adapter.py::{adapt_character, adapt_character_by_name}` | raw character → Actor；注意 light cone/relic 参数当前仍为占位 |
| `src/hsr_nous/sim_schema/examples/build.yaml`、`stage.yaml` | 可运行输入形状的最短入口 |
| `src/hsr_nous/sim_schema/actor.py`、`action.py`、`encounter.py` | 仿真核心数据类 |
| `src/hsr_nous/sim_schema/rulebook.yaml` | 公式、模式与遗器 affix 等可执行规则数据 |
| `src/hsr_nous/sim_schema/docs/06_relics.md` | build relic 的词条/roll 口径和当前范围 |
| `src/hsr_nous/sim/compile/__init__.py::{compile_encounter, compile_encounter_yaml}` | build/stage → `CompiledEncounter` 的公开编译入口 |
| `src/hsr_nous/sim/compile/build_compiler.py::BuildCompiler` | 队伍、角色模板、光锥、遗器、策略、hooks 的绑定与严格校验 |
| `src/hsr_nous/sim/compile/stage_compiler.py::StageCompiler` | stage/敌人/波次/终止规则编译 |
| `src/hsr_nous/sim/compile/compiled.py` | 不可变编译产物 schema |
| `src/hsr_nous/sim/engine.py::CombatEngine` | 战斗主循环和各运行时组件协调 |
| `src/hsr_nous/sim/{scheduler,bus,hooks,modifiers,pipeline,resources,policy_api}.py` | 行动调度、事件、效果、modifier、结算、能量和策略的分层实现 |
| `src/hsr_nous/sim/state.py::{ActorState, BattleState}` | 运行快照与最终结果结构 |
| `src/hsr_nous/sim/montecarlo.py::{run_distribution, DistributionStats}` | 多随机种子结果聚合 |
| `src/hsr_nous/agents/tools/{data_tools,sim_tools}.py` | 给 LLM 使用的数据查询与模拟 tool 边界 |
| `src/hsr_nous/agents/llm.py::make_chat_model` | OpenAI-compatible 模型配置 |
| `src/hsr_nous/api/orchestrator.py::Orchestrator` | packaged 代码中的五 Agent 顺序编排 |
| `agent/main.py`、`agent/orchestrator.py` | 包外的旧/平行交互 CLI 路径；与 `src/hsr_nous/` 版本需区别对待 |

### 待确认

- **待确认：** 当前测试在本 checkout 和目标 Python 环境中的通过情况；本轮没有安装依赖或运行 pytest。
- **待确认：** README 所述各机制的数值精度和覆盖率；源码存在不等同于已与游戏逐位验证。
- **待确认：** `build.yaml` 如何从真实 scanner/archiver 单件库存自动生成；当前仓库未发现 v4 scanner adapter。
- **待确认：** `character_adapter.adapt_character` 的 light cone/relic 参数明确仍是占位，旧 adapter 路径不能当作完整配装数值实现。
- **待确认：** account client 的端点、DS 签名和字段在当前 HoYoLAB/米游社环境是否仍可用；源码自己也标注非官方和可能轮换。
- **待确认：** account snapshot 没有单件遗器/材料库存；是否能由合法公开接口补齐。
- **待确认：** `agent/` 与 `src/hsr_nous/agents` 两套实现的权威关系，以及最终应该调用哪一个。
- **待确认：** `hsr_nous.api.__init__` 当前没有 re-export `Orchestrator`，调用方需要直接引用子模块；这是否是有意的公共 API 边界。
- **待确认：** `pipeline/update.py` 的 StarRailRes raw URL 使用 `master`，而 `pipeline/loader.py` 的远程读取 URL 使用 `main`；两个分支名在当前上游是否都有效、是否指向同一数据版本。
- **待确认：** 部分源码使用的 YAML/LangChain 子依赖是否全部由当前 lockfile/传递依赖可靠提供；需要实际安装验证。
- **待确认：** StarRailRes、Fandom、Hakushin 等数据源各自的更新稳定性、使用条款和 attribution 要求。

---

## 跨项目总表

| 项目 | 上游关系 | 数据采集角色 | 数据格式角色 | 数值计算角色 | 可借鉴的工程角色 | 当前最重要边界 |
|---|---|---|---|---|---|---|
| `reliquary-archiver` | 生产 HSR-Scanner/Fribbels v4；实时推送给 `hsr-optimizer` | **网络包全量 + 增量**：遗器、光锥、角色、材料、抽卡资源 | v4 JSON + tagged WebSocket events；副词条带 `count/step` | 只做协议值到导出值的确定性转换，不做配装搜索 | Rust 抓包抽象、Exporter trait、实时事件/多账号状态 | 协议/ToS、平台权限、构建期远程数据、公共 API 稳定性 |
| `HSR-Scanner` | 生产同族 v4；文件被 `hsr-optimizer` 消费 | **桌面 OCR**：遗器、光锥、角色 | v4 JSON；可选 SRO v1；无 materials/gacha/增量 | OCR 识别与基础格式转换，不做配装优化 | 进度、后台线程、中断、截图/OCR pipeline | Windows/英文/16:9 依赖，SPD 隐藏小数，GPLv3，随机 `_uid` |
| `hsr-optimizer` | 明确消费前两者；不生产账号原始数据 | 不采集游戏本体；从文件和本地 WebSocket ingest | **v4 的实际消费者/canonical 适配参考**；另有内部 save schema | **单件评分、潜力、筛选、六件套枚举、面板与技能指标** | Worker/WebGPU 并行、取消/进度、状态合并与持久化 | 私有前端应用而非 library/service；算法与 UI/browser 状态耦合程度待确认 |
| `HSR_Nous` | 使用 StarRailRes/Fandom/Hakushin；当前未接前两者 v4 | HoYoLAB 非官方 API 只能取部分角色/战绩/体力；不是完整 inventory | StarRailRes dict/raw schema + build/stage/template YAML + compiled schema | **基础属性、affix、战斗编译/模拟、Monte Carlo**；真实单件库存接线缺失 | 数据/adapter/schema/sim/agent 分层、严格编译校验、事件驱动模拟 | Python 而非 Rust；v4 adapter 缺失；账号/遗器 adapter 不完整；实现精度未在本轮验证 |

### 四者关系速查

```text
游戏账号
  ├─ 网络数据包 ──> reliquary-archiver ──┬─ v4 JSON ───────────────┐
  │                                      └─ /ws 增量事件 ─────────┤
  └─ 可见 UI ─────> HSR-Scanner ──────────── v4 JSON ───────────────┤
                                                                  v
                                                        hsr-optimizer
                                                        解析 / 管理 / 评分 / 搜索

StarRailRes + Fandom + Hakushin ──> HSR_Nous
                                    静态数据 / YAML schema / 战斗模拟 / Agent 参考

当前缺口：v4 真实单件库存 ──X──> HSR_Nous 的 build/sim 输入（尚无现成 adapter）
```

### 按未来问题反查入口

| 以后要解决的问题 | 第一查找位置 | 第二查找位置 |
|---|---|---|
| 如何拿到真实遗器唯一 ID、精确副词条 roll、材料数量？ | `reliquary-archiver/src/export/fribbels/` | `hsr-optimizer/src/lib/importer/kelzFormatParser.tsx` |
| 不抓包时怎样从游戏 UI 导出？ | `HSR-Scanner/src/services/scanner/` | `HSR-Scanner/src/config/` |
| v4 文件/实时事件到底怎样被消费者解释？ | `hsr-optimizer/.../kelzFormatParser.tsx` | `hsr-optimizer/.../scannerStore.ts` |
| 如何筛选遗器并枚举六件套？ | `hsr-optimizer/src/lib/relics/relicFilters.ts` | `hsr-optimizer/src/lib/optimization/optimizer.ts`、`relicSetSolver.ts` |
| 如何给单件遗器算当前/未来/潜力分？ | `hsr-optimizer/src/lib/relics/scoring/relicScorer.ts` | 同目录各 `*Score.ts` |
| 如何算角色/光锥基础属性与词条表？ | `HSR_Nous/src/hsr_nous/pipeline/loader.py` | `hsr-optimizer/src/lib/relics/statCalculator.ts` |
| 如何把 build 编译成可复现战斗输入？ | `HSR_Nous/src/hsr_nous/sim/compile/` | `HSR_Nous/src/hsr_nous/sim_schema/examples/` |
| 如何跑确定性战斗/多随机种子评估？ | `HSR_Nous/src/hsr_nous/sim/engine.py` | `HSR_Nous/src/hsr_nous/sim/montecarlo.py` |
| 如何设计长任务进度与取消？ | `hsr-optimizer/src/lib/optimization/optimizer.ts`、`worker/` | `HSR-Scanner/src/main.py::{ScannerThread, InterruptListener}` |
| 如何设计 Agent 的工具边界？ | `HSR_Nous/src/hsr_nous/agents/tools/` | `HSR_Nous/src/hsr_nous/api/orchestrator.py` |

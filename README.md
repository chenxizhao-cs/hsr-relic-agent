# HSR Relic Agent

一个面向《崩坏：星穹铁道》的遗器强化决策 Agent，也是清华大学 Rust 课程项目。

你先告诉系统准备培养哪位角色；系统会从 Reliquary 账号或内置脱敏 Demo 的遗器库存中推荐下一件值得强化的遗器。每次录入强化结果后，它会更新同一件遗器并重新判断：

- **Continue**：继续强化当前遗器；
- **Hold**：暂时保留，先观察其他候选；
- **Stop**：对当前角色停止投入，切换到其他遗器。

内置 Demo 来自真实 Reliquary v4 账号的脱敏副本，包含 64 个角色、3001 件源遗器和 391 个光锥。当前强化模型实际接收其中 2971 件五星、+3 检查点遗器；页面会明确显示其余 30 件被跳过。真实 Fribbels 闭环已用 Blade（刃）验证，其他角色需要角色和已装备光锥均达到 80 级才会在页面中开放选择。

## 几分钟启动 Web Demo

### 1. 准备环境

需要：

- 支持 Rust 2024 Edition 的稳定版 Rust；
- Node.js 22 或更高版本；
- Git。

项目不会把 1 GB 以上的第三方源码提交到本仓库。首次使用时，在项目根目录下载固定版本的 Fribbels：

```bash
mkdir -p upstream
git clone https://github.com/fribbels/hsr-optimizer.git upstream/hsr-optimizer
git -C upstream/hsr-optimizer checkout df630a0488a64eeb740e4e0c14f265d96b9f6f8f
npm ci --prefix upstream/hsr-optimizer
```

Adapter 会检查上游 commit 和工作区是否干净，不会修改 Fribbels 业务源码。

当前固定版上游在 `npm ci` 时可能显示 `i18next` patch 版本警告和 npm audit 提示；本项目已验证这些提示不阻止 Demo 构建和启动。不要直接在这个固定上游目录运行 `npm audit fix`，否则可能改动 lockfile，导致 Adapter 的干净工作区检查失败。

### 2. 准备 Adapter 和页面资源

```bash
node web/prepare.mjs
```

这一步会生成固定版本的游戏静态推荐数据库、构建薄 Fribbels Adapter，并从上游 Assets 层生成当前 Demo 使用的角色、遗器和属性图片映射。

### 3. 启动

```bash
cargo run --manifest-path hsr-relic-web/Cargo.toml
```

浏览器打开：<http://127.0.0.1:3000>

停止服务时按 `Ctrl+C`。如果 3000 端口被占用，可以先设置 `HSR_WEB_PORT=3001`。

## Windows 获取真实账号 JSON

1. 从 [Npcap 官网](https://npcap.com/)安装 Npcap。安装时勾选 `Install Npcap in WinPcap API-compatible Mode`；使用 Wi-Fi 时再勾选 `Support raw 802.11 traffic (and monitor mode) for wireless adapters`。
2. 从 [Reliquary Archiver Releases](https://github.com/IceDynamix/reliquary-archiver/releases/)下载最新版 `reliquary-archiver-pcap-x64.exe`。
3. 启动游戏并停在列车登录画面的 `Click to Start`，先不要进入游戏。
4. 运行 Reliquary，等待它显示 `Waiting for login...`。此时 `Export not ready` 是正常状态。
5. 回到游戏点击 `Click to Start` 并完整进入账号。成功后 Reliquary 会显示 `Connected!`，角色、遗器和光锥数量应不再为 0。
6. 点击 Reliquary 的 Export / Download，保存得到的 `archive_output-日期时间.json`，然后在本项目页面点击“导入 Reliquary JSON”。

若启动 Reliquary 前已经进入游戏，请退出登录后重试。原始 JSON 含账号 UID、资源数量和物品内部 ID，不要提交到 GitHub 或公开分享。更多排查和隐私说明见 [完整 Windows 教程](docs/reliquary-import-windows.md)。

## 怎么试玩

### 方式一：直接操作强化工作台

1. 点击“加载示例账号”，或选择“导入 Reliquary JSON”上传自己的 `archive_output-*.json`；
2. 查看角色、遗器、光锥和装备关系导入摘要；
3. 在左侧选择一个当前评价器可用的目标角色；
4. 中间会先按目标角色的游戏静态套装推荐筛选，再显示 Rust Decision Engine 排序后的遗器候选；
5. 点击一件遗器，查看当前评分、副属性和平均满级潜力；
6. 选择本次变化的副属性并填写增量；
7. 点击“记录强化结果”，查看 Continue / Hold / Stop、原因和新的推荐；
8. 需要重新开始时，点击右上角“重置试用”。

输入的是**本次增加量**，不是强化后的总值。百分比属性填写百分点，例如暴击率增加 `3.24`。

内置真实规模 Demo 的推荐和判断取决于所选角色、遗器当前状态与录入的强化结果，不预设必然得到某一种判断。若要快速、可重复地观察 Continue / Hold / Stop 三条分支，可运行后文的 CLI Mock 对照模式；它继续使用小型教学 fixture。

### 方式二：使用自然语言 Agent

点击右上角“模型设置”，配置 OpenAI-compatible 模型，然后在页面上方输入：

> 我想培养 Blade，材料比较紧，帮我看看下一件最值得强化什么。

Agent 会先把自然语言转换为受控的结构化培养意图：目标角色、材料压力、风险倾向和培养目标。Rust 校验这些枚举后选择保守、均衡或高潜力策略，再由同一个 `DecisionEngine` 计算候选。没有表达额外偏好时使用原来的均衡策略。

- 保守：强调当前质量，并加重剩余强化步数的成本；
- 均衡：保持原来的“平均满级正收益 / 剩余步数 × Build 比率”；
- 高潜力：参考 Fribbels best 上限，并弱化剩余步数惩罚。

实际完成一次强化后，可以在同一对话继续输入：

> 刚才那件从 +3 升到 +6，暴击率增加了 3.24。

Agent 会调用 `record_upgrade_result`；Rust 更新同一件遗器、预算和历史，重新计算 Continue / Hold / Stop，再让 Agent 根据 Tool Result 等待下一次观察或切换候选。增量必须是游戏中实际看到的精确增加量；“出了防御”“歪了一次生命”等没有区分固定值/百分比或缺少数值的描述不会被猜测，Agent 应先读取当前状态并追问。

页面会通过 SSE 实时追加模型请求、提取后的结构化意图、Rust 采用的策略、Tool 进度、确定性决策和真实 Token usage；任务运行时可以点击“取消本次计算”。如果角色缺失或关键偏好无法可靠归类，Agent 可以先读取当前状态再追问。

LLM 不自己计算遗器分数，也不重新实现候选排序。完整链路是：

```text
Web UI
  → Rust Web API
  → Agent Runtime
  → Agent Tools
  → Rust Decision Engine
  → Fribbels Evaluator
```

Fribbels 提供确定性评分和 Build 数值；LLM 负责把用户语言转换为有限偏好并选择工具；Rust 将偏好映射到受控策略，计算候选顺序及 Continue / Hold / Stop。模型不能提交评分、权重、阈值或决策覆盖。

## 模型设置与 Token 费用

Web 设置支持：

- API Endpoint；
- API Key；
- Model；
- Context Length；
- Reasoning Mode；
- Input / Output Token Price；
- Token Budget。

API Key 只保存在当前 Rust 服务的内存会话中，不写入浏览器存储或普通 API 响应。价格由使用者按自己的服务填写，单位是“所选货币 / 1M tokens”；默认价格为 0，项目不会内置可能过期的价格。

每次成功的模型响应必须包含真实 input/output usage。系统不会自行估算 Token；累计用量达到预算后，会在下一次模型请求发出前停止。

## 实时进度、历史与 Session JSON

每次 Agent 任务都会产生统一的结构化 Trace。Web 实时显示当前阶段和真实等待时间，不虚构百分比；同一批事件在任务结束后进入“Agent 会话与任务历史”，包括用户消息、模型调用、Tool Call、Tool Result、Rust 决策、usage、回复、错误或取消原因。

页面中的“保存 JSON”会下载版本化的完整会话，包含：

- 多轮模型 messages；
- 全部 Agent 任务及事件 Trace；
- 遗器账号状态、目标、当前选择、强化历史与预算；
- usage、费用和不含密钥的模型配置。

“加载 JSON”可以恢复这些内容并继续对话。API Key 有意不写入文件；加载后沿用目标 Web 会话当前服务端保存的 Key，必要时请重新在模型设置中填写。

取消不是前端隐藏结果：请求会传播到 Agent Runtime；等待模型时会中止 HTTP future，Fribbels 计算时会终止 Node 子进程。取消前已经完成的 Trace、usage 和 Tool 状态会保留。

也可以在启动前通过环境变量设置默认值：

```bash
export HSR_LLM_API_ENDPOINT="https://api.openai.com/v1"
export HSR_LLM_API_KEY="<your-api-key>"
export HSR_LLM_MODEL="<model-id>"
export HSR_LLM_CONTEXT_LENGTH="4096"
export HSR_LLM_REASONING_MODE="disabled"
export HSR_LLM_INPUT_PRICE_PER_MILLION="0"
export HSR_LLM_OUTPUT_PRICE_PER_MILLION="0"
export HSR_LLM_TOKEN_BUDGET="20000"
```

本地无鉴权的 OpenAI-compatible 服务可以留空 API Key。不同服务对 `reasoning_effort` 和 `max_completion_tokens` 的支持可能不同；不支持思考参数时选择 Disabled。

> 当前服务只适合本机或可信局域网课堂演示，没有登录、TLS、数据库或生产级限流。不要把它直接暴露到公网，也不要通过不受信任的明文页面提交 API Key。

## CLI Demo

如果只想查看确定性强化闭环：

```bash
node adapters/recommendations/prepare.mjs
node adapters/fribbels/build.mjs
cargo run --manifest-path hsr-relic-agent/Cargo.toml
```

交互模式和 Mock 对照模式：

```bash
cargo run --manifest-path hsr-relic-agent/Cargo.toml -- --interactive
cargo run --manifest-path hsr-relic-agent/Cargo.toml -- --mock
```

## 测试

先运行一次 `node web/prepare.mjs`，然后：

```bash
cargo test --manifest-path hsr-agent-runtime/Cargo.toml
cargo test --manifest-path hsr-relic-web/Cargo.toml
cargo test --manifest-path hsr-relic-agent/Cargo.toml --features fribbels-integration
```

完整的 Agent HTTP 协议 → Rust Tools → 真实 Fribbels 端到端测试是显式运行项：

```bash
cargo test --manifest-path hsr-relic-web/Cargo.toml --test agent_e2e -- --ignored
```

该测试的模型端是本机脚本化 OpenAI-compatible 服务，用于复现协议、usage、费用和预算行为，不代表真实模型的语言质量测试。

## 当前版本边界

当前 Demo 已实现：

- 自有 Rust `AccountState` 和强化状态更新；
- 固定版本的游戏静态角色—套装/位面/主副属性推荐数据，经自有 JSON 边界加载；
- Fribbels 遗器当前评分、平均满级潜力及参考 Build 数值；
- Rust 候选排序和 Continue / Hold / Stop；
- Web 与 CLI 共用同一个 core；
- LLM Tool 调用、模型配置、真实 Token usage、费用和 Token Budget；
- LLM 提取目标、材料压力、风险倾向和培养目标，Rust 以三种有限策略确定性计算候选；
- 多轮 Agent 强化闭环：自然语言观察 → Tool → Rust 状态更新与三态决策 → 继续或重新推荐；
- SSE 实时 Agent Trace，以及可向模型请求和 Fribbels 子进程传播的取消；
- 多轮模型上下文、历史任务浏览和版本化 Session JSON 保存/加载。
- Reliquary Archiver v4 原始 JSON 上传、结构化校验、装备关系转换和事务式会话替换；
- 由真实账号生成的可重复脱敏 Demo，且与用户上传共用同一个 Rust importer。

仍未实现：

- 完整角色技能、完整队伍 DPS 或最终概率模型；
- 通用 Planner、自动读取游戏内强化结果或任意自然语言数值推断；
- 公网部署所需的认证、TLS 和密钥管理。

旧版 Blade / Seele 的当前伤害指标只是 Fribbels 中的简化普通攻击参考，不能视为完整实战收益。当前强化步数和阈值仍是课堂 Demo 假设。

## 项目结构

```text
hsr-relic-agent/       Rust core、Decision Engine、Evaluator、CLI
hsr-agent-runtime/     ModelConfig、Provider、Agent Tools、usage/budget
hsr-relic-web/         薄 Rust Web/API 与内存会话
web/                   独立前端与 AssetProvider
adapters/fribbels/     自有 JSON ↔ Fribbels 薄 Adapter
adapters/recommendations/ 固定游戏配置 → 自有静态推荐 JSON
adapters/reliquary/    私有 Reliquary JSON → 可公开脱敏 Demo
data/.generated/       本地生成且不提交的静态推荐数据库
data/private/          本机私有原始账号，Git 永久忽略
fixtures/              小型 Scanner fixture 与脱敏 Reliquary Demo
docs/                  用户导入教程
research/              上游源码与接口索引、集成实验记录
upstream/              本地第三方 checkout，不提交到本仓库
```

更详细的实现说明：

- [系统设计](DESIGN.md)
- [Web Demo 说明](web/README.md)
- [Agent Runtime](hsr-agent-runtime/README.md)
- [Rust core 与 CLI](hsr-relic-agent/README.md)
- [Fribbels Adapter](adapters/fribbels/README.md)
- [模拟数据](fixtures/README.md)
- [Windows 导出与导入 Reliquary 教程](docs/reliquary-import-windows.md)
- [上游源码索引](research/upstream-index.md)

## 第三方项目与致谢

- [Fribbels HSR Optimizer](https://github.com/fribbels/hsr-optimizer)：遗器评分、Build 计算和视觉资源映射；代码采用 MIT License。
- [DimbreathBot/TurnBasedGameData](https://github.com/DimbreathBot/TurnBasedGameData)：游戏静态 `AvatarRelicRecommend.json` 来源；本项目固定 commit 后在本地转换，不重新发布完整上游文件。来源仓库未声明许可证。
- [HSR-Scanner](https://github.com/kel-z/HSR-Scanner)：v4 模拟数据格式参考。
- [Reliquary Archiver](https://github.com/IceDynamix/reliquary-archiver)：真实账号 v4 JSON 来源；本项目读取其公开格式，不复制其抓包实现。项目采用 MIT License。
- [HSR_Nous](https://github.com/pzc2004/HSR_Nous)：仅保留调研参考，本版本未集成。

本仓库不提交第三方源码或生成出的游戏图片资源，也没有修改 `upstream/` 中的第三方业务源码。游戏名称、角色和美术资源权利归原权利人所有；本项目为非官方课堂 Demo。

# Web Demo：遗器培养终端

独立单页，沿用 v0.2 的 fixture、Evaluator 与决策规则，并在 v0.3 增加 Minimal Agent Runtime。原生 HTML/CSS/ES Modules + Rust Axum；不需要前端开发服务器，浏览器运行时不加载 Fribbels 应用。

## 启动与试用

在 workspace 根目录运行（需要 Rust、Node 22+、现有上游 checkout）：

```bash
# 首次未安装时执行；按上游 lockfile 安装依赖，不修改其业务源码
npm ci --prefix upstream/hsr-optimizer
node web/prepare.mjs
cargo run --manifest-path hsr-relic-web/Cargo.toml
```

打开 <http://127.0.0.1:3000>。准备脚本检查固定上游版本、构建已有计算 Adapter，并调用上游 Assets 生成资源清单，逐一检查图片存在。源码/fixture/上游版本变化后重新准备；平时只需启动 Rust 服务。修改前端后刷新页面。

让同一局域网的同学试用：

```bash
cargo run --manifest-path hsr-relic-web/Cargo.toml -- --lan
```

同学打开 `http://你的局域网IP:3000`；主机保持运行，可能需要允许系统防火墙访问。默认只监听本机；本轮没有公网部署、认证、TLS 或生产级限流，不要直接暴露公网。尤其不要通过不受信任的明文局域网页面提交 API Key。端口冲突时设置 `HSR_WEB_PORT=3001`。Ctrl+C 停止服务；显式 `--mock` 可用 Mock 对照，不会自动降级。

### 配置真实模型

可在页面右上角“模型设置”填写 OpenAI-compatible Endpoint、Key、模型 ID、Context Length、Reasoning Mode、输入/输出价格和 Token Budget。Key 留空会保留当前服务端会话中的值，勾选“清除”才会删除；页面状态只返回是否已配置，不返回 Key。

也可以在服务启动前设置 `HSR_LLM_API_ENDPOINT`、`HSR_LLM_API_KEY`、`HSR_LLM_MODEL`、`HSR_LLM_CONTEXT_LENGTH`、`HSR_LLM_REASONING_MODE`、`HSR_LLM_INPUT_PRICE_PER_MILLION`、`HSR_LLM_OUTPUT_PRICE_PER_MILLION` 和 `HSR_LLM_TOKEN_BUDGET`。环境配置是新会话的初始值，页面修改仅影响当前内存会话。

输入价格和输出价格的单位均为“用户选定的货币 / 1M tokens”。使用服务商当日价格自行填写；默认 0 表示只统计 token、不计算未知价格。Agent 区域显示 API 响应中的真实 prompt/completion usage、调用次数、累计费用和预算。达到预算后，Runtime 在下一次请求发出前返回明确错误。

当前 Provider 使用 Chat Completions function tools。支持无 Key 的本地 OpenAI-compatible 服务；不是所有服务都接受 `reasoning_effort` 或 `max_completion_tokens`，不兼容时将 Reasoning Mode 设为 Disabled，并按服务能力配置 Context Length。

### 可复现观察

1. 选择刃；也可切换希儿比较排序。
2. 选手部 `9100002`，录入暴击率增量 `3.24`：同一遗器 +3 → +6，得到 Continue。
3. 选择头部 `9100001`（必要时打开全部库存），新增防御力% `5.4`：+0 → +3，得到 Hold。
4. 在全部库存再次点击它，显式恢复观察；再录入防御力% `5.4`：同一件 +3 → +6，得到 Stop，并改推其他件。
5. 观察预算从 8 → 5、三条历史及各次原因。Reset 恢复最初账号，不影响其他独立会话。

输入是本次增量，百分比填百分点，不是总值。只做现有 core 的字段校验，不新增真实游戏 roll 概率或完整合法值验证。

## 模块与边界

| 模块 | 职责 |
|---|---|
| `index.html` / `style.css` / `app.js` | 独立角色面板、遗器卡片、观察表单、历史与结果展示；保留 API 返回的候选顺序 |
| `api.js` | 同源 JSON 请求、会话令牌、错误传递 |
| `asset-provider.js` | 界面唯一图片入口；只读生成的资源映射 |
| `asset-manifest.ts` / `prepare.mjs` | 构建时调用真正的上游 Assets，生成 `.generated/assets.json`，不手工维护图片 URL |
| `../hsr-relic-web/src/lib.rs` | 薄 API、会话内存、动作串行化、版本校验、进度与取消、成功后提交快照 |
| `../hsr-relic-web/src/dto.rs` | 内部模型到 Web 展示 DTO；结构化错误到中文提示 |
| `../hsr-agent-runtime/` | ModelConfig、OpenAI-compatible Provider、Agent Runtime、薄 Tool Layer、AgentEvent 与 UsageLedger |
| `../hsr-relic-agent/` | CLI 共用的原有 core；评分、排序、状态更新和三态判断仍全部在这里 |

一次 Web 操作在独立的 engine 状态副本上执行，评价与快照完成后才替换会话状态。失败或取消不提交账号修改。评价在阻塞工作线程中运行，进度读取不等待账号锁；页面显示等待秒数并提供取消。这里是任务存活/耗时提示，不虚构 Fribbels 内部完成百分比。

### 当前 API

- `POST /api/session`：创建独立模拟会话，返回令牌与初始 state。
- `GET /api/state`：读取最后成功提交的完整展示快照。
- `POST /api/action`：`target / select / recommend / upgrade / reset`；必须提供 `expected_revision`。强化另传 `relic_id / expected_level / stat / increase`。
- `POST /api/model-config`：更新当前会话的模型配置；必须提供 `expected_revision`。Key 留空保留、`clear_api_key: true` 清除。
- `POST /api/agent`：提交自然语言目标并运行模型 → Tool → Decision Engine 闭环；必须提供 `expected_revision / input`。
- `GET /api/progress`、`POST /api/cancel`：查看进行中状态与取消当前操作。
- 后续请求携带 `x-demo-session`。失败返回 `{error: {code, message}}`；不会靠解析 CLI 文本工作。

例如强化动作：

```json
{"action":"upgrade","expected_revision":2,"relic_id":"9100002","expected_level":3,"stat":"crit_rate","increase":3.24}
```

响应包含 `revision / target_id / selected_id / inventory / recommendations / selected_evaluation / remaining_budget / history / last_result / model_config / usage / last_agent`。`model_config` 不含 Key；`usage` 来自服务端 ledger。分值、排序、限制和决策由 Rust 提供；前端只做字段格式化、ID 联结和交互。

Agent 当前有 `set_target_character`、`get_current_state`、`get_relic_candidates`、`get_next_relic_recommendation`、`get_upgrade_history` 五个 Tool。详细协议见 [Runtime 文档](../hsr-agent-runtime/README.md)。

## 视觉资源来源

固定上游：[Fribbels HSR Optimizer](https://github.com/fribbels/hsr-optimizer/tree/df630a0488a64eeb740e4e0c14f265d96b9f6f8f)。

| 当前上游位置 / symbol | 本项目使用方式 |
|---|---|
| `src/lib/rendering/assets.ts`：`getCharacterAvatarById` / `getCharacterPortraitById` / `getCharacterPreviewById` | 由角色 ID 获取头像、立绘、预览图；预览映射已生成，当前主面板使用 portrait |
| 同文件 `getSetImage`，依赖上游 `setToId` 和 Parts | 以套装名与部位获取遗器图片，不复制部位后缀规则 |
| 同文件 `getStatIcon` / `getDefaultRelic` | 内部 Stat → 上游常量的边界映射，获得属性图标与失败兜底 |
| `src/data/game_data.json` | fixture 的 set_id → 上游套装名 |
| `src/lib/tabs/tabRelics/RelicPreview.tsx` / `src/lib/tabs/tabRelics/relicPreview/RelicStatRow.tsx` | 参考信息层次与 Assets 调用方式；未复制 React/store/i18next 组件 |
| `public/assets/` | Rust 以 `/assets/` 只读提供本地图片；不热链第三方网页 |

资源映射绑定当前 fixture，不是全角色/全套装资源服务。中文展示名称与两位角色文案是小规模界面配置；图片路径仍由上游方法生成。代码与素材授权边界见页面 [开源致谢](credits.html)：Fribbels 代码 MIT；游戏美术权利归原权利人，不宣称这些图片获得 MIT 再授权。没有复制第三方业务源码。

Rust HTTP 服务用法参考 [Axum 官方 serve 文档](https://docs.rs/axum/latest/axum/fn.serve.html)。

## 测试与当前限制

```bash
cargo test --offline --manifest-path hsr-agent-runtime/Cargo.toml
cargo test --manifest-path hsr-relic-web/Cargo.toml
cargo test --manifest-path hsr-relic-agent/Cargo.toml --features fribbels-integration
```

包含 socket 的 Provider 协议测试和端到端脚本服务测试在受限 sandbox 中可能需要网络权限。端到端测试显式忽略，运行：

```bash
cargo test --offline --manifest-path hsr-relic-web/Cargo.toml --test agent_e2e -- --ignored --nocapture
```

该测试使用本机脚本化 OpenAI-compatible HTTP 服务验证完整 wire protocol、真实 Fribbels Tool 执行、exact usage/cost 和预算阻断；它不是外部大模型质量测试。

Web 测试覆盖三种判断、同件状态与历史更新、Hold 恢复、Stop 换件、不同目标排序、错误回滚、重复提交拒绝、会话隔离、重置、忙碌状态与取消前不提交。默认测试用 Mock 隔离 HTTP 行为；真实计算由 core 集成测试和启动后的浏览器流程验证。

当前验证：core/CLI/Fribbels 37 项、Agent Runtime 9 项、Web 5 项常规测试和 1 项显式运行的 Agent → 真实 Fribbels 端到端测试通过；三者 clippy 均无警告。浏览器已验证 Agent 设置目标、获取下一件推荐、展示逐次 usage/cost/Tool 轨迹，以及 budget 达线后不再请求。没有可用 API Key 或本地模型时，外部真实模型调用需要由使用者按上述方式配置后验证。

Demo 简化：每次固定加载当前 fixture、8 步强化预算；没有账号上传或数据库。会话保留在服务内存，浏览器 sessionStorage 保存会话令牌，刷新可继续；复制标签页可能继承浏览器的 sessionStorage，独立打开地址才创建独立会话。重启服务会丢失模型配置、usage、Agent 轨迹和账号状态；最多 64 会话，新建时清理闲置超过两小时的会话。当前只保存最近一次 Agent 轨迹，不保留多轮 messages，不做 R5 持久化。

现有 `/api/progress` 和 cancel flag 只在请求边界检查；等待阻塞 HTTP 响应时不能立即中止 socket，也没有 SSE/WebSocket 实时事件。因此它们只是 R4 的接口基础，不算 R4 完整实现。`AgentEvent` 已统一模型请求、usage、Tool 和回复事件，下一轮可改为流式发送。

评价边界保持 v0.2：真实评分、平均满级潜力和参考 Build 面板；旧版 Blade / Seele 的简化普攻不是完整技能输出或 DPS。缺失四个部位按库存补齐只是参考假设；升级成本、阈值仍为 Demo 启发式。

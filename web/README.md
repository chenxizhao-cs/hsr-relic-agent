# Web Demo：遗器培养终端

独立单页，沿用 v0.2 的 fixture、Evaluator 与决策规则。原生 HTML/CSS/ES Modules + Rust Axum；不需要前端开发服务器，浏览器运行时不加载 Fribbels 应用。

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

同学打开 `http://你的局域网IP:3000`；主机保持运行，可能需要允许系统防火墙访问。默认只监听本机；本轮没有公网部署、认证、TLS 或生产级限流，不要直接暴露公网。端口冲突时设置 `HSR_WEB_PORT=3001`。Ctrl+C 停止服务；显式 `--mock` 可用 Mock 对照，不会自动降级。

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
| `../hsr-relic-agent/` | CLI 共用的原有 core；本轮仅给 DecisionEngine 增加条件性 Clone 派生，规则不变 |

一次 Web 操作在独立的 engine 状态副本上执行，评价与快照完成后才替换会话状态。失败或取消不提交账号修改。评价在阻塞工作线程中运行，进度读取不等待账号锁；页面显示等待秒数并提供取消。这里是任务存活/耗时提示，不虚构 Fribbels 内部完成百分比。

### 当前 API

- `POST /api/session`：创建独立模拟会话，返回令牌与初始 state。
- `GET /api/state`：读取最后成功提交的完整展示快照。
- `POST /api/action`：`target / select / recommend / upgrade / reset`；必须提供 `expected_revision`。强化另传 `relic_id / expected_level / stat / increase`。
- `GET /api/progress`、`POST /api/cancel`：查看进行中状态与取消当前操作。
- 后续请求携带 `x-demo-session`。失败返回 `{error: {code, message}}`；不会靠解析 CLI 文本工作。

例如强化动作：

```json
{"action":"upgrade","expected_revision":2,"relic_id":"9100002","expected_level":3,"stat":"crit_rate","increase":3.24}
```

响应包含 `revision / target_id / selected_id / inventory / recommendations / selected_evaluation / remaining_budget / history / last_result`。分值、排序、限制和决策由 Rust 提供；前端只做字段格式化、ID 联结和交互。新增账号导入或 Agent 可以复用 core，但本轮不预设其具体接口。

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
cargo test --manifest-path hsr-relic-web/Cargo.toml
cargo test --manifest-path hsr-relic-agent/Cargo.toml --features fribbels-integration
```

Web 测试覆盖三种判断、同件状态与历史更新、Hold 恢复、Stop 换件、不同目标排序、错误回滚、重复提交拒绝、会话隔离、重置、忙碌状态与取消前不提交。默认测试用 Mock 隔离 HTTP 行为；真实计算由 core 集成测试和启动后的浏览器流程验证。

本轮验证：core 37 项测试、Web 4 项测试通过；Web clippy 无警告。已启动真实 Fribbels 服务，在浏览器操作完成 Continue → Hold → 恢复 → Stop → 换件，以及目标切换、刷新继续、重置、非法主副属性冲突时不扣预算。390px 窄屏无横向溢出，资源加载正常；真实 API 运算期间可查询进度，取消后账号快照不变。局域网多设备连通与公网部署未在本轮实际验证。

Demo 简化：每次固定加载当前 fixture、8 步预算；没有账号上传或数据库。会话保留在服务内存，浏览器 sessionStorage 保存会话令牌，刷新可继续；复制标签页可能继承浏览器的 sessionStorage，独立打开地址才创建独立会话。重启服务丢失状态；最多 64 会话，新建时清理闲置超过两小时的会话。无 LLM、Nous、Reliquary。

评价边界保持 v0.2：真实评分、平均满级潜力和参考 Build 面板；旧版 Blade / Seele 的简化普攻不是完整技能输出或 DPS。缺失四个部位按库存补齐只是参考假设；升级成本、阈值仍为 Demo 启发式。

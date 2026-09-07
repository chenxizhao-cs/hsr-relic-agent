# Windows 导出 Reliquary JSON 并导入本项目

本项目只需要 Reliquary Archiver 最终生成的 `archive_output-*.json`。不需要开启 Fribbels Live Import，也不需要把抓包文件或账号密码交给本项目。

## 1. 导出前准备

1. 从 [Npcap 官网](https://npcap.com/)下载并安装 Npcap。安装时勾选 `Install Npcap in WinPcap API-compatible Mode`；如果电脑通过 Wi-Fi 连接网络，再勾选 `Support raw 802.11 traffic (and monitor mode) for wireless adapters`。
2. 从 [Reliquary Archiver Releases](https://github.com/IceDynamix/reliquary-archiver/releases/) 下载最新版 `reliquary-archiver-pcap-x64.exe`。只从项目正式发布页获取程序。官方当前也提供不依赖 Npcap 的 pktmon 版本，但本教程按 pcap 版本说明。
3. 启动《崩坏：星穹铁道》，停在列车登录画面的 `Click to Start`，不要提前进入游戏世界。
4. 双击运行 Reliquary Archiver，等待界面显示 `Waiting for login...`。此时按钮显示 `Export not ready` 是正常现象。
5. 回到游戏点击 `Click to Start`，直到完整进入游戏世界。如果 Archiver 成功识别，状态会变成 `Connected!`；角色、遗器、光锥和材料数量会从 0 变为账号数据，下载按钮也会可用。
6. 确认角色和遗器不是 0 后，点击 Export / Download 并选择保存位置。最终会得到类似 `archive_output-2026-09-07T22-33-16.json` 的文件。

长期游玩账号通常会显示数十个角色、大量遗器和光锥，但不要求这些数字与游戏背包页面逐项完全相同。对本项目最关键的是 Characters 和 Relics 已正常读取。

如果启动 Archiver 前已经进入游戏，需要退出登录并重新进入。VPN、网络过滤工具或 Wi-Fi 状态也可能影响识别；具体排查以 [Reliquary 官方 README](https://github.com/IceDynamix/reliquary-archiver#readme) 为准。

## 2. 导入 HSR Relic Agent

1. 按项目根目录 README 启动 Web Demo，并打开 <http://127.0.0.1:3000>。
2. 点击页面顶部“导入 Reliquary JSON”。
3. 选择刚才生成的 `archive_output-*.json`。
4. 页面成功后会显示角色数量、源遗器数量、进入当前强化模型的遗器数量、光锥数量和装备关系识别状态。
5. 选择账号中的可用目标角色，进入现有推荐与强化闭环。

当前强化模型只处理五星遗器的 `+0/+3/+6/+9/+12/+15` 检查点。其他稀有度或 `+1/+2/+4` 等中间等级仍可存在于原始文件中，但不会被改写或假装成检查点；页面会把它们计入“跳过”数量。

## 3. 隐私说明

Reliquary 原始导出可能包含玩家 UID、星琼/古老梦华数量、材料库存，以及遗器和光锥的账号内部唯一 ID。请把原始 JSON 当作私密账号数据：

- 不要提交到 GitHub、课程仓库或公开网盘；
- 本项目上传接口只应在本机或可信局域网使用，不要把当前无鉴权 Demo 暴露到公网；
- Rust 服务不会在普通日志中打印完整上传内容或玩家 UID；
- 导入后的 Session JSON 不含顶层玩家 UID 和抽卡资源，但包含账号遗器状态，仍应谨慎分享。

仓库内置的 `fixtures/reliquary-v4-demo.json` 已将玩家 UID 和开拓者选择设为 `null`、抽卡资源归零、材料数组清空，并替换全部遗器/光锥 `_uid`。清洗脚本位于 `adapters/reliquary/sanitize.mjs`；原始文件只存放在被 Git 忽略的 `data/private/`。

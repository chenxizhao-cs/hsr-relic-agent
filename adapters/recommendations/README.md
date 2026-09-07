# 游戏静态推荐数据 Adapter

`prepare.mjs` 从固定版本的游戏资源快照读取 `AvatarRelicRecommend.json`，校验文件 SHA-256，并转换成项目自己的 JSON v1。它不复制上游实现，也不读取账号数据。

```bash
node adapters/recommendations/prepare.mjs
```

输出位于 `data/.generated/character-relic-recommendations-v1.json`，不会提交到 Git。固定来源为：

- repository：<https://github.com/DimbreathBot/TurnBasedGameData>
- commit：`8cdb905dc2f8e6fffa9be4eb07af3e34435d6091`
- path：`ExcelOutput/AvatarRelicRecommend.json`
- SHA-256：`26e5b39b7471abc4c1f51481611dfd890d8cd108869d129b05bd9bfa23408447`

转换后每个角色包含：外圈套装 ID、位面套装 ID、躯干/脚部/位面球/连结绳主属性候选，以及副属性候选。源文件中的 `ScoreRankList` 含义尚未从当前源码确认，因此不导入。

该数据来自社区提取的游戏客户端配置，不是米哈游公开 API；来源仓库未声明许可证。本项目只在用户本地按固定版本生成，不把完整上游数据重新发布到公开仓库。游戏数据权利归原权利人所有。

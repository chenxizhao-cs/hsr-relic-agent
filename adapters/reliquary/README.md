# Reliquary v4 导入与 Demo 脱敏

Rust core 直接读取 Reliquary Archiver 的 `source = "reliquary_archiver"`、`version = 4` JSON。`sanitize.mjs` 只用于从私有真实导出生成可公开的长期 Demo，不参与线上用户上传。

```bash
node adapters/reliquary/sanitize.mjs \
  data/private/reliquary-account-private.json \
  fixtures/reliquary-v4-demo.json
```

清洗规则：

- 顶层 `metadata.uid` 和 `metadata.trailblazer` 设为 `null`；
- 星琼和古老梦华设为 `0`；
- 删除本项目不使用的材料库存；
- 遗器和光锥 `_uid` 分别替换为稳定、唯一、只用于 Demo 的数字字符串；
- 保留角色、遗器、光锥、等级、词条、装备关系、`lock` / `discard` 以及 Reliquary v4 的其余相关结构。

原始文件必须放在 `data/private/`，该目录已被 Git 忽略。不要把原始账号导出复制到 `fixtures/` 或提交到 Git。

格式来源：<https://github.com/IceDynamix/reliquary-archiver>，本项目按本地固定 checkout `cb109f17a4a15b7604cfe9d078a8735e7735cd25` 的 `src/export/fribbels/models.rs` 校验字段。Reliquary Archiver 使用 MIT License。

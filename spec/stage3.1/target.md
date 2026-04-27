# 目标（Stage 3.1）

在 [Stage 3](../stage3/target.md) 既有的「办公文档轻量预览」基础上，做四项体验优化：

| # | 主题 | 简述 |
| --- | --- | --- |
| O1 | EPUB 封面预览 | 选中 `.epub` 文件 → 预览区直接显示电子书封面图（不解析正文）。 |
| O2 | 跨平台持久缓存 | 预览中间产物（PPTX→PDF / Quick Look PNG / EPUB 封面）落到平台标准缓存目录，重启不丢。 |
| O3 | EPUB 跳过 Quick Look | macOS 上不再优先调用 `qlmanage` 渲染 epub，直接用 zip 解析提取封面，秒开且稳定命中缓存。 |
| O4 | 根目录跳过磁盘统计 | 选中文件树最根部的节点（家目录、`/`、卷、盘符、iCloud Drive 等）时，不再调用 `du -sk` 计算占用，避免长耗时与权限弹窗。 |

设计原则（沿用 Stage 3）：

1. **不破坏既有分支**：`Spreadsheet / Pdf / Docx / Pptx / OfficeImage / Unsupported` 等 PreviewPayload 分支与前端渲染器一概不变。
2. **无新增系统依赖**：EPUB 封面解析仅依赖 `zip` crate（已被 `calamine` 间接引入），不要求 LibreOffice / qlmanage。
3. **缓存键稳定**：沿用 `djb2(path) + mtime + size` 的 cache key，文件未修改即命中。
4. **降级路径明确**：解析失败时一律走 `PreviewPayload::Unsupported`，前端展示文字原因。

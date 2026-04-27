# 需求设计文档（Stage 3.1）

> 基于 [target.md](target.md)。本阶段只动 [src/commands/preview.rs](../../src/commands/preview.rs) 与 [Cargo.toml](../../Cargo.toml)，前端无改动。

---

## 1. 阶段目标

| # | 目标 |
| --- | --- |
| G1 | 选中 `.epub` 文件 → 预览区显示封面图（复用 Stage 3 的 `OfficeImage` 分支，`engine = "EPUB Cover"`）。 |
| G2 | 预览中间产物（PPTX→PDF / Quick Look PNG / EPUB 封面）写入平台持久缓存目录；同一文件未修改时跨进程、跨重启均命中。 |
| G3 | macOS 上 `.epub` 不再优先尝试 `qlmanage`，避免 30 s 超时空转。 |
| G4 | 选中树根节点时 `preview` 返回 `Directory { total_size: None, .. }`，前端展示 `—`。 |
| G5 | 任意 EPUB 解析步骤失败均不 panic，回落到 `Unsupported { reason }`。 |

非目标：

- 不解析 EPUB 正文 / 章节 / 目录；只取封面。
- 不支持 `.mobi / .azw3` 等非 ZIP 容器格式。
- 不对 PDF / DOCX / XLSX 增加服务端图片缓存（这三者非 macOS 路径上根本没有图片产物可缓存；macOS Quick Look 路径已纳入 G2）。
- 不缓存「失败结果」（负缓存）；qlmanage 失败仍每次重试。

---

## 2. 大小与超时上限

| 项 | 值 | 说明 |
| --- | --- | --- |
| `EPUB_LIMIT` | 200 MiB | 超限时返回 `Unsupported`。 |
| EPUB 封面读取 | 流式 | 单条 ZIP 条目 read_to_end，不预加载整个 epub。 |
| 缓存命中检查 | O(1) | 按候选扩展名拼路径检查 `exists()`。 |

---

## 3. 后端：preview.rs 改动

### 3.1 EPUB 封面分支

复用 Stage 3 已有的 `PreviewPayload::OfficeImage`，**不新增枚举分支**：

```rust
PreviewPayload::OfficeImage {
    image_path,                 // 缓存目录中的封面图绝对路径
    size,                       // 原 epub 文件大小
    engine: "EPUB Cover".into() // 区分于 "macOS Quick Look"
}
```

前端 [ui/js/preview.js](../../ui/js/preview.js) 的 `case "officeImage"` 分支已经支持「仅首页缩略图」展示，无需改动。

### 3.2 路由

`preview_blocking` 中扩展名匹配：

```text
.epub  → preview_epub(p, size, &meta)   // 直接走 zip 解析，不优先 quicklook
```

`.xlsx / .pdf / .docx / .pptx` 的路由不变（macOS 仍优先 `try_quicklook`）。

### 3.3 EPUB 封面提取算法

```text
1. 打开 epub 作为 ZipArchive
2. 读 META-INF/container.xml，提取 <rootfile full-path="..."> 得到 OPF 路径
3. 读 OPF，按以下顺序在 manifest 里找封面 href：
     a) EPUB 3:  <item properties="cover-image" href="..."/>
     b) EPUB 2:  <meta name="cover" content="<id>"/> + <item id="<id>" href="..."/>
     c) 兜底:    media-type 以 image/ 开头且 id/href 含 "cover" 的第一个 item
4. href 与 OPF 同目录拼接 → resolve_zip_path() 规范化（去 ./ 和 ..）
5. 提取该 ZIP 条目，写入 <cache_root>/<key>.cover.<ext>
6. 返回 OfficeImage { image_path = 该缓存文件 }
```

XML 解析为内嵌的轻量解析器（`iter_open_tags` + `parse_xml_attrs` + `attr_value`），不引入新的 XML 库，namespace 前缀通过「`name` 或 `ends_with(":name")`」匹配。

### 3.4 跨平台持久缓存目录

新增助手 `preview_cache_root() -> PathBuf`，被以下三处统一调用：
- `convert_pptx_via_soffice()`
- `try_quicklook()`（macOS）
- `preview_epub()`

解析顺序（基于 `dirs::cache_dir()`，再追加 `qfinder/preview/`）：

| 平台 | 路径 |
| --- | --- |
| macOS | `~/Library/Caches/qfinder/preview/` |
| Linux | `$XDG_CACHE_HOME/qfinder/preview/` 或 `~/.cache/qfinder/preview/` |
| Windows | `%LOCALAPPDATA%\qfinder\preview\` |
| 兜底 | `<system temp>/qfinder-preview/` |

缓存键沿用 Stage 3 的 `cache_key_for(path, size, mtime) = "{djb2(path):x}-{mtime}-{size}"`，文件未修改即命中。

### 3.5 EPUB 与 Quick Look 的关系

```text
preview_blocking
└── ext == "epub" → preview_epub(...)            // 不再调 try_quicklook
    └── 命中缓存？ → 是：返回 OfficeImage(cached)
                    否：extract_epub_cover() → 写缓存 → 返回 OfficeImage
```

> 背景：iCloud 同步过来或部分 DRM-free 的 epub，`qlmanage` 不会生成 PNG，会一直跑到内置 30 s 超时才失败。Stage 3 的实现先 `try_quicklook`、失败再回落，导致用户「每次都等 30 s」。Stage 3.1 反转优先级，由 `preview_epub` 直接秒开。

### 3.6 根节点跳过 du

`preview_dir(p)` 中：

```rust
let total_size = if is_root_path(p) { None } else { du_size(p) };
```

`is_root_path(p)` 判定规则（命中任一即为根）：

1. `p.parent().is_none()` —— 文件系统根（`/`、Windows 盘符根 `C:\`）
2. `p == dirs::home_dir()`
3. `p` 出现在 `platform::list_roots()` 返回列表中（覆盖 macOS 的 iCloud Drive、`/Volumes/*`，Linux 的 `/media/<user>/*`、`/mnt`、`/media`，Windows 的盘符）

返回 `total_size = None` 时，前端 `case "directory"` 会展示 `—`，已有逻辑无需改动。

---

## 4. 依赖

| 项 | 状态 |
| --- | --- |
| `zip = { version = "7", default-features = false, features = ["deflate"] }` | 新增（之前由 `calamine` 间接引入，本阶段显式声明并启用 `deflate`）。 |
| `dirs` | 已有（Stage 1 起即引入）。 |

无前端依赖增减。

---

## 5. 兼容性与降级

| 场景 | 行为 |
| --- | --- |
| epub 无 `META-INF/container.xml` 或 OPF 损坏 | `Unsupported { reason: "解析 EPUB 失败：..." }` |
| OPF manifest 找不到封面 | `Unsupported { reason: "未在 EPUB 中找到封面图片" }` |
| 缓存目录无法创建 | `Unsupported { reason: "无法创建缓存目录：..." }`（pptx/epub 同此处理） |
| `dirs::cache_dir()` 解析失败 | 回退 `<temp>/qfinder-preview/`，行为与 Stage 3 一致 |
| 根节点选中 | `Directory { total_size: None }`，前端展示 `—` |
| 非根目录 | 行为与 Stage 1 完全相同（仍调用 `du -sk`） |

# 任务拆解文档（Stage 3.1）

> 配套 [target.md](target.md) / [requirements.md](requirements.md)。
> 编号规则：`T3.1-NN`。每个任务标注 **目的 / 内容 / 输出 / 验收**。

---

## 阶段 0：依赖

### T3.1-01 显式声明 zip 依赖
- **目的**：EPUB 解析需要按需读取 ZIP 条目，显式依赖避免依赖 `calamine` 的间接导出。
- **内容**：在 [Cargo.toml](../../Cargo.toml) `[dependencies]` 追加：

  ```toml
  zip = { version = "7", default-features = false, features = ["deflate"] }
  ```

- **输出**：更新后的 `Cargo.toml` / `Cargo.lock`。
- **验收**：`cargo build` 通过，`cargo tree` 显示 `zip v7.x` 在 qfinder 直接依赖中。

---

## 阶段 1：EPUB 封面预览

### T3.1-02 添加 EPUB 路由
- **目的**：将 `.epub` 接入 `preview_blocking`。
- **内容**：在 [src/commands/preview.rs](../../src/commands/preview.rs) 的扩展名 `match` 中追加 `"epub"` 分支，直接调用 `preview_epub(&p, size, &meta)`，**不**调用 `try_quicklook`。
- **输出**：`preview.rs`。
- **验收**：`cargo check` 通过；选中任意 epub 不再触发 30s 等待。

### T3.1-03 实现 preview_epub
- **目的**：核心入口：缓存命中检查 + 调用提取器 + 错误降级。
- **内容**：
  - 常量 `EPUB_LIMIT = 200 << 20`。
  - 超限 → `too_large(size, EPUB_LIMIT, "EPUB")`。
  - 命中缓存 `<root>/<key>.cover.<ext>`（按 `EPUB_COVER_EXTS = ["jpg","jpeg","png","gif","webp","svg"]` 探测）→ 直接返回 `OfficeImage`。
  - 否则调 `extract_epub_cover()`，成功 → 返回 `OfficeImage { engine: "EPUB Cover" }`；返回 `None` → `Unsupported { reason: "未在 EPUB 中找到封面图片" }`；`Err` → `Unsupported { reason: "解析 EPUB 失败：{e}" }`。
- **输出**：`preview.rs`。
- **验收**：单文件多次预览，第二次不重新写缓存（mtime/size 不变）。

### T3.1-04 实现 extract_epub_cover
- **目的**：按 [requirements.md §3.3](requirements.md) 算法提取封面。
- **内容**：
  - 用 `zip::ZipArchive::new(File)` 打开。
  - `read_zip_to_string("META-INF/container.xml")` → `extract_attr(xml, "rootfile", "full-path")`。
  - `read_zip_to_string(opf_path)` → `find_cover_href(opf)`。
  - 拼路径 + `resolve_zip_path` → `zip.by_name(...)` → `read_to_end` → 写入缓存 `<key>.cover.<ext>`。
- **输出**：`preview.rs`。
- **验收**：手测三类样本（EPUB 3 properties / EPUB 2 meta cover / 兜底 cover.* 命名）均能出图。

### T3.1-05 内嵌轻量 XML 解析器
- **目的**：避免新增 quick-xml 之类依赖（OPF 内容简单，足以手写）。
- **内容**：
  - `iter_open_tags(xml, tag) -> Vec<String>`：返回所有 `<tag …>` 起始标签内部串。
  - `parse_xml_attrs(inner) -> Vec<(String, String)>`：解析属性对，支持单/双引号。
  - `attr_value(attrs, name)`：按 `name` 或 `ends_with(":name")` 匹配（兼容 namespace 前缀）。
  - `extract_attr(xml, tag, attr)`：组合上面三者，返回首个匹配。
  - `find_cover_href(opf)` / `find_meta_cover_id(opf)`：按 EPUB 2/3 规则定位封面 href。
  - `resolve_zip_path(p)`：去 `./`、解析 `..`、规范化分隔符。
- **输出**：`preview.rs`。
- **验收**：上述函数单元覆盖三种 EPUB 元数据形态；含 `<dc:identifier>` 等带前缀标签时不误命中。

---

## 阶段 2：跨平台持久缓存

### T3.1-06 新增 preview_cache_root
- **目的**：统一缓存根目录，取代 `std::env::temp_dir().join("qfinder-preview")`。
- **内容**：

  ```rust
  fn preview_cache_root() -> PathBuf {
      if let Some(base) = dirs::cache_dir() {
          return base.join("qfinder").join("preview");
      }
      std::env::temp_dir().join("qfinder-preview")
  }
  ```

- **输出**：`preview.rs`。
- **验收**：在 macOS 启动后 `ls ~/Library/Caches/qfinder/preview/` 出现缓存文件；Linux / Windows 路径符合 [requirements.md §3.4](requirements.md)。

### T3.1-07 替换三处缓存目录调用点
- **目的**：把硬编码 temp 目录全部换成 `preview_cache_root()`。
- **内容**：替换 `convert_pptx_via_soffice` / `try_quicklook` / `preview_epub` 三处。
- **输出**：`preview.rs`。
- **验收**：`grep -n "qfinder-preview" src/` 仅在 `preview_cache_root` 的回退分支出现一次。

---

## 阶段 3：EPUB 不再走 Quick Look

### T3.1-08 移除 epub 的 quicklook 优先尝试
- **目的**：避免 iCloud / 部分 DRM-free epub 在 `qlmanage` 处空转 30 s。
- **内容**：T3.1-02 中已直接调用 `preview_epub`，不再 `#[cfg(target_os = "macos")] try_quicklook`。
- **输出**：`preview.rs`。
- **验收**：以「临高启明.epub」（约 19 MB，iCloud 同步）为例，第一次预览 < 1 s，第二次命中缓存 < 100 ms。

---

## 阶段 4：根节点跳过 du

### T3.1-09 新增 is_root_path
- **目的**：判定预览的目录路径是否属于「文件树最根部」。
- **内容**：
  - `p.parent().is_none()` → true（文件系统根 / Windows 盘符根）
  - `p == dirs::home_dir()` → true
  - `p` 出现在 `platform::list_roots()` 列表 → true
- **输出**：`preview.rs`。
- **验收**：单测覆盖 `/`、`~/`、iCloud Drive、`/Volumes/Macintosh HD` 等。

### T3.1-10 preview_dir 跳过根节点 du
- **目的**：根节点 du 可能耗时几十秒，且 macOS 会触发权限弹窗。
- **内容**：

  ```rust
  let total_size = if is_root_path(p) { None } else { du_size(p) };
  ```

- **输出**：`preview.rs`。
- **验收**：选中家目录 / `/Volumes/*` 时预览面板「磁盘占用 (du)」显示 `—`，且无 spinner 滞留。

---

## 阶段 5：回归

### T3.1-11 验收手测脚本
- **目的**：保证 Stage 3 既有四类（xlsx / pdf / docx / pptx）行为未退化。
- **内容**：依次预览样本：
  1. xlsx 5 sheets 大文件 → 表格分支
  2. pdf 多页 → PDF.js 分支（macOS 上仍走 Quick Look 缩略图）
  3. docx 含图 → mammoth 分支（macOS 上仍走 Quick Look）
  4. pptx 含动画 → LibreOffice 分支（macOS 上仍走 Quick Look）
  5. epub iCloud 同步 → 封面分支（< 1 s）
  6. 家目录、`/`、iCloud Drive 根 → `Directory { total_size = None }`
- **输出**：手测记录。
- **验收**：六个用例全部预期行为，且二次访问命中缓存。

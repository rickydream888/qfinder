# 任务拆解文档（Stage 3）

> 配套 [target.md](target.md) / [requirements.md](requirements.md)。
> 编号规则：`T3-NN`。每个任务标注 **目的 / 内容 / 输出 / 验收**。

---

## 阶段 0：依赖与资源

### T3-01 添加 Rust 依赖
- **目的**：引入 calamine 解析 xlsx。
- **内容**：在 [Cargo.toml](../../Cargo.toml) `[dependencies]` 末尾追加：

  ```toml
  calamine = "0.34"
  ```

- **输出**：更新后的 `Cargo.toml`、`Cargo.lock`。
- **验收**：`cargo check` 通过。

### T3-02 拉取前端第三方库
- **目的**：把 PDF.js / mammoth.js 落地到 `ui/vendor/`。
- **内容**：新建 `scripts/fetch-vendor.sh`，下载：
  - `https://cdnjs.cloudflare.com/ajax/libs/pdf.js/4.10.38/pdf.min.mjs` → `ui/vendor/pdfjs/pdf.min.mjs`
  - `https://cdnjs.cloudflare.com/ajax/libs/pdf.js/4.10.38/pdf.worker.min.mjs` → `ui/vendor/pdfjs/pdf.worker.min.mjs`
  - `https://cdnjs.cloudflare.com/ajax/libs/mammoth/1.8.0/mammoth.browser.min.js` → `ui/vendor/mammoth/mammoth.browser.min.js`

  并在 `ui/vendor/LICENSES.md` 标注两库的许可（Apache-2.0 / BSD-2-Clause）。
- **输出**：脚本 + 已下载文件。
- **验收**：`ls ui/vendor/pdfjs ui/vendor/mammoth` 列出对应文件；文件非空。

---

## 阶段 1：后端预览扩展

### T3-03 扩展 PreviewPayload
- **目的**：新增 `Spreadsheet / Pdf / Docx / Pptx / Unsupported` 分支（见 requirements §3.1）。
- **内容**：编辑 [src/commands/preview.rs](../../src/commands/preview.rs)。
- **输出**：变更后的 `preview.rs`。
- **验收**：`cargo check` 通过；JSON 序列化字段保持 camelCase。

### T3-04 实现路由与大小校验
- **目的**：在 `preview_blocking` 中按扩展名分发到新分支。
- **内容**：
  - 抽出常量：`XLSX_LIMIT = 50<<20`、`PDF_LIMIT = 100<<20`、`DOCX_LIMIT = 20<<20`、`PPTX_LIMIT = 50<<20`、`SPREADSHEET_MAX_ROWS = 100`、`SPREADSHEET_MAX_COLS = 20`。
  - 超限时返回 `Unsupported { reason: "文件过大（{size}），跳过预览", size }`。
- **输出**：`preview.rs`。
- **验收**：手工传一个 51MB 假文件名（mock）路径走单测分支。

### T3-05 XLSX 解析
- **目的**：实现 `parse_xlsx(path) -> AppResult<PreviewPayload>`。
- **内容**：按 requirements §3.3 写函数，cell 类型转换抽到 `cell_to_string`。捕获 calamine 错误，转为 `Unsupported { reason: "无法解析 xlsx：{e}", size }`。
- **输出**：`preview.rs`。
- **验收**：单元测试或手测一个 5 sheet 的 xlsx，行列数与截断标记符合。

### T3-06 PPTX → PDF 转换
- **目的**：实现 `convert_pptx(path) -> AppResult<PreviewPayload>`。
- **内容**：
  - `find_soffice() -> Option<PathBuf>`：依次查 `soffice`、`libreoffice`、`/Applications/LibreOffice.app/Contents/MacOS/soffice`。
  - 缓存目录 `std::env::temp_dir().join("qfinder-preview")`，按 `djb2(path)+mtime+size` 命名 PDF。
  - 命中缓存（文件存在且 mtime 未变）则直接返回。
  - 否则启动 soffice 子进程，自实现轮询超时（每 100ms `try_wait`，30s 总超时；超时则 `kill`）。
  - 失败 → `Unsupported`。
  - 缺 soffice → `Unsupported { reason: "需要安装 LibreOffice 才能预览 .pptx 文件", size }`。
- **输出**：`preview.rs`。
- **验收**：在装有 LibreOffice 的机器上，能成功生成 PDF；删除可执行文件 PATH 后返回 Unsupported。

### T3-07 PDF / DOCX 直通
- **目的**：实现 `preview_pdf`、`preview_docx`，仅做大小校验。
- **内容**：直接返回 `Pdf { path, size }` / `Docx { path, size }`。
- **输出**：`preview.rs`。
- **验收**：cargo check 通过。

---

## 阶段 2：前端渲染

### T3-08 表格渲染（spreadsheet）
- **目的**：处理 `payload.kind === "spreadsheet"` 分支。
- **内容**：
  - 头部 summary：sheet 名、总行列、其它 sheet 列表、截断标记。
  - `<table class="qf-sheet">` 渲染 headers + rows，所有 cell `escapeHtml` 并设 `title`。
- **输出**：[ui/js/preview.js](../../ui/js/preview.js)。
- **验收**：AC-1 ~ AC-3。

### T3-09 PDF 渲染器（pdfjs 懒加载）
- **目的**：处理 `pdf` 与 `pptx`（共用）分支。
- **内容**：
  - 新建 `ui/js/preview-pdf.js`：
    - 暴露 `QF.PreviewPDF.render(container, fileSrcPath)`；
    - 内部使用 ESM 动态 `import("./vendor/pdfjs/pdf.min.mjs")`；
    - 设置 `GlobalWorkerOptions.workerSrc = "vendor/pdfjs/pdf.worker.min.mjs"`；
    - 加载 → 取首页 → 按容器宽度计算 scale → render 到 canvas；
    - 提供 `cancelCurrent()`，在 PreviewPane 切换文件时调用。
  - 在 [ui/index.html](../../ui/index.html) `</body>` 前追加 `<script type="module" src="js/preview-pdf.js"></script>`。
- **输出**：新文件 + index.html / preview.js 修改。
- **验收**：AC-4，AC-10。

### T3-10 DOCX 渲染器（mammoth 懒加载）
- **目的**：处理 `docx` 分支。
- **内容**：
  - 新建 `ui/js/preview-docx.js`：暴露 `QF.PreviewDocx.render(container, fileSrcPath)`；
  - 首次调用时若 `window.mammoth` 未加载，则注入 `<script src="vendor/mammoth/mammoth.browser.min.js">` 并 await `load`；
  - `fetch(fileSrcPath).then(r => r.arrayBuffer())` → `mammoth.convertToHtml({arrayBuffer})`；
  - 截断算法（requirements §4.3）；
  - 注入到 container 内的 `<div class="qf-docx">`。
- **输出**：新文件 + index.html / preview.js 修改。
- **验收**：AC-5；超长文档末尾出现截断提示。

### T3-11 集成到 PreviewPane.render
- **目的**：在 [ui/js/preview.js](../../ui/js/preview.js) 的 `switch (p.kind)` 增加 `spreadsheet / pdf / docx / pptx / unsupported` 5 个 case。
- **内容**：
  - 通用：渲染头部（路径 + icon + 文件名 + 大小）；
  - 调用对应渲染器；
  - 在 `_loadNow` 开头取消上一次的 PDF 渲染任务（如有）。
- **输出**：`preview.js`。
- **验收**：四种类型均可显示；切换不残留。

### T3-12 样式
- **目的**：加入 requirements §6 的 CSS。
- **内容**：追加到 [ui/css/app.css](../../ui/css/app.css) 末尾。
- **输出**：`app.css`。
- **验收**：表格、canvas、警告框样式正确。

---

## 阶段 3：联调

### T3-13 端到端测试
- **目的**：覆盖 AC-1 ~ AC-10。
- **内容**：手工准备样本文件：
  - `tiny.xlsx`（5×3）、`big.xlsx`（200×25）、`multi.xlsx`（≥3 sheet）；
  - `sample.pdf`、`sample.docx`、`big.docx`（≥150KB HTML）；
  - `slides.pptx`；
  - 51MB 假 xlsx（用 `dd if=/dev/zero of=huge.xlsx bs=1m count=51`）。
- **输出**：人工记录验收结果。
- **验收**：10 条 AC 全部通过；如有问题在 `revision.md` 记录修复。

---

## 里程碑

| M | 涵盖任务 | 标志 |
| --- | --- | --- |
| M1 | T3-01 ~ T3-02 | 依赖与第三方库就位，`cargo check` 通过。 |
| M2 | T3-03 ~ T3-07 | 后端可对四种格式返回正确 payload（含大小拦截、解析失败兜底）。 |
| M3 | T3-08 ~ T3-12 | 前端四种渲染分支可见、样式正确。 |
| M4 | T3-13 | AC 全通过，结项。 |

---

## 依赖图

```
T3-01 ─┐
T3-02 ─┤
       └─► T3-03 ─► T3-04 ─┬─► T3-05 ─┐
                           ├─► T3-06 ─┤
                           ├─► T3-07 ─┤
                                       └─► T3-08 ─┐
                                          T3-09 ─┤
                                          T3-10 ─┼─► T3-11 ─► T3-12 ─► T3-13
```

---

## 风险与备注

- **PDF.js Worker 路径**：Tauri 的 `tauri://localhost` 与本地 ESM `import.meta.url` 解析需要实测；若 worker 加载失败，回退为同线程模式 `useWorker: false`（性能下降但可用）。
- **mammoth.js 体积**：~500KB；可接受。**仅按需加载**避免拖慢启动。
- **soffice 启动开销**：首次调用 ~2 s（macOS 下需启动 LibreOffice 主进程）；重复调用快很多。我们靠产物缓存避免重复转换。
- **临时目录积累**：本阶段不主动清理 `qfinder-preview`，留作后续清理任务（可在 stage4 加入启动期 LRU）。
- **大文件 calamine**：xlsx 是 zip 容器，calamine 必须解压整个 sheet1 XML，超大表（如 1000 万行）可能内存不友好；50 MiB 文件大小阈值已经足够保守。
- **未来扩展**：`.xls / .doc / .ppt` 旧格式与多 sheet 切换、PDF 翻页留待 Stage 4。

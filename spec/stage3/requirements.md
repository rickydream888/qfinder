# 需求设计文档（Stage 3）

> 基于 [target.md](target.md)。本阶段在 [src/commands/preview.rs](../../src/commands/preview.rs) 与 [ui/js/preview.js](../../ui/js/preview.js) 之上扩展，新增 4 种 PreviewPayload 分支与对应的前端渲染器。

---

## 1. 阶段目标

| # | 目标 |
| --- | --- |
| G1 | 选中 `.xlsx` 文件 → 预览区显示首 sheet 的前 100 行 × 20 列表格，超出范围明示「已截断」。 |
| G2 | 选中 `.pdf` 文件 → 预览区使用 PDF.js 渲染第 1 页为图像，可视区自适应宽度。 |
| G3 | 选中 `.docx` 文件 → 预览区使用 mammoth.js 在前端把文档转 HTML，截取前 N 段或前 N 字符。 |
| G4 | 选中 `.pptx` 文件 → 若系统存在 `soffice` 命令，则后端转出临时 PDF 并按 G2 渲染首页；否则提示「需要安装 LibreOffice 才能预览此格式」。 |
| G5 | 4 类格式各设独立的「文件大小」阈值，超过即不渲染并展示提示。 |
| G6 | 任意文件解析 / 渲染失败时，预览区显示错误信息，不影响其他面板与树。 |
| G7 | 离线运行：第三方前端库（PDF.js / mammoth.js）位于 `ui/vendor/`，纳入版本控制。 |

非目标：

- 不支持 `.xls / .doc / .ppt` 旧版二进制格式（同样需 LibreOffice，体验差异大，留作后续）。
- 不支持加密 / 受密码保护的文档（捕获错误，提示「无法解析」即可）。
- 不支持多 sheet 切换、PDF 翻页、PPTX 多页 —— 本阶段定位「首屏预览」。
- 不实现导出 / 打印 / 复制等交互。

---

## 2. 大小与内容上限

| 格式 | 文件大小上限 | 内容截断 |
| --- | --- | --- |
| `.xlsx` | 50 MiB | 首 sheet 前 100 行 × 20 列；解析期硬截断 |
| `.pdf`  | 100 MiB | 仅渲染 page 1 |
| `.docx` | 20 MiB | mammoth 转 HTML 后取前 200 个块级元素 **且** 前 100 KB HTML 字节，孰严格 |
| `.pptx` | 50 MiB（原文件） | 转出 PDF 后只渲染 page 1；转换超时 30 s |

所有上限以常量集中定义在 [preview.rs](../../src/commands/preview.rs)，便于后续调整。

---

## 3. 后端：PreviewPayload 扩展

### 3.1 新增分支（追加到既有枚举）

```rust
#[serde(rename_all = "camelCase")]
Spreadsheet {
    sheet_name: String,
    headers: Vec<String>,           // 始终是第 0 行（最多 20 列）
    rows: Vec<Vec<String>>,         // 不含 headers，最多 99 行
    total_rows: u32,                // 全表实际行数（从 calamine 拿到的 dimension）
    total_cols: u32,
    truncated_rows: bool,
    truncated_cols: bool,
    other_sheets: Vec<String>,
    size: u64,
},
#[serde(rename_all = "camelCase")]
Pdf {
    path: String,                   // 通过 convertFileSrc 直接给 PDF.js
    size: u64,
},
#[serde(rename_all = "camelCase")]
Docx {
    path: String,                   // 前端 fetch + mammoth.js 解析
    size: u64,
},
#[serde(rename_all = "camelCase")]
Pptx {
    pdf_path: String,               // 转换后的临时 PDF
    size: u64,                      // 原 pptx 大小
},
#[serde(rename_all = "camelCase")]
OfficeImage {
    image_path: String,             // Quick Look 生成的首页 PNG（适用 4 种格式）
    size: u64,
    engine: String,                 // 例: "macOS Quick Look"
},
#[serde(rename_all = "camelCase")]
Unsupported {
    reason: String,                 // 例：「需要 LibreOffice」「文件超过 50 MiB」「损坏的 xlsx」
    size: u64,
},
```

> `Unsupported` 取代「pptx 缺 LibreOffice」「文件过大」「解析失败」等多种降级情况，前端只需一个分支处理。

### 3.2 路由（preview_blocking 内）

```text
ext 是 xlsx/pdf/docx/pptx 之一：
  if cfg!(macos) && size <= 对应上限 && qlmanage 可用：
      try_quicklook(p) → OfficeImage  (优先路径)
  失败 / 未命中时走各格式原逻辑：
      xlsx → calamine    → Spreadsheet | Unsupported
      pdf  → 直通       → Pdf         | Unsupported
      docx → 直通       → Docx        | Unsupported
      pptx → LibreOffice → Pptx        | Unsupported
其它                       → 维持原 Text / Image / Other 路径
```

> macOS 上 Quick Look 为默认预览手段：装了 MS Office 后由 Office 插件渲染，忈雅度接近原版；
> 未装 Office 时使用系统默认 QL 插件。Quick Look 失败（不常见）时静默回退到原逻辑。
> 缺点：输出为位图，用户无法选中文本。

### 3.3 XLSX 解析（calamine）

- 引入依赖 `calamine = "0.34"`（features 默认即可）。
- 流程：
  1. `Sheets::open_workbook_auto(path)` → 打开 xlsx。
  2. `sheet_names()`，取第 0 个为 `sheet_name`，其余进 `other_sheets`。
  3. `worksheet_range(&sheet_name)?` 读 Range。
  4. `range.get_size()` → `(total_rows, total_cols)`。
  5. 遍历 `range.rows()`，对每行 `take(20)`，对所有行 `take(100)`，cell 转字符串：
     - `Data::Empty` → `""`
     - `Data::String(s)` → `s.to_string()`
     - `Data::Bool(b)` → `b.to_string()`
     - `Data::Int(i)` → `i.to_string()`
     - `Data::Float(f)` → 整数判断后输出（避免 `1.0` 显示）
     - `Data::DateTime(d)` → ISO8601 字符串（calamine 提供 `as_datetime()`）
     - `Data::DateTimeIso(s)` / `Data::DurationIso(s)` → 原样
     - `Data::Error(e)` → `format!("#{:?}", e)`
  6. 第 0 行作为 `headers`，其余进 `rows`。若总行数 ≤ 1 则 `headers = []`、`rows` 为该行。
  7. `truncated_rows = total_rows > 100`，`truncated_cols = total_cols > 20`。

### 3.4 PDF / DOCX

仅做存在性 + 大小校验，构造分支返回。`path` 字段使用 [platform::path_to_string](../../src/platform.rs) 规范化。

### 3.5 PPTX 与 Quick Look 报底

- macOS 上 4 种格式都默认优先走 Quick Look（`qlmanage -t -s 1600 -o <work_dir> <file>`），
  输出 PNG 后返回 `OfficeImage { image_path, size, engine }` 分支。
- LibreOffice 路径（仅 PPTX 会使用，其他格式未命中 Quick Look 时仍走原逻辑）：
  - 检测顺序：[platform::has_command](../../src/platform.rs)("soffice") / "libreoffice" / `/Applications/LibreOffice.app/Contents/MacOS/soffice`
  - `--convert-to pdf` 生成 PDF，前端走 PDF.js 首页渲染。
- 输出目录：`std::env::temp_dir().join("qfinder-preview")`。
- 缓存键：`{:x}-{mtime}-{size}` 的 djb2 哈希。
- 子进程统一 30s 超时，超时 kill。
- LibreOffice / Quick Look 都不可用 → `Unsupported`。

---

## 4. 前端：渲染层

### 4.1 第三方资源布局

```
ui/vendor/
  pdfjs/
    pdf.min.mjs               # PDF.js v4.x ESM 主库
    pdf.worker.min.mjs        # 同版本 Worker
  mammoth/
    mammoth.browser.min.js    # mammoth.js v1.x
```

由 `scripts/fetch-vendor.sh`（macOS / Linux）一次性下载并提交。各文件均带许可声明（PDF.js: Apache-2.0；mammoth.js: BSD-2-Clause），整体附 `ui/vendor/LICENSES.md`。

### 4.2 引入策略

- `pdfjs` 与 `mammoth` 均**按需懒加载**：仅在首次需要渲染对应类型时动态注入 `<script>` / `import()`。
- 加载失败（例如打包遗漏）：分支显示「无法加载预览组件」。

### 4.3 preview.js render 分支

```text
case "spreadsheet":
   渲染 sheet 名 + 截断提示 + <table class="qf-sheet">
case "pdf":
   懒加载 pdfjs → getDocument(convertFileSrc).getPage(1) → 渲染至 canvas
case "docx":
   懒加载 mammoth → fetch(convertFileSrc).arrayBuffer() → mammoth.convertToHtml({arrayBuffer})
   → 截断 HTML（先取前 100KB 字节，再用 DOMParser 解析后保留前 200 块级元素）
case "pptx":
   走 case "pdf" 同样的渲染逻辑（payload.pdfPath）
case "unsupported":
   显示 reason
```

PDF 渲染：

- 创建 `<canvas>` 宽度跟随预览容器（`clientWidth - 32`，留 padding）。
- `viewport = page.getViewport({ scale: 1 })`，`scale = container.width / viewport.width`，重新 `getViewport({scale})`。
- `page.render({ canvasContext, viewport })`。
- 渲染完成后销毁 `pdfDocument`。
- 切换文件需取消上一次渲染（`renderTask.cancel()`）。

DOCX 渲染：

- 截断算法：

  1. mammoth 输出完整 HTML 字符串 `html`；
  2. 若 `html.length > 100*1024`：`html = html.slice(0, 100*1024)` + `<p>…（已截断）</p>`；
  3. `parser.parseFromString` 后遍历 body 顶层子元素，超过 200 个移除，最后追加截断提示。
- 渲染容器加 class `qf-docx`，CSS 控制最大宽度、字号。

XLSX 渲染：

- `<thead>` 渲染 `headers`（若为空则跳过）；
- `<tbody>` 渲染 `rows`；
- 单元格做 `escapeHtml`；超长字符串 CSS `text-overflow: ellipsis`，hover 显示完整内容（`title`）。
- 头部摘要：`{sheet_name} · 共 {total_rows} 行 × {total_cols} 列`，截断时追加红字提示。

### 4.4 安全

- mammoth 输出已是清理过的 HTML（不含 `<script>`），但仍**通过 DOMParser 解析后用 textContent + 白名单标签**重新串成最终 HTML？—— 本阶段判断风险可接受（mammoth.js 是社区主流库，输出受信），不再二次清洗。
- `convertFileSrc` 需要 `protocol-asset` + `assetProtocol.scope` 包含目标路径；当前 [tauri.conf.json](../../tauri.conf.json) 配置为 `["**"]`，满足需求；后续阶段再收紧。
- PDF.js worker 通过 `import.meta.url` 解析路径，需要确认 Tauri 的 `tauri://` scheme 下 ESM 工作正常。若无法直接 `import()`，退化为 `<script type="module">` + 全局变量。

---

## 5. 错误模型

新增错误码（`AppError`）：

| code | 触发场景 |
| --- | --- |
| `TOO_LARGE` | 文件超过对应格式的上限（统一以 `Unsupported` 分支返回，不抛异常） |
| `PARSE_FAILED` | calamine / soffice 解析或转换失败 |

> 实现时为减少改动，可不新增 enum 变体，仅在 `Unsupported.reason` 中区分文案；保留 `AppError::Internal` 作为兜底。

---

## 6. CSS

新增 [ui/css/app.css](../../ui/css/app.css)：

```css
.qf-sheet           { border-collapse: collapse; font-size: 12px; }
.qf-sheet th, td    { border: 1px solid #dee2e6; padding: 2px 6px;
                      max-width: 240px; overflow: hidden;
                      text-overflow: ellipsis; white-space: nowrap; }
.qf-sheet thead th  { background: #f1f3f5; position: sticky; top: 0; }
.qf-sheet-summary   { color: #6c757d; font-size: 12px; margin-bottom: 6px; }
.qf-sheet-trunc     { color: #d6336c; }

.qf-pdf-canvas      { display: block; max-width: 100%;
                      box-shadow: 0 0 0 1px #dee2e6; margin-top: 8px; }

.qf-docx            { max-width: 880px; line-height: 1.55; }
.qf-docx p          { margin: 0 0 .6em; }
.qf-docx table      { border-collapse: collapse; }
.qf-docx td,
.qf-docx th         { border: 1px solid #dee2e6; padding: 2px 6px; }

.qf-preview-warn    { color: #b54708;
                      background: #fff7ed; border: 1px solid #fed7aa;
                      padding: 8px 12px; border-radius: 4px; }
```

---

## 7. 验收标准

| AC | 描述 |
| --- | --- |
| AC-1 | 选中一个 50 行 5 列 xlsx，表格完整显示，summary 显示 `50 行 × 5 列`，无截断提示。 |
| AC-2 | 选中一个 200 行 25 列 xlsx，表格只渲染 100 行 20 列，summary 显示原始行列数并标红「行已截断 / 列已截断」。 |
| AC-3 | 选中一个 ≥3 sheet 的 xlsx，summary 列出其它 sheet 名（仅展示，不可切换）。 |
| AC-4 | 选中 5 页 PDF，预览区显示第 1 页图像，宽度等于容器；切换到另一 PDF，旧渲染任务被取消（无重叠绘制）。 |
| AC-5 | 选中 docx，正文段落渲染，长文档末尾出现「（已截断）」提示。 |
| AC-6 | 在已安装 LibreOffice 的 macOS 上选中 pptx，~3 秒内显示首页 PDF；再次选中同一文件秒级响应（命中缓存）。 |
| AC-7 | 在未安装 LibreOffice 的环境中选中 pptx，显示「需要安装 LibreOffice 才能预览此格式」。 |
| AC-8 | 任一格式的文件超过对应大小阈值时，显示「文件过大，已跳过预览」提示与原文件大小。 |
| AC-9 | 文件损坏（手工把 xlsx 截断一半）时，预览区显示「无法解析」，应用不崩溃。 |
| AC-10 | 关闭预览面板 / 切换文件后，PDF.js 的 worker 不残留（DevTools Memory 取消引用）。 |

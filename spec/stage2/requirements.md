# 需求设计文档（Stage 2）

> 本文档基于 [target.md](target.md) 编写，目标是把文件系统树（以及预览区头部）中目录、文件的 Bootstrap Icons 替换为参考 [PKief Material Icon Theme](https://github.com/PKief/vscode-material-icon-theme) 的、按文件名 / 扩展名 / 目录名细分的 SVG 图标方案。
>
> 本阶段不改动 Stage 1 已经稳定的功能，仅新增 / 替换图标渲染相关的代码与资源。

---

## 1. 阶段目标

| # | 目标 |
| --- | --- |
| G1 | 文件系统树中的每个节点（目录 / 文件）显示与 Material Icon Theme 等价的、按类型细分的 SVG 图标。 |
| G2 | 目录的展开 / 收起状态使用不同图标（与主题一致，例如 `folder` ↔ `folder-open`、`folder-rust` ↔ `folder-rust-open`）。 |
| G3 | 预览区顶部、对话框等其他出现节点图标的位置，统一使用同一套图标方案。 |
| G4 | 仅适配 Light Mode：当主题为某个 icon 提供 `*_light` 变体时，使用 light 变体；否则使用默认变体。 |
| G5 | 资源不直接引用 `resources/` 目录；运行时从 `ui/icons/material/` 加载。 |
| G6 | 启动期与渲染期的图标解析具备可接受的性能（不得明显拖慢 Stage 1 的展开 / 滚动响应）。 |

非目标：

- 不实现 Dark Mode、High Contrast；
- 不实现“用户自定义图标 / 图标包切换 / 隐藏文件夹箭头”等扩展能力；
- 不在节点上叠加 git / 错误 / 修改状态等装饰角标。

---

## 2. 资源来源与版权

资源来源：`resources/pkief.material-icon-theme/`

```
resources/pkief.material-icon-theme/
  icons/                     # 1221 个 SVG 图标文件
  material-icons.json        # 完整的图标映射定义（约 1.2 MB）
```

约束（沿用 Stage 1）：

- `resources/` **不会** 提交到代码仓库；
- 运行时 **不得** 通过相对路径直接引用 `resources/`；
- 需要使用的资源在 **构建期 / 一次性脚本** 中拷贝到 `ui/icons/material/`，此目录纳入版本控制。

授权：Material Icon Theme 使用 MIT 协议，需在仓库根目录或 `ui/icons/material/LICENSE` 保留其原始许可声明。

---

## 3. 总体方案

### 3.1 架构

```
┌─────────────────────────────────────────┐
│ 构建期 / 一次性脚本（PowerShell + 脚本）   │
│   读 resources/.../material-icons.json   │
│   →  生成 ui/icons/material/manifest.json│
│   →  拷贝 SVG 至 ui/icons/material/svg/  │
└────────────────────┬────────────────────┘
                     │
┌────────────────────▼────────────────────┐
│ 运行时（前端 JavaScript：IconResolver）   │
│   加载 manifest.json → 构建查找表         │
│   resolveFile(name) → 图标名              │
│   resolveFolder(name, expanded) → 图标名  │
│   iconUrl(name) → ui/icons/material/svg/x │
└────────────────────┬────────────────────┘
                     │
┌────────────────────▼────────────────────┐
│ 渲染层（tree.js / preview.js）            │
│   <span class="qf-icon">                  │
│     <img src="…" alt="" loading="lazy">   │
│   </span>                                 │
└─────────────────────────────────────────┘
```

### 3.2 关键决策

| 决策 | 选择 | 理由 |
| --- | --- | --- |
| 图标交付形式 | 静态 SVG 文件（`<img src>`） | 数量大（1221 个），inline 注入会显著增大 HTML / 内存；浏览器对 SVG 文件有缓存。 |
| 是否裁剪图标集 | **不裁剪**，全量拷贝 | 目标要求“和参考一样细致”，且 SVG 总体积可接受（实际打包后约 4–6 MB）。 |
| manifest 形态 | 在 `ui/icons/material/manifest.json` 重新生成的 “瘦身” 版本，仅保留 light mode 需要的字段，并把 `iconDefinitions[*].iconPath` 还原成 `xxx.svg` 文件名。 | 减少前端解析体积、避免 `./../icons/` 路径前缀污染。 |
| 解析时机 | 启动一次，常驻内存 | Map 查找 O(1)，避免在每个节点渲染时反复 fetch。 |
| 缓存 `iconUrl` | 在 `IconResolver` 内做内存缓存（`Map<key, url>`） | 滚动 / 折叠时同名节点反复出现。 |
| 与 Bootstrap Icons 共存 | 工具栏、对话框中的纯 UI 图标继续用 Bootstrap Icons；**节点图标**（树节点 + 预览头）改用 Material 图标。 | 工具栏图标语义不属于“文件 / 目录”，无需替换。 |

---

## 4. 资源拷贝与 manifest 生成（构建期）

### 4.1 输入与输出

输入：`resources/pkief.material-icon-theme/material-icons.json`、`resources/pkief.material-icon-theme/icons/*.svg`

输出：

```
ui/icons/material/
  svg/                       # 全量 SVG（仅文件名，不带子目录）
    folder.svg
    folder-open.svg
    folder-rust.svg
    folder-rust-open.svg
    rust.svg
    …
  manifest.json              # 见 §4.2
  LICENSE                    # 复制自上游
```

### 4.2 `manifest.json` 数据结构（瘦身后）

```jsonc
{
  "defaults": {
    "file": "file",
    "folder": "folder",
    "folderExpanded": "folder-open",
    "rootFolder": "folder-root",
    "rootFolderExpanded": "folder-root-open"
  },
  // 文件名（完整名）→ icon name
  "fileNames": { ".gitignore": "git", "Cargo.toml": "cargo", … },
  // 文件扩展名（去掉前导点；包含多段，例如 "tsconfig.json"）→ icon name
  "fileExtensions": { "rs": "rust", "tsx": "react_ts", … },
  // 语言 id → icon name（用于 §5.1 步骤 3 的扩展回退）
  "languageIds": { "rust": "rust", "javascript": "javascript", … },
  // 目录名 → icon name（折叠态）
  "folderNames": { "src": "folder-src", ".github": "folder-github", … },
  // 目录名 → icon name（展开态）
  "folderNamesExpanded": { "src": "folder-src-open", … },
  // Light mode 覆盖：相同的 4 张表 + rootFolderNames(Expanded)
  "light": {
    "fileNames": { … },
    "fileExtensions": { … },
    "folderNames": { … },
    "folderNamesExpanded": { … },
    "languageIds": { … }
  }
}
```

生成规则：

1. 解析 `iconDefinitions[name].iconPath`（形如 `./../icons/<file>.svg`），仅取末尾 `<file>.svg`，记录为合法 icon 名 `name`；
2. 上面五张映射表（`fileExtensions`、`fileNames`、`folderNames`、`folderNamesExpanded`、`languageIds`）的值原样复制；
3. `defaults` 取自顶层的 `file`、`folder`、`folderExpanded`、`rootFolder`、`rootFolderExpanded`；
4. `light` 子对象只保留上述五张表 + 两张 root 表；
5. 同时拷贝所有被引用的 SVG（取并集，未被引用的直接跳过以减小体积——实际上 1221 个全部被引用，所以等价于全量拷贝）；
6. 生成结束后将 `LICENSE` 拷贝到 `ui/icons/material/LICENSE`。

### 4.3 实现形式

- 使用一个 PowerShell 脚本 `scripts/build-material-icons.ps1`（运行环境与开发机一致），手动执行；
- 不接入 `cargo build` 的 `build.rs`，避免在每次编译时拖慢；
- 文档在 [tasks.md](tasks.md) 的 T2-01、T2-02 中有详细命令。

---

## 5. 图标解析规则（运行时）

模块：`ui/js/icon-resolver.js`，导出 `window.QF.Icons`。

### 5.1 文件解析顺序

输入：`name`（节点的显示名，例如 `Cargo.toml`、`README.md`、`a.tar.gz`）。

```
1. light.fileNames[name]            ─┐
2. fileNames[name]                   ├ 完全匹配文件名
                                    ─┘
3. 多段后缀展开（最长优先）：
     例如 a.tar.gz → 试 "tar.gz" → 试 "gz"
     对每个候选 ext 依次：
        light.fileExtensions[ext]
        fileExtensions[ext]
        若值为空字符串（即映射存在但无图标，如 "js" / "ts"），
        则视为命中下一步的 languageId 回退。
4. 扩展名 → 语言 id 回退表（内置常量，见 §5.3）：
     ext → languageId → light.languageIds / languageIds
5. defaults.file（light 无 file 覆盖，统一用顶层默认）
```

匹配过程对 `name` 与扩展名 **大小写不敏感**（先 `toLowerCase()` 再查表）。

### 5.2 目录解析顺序

输入：`name`、`expanded`（布尔）、`isRoot`（布尔，仅根节点为 true）。

```
若 isRoot：
  if expanded:
    light.rootFolderNamesExpanded[name] → rootFolderNames（fallback）
    → defaults.rootFolderExpanded
  else:
    light.rootFolderNames[name] → rootFolderNames
    → defaults.rootFolder
否则：
  if expanded:
    light.folderNamesExpanded[name] → folderNamesExpanded
    → defaults.folderExpanded
  else:
    light.folderNames[name] → folderNames
    → defaults.folder
```

匹配 **大小写不敏感**。

### 5.3 扩展名 → 语言 id 回退表

主题中有部分扩展名（`js`、`ts`、`json`、`html`、`css`、`xml`、`go`、`java`、`kt`、`rb`、`php`、`sh`、`bat`、`ps1`、`yml/yaml`、`toml`、`ini`、`sql`、`c/cpp/h/hpp`、`cs`、`swift` 等）在 `fileExtensions` 中映射为空字符串，依赖 VSCode 的 `languageIds` 解析。本项目内置一张精简表覆盖常见情况：

```js
const EXT_TO_LANG = {
  js: "javascript", mjs: "javascript", cjs: "javascript",
  ts: "typescript", mts: "typescript", cts: "typescript",
  json: "json", jsonc: "json",
  html: "html", htm: "html",
  css: "css",
  xml: "xml",
  go: "go",
  java: "java", kt: "kotlin", kts: "kotlin",
  rb: "ruby", php: "php",
  sh: "shellscript", bash: "shellscript", zsh: "shellscript", fish: "shellscript",
  bat: "bat", cmd: "bat",
  ps1: "powershell", psm1: "powershell", psd1: "powershell",
  yml: "yaml", yaml: "yaml",
  toml: "toml", ini: "ini",
  sql: "sql",
  c: "c", h: "c",
  cpp: "cpp", cc: "cpp", cxx: "cpp", hpp: "cpp", hh: "cpp", hxx: "cpp",
  cs: "csharp",
  swift: "swift",
  md: "markdown", markdown: "markdown"
};
```

> 该表只用于第 4 步回退，且仅在第 3 步未命中或命中值为空字符串时使用。如果未来发现遗漏，再补即可。

### 5.4 接口定义

```js
window.QF.Icons = {
  // 启动初始化，返回 Promise；在 app.js 主流程之前 await
  init(): Promise<void>,

  // 解析节点对应的 icon name；不发生 IO
  resolveFile(name: string): string,
  resolveFolder(name: string, expanded?: boolean, isRoot?: boolean): string,

  // 把 icon name 转成可被 <img src> 使用的相对 URL
  // 例如 "rust" → "icons/material/svg/rust.svg"
  iconUrl(iconName: string): string,

  // 一站式辅助：直接产出 <img> HTML 字符串（已转义、已 lazy）
  iconImg(iconName: string, opts?: { className?: string }): string
};
```

错误处理：

- `init()` 失败时回退到“全部使用 Bootstrap Icons 旧逻辑”，并通过 `QF.showError` 弹一次性提示，不阻塞应用启动。
- `iconUrl()` 收到未知 `iconName` 时返回 `defaults.file` 的 URL（保证 `<img>` 不出现 broken）。

---

## 6. 渲染层改造

### 6.1 树节点（[ui/js/tree.js](ui/js/tree.js)）

- 替换 `createNodeEl` 中的图标段：从 `<i class="bi bi-folder-fill" …>` / `<i class="bi bi-file-earmark">` 改为 `QF.Icons.iconImg(...)` 生成的 `<img class="qf-icon-img">`；
- 在 `expand` / `collapse` 切换 `expanded` 状态时重新设置 `<img src>`（仅当目录的展开态图标与折叠态不同时才需要更新——对绝大多数目录都不同）；
- 根节点解析时传 `isRoot=true`；
- 重命名 / 拖动浮层（`dragImage`）使用同一套图标。

### 6.2 预览头（[ui/js/preview.js](ui/js/preview.js)）

- 头部的 `<i class="bi bi-folder2-open">` / `<i class="bi bi-file-earmark*">` 改为 `QF.Icons.iconImg(...)`；
- 目录预览始终使用 `expanded=true` 的图标。

### 6.3 样式（[ui/css/app.css](ui/css/app.css)）

新增：

```css
.qf-icon-img {
  width: 16px;
  height: 16px;
  flex: 0 0 16px;
  object-fit: contain;
  vertical-align: -3px;        /* 与文字基线对齐 */
  user-select: none;
  -webkit-user-drag: none;     /* 防止图标自身触发原生拖拽 */
  pointer-events: none;        /* 让点击穿透到 .qf-node */
}
.qf-preview .qf-icon-img {
  width: 20px;
  height: 20px;
  vertical-align: -4px;
}
```

原有 `.qf-icon` 的最小宽度 / 间距规则保持不变。

---

## 7. 性能与体积

| 维度 | 预算 | 说明 |
| --- | --- | --- |
| `manifest.json` 体积 | < 200 KB | 去掉 iconDefinitions / highContrast 等冗余后 |
| 启动初始化耗时 | < 50 ms | 单次 fetch + JSON.parse + 5 张 Map 构建 |
| 单节点解析耗时 | O(1)（最长扩展名链 ≤ 5 段） | 命中或穷尽即停 |
| SVG 加载 | 首次按需，浏览器缓存 | 数千节点的滚动场景会触发同图标多次复用，由浏览器 `<img>` 缓存命中 |
| 内存 | < 5 MB（manifest + 解析缓存） | 1221 张 SVG 文件常驻磁盘，未加载的不入内存 |

---

## 8. 兼容性与回退

- `QF.Icons.init()` 失败：自动启用 Bootstrap Icons 兼容模式（保留 Stage 1 的旧渲染分支），不影响主流程；
- 节点 `name` 为空字符串：按 `defaults.file` / `defaults.folder` 渲染；
- Windows 中带尾随 `\` 的路径（如 `C:\`）取不到 basename：直接使用整个 `name`（`C:\`）查表，未命中时落到根目录默认；
- 拖动 / 重命名过程中节点临时无 `name`：复用上一次的图标 URL（不重渲染）。

---

## 9. 验收标准

| # | 标准 |
| --- | --- |
| AC-1 | `Cargo.toml`、`package.json`、`Dockerfile`、`.gitignore`、`README.md`、`LICENSE` 等常见文件名显示与主题一致的专属图标。 |
| AC-2 | `.rs / .tsx / .py / .go / .json / .yaml / .md / .png / .zip / .pdf / .exe / .ttf` 等扩展名显示对应图标；`.js / .ts / .html / .css` 经语言回退后显示正确图标。 |
| AC-3 | `src / .github / node_modules / target / dist / docs / public / assets / tests` 等目录名显示主题中的对应文件夹图标，**且** 展开后图标切换为 `*-open` 变体。 |
| AC-4 | Light 变体：当主题为某 icon 提供 `*_light`（例如 `folder-jinja_light`）时使用 light 版本。 |
| AC-5 | 树滚动 / 展开 1000+ 节点时 FPS 与 Stage 1 持平，无明显卡顿。 |
| AC-6 | 关闭 / 重启应用，预览模式打开图片或预览目录时，预览头的图标也使用新方案。 |
| AC-7 | 临时移除 `ui/icons/material/manifest.json`：应用仍能启动，控制台报一次错并降级为 Bootstrap Icons 渲染。 |
| AC-8 | `resources/` 目录被删除后 `cargo run` 仍可启动并正常显示所有图标（即运行时不依赖 `resources/`）。 |

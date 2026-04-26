# 任务拆解文档（Stage 2）

> 配套文档：[target.md](target.md)、[requirements.md](requirements.md)
>
> 任务编号规则：`T2-<序号>`。每个任务都包含 **目的 / 内容 / 输出 / 验收**。

---

## 阶段 0：资源准备

### T2-01 编写资源生成脚本
- **目的**：把 `resources/pkief.material-icon-theme/` 转换成 `ui/icons/material/`（瘦身 manifest + 全量 SVG + LICENSE）。
- **内容**：
  - 新建 `scripts/build-material-icons.ps1`；
  - 步骤：
    1. 读取 `resources/pkief.material-icon-theme/material-icons.json`；
    2. 构造瘦身后的 `manifest.json`（结构见 [requirements.md §4.2](requirements.md)）：
       - `defaults` ← 顶层 `file / folder / folderExpanded / rootFolder / rootFolderExpanded`；
       - `fileNames / fileExtensions / folderNames / folderNamesExpanded / languageIds` ← 顶层同名字段；
       - `light` ← 顶层 `light` 中的同名字段（缺失则置 `{}`）；
       - 不写入 `iconDefinitions / highContrast / hidesExplorerArrows`；
    3. 创建 `ui/icons/material/svg/`，把 `resources/.../icons/*.svg` 全量拷贝；
    4. 写入 `ui/icons/material/manifest.json`（UTF-8 无 BOM、紧凑格式）；
    5. 把 `resources/pkief.material-icon-theme/LICENSE`（若存在）拷贝到 `ui/icons/material/LICENSE`，否则写入一段固定的 MIT 声明并附 PKief 仓库链接。
- **输出**：脚本文件 + 一次执行后的 `ui/icons/material/` 目录。
- **验收**：
  - `Test-Path ui/icons/material/manifest.json` 为 true；
  - `(Get-ChildItem ui/icons/material/svg -File).Count` 等于 `(Get-ChildItem resources/pkief.material-icon-theme/icons -File).Count`；
  - manifest 体积 < 200 KB；
  - JSON 通过 `ConvertFrom-Json` 解析无误。

### T2-02 首次执行脚本并提交
- **目的**：把生成产物纳入版本控制。
- **内容**：
  - 执行 `pwsh -File scripts/build-material-icons.ps1`；
  - 在 `.gitignore` 中确认 `ui/icons/material/` 未被忽略；
  - `resources/pkief.material-icon-theme/` 仍由 `.gitignore` 忽略。
- **输出**：纳入仓库的 `ui/icons/material/`。
- **验收**：`git status` 显示 `ui/icons/material/` 为新增；`resources/` 子项不出现。

---

## 阶段 1：前端解析层

### T2-03 实现 `IconResolver`
- **目的**：在前端按 [requirements.md §5](requirements.md) 实现解析逻辑。
- **内容**：
  - 新建 `ui/js/icon-resolver.js`；
  - 暴露 `window.QF.Icons`，包含：
    - `init()`：fetch `icons/material/manifest.json`（相对 `ui/index.html` 的路径），解析后构建 5 张 `Map`（统一 lower-case 化 key）；保存 `defaults`；
    - `resolveFile(name)`、`resolveFolder(name, expanded, isRoot)`、`iconUrl(name)`、`iconImg(name, opts)`；
  - 内存缓存：
    - `_resolveCacheFile: Map<lowerName, iconName>`；
    - `_resolveCacheFolder: Map<"<lowerName>|exp|root", iconName>`；
    - `_urlCache: Map<iconName, url>`；
  - 错误降级：`init()` 抛错时设置 `this._fallback = true`，所有 `resolveXxx` 返回特殊哨兵 `__bootstrap__`，渲染层据此回退到旧的 `<i class="bi …">`。
- **输出**：`ui/js/icon-resolver.js`。
- **验收**：
  - 浏览器控制台调 `await QF.Icons.init(); QF.Icons.resolveFile("Cargo.toml")` 返回 `cargo`；
  - `QF.Icons.resolveFile("a.tar.gz")` 返回 `tar` 或主题中实际命中的图标名；
  - `QF.Icons.resolveFolder("src", true)` 返回 `folder-src-open`；
  - `QF.Icons.resolveFolder("src", false)` 返回 `folder-src`；
  - `QF.Icons.iconUrl("rust")` === `"icons/material/svg/rust.svg"`。

### T2-04 在 `index.html` 中引入并尽早 `init`
- **目的**：让 `IconResolver` 在 FileTree 创建之前就绪。
- **内容**：
  - 在 [ui/index.html](ui/index.html) 中按顺序引入 `tauri-api.js → icon-resolver.js → preview.js → tree.js → shortcuts.js → app.js`；
  - 修改 [ui/js/app.js](ui/js/app.js) 启动入口：在创建 FileTree / PreviewPane 之前 `await QF.Icons.init();`；失败时调用 `QF.showError` 并继续启动（fallback 模式）。
- **输出**：修改后的 `index.html` / `app.js`。
- **验收**：启动后 DevTools Network 中能看到 `manifest.json` 200 响应；删除 manifest 后启动应用仍能加载（树仍可用，节点退化为 Bootstrap Icons）。

---

## 阶段 2：渲染层接入

### T2-05 替换 `tree.js` 中的节点图标
- **目的**：树节点使用 Material 图标。
- **内容**：
  - 在 `createNodeEl` 中：
    - 计算 `iconName = entry.isDir ? QF.Icons.resolveFolder(entry.name, false, !!entry.isRoot) : QF.Icons.resolveFile(entry.name);`
    - 渲染 `<img class="qf-icon-img" src="${QF.Icons.iconUrl(iconName)}" alt="" draggable="false" loading="lazy">`；
    - 当 `iconName === "__bootstrap__"` 时回退到旧的 `<i class="bi …">` HTML；
  - 在 `expand(node)` / `collapse(node)` 中：
    - 仅对目录重新计算 `iconName`（传入新的 `expanded`），用 `imgEl.src = QF.Icons.iconUrl(iconName)` 直接更新；
  - 重命名完成（`finishRename`）后用同一逻辑刷新当前节点图标；
  - `dragImage` 浮层中也使用 `QF.Icons.iconImg`，保持视觉一致。
- **输出**：修改后的 [ui/js/tree.js](ui/js/tree.js)。
- **验收**：
  - 启动后家目录、`src`、`target`、`Cargo.toml`、`README.md` 等显示与主题一致的图标；
  - 展开 `src` 目录后，其图标从 `folder-src` 变为 `folder-src-open`；
  - 拖动节点时浮层显示 Material 图标。

### T2-06 替换 `preview.js` 中的预览头图标
- **目的**：预览区头部使用同一图标方案。
- **内容**：
  - `render(path, payload)` 中根据 `payload.kind` 决定：
    - 目录：`QF.Icons.resolveFolder(name, true, false)`；
    - 文件（Text / Image / ImageTooLarge / Other）：`QF.Icons.resolveFile(name)`；
  - 用 `QF.Icons.iconImg(iconName, { className: "qf-icon-img" })` 替换原 `<i class="bi …">`；
  - 加载中 `qf-loading` 模块不变。
- **输出**：修改后的 [ui/js/preview.js](ui/js/preview.js)。
- **验收**：选中目录 / 各类型文件后预览头部图标正确切换。

### T2-07 调整样式
- **目的**：保证图标尺寸、对齐、不干扰交互。
- **内容**：在 [ui/css/app.css](ui/css/app.css) 新增 `.qf-icon-img` 与 `.qf-preview .qf-icon-img` 规则（见 [requirements.md §6.3](requirements.md)）；旧的 `.qf-icon` 规则保留。
- **输出**：修改后的 `app.css`。
- **验收**：树中文字与图标对齐；图标自身不响应点击（点击仍能选中节点）；图标不可被原生拖拽（不会触发浏览器“另存图片”）。

---

## 阶段 3：联调与回归

### T2-08 端到端联调
- **目的**：验证所有 [requirements.md §9](requirements.md) 的验收标准。
- **内容**：在 Windows 环境下：
  1. `cargo run` 启动；
  2. 展开家目录、`C:\`，逐项核对 AC-1 ~ AC-6；
  3. 删除 `ui/icons/material/manifest.json`，重新启动，验证 AC-7；
  4. 把 `resources/` 目录临时改名，再次 `cargo run`，验证 AC-8；测试结束后恢复。
- **输出**：通过的人工验证；如有问题记录到 [revision.md](revision.md)。
- **验收**：8 条 AC 全部通过。

### T2-09 性能抽测
- **目的**：确认引入图标资源后未拖慢主要交互（AC-5）。
- **内容**：
  - 选一个含 1000+ 子项的目录展开（如 `node_modules` 或大型驱动器根）；
  - DevTools Performance 录制一次滚动 + 一次折叠 / 展开；
  - 对比 Stage 1 同操作的帧时间（口头记录即可）；
- **输出**：人工记录结论。
- **验收**：无明显掉帧；启动时 `manifest.json` 解析 < 50 ms。

---

## 里程碑

| M | 涵盖任务 | 标志 |
| --- | --- | --- |
| M1 | T2-01 ~ T2-02 | `ui/icons/material/` 资源齐备，可在浏览器手动打开 SVG 验证。 |
| M2 | T2-03 ~ T2-04 | 控制台可手动调用 `QF.Icons.resolveXxx`，返回值正确；应用启动不报错。 |
| M3 | T2-05 ~ T2-07 | UI 中所有节点 / 预览头的图标已切换为 Material 方案。 |
| M4 | T2-08 ~ T2-09 | 全部 AC 通过；性能可接受；可结项。 |

---

## 依赖图

```
T2-01 ──► T2-02 ──► T2-03 ──► T2-04 ──► T2-05 ──► T2-08 ──► T2-09
                                ├─────► T2-06 ──┘
                                └─────► T2-07 ──┘
```

---

## 风险与备注

- **SVG 文件名重复风险**：上游 `iconPath` 形如 `./../icons/<file>.svg`，文件名在 `icons/` 内本就唯一，直接平铺到 `svg/` 下不会冲突。脚本应额外校验：发现冲突立即中断。
- **路径分隔符**：manifest 中所有 key 由开发期生成，已经是 `/` 风格；前端 `iconUrl` 也只产出 `/` 风格 URL，不存在 Windows `\` 转义问题。
- **大写文件系统**：解析层全部 lower-case 化 key 与查询值，匹配大小写不敏感。
- **未匹配项**：兜底显示 `defaults.file / folder / folderExpanded`，保证不出现 broken `<img>`。
- **未来 Dark Mode**：本阶段不做，但 manifest 结构已为 `light` 预留对称的 `highContrast / dark` 字段位置（需要时再追加）。

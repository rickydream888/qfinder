# 需求设计文档（Stage 1）

> 本文档基于 [target.md](target.md) 编写，旨在把目标文档的描述细化为可实现、可验证的需求规格，为后续编码与拆解提供依据。

## 1. 项目概述

### 1.1 项目名称
`qfinder`：一个跨平台的文件管理与预览工具。

### 1.2 技术栈
| 层级 | 技术 |
| --- | --- |
| 桌面框架 | Tauri **2.x**（不得使用 1.x） |
| 后端语言 | Rust（edition 2024，见 [Cargo.toml](Cargo.toml)） |
| 前端 | 原生 HTML + CSS + JavaScript |
| 前端依赖 | Bootstrap（CSS / JS）、Bootstrap Icons、jQuery |
| 构建工具 | Cargo、Tauri CLI；**禁止** 使用 node.js / npm / npx / yarn / pnpm / vite 等前端构建工具 |

### 1.3 平台支持
Windows、macOS、Linux 三端，仅 Light Mode。

### 1.4 资源管理约定
- `resources/` 目录不会提交到仓库，仅作为开发期素材；
- 所有需要被运行时使用的前端静态资源必须复制到 Tauri 的 `dist`（或等价目录，例如 `ui/`）后引用；
- 不得在前端 HTML/JS 中通过相对路径直接引用 `resources/` 目录。

---

## 2. 总体功能架构

应用包含 **两种工作模式**，通过顶部的“模式切换按钮”切换：

1. **管理模式（默认）**：左右两列文件系统树，支持跨树拖拽 / 复制 / 剪切 / 粘贴；
2. **预览模式**：左侧为文件系统树，右侧为预览区。

界面结构自顶向下：

```
┌─────────────────────────────────────────────────────────────┐
│ TopBar：[模式切换按钮]                       [后台任务展示区] │
├─────────────────────────────────────────────────────────────┤
│ Body：                                                       │
│   - 管理模式： [FileTree A] | [FileTree B]                   │
│   - 预览模式： [FileTree]   | [PreviewPane]                  │
└─────────────────────────────────────────────────────────────┘
```

每个 `FileTree` 顶部有独立的 “显示隐藏目录和文件” 复选框（默认未选中）。

---

## 3. 功能需求

### 3.1 文件系统树（FileTree）

#### 3.1.1 根节点（按平台）

| 平台 | 根节点列表 |
| --- | --- |
| Windows | 用户家目录；所有驱动器（含网络驱动器） |
| macOS | 用户家目录；系统根目录（一般为 `Macintosh HD`，即 `/`）；iCloud 目录（若存在）；`/Volumes` 下除系统盘外的挂载卷 |
| Linux | 用户家目录；系统根目录 `/`；挂载的光盘和其他可移动介质（典型路径如 `/media/<user>`、`/mnt`、`/run/media/<user>`） |

> 由后端 Rust 通过 `tauri::command` 提供 `list_roots()` 接口返回平台特定的根列表。

#### 3.1.2 节点展开 / 收起
- 点击节点前的展开/收起符号（▶/▼）触发展开或收起；
- 双击节点（仅目录）等效于展开 / 收起；
- **每次展开都重新调用后端加载该目录下的子项**（不缓存旧数据）；
- 展开为 **向下层级式**（缩进式 tree），不得在右侧水平展开新窗格（区别于 macOS Finder 的 columns 视图）；
- 加载过程中节点显示 loading 占位（如旋转图标）。

#### 3.1.3 隐藏文件
- 顶部 checkbox 控制本树是否展示隐藏目录和文件；
- 平台规则：
  - Unix（macOS、Linux）：以 `.` 开头视为隐藏；
  - Windows：文件属性带 `HIDDEN` 或 `SYSTEM` 视为隐藏；
- 切换 checkbox 时刷新 **当前已展开的所有节点**。

#### 3.1.4 选中与高亮
- 单击选中节点：仅给被选中的节点加背景色（高亮），其它区域无背景；
- 选中状态在树内单选，跨树时各自维持自己的选中状态；
- 仅当前选中节点的再次单击触发重命名（见 3.1.7），不要把"hover 背景色"误用为选中。

#### 3.1.5 拖拽
- 支持鼠标拖动目录或文件以 **移动**（不是复制）到目标目录；
- 拖动过程中必须有视觉反馈（如半透明的拖动幻影或自定义 drag image）；
- 目标节点（drop target）在 hover 期间需高亮提示；
- 同树内、跨树（管理模式下两列树之间）均可拖拽；
- 不能将目录拖入自身或其子目录，否则弹窗提示非法操作；
- 拖拽完成提交一个后台任务（见 3.3）。

#### 3.1.6 键盘快捷键
| 操作 | Windows / Linux | macOS |
| --- | --- | --- |
| 复制 | `Ctrl+C` | `Cmd+C` |
| 剪切 | `Ctrl+X` | `Cmd+X` |
| 粘贴 | `Ctrl+V` | `Cmd+V` |
| 删除 | `Delete` | `Delete` |

- “删除” = 移动到回收站 / 废纸篓 / 垃圾桶（**可恢复**），不得真删；
- “先剪切再粘贴” 必须等价于 **移动**（rename / move），不得通过 “复制 + 删除” 实现，避免数据丢失风险；
- 粘贴目标 = 当前选中的目录；若选中的是文件，粘贴到该文件所在目录；
- 跨树粘贴：从树 A 中剪/复制，在树 B 选中目标后粘贴，必须支持。

#### 3.1.7 重命名（inline）
- 触发条件：节点处于已选中状态时，**再次单击** 进入 inline 重命名输入框；
- 必须严格区分双击（展开 / 打开）与 “单击 → 间隔 → 单击”：实现时通过判断 “单击发生时已选中”+“与上次 click 的时间间隔 > 双击阈值（约 400ms）” 来识别；
- 输入框获得焦点并全选文件名（不含扩展名）；
- `Enter` 提交，`Esc` 取消；失焦默认提交；
- 重名校验交给后端：若目标已存在，弹窗报错并恢复原名；
- 提交后形成一个后台 “重命名” 任务（见 3.3）。

#### 3.1.8 双击文件
- 双击目录 → 展开 / 收起；
- 双击文件 → 调用系统默认程序打开（Tauri `opener` plugin 或 `open` crate）。

#### 3.1.9 视觉规范
- 不要给整个树或行加背景，**只有选中项** 有背景色；
- 目录与文件用 Bootstrap Icons 区分（如 `bi-folder` / `bi-folder2-open` / `bi-file-earmark`）。

### 3.2 预览区（PreviewPane）

仅在 **预览模式** 下生效。根据当前选中项类型展示不同内容：

#### 3.2.1 选中目录
展示：
- 直接子目录数量（不递归）；
- 直接子文件数量（不递归）；
- 总磁盘用量：
  - macOS / Linux 且系统存在 `du` 命令时，调用 `du -sk <path>` 并将结果换算为人类友好（B / KB / MB / GB / TB，2 位小数）；
  - Windows 或不存在 `du` 时，本字段不展示（或显示 "—"）。

#### 3.2.2 选中纯文本文件
- 判定规则：
  - 优先按扩展名白名单（如 `.txt .md .markdown .html .htm .css .js .ts .json .xml .yaml .yml .toml .ini .conf .log .csv .tsv .rs .py .java .c .cpp .h .hpp .sh .bat .ps1`）；
  - 若系统存在 `file` 命令（macOS / Linux），可二次确认 `file --mime <path>` 输出含 `text/` 即视为文本；
- 文件大小 > 10 KB 时，仅读取最前 10 KB 用于预览；
- 预览区内换行（保留原始换行符），并按预览区尺寸做滚动 / 截断（超出部分不显示，无横向滚动则换行）；
- 字符编码统一按 UTF-8 解析，非 UTF-8 字符以替换符显示，不报错。

#### 3.2.3 选中可显示的图片文件
- 支持类型：`png jpg jpeg gif bmp webp svg ico`；
- 文件大小 ≤ 20 MB：在预览区按 contain 规则等比缩放显示；
- 文件大小 > 20 MB：不渲染图片，显示 "文件大小：X.XX MB，超过预览限制"。

#### 3.2.4 其它类型
- 显示文件大小（人类友好单位）。

#### 3.2.5 通用
- 切换选中或切换模式时立即刷新预览；
- 预览模式下若未选中任何项，预览区显示提示信息（如 "请在左侧选择一个目录或文件"）。

### 3.3 后台任务系统

#### 3.3.1 任务类型
- `Rename`（重命名 X 到 Y）
- `Delete`（删除 X，移动到回收站）
- `Copy`（复制 X 到 Y）
- `Move`（移动 X 到 Y，含剪切+粘贴）

#### 3.3.2 并发约束
- **同一时间只允许一个后台任务执行**；
- 已有未完成任务时再触发新任务（含拖拽、快捷键、重命名提交等）：弹出模态对话框 "已有后台任务正在运行，暂不支持添加新的任务"，不创建新任务；
- 在任务进行期间仍允许：
  - 文件树展开 / 收起；
  - 选中节点；
  - 预览目录或文件。

#### 3.3.3 任务展示区（位于 TopBar 右侧）
- 文本内容：
  - `Rename` → "重命名 <旧名> 到 <新名>"
  - `Delete` → "删除 <路径>"
  - `Copy` → "复制 <源> 到 <目标>"
  - `Move` → "移动 <源> 到 <目标>"
- 计时：以秒为单位显示已运行时间（前端定时器，每 500ms 刷新）；
- 状态：`进行中` / `已完成`；
- 任务完成后保留显示 N 秒（建议 5s）后清空，或显式由用户关闭。

#### 3.3.4 后端接口
后端使用一个 `Mutex<Option<Task>>` 或类似全局状态确保串行；通过 Tauri `event` 向前端推送 `task://started`、`task://progress`、`task://finished`、`task://failed`。

### 3.4 顶部模式切换
- 默认进入 **管理模式**，按钮文字为 "切换到预览模式"；
- 点击后切换到 **预览模式**，按钮文字变为 "切换到管理模式"；
- 切换时保留各树的当前展开状态与选中状态（实现层面：树组件常驻，仅切换右侧面板）。

### 3.5 错误与权限
- 任何因权限不足、路径不存在、目标已存在、非法目标（如目录拖入自身子目录）等导致的失败：
  - 后端返回结构化错误（`code` + `message`）；
  - 前端弹出 Bootstrap modal 提示用户错误信息；
  - **不执行** 该操作，不破坏现有状态。

---

## 4. 非功能需求

| 维度 | 要求 |
| --- | --- |
| 性能 | 单目录子项 ≤ 5000 时展开应在 1s 内完成；预览文本截断为 10 KB 以避免阻塞 |
| 可用性 | 操作有视觉反馈（hover、loading、拖动幻影、对话框） |
| 可移植 | 不能内嵌平台专属 shell 脚本（`du`、`file` 调用前需 `which` 探测） |
| 安全 | 所有文件路径在后端二次校验，禁止前端直接拼接命令；删除统一走回收站 API |
| 国际化 | 当前仅简体中文文案 |
| 主题 | 仅 Light Mode |

---

## 5. 模块划分

### 5.1 Rust 后端模块（`src/`）

| 模块 | 职责 |
| --- | --- |
| `main.rs` | Tauri 应用入口，注册命令、初始化插件 |
| `commands/roots.rs` | `list_roots()` 平台特定根节点 |
| `commands/fs_tree.rs` | `read_dir(path, show_hidden)` 返回子项列表 |
| `commands/preview.rs` | `preview(path)` 返回预览数据（目录摘要 / 文本片段 / 图片元数据 / 大小） |
| `commands/ops.rs` | `rename / copy / move_ / delete_to_trash` 提交后台任务 |
| `task/mod.rs` | 全局任务管理器（`Mutex<Option<Task>>`），事件推送 |
| `platform/mod.rs` | 平台差异封装（隐藏判定、根列表、`du` / `file` 探测） |
| `error.rs` | 统一错误类型 + 序列化 |

### 5.2 前端模块（`ui/`，从 `resources/` 拷贝并扩展）

| 文件 | 职责 |
| --- | --- |
| `index.html` | 主页面骨架（TopBar + Body） |
| `css/app.css` | 自定义样式（含 inline rename、选中高亮、拖拽幻影） |
| `css/bootstrap.min.css` | Bootstrap 样式 |
| `js/app.js` | 应用入口，模式切换、对话框管理 |
| `js/tree.js` | 文件树组件（展开/选中/重命名/拖拽） |
| `js/preview.js` | 预览面板渲染 |
| `js/shortcuts.js` | 快捷键（依平台分发 Ctrl/Cmd） |
| `js/tauri.js` | 与后端 `invoke` / `event` 的封装 |
| `js/jquery.min.js` `js/bootstrap.min.js` | 第三方依赖 |
| `icons/...` | Bootstrap Icons 字体与样式 |

> Windows 路径中的 `\` 在 JS 字符串里要使用 JSON / 模板转义（或一律以 forward-slash 形式传输，仅显示时本地化），避免转义 bug。

---

## 6. 后端 API 概要

```text
invoke("list_roots")                        -> Vec<RootEntry>
invoke("read_dir", { path, showHidden })    -> Vec<DirEntry>
invoke("preview",  { path })                -> PreviewPayload
invoke("op_rename", { path, newName })      -> TaskId
invoke("op_copy",   { src, dstDir })        -> TaskId
invoke("op_move",   { src, dstDir })        -> TaskId   // 含拖拽与剪切粘贴
invoke("op_delete", { path })               -> TaskId   // 移到回收站
invoke("open_default", { path })            -> ()
invoke("current_task")                      -> Option<TaskInfo>

event "task://started"  : TaskInfo
event "task://finished" : TaskInfo
event "task://failed"   : { id, message }
```

数据结构：

```rust
struct RootEntry { label: String, path: String, kind: RootKind }
enum  RootKind   { Home, SystemRoot, Drive, Volume, ICloud, Removable }

struct DirEntry  { name: String, path: String, is_dir: bool, is_hidden: bool, size: Option<u64> }

enum  PreviewPayload {
    Directory { sub_dirs: u64, sub_files: u64, total_size: Option<u64> },
    Text      { content: String, truncated: bool, total_size: u64 },
    Image     { path: String, size: u64 },
    ImageTooLarge { size: u64 },
    Other     { size: u64 },
}

struct TaskInfo {
    id: String,
    kind: TaskKind,           // Rename | Copy | Move | Delete
    description: String,
    started_at_ms: u64,
    status: TaskStatus,       // Running | Done | Failed
}
```

---

## 7. 关键实现注意点（来自 target.md "注意点"）

1. 先剪切再粘贴 ≡ 移动（`fs::rename` 或 `move_items_to`），**不得** 用 “复制 → 删除” 实现；
2. 所有路径在 JS 内部统一以 forward-slash 形式持有，仅展示时本地化；后端接收路径后用 `PathBuf::from` 重建；
3. 拖拽必须设置自定义 `dragImage`（或使用半透明克隆节点）；
4. 双击展开 / 单击-间隔-单击 重命名通过 `dblclick` 与基于时间窗口的状态机区分；
5. 删除统一走 Tauri `trash` 能力（如 [`trash`](https://crates.io/crates/trash) crate）；
6. 严禁引入任何 node.js / npm / npx 工具链。

---

## 8. 验收用例（节选）

- AC-01：默认启动进入管理模式，TopBar 按钮显示 "切换到预览模式"；
- AC-02：Windows 上根节点包含家目录与所有驱动器（含网络驱动器）；
- AC-03：勾选 “显示隐藏目录和文件” 后，已展开节点立即出现 `.foo` / 系统隐藏文件；
- AC-04：双击目录展开 / 收起；选中后再单击进入 inline 重命名（与双击不冲突）；
- AC-05：跨左右两树拖拽文件触发 `Move` 任务；
- AC-06：删除文件后能在系统回收站中找到并恢复；
- AC-07：选中 30 KB 的 `.md` 文件，预览仅显示前 10 KB，且 UI 标识 "已截断"；
- AC-08：选中 25 MB 的 `.png`，提示 "文件大小：25.00 MB，超过预览限制"；
- AC-09：执行复制时，再次触发任何文件操作均弹窗 "已有后台任务正在运行"；
- AC-10：将目录拖入其自身子目录时弹窗提示非法操作并不执行任何变更。

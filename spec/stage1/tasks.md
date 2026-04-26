# 任务拆解文档（Stage 1）

> 配套文档：[target.md](target.md)、[requirements.md](requirements.md)
>
> 任务编号规则：`T<阶段>-<序号>`。每个任务都包含 **目的 / 输入 / 输出 / 验收**，便于独立完成与验收。
>
> 推荐执行顺序与依赖见末尾 "里程碑" 与 "依赖图"。

---

## 阶段 0：项目脚手架

### T0-01 初始化 Tauri 2 项目骨架
- **目的**：在现有空白 Cargo 项目上接入 Tauri 2，能 `cargo tauri dev` 启动空白窗口。
- **内容**：
  - 在 [Cargo.toml](Cargo.toml) 添加 `tauri = "2"`、`tauri-build = "2"`、`serde`、`serde_json`、`thiserror`、`anyhow`、`tokio`（with `rt-multi-thread`、`fs`、`process`）依赖；
  - 添加 `[build-dependencies] tauri-build = "2"`；
  - 创建 `tauri.conf.json`，设置：
    - `productName = "qfinder"`
    - `identifier = "com.qfinder.app"`
    - `app.windows[0]` 默认尺寸 1280×800、`title = "qfinder"`
    - `app.security.csp` 合理配置（允许加载本地资源、Bootstrap CSS / JS）
    - `build.frontendDist = "ui"`、`build.devUrl` 留空（无 dev server）
  - 创建 `build.rs` 调用 `tauri_build::build()`；
  - 修改 [src/main.rs](src/main.rs) 改为 `tauri::Builder::default().run(...)`；
- **输出**：可运行的最小 Tauri 应用窗口，加载 `ui/index.html`（占位）。
- **验收**：本机 `cargo run` 能弹出窗口，标题 "qfinder"。

### T0-02 前端目录与资源迁移
- **目的**：建立独立 `ui/` 目录并从 `resources/` 拷贝一次性资源。
- **内容**：
  - 新建 `ui/`、`ui/css/`、`ui/js/`、`ui/icons/`、`ui/icons/fonts/`；
  - 从 `resources/` 拷贝 Bootstrap CSS/JS、jQuery、Bootstrap Icons 到 `ui/`；
  - 创建 `ui/index.html` 引入这些资源（顺序：jquery → bootstrap.bundle → app.js）；
  - 在 `.gitignore` 中保留对 `resources/` 的忽略；
- **验收**：`ui/index.html` 在浏览器或 Tauri 窗口打开后样式生效，控制台无 404。

### T0-03 平台与命令注册脚手架
- **目的**：建立 Rust 模块文件结构。
- **内容**：
  - 创建 `src/lib.rs`（如使用 lib + bin 模式）或在 `main.rs` 内 `mod`：
    - `mod commands { pub mod roots; pub mod fs_tree; pub mod preview; pub mod ops; }`
    - `mod task;`
    - `mod platform;`
    - `mod error;`
  - 在 `Builder` 中通过 `.invoke_handler(tauri::generate_handler![...])` 注册占位命令（先返回 `unimplemented!()` 或空数据）；
- **验收**：`cargo build` 成功，前端可成功 `invoke("list_roots")`（即便返回空）。

---

## 阶段 1：基础数据通道（前后端通信）

### T1-01 错误类型与序列化
- **目的**：统一前后端错误协议。
- **内容**：在 `src/error.rs` 定义 `enum AppError { Io, Permission, NotFound, AlreadyExists, IllegalTarget, BusyTask, Internal(String) }`；实现 `serde::Serialize` 输出 `{ code, message }`；提供 `pub type AppResult<T> = Result<T, AppError>`。
- **验收**：命令返回 `AppResult` 时前端能拿到结构化错误对象。

### T1-02 前端 invoke / event 封装
- **目的**：在 `ui/js/tauri.js` 封装 `invoke(cmd, args)` 与 `listen(event, cb)`，统一错误弹窗。
- **内容**：基于 `window.__TAURI__` API 包装 Promise；错误统一进入 `showErrorDialog(err)`（Bootstrap modal）。
- **验收**：调用一个会失败的命令能弹出错误对话框，含 `code`、`message`。

---

## 阶段 2：文件系统数据层

### T2-01 平台根节点 `list_roots`
- **目的**：实现按平台返回根节点列表。
- **内容**：
  - `platform::roots()`：
    - Windows：`std::env::var("USERPROFILE")` + 枚举驱动器（`GetLogicalDrives` 或迭代 `A:\..Z:\` 检查 `Path::new("X:\\").exists()`），网络驱动器（`WNetEnumResource` 或简单复用 `GetLogicalDrives` 已包含的映射盘）；
    - macOS：`$HOME`、`/`、`~/Library/Mobile Documents/com~apple~CloudDocs`（若存在），`/Volumes/*` 排除系统卷；
    - Linux：`$HOME`、`/`、`/media/<user>/*`、`/run/media/<user>/*`、`/mnt/*`（仅存在则列出）；
  - 注册 `#[tauri::command] fn list_roots() -> AppResult<Vec<RootEntry>>`；
- **验收**：在三个平台调用返回的根节点符合规范。

### T2-02 目录读取 `read_dir`
- **目的**：返回某路径下子项，支持隐藏过滤。
- **内容**：
  - `#[tauri::command] fn read_dir(path: String, show_hidden: bool) -> AppResult<Vec<DirEntry>>`；
  - 使用 `std::fs::read_dir`，对每项 `metadata()` 取 `is_dir`、`len`；
  - 隐藏判定：
    - Unix：`name.starts_with('.')`；
    - Windows：调用 `std::os::windows::fs::MetadataExt::file_attributes()` 检查 `FILE_ATTRIBUTE_HIDDEN | FILE_ATTRIBUTE_SYSTEM`；
  - 返回结果按 `(is_dir desc, name asc, case-insensitive)` 排序；
- **验收**：勾/不勾 `show_hidden` 时返回结果集合差异符合预期；权限不足返回 `AppError::Permission`。

### T2-03 系统命令探测工具
- **目的**：封装 `which("du")` / `which("file")`，仅在存在时使用。
- **内容**：`platform::has_command(name) -> bool`，使用 `which` crate 或自实现 PATH 扫描。
- **验收**：单元测试覆盖 “存在 / 不存在” 两种情形。

---

## 阶段 3：预览能力

### T3-01 文件类型与大小判定
- **目的**：实现扩展名白名单 + `file --mime` 兜底判定。
- **内容**：`preview::classify(path) -> Kind { Text | Image | Other }`；
- **验收**：常见扩展名命中正确；存在 `file` 命令时对无扩展名文本能识别。

### T3-02 预览命令 `preview`
- **目的**：返回 `PreviewPayload`。
- **内容**：
  - 目录：统计直接子目录 / 子文件数；macOS / Linux 且 `du` 存在时调 `du -sk`，将结果换算字节并人类可读；
  - 文本：读取最多 10 KB（`take(10240)`），UTF-8 lossy 解码；返回 `truncated` 标志与 `total_size`；
  - 图片：`size > 20MB` 走 `ImageTooLarge`，否则返回 `Image { path, size }`，前端直接 `<img src="convertFileSrc(path)">`；
  - 其它：返回 `Other { size }`；
- **验收**：分别对目录、`.md`、`.png`、`.zip` 选中时返回结构正确。

### T3-03 前端 PreviewPane 渲染
- **目的**：在 `ui/js/preview.js` 根据 payload 渲染。
- **内容**：
  - 目录：表格展示子目录数 / 文件数 / 总大小（缺省显示 "—"）；
  - 文本：`<pre>` 元素保留换行，超出区域纵向滚动或截断；显示 "已截断（仅展示前 10KB）" 提示；
  - 图片：`<img class="preview-image">`，CSS `max-width:100%; max-height:100%; object-fit:contain;`；
  - 大图 / 其它：显示 “文件大小：X.XX MB” 与必要提示；
- **验收**：手动用各类文件验证 UI 正确。

---

## 阶段 4：文件树前端组件

### T4-01 树视图骨架
- **目的**：在 `ui/js/tree.js` 实现可复用的 `FileTree` 组件（构造 `new FileTree(containerEl, options)`）。
- **内容**：
  - 顶部 checkbox "显示隐藏目录和文件"；
  - 加载根节点 → 渲染列表项：`<div class="ft-node" data-path>` 含展开符号 + 图标 + 名称；
  - 缩进通过 `padding-left: depth * 16px`；
  - 仅给 `.selected` 加背景色，行无 hover 背景；
- **验收**：树能渲染、初始无展开。

### T4-02 展开 / 收起 + 重新加载
- **目的**：单击展开符或双击节点切换展开；展开时调用 `read_dir` 重新加载。
- **内容**：
  - `dblclick` 事件区分目录（展开/收起）与文件（系统打开，调 `open_default`）；
  - 展开过程显示 loading spinner（Bootstrap `spinner-border-sm`）；
- **验收**：每次展开都触发一次 `read_dir`；UI 状态正确。

### T4-03 选中状态
- **目的**：单击节点选中，并高亮；树间互相独立。
- **内容**：维护 `this.selectedPath`；触发 `tree:select` 自定义事件供外部监听。
- **验收**：跨树点击各自高亮，互不干扰；选中变化驱动预览。

### T4-04 inline 重命名
- **目的**：实现 “选中后再单击 → 进入重命名输入框”。
- **内容**：
  - 状态机：`click` → 若 `target` 已是 `selectedPath` 且距上次 `click` ≥ 400ms 且不在 `dblclick` 窗口内 → 进入重命名；
  - 用 `<input type="text">` 替换名称文本，`Enter` 提交（调 `op_rename`），`Esc` 取消，失焦默认提交；
  - 输入框自动选中文件名主体（不含扩展名）；
- **验收**：双击不会误触发重命名；快速连击仍走 dblclick。

### T4-05 拖拽移动
- **目的**：使用 HTML5 DnD 实现移动操作。
- **内容**：
  - `draggable="true"`；`dragstart` 设置 `dataTransfer` 携带源路径，并设自定义 dragImage（节点的克隆）；
  - 目标目录在 `dragover` 时高亮，`drop` 调 `op_move`；
  - 校验：目标必须是目录；目标不能等于源或源的子路径；
  - 跨树支持（管理模式两列树之间）；
- **验收**：拖拽有视觉反馈；非法目标弹窗。

### T4-06 快捷键（剪贴板模型）
- **目的**：在 `ui/js/shortcuts.js` 实现内部剪贴板：`{ mode: 'copy'|'cut', path }`。
- **内容**：
  - 平台键位：通过 `navigator.platform` 或后端 `os_family` 判断使用 `Cmd` 还是 `Ctrl`；
  - `C` → 记录 copy；`X` → 记录 cut；`V` → 根据当前选中目录调用 `op_copy` 或 `op_move`；`Delete` → 调 `op_delete`；
  - 粘贴选中的目标若为文件，则使用其父目录；
- **验收**：剪切+粘贴 = 移动（后端走 rename / move）；删除走回收站。

---

## 阶段 5：文件操作与后台任务

### T5-01 任务管理器
- **目的**：实现全局唯一任务执行器。
- **内容**：
  - `task::Manager`：内部 `tokio::sync::Mutex<Option<TaskInfo>>` + `try_lock` 行为；
  - 提供 `try_start(kind, description, fut)`：若已有进行中任务，返回 `AppError::BusyTask`；否则生成 `TaskInfo`、emit `task://started`、`tokio::spawn` 执行 `fut`，完成 emit `task://finished` 或 `task://failed`；
- **验收**：并发触发两个任务时第二个返回 `BusyTask`。

### T5-02 文件操作命令
- **目的**：实现 `op_rename / op_copy / op_move / op_delete`。
- **内容**：
  - `op_rename`：`fs::rename` 同目录改名；同名校验返回 `AlreadyExists`；
  - `op_copy`：递归复制（自实现或使用 `fs_extra`），冲突时返回 `AlreadyExists`；
  - `op_move`：先尝试 `fs::rename`；跨设备失败时 fallback 到 “复制 + 删除原件”，但**对外语义仍为 Move**；
  - `op_delete`：使用 `trash` crate 移到回收站；
  - 所有操作都通过 `Manager::try_start` 包装，串行执行；
- **验收**：每种操作都正确产生事件流；删除项可在系统回收站找到。

### T5-03 系统默认打开 `open_default`
- **目的**：双击文件用系统默认程序打开。
- **内容**：使用 `tauri-plugin-opener`（推荐）或 `opener` crate；
- **验收**：双击 `.txt` 用系统默认编辑器打开。

---

## 阶段 6：界面集成

### T6-01 顶部栏
- **目的**：实现模式切换按钮 + 任务展示区。
- **内容**：
  - 左侧 `<button id="mode-toggle">`，文字根据当前模式更新；
  - 右侧 `<div id="task-area">`，监听 `task://*` 事件渲染目标 / 计时（500ms 定时器）/ 状态；任务完成 5s 后清空；
- **验收**：模式按钮文字正确切换；任务运行可见计时；完成后状态变 "已完成"。

### T6-02 主体布局
- **目的**：实现两种模式的主体。
- **内容**：
  - 管理模式：`<div class="dual-tree">` 左右各一个 `FileTree`，CSS Grid 50/50；
  - 预览模式：左 `FileTree` + 右 `PreviewPane`；
  - 切换不重建组件实例，仅改可见性以保留状态；
- **验收**：切换模式平滑，状态保留。

### T6-03 全局对话框组件
- **目的**：统一错误 / 忙碌 / 非法操作提示。
- **内容**：在 `ui/js/app.js` 提供 `showInfoDialog(msg)`、`showErrorDialog(err)`、`showBusyDialog()`；基于 Bootstrap modal。
- **验收**：所有触发点（拖拽非法、忙碌、操作失败）都能正确弹窗。

---

## 阶段 7：联调与验收

### T7-01 端到端用例联调
- 按 [requirements.md §8](requirements.md) AC-01 ~ AC-10 逐条手动验证；
- 修复发现的回归。

### T7-02 三平台冒烟
- Windows / macOS / Linux 各启动一次：根节点正确、隐藏文件切换正确、删除可恢复、`du` / `file` 探测正确。

### T7-03 README 与运行说明
- 在仓库根新增 `README.md`：环境要求、`cargo tauri dev`、`cargo tauri build`、平台前置依赖（如 macOS 的 Xcode CLT、Linux 的 webkit2gtk）。

---

## 里程碑

| 里程碑 | 包含任务 | 产出 |
| --- | --- | --- |
| M1 脚手架 | T0-01 ~ T0-03、T1-01 ~ T1-02 | 可启动的 Tauri 应用 + 通信协议 |
| M2 文件系统 | T2-01 ~ T2-03、T4-01 ~ T4-03 | 双列文件树可浏览（无操作） |
| M3 预览 | T3-01 ~ T3-03、T6-02 部分 | 预览模式可用 |
| M4 操作 | T4-04 ~ T4-06、T5-01 ~ T5-03 | 增删改移完整可用 |
| M5 集成 | T6-01 ~ T6-03、T7-01 ~ T7-03 | 三平台可发布 |

---

## 依赖图（简化）

```
T0-01 ─┬─ T0-02 ─┐
       └─ T0-03 ─┴─ T1-01 ─ T1-02 ─┬─ T2-01 ─┐
                                   ├─ T2-02 ─┤
                                   └─ T2-03 ─┤
                                             ├─ T3-01 ─ T3-02 ─ T3-03
                                             ├─ T4-01 ─ T4-02 ─ T4-03 ─┬─ T4-04
                                             │                          ├─ T4-05
                                             │                          └─ T4-06
                                             └─ T5-01 ─ T5-02 ─ T5-03
T3-03 / T4-* / T5-* ──► T6-01 / T6-02 / T6-03 ──► T7-01 ─ T7-02 ─ T7-03
```

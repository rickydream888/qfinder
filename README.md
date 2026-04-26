# qfinder

跨平台的轻量级文件管理与预览工具。基于 **Rust + Tauri 2** 构建，前端使用原生 HTML / CSS / JavaScript（Bootstrap + jQuery，无 Node.js 工具链）。

支持 Windows、macOS、Linux，仅 Light Mode。

---

## 功能特性

- **双模式界面**
  - **管理模式**（默认）：左右两列文件系统树，可跨树拖拽 / 复制 / 剪切 / 粘贴；
  - **预览模式**：左侧文件系统树 + 右侧预览区。
- **跨平台根节点**
  - Windows：用户家目录 + 所有驱动器（含网络驱动器）；
  - macOS：用户家目录 + 系统根 + iCloud Drive + `/Volumes` 下的非系统卷；
  - Linux：用户家目录 + `/` + `/media/<user>` 等挂载点。
- **文件系统树**
  - 层级式展开 / 收起（每次展开重新加载内容）；
  - 拖拽移动（含跨树）；
  - 选中后再单击进入 inline 重命名（与双击展开 / 打开严格区分）；
  - 复制 / 剪切 / 粘贴 / 删除（删除走系统回收站）；
  - "显示隐藏目录和文件" 复选框；
  - 同名冲突时弹框选择 **合并目录 / 替换目录 / 替换文件 / 取消**。
- **预览**
  - 目录：子目录 / 子文件数；macOS / Linux 上若有 `du` 命令则统计磁盘占用；
  - 纯文本：截断到 10 KB；
  - 图片：自适应缩放，超过 20 MB 仅提示大小；
  - 其他：仅展示大小；
  - 后台 `spawn_blocking` + 前端去抖 + 序号丢弃过期结果，网络盘上不卡 UI；
  - 加载超过 250 ms 显示居中转圈动画。
- **后台任务**
  - 任意时刻同时只允许一个写操作（重命名 / 删除 / 复制 / 移动）；
  - 顶部任务区显示描述、耗时、状态；
  - 进行中再次发起会被拒绝并弹框提示。
- **快捷键**
  - Windows / Linux：`Ctrl+C` / `Ctrl+X` / `Ctrl+V` / `Delete`；
  - macOS：`Cmd+C` / `Cmd+X` / `Cmd+V` / `Delete`；
  - 在重命名输入框 / 弹框中按 `Delete` 不会触发删除。
- **细分图标**
  - 树节点 / 预览头部使用基于 [PKief Material Icon Theme](https://github.com/PKief/vscode-material-icon-theme) 的图标方案；
  - 按 **文件名 → 多段扩展名 → 语言 id 回退 → 默认** 顺序匹配，目录区分展开 / 折叠 / 根节点；
  - Light 变体优先；图标资源缺失时自动回退到 Bootstrap Icons。

---

## 技术栈

| 层级 | 选型 |
| --- | --- |
| 桌面框架 | [Tauri 2.x](https://tauri.app/)（**不**使用 1.x） |
| 后端语言 | Rust，edition 2024 |
| 前端 | 原生 HTML + CSS + JavaScript |
| 前端依赖 | Bootstrap、Bootstrap Icons、jQuery（已 vendored 到 `ui/`） |
| 构建工具 | Cargo；**禁止** Node.js / npm / npx / yarn / pnpm / vite 等 |

主要依赖：`tauri`（含 `protocol-asset`）、`serde`、`serde_json`、`thiserror`、`tokio`（rt-multi-thread / fs / process / sync / macros / time）、`trash`、`which`、`dirs`、`open`、`uuid`，Windows 下额外使用 `windows-sys`。

---

## 项目结构

```
qfinder/
├── Cargo.toml                 # Rust 包定义
├── build.rs                   # tauri_build::build()
├── tauri.conf.json            # Tauri 2 配置（前端 dist 指向 ui/）
├── capabilities/default.json  # Tauri 权限能力
├── icons/                     # 应用图标
├── src/
│   ├── main.rs                # 命令注册 + 任务管理
│   ├── error.rs               # AppError / AppResult，结构化序列化
│   ├── platform.rs            # 跨平台抽象（is_hidden / list_roots / 等）
│   ├── task.rs                # 单任务执行管理
│   └── commands/
│       ├── fs_tree.rs         # read_dir
│       ├── preview.rs         # 异步预览（spawn_blocking）
│       └── ops.rs             # rename / copy / move / delete / open_default
├── ui/                        # 前端静态资源（已纳入版本控制）
│   ├── index.html
│   ├── css/{app.css, bootstrap.min.css}
│   ├── js/{app.js, tauri-api.js, icon-resolver.js,
│   │       tree.js, preview.js, shortcuts.js, ...}
│   └── icons/
│       ├── bootstrap-icons.*  # Bootstrap Icons
│       └── material/          # Material Icon Theme 资源（脚本生成）
│           ├── manifest.json
│           ├── svg/*.svg
│           └── LICENSE
├── scripts/
│   └── build-material-icons.ps1   # 生成 ui/icons/material/
├── resources/                 # 开发期素材（git ignored）
└── spec/
    ├── stage1/                # 第一阶段：目标 / 需求 / 任务 / 修订
    └── stage2/                # 第二阶段：图标方案
```

---

## 构建与运行

### 前置条件

- Rust 工具链（`rustup` 安装的 stable，edition 2024 需较新版本，建议 1.85+）；
- 平台依赖：
  - **Windows**：WebView2 Runtime（Win11 自带）；
  - **macOS**：Xcode Command Line Tools；
  - **Linux**：`webkit2gtk-4.1`、`libayatana-appindicator3`、`librsvg2`（具体名称随发行版）。
- PowerShell（仅当需要重新生成 Material 图标资源时）。

### 开发运行

```powershell
cargo run
```

首次构建会编译 Tauri，时间稍长。窗口标题为 `qfinder`，默认 1280×800。

### 发布构建

```powershell
cargo build --release
```

发布配置开启 `lto`、`opt-level = "s"`、`strip = true`，以减小体积。

### 重新生成图标资源（可选）

只有在更新 `resources/pkief.material-icon-theme/` 后才需要执行：

```powershell
powershell -ExecutionPolicy Bypass -File scripts/build-material-icons.ps1
```

脚本会清空并重建 `ui/icons/material/`。

---

## 资源约定

- `resources/` 目录 **不会** 提交到代码仓库，仅作开发期素材；
- 运行时所需的前端资源全部位于 `ui/`，纳入版本控制；
- 前端代码 **不得** 通过相对路径引用 `resources/` 目录。

---

## 文档

- 第一阶段（基础功能）
  - 目标：[spec/stage1/target.md](spec/stage1/target.md)
  - 需求：[spec/stage1/requirements.md](spec/stage1/requirements.md)
  - 任务：[spec/stage1/tasks.md](spec/stage1/tasks.md)
  - 修订：[spec/stage1/revision.md](spec/stage1/revision.md)
- 第二阶段（细分图标）
  - 目标：[spec/stage2/target.md](spec/stage2/target.md)
  - 需求：[spec/stage2/requirements.md](spec/stage2/requirements.md)
  - 任务：[spec/stage2/tasks.md](spec/stage2/tasks.md)

---

## 第三方资源与协议

- [Bootstrap](https://getbootstrap.com/) — MIT
- [Bootstrap Icons](https://icons.getbootstrap.com/) — MIT
- [jQuery](https://jquery.com/) — MIT
- [Material Icon Theme by PKief](https://github.com/PKief/vscode-material-icon-theme) — MIT，详见 `ui/icons/material/LICENSE`

本项目自身代码版权归项目作者所有。

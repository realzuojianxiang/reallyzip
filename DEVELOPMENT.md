# RustZip 开发文档

> 用 Rust 编写的图形化压缩 / 解压工具（参考 WinRAR 的体验）。
> 本文档面向后续在本仓库上继续开发的工程师，覆盖架构设计、模块职责、核心实现、构建与测试、发布与扩展。

---

## 1. 项目概览

| 项目 | 说明 |
| --- | --- |
| 名称 | RustZip |
| 语言 / 工具链 | Rust（edition 2024），cargo 1.92+ |
| GUI 框架 | eframe / egui 0.35（纯 Rust 即时模式 GUI） |
| 压缩引擎 | `zip` crate 8.6（支持 Deflate / Stored / AES-256 加密） |
| 目标平台 | Windows（右键菜单集成为 Windows 专属，其余逻辑跨平台） |
| 许可证 | 未指定（内部项目） |

### 核心能力

- 创建 / 解压 ZIP，六档压缩级别（存储 ~ 最大压缩）
- AES-256 密码加密与解密（`zip` crate 的 AesMode::Aes256）
- 分卷压缩（`.zip.001` / `.002` …，与 7-Zip 一致的裸切分方案），打开时自动合并
- 不解压直接浏览压缩包目录树、预览文本 / 二进制文件
- 向已有压缩包追加文件、从压缩包中删除条目（无损重写）
- CRC32 完整性测试
- Windows 资源管理器右键菜单集成
- 拖拽文件到窗口即可压缩

---

## 2. 目录结构

```
rust-zip/
├── Cargo.toml            # 依赖与构建配置
├── Cargo.lock
├── .gitignore            # 忽略 /target 与 .workbuddy/
├── src/
│   ├── main.rs           # 程序入口：子系统属性、窗口装配、字体/主题安装
│   ├── app.rs            # 主窗口：文件管理 + 压缩包浏览 + 全部 UI 绘制
│   ├── app/
│   │   └── dialogs.rs    # 各类对话框（压缩/解压/密码/设置/关于/预览/提示）
│   ├── archive.rs        # ZIP 引擎：列举/创建/追加/删除/解压/测试
│   ├── cli.rs            # 命令行入口解析（供右键菜单调用）
│   ├── shell.rs          # Windows 右键菜单注册（HKCU）
│   ├── task.rs           # 后台任务框架（工作线程 + 进度通道）
│   ├── util.rs           # 工具函数：格式化、文件类型、路径安全
│   ├── volume.rs         # 分卷切分 / 合并
│   ├── tests.rs          # 无界面引擎测试
│   └── ui/
│       ├── mod.rs        # UI 模块声明
│       ├── theme.rs      # 中文字体加载 + 整体视觉风格（调色板/圆角/投影）
│       └── icons.rs      # 手绘矢量图标（Painter 绘制，避免依赖字体表情）
```

### 模块依赖关系

```
main.rs
  └── app.rs ──┬── archive.rs ──┬── task.rs
               │                └── volume.rs
               ├── cli.rs
               ├── shell.rs
               ├── ui/{theme,icons}.rs
               └── app/dialogs.rs
```

- `main.rs` 只做装配，不包含业务逻辑。
- `app.rs` 是 UI 与业务逻辑的粘合层，持有应用状态并分发动作。
- `archive.rs` / `volume.rs` 是**无界面核心引擎**，可独立测试。
- `task.rs` 提供统一的耗时代码执行与进度上报机制。

---

## 3. 架构设计

### 3.1 分层

项目按「UI 层 → 引擎层 → 工具层」三层组织：

1. **UI 层**（`app.rs`、`app/dialogs.rs`、`ui/*`）：egui 即时模式界面，负责交互、状态展示、对话框。
2. **引擎层**（`archive.rs`、`volume.rs`）：纯逻辑，不依赖任何 GUI 类型，唯一外部依赖是 `task::Reporter`（用于进度上报）。
3. **工具层**（`util.rs`、`task.rs`、`shell.rs`、`cli.rs`）：通用函数、任务框架、系统集成。

**关键设计**：引擎层不直接感知 UI，通过注入的 `Reporter` 上报进度，UI 通过轮询 `RunningJob` 获取进度。这让引擎可以被无界面测试（见 `tests.rs`）。

### 3.2 后台任务模型（`task.rs`）

耗时操作（压缩 / 解压 / 测试 / 打开大压缩包）必须在工作线程执行，避免阻塞 UI：

```
UI 线程                    工作线程（std::thread::spawn）
  │  task::spawn(ctx, 标题, 闭包) ─────────────▶ 执行闭包
  │                                                 │
  │  RunningJob.poll() 每帧抽干消息 ◀── mpsc 通道 ──┘
  │      ├─ JobMsg::Total                 （总工作量）
  │      ├─ JobMsg::Progress{ done,label }（进度）
  │      ├─ JobMsg::Log                   （日志）
  │      └─ JobMsg::Finished(Result)      （结束 + 产物）
```

- **取消**：`Reporter.cancel` 是一个 `Arc<AtomicBool>`，工作线程在循环里调用 `check_cancel()` 主动退出。
- **进度**：`Reporter.total()` / `progress()` 发送消息并 `request_repaint()` 唤醒 UI。
- **产物**：`JobOutcome` 携带打开后的 `OpenArchive` 或预览临时文件路径。

### 3.3 数据模型（`archive.rs`）

- `ArchiveEntry`：压缩包内单个条目的元数据（路径、类型、大小、CRC、修改时间、加密、压缩方式）。
- `OpenArchive`：已打开的压缩包，持有条目列表与统计信息；`Drop` 时自动清理合并分卷产生的临时文件。
- `Node`：压缩包内**某一层目录**下可见的项（通过 `children_of` 由 `ArchiveEntry` 推导，自动补齐隐式目录）。
- `Level` / `CreateOptions` / `ExtractOptions` / `Overwrite`：压缩 / 解压的配置项。

### 3.4 压缩包内目录浏览

`children_of(entries, dir)` 把扁平的 `ArchiveEntry` 列表按 `dir/` 前缀聚合成树形一层的 `Node` 列表：

- 深层条目（含 `/`）归并到直接子目录，并累加大小、合并加密标记、取最新修改时间。
- 隐式目录（zip 中只有文件而无显式目录条目）会被自动补齐。

---

## 4. 核心实现要点

### 4.1 打开与分卷合并

`archive::open(path)`：

1. 若 `path` 是分卷（`.001`），调用 `volume::collect_volumes` 收集完整序列，`volume::merge_volumes` 合并到临时文件。
2. 用 `ZipArchive::by_index_raw`（无需密码）读取全部条目元数据。
3. 统计 `total_size` / `total_packed` / `has_encrypted`。
4. 若是合并的分卷，`OpenArchive` 标记 `temp_merged`，`Drop` 时删除临时文件。

### 4.2 创建 / 加密 / 分卷

`archive::create(sources, dest, opt, rep)`：

1. `collect_items` 递归收集源文件为 `Item` 列表（含相对路径 `rel` 与大小，用于进度总计）。
2. 若分卷，先写到临时文件，完成后再 `volume::split_file` 切分。
3. 逐项写入：目录用 `add_directory`，文件用 `start_file` + 流式写入。
4. 加密：`SimpleFileOptions::with_aes_encryption(AesMode::Aes256, pw)`，目录与文件均支持。
5. 压缩级别：`Level::Store` 用 `Stored`，其余用 `Deflated` + 对应 1/3/6/8/9 级。

### 4.3 追加与删除

- **追加** `append`：用 `ZipWriter::new_append(file)` 打开已有压缩包，跳过同名条目写入。
- **删除** `delete_entries`：用 `raw_copy_file` 无损重写（不重新压缩），先写临时文件再原子替换；支持按路径前缀删除整个目录。

### 4.4 解压与路径安全

`archive::extract`：

- 支持按 `selection` 解压子集（含子项）。
- **路径安全** `safe_out_path`：拒绝 `..` 穿越、拒绝含 `:` 的非法路径，防止 zip 炸弹式路径注入。
- 同名文件处理：`Overwrite::{Always, Skip, AutoRename}`，自动重命名用 `util::unique_path`。
- 加密条目用 `by_index_decrypt`，密码错误映射为友好中文错误。

### 4.5 完整性测试

`archive::test`：逐条读取文件流并交给 `zip` crate 校验 CRC，统计正常 / 异常数量，异常项记入日志。

### 4.6 分卷方案（`volume.rs`）

与 7-Zip 一致的**裸切分**：`archive.zip` → `archive.zip.001`、`.002`…

- `collect_volumes`：从 `.001` 开始顺序收集，遇缺号即停。
- `merge_volumes`：按序拼接为临时文件。
- `split_file`：按 `part_size` 切分，处理分卷边界 `written==0` 的空卷清理。
- 常见预设：软盘 1.44MB / 10MB / 100MB / 700MB / 4095MB。

### 4.7 后台任务取消

工作线程在每次读写循环中调用 `rep.check_cancel()`；解压时若 `rep.cancelled()` 为真，删除已写入的部分文件并 `bail!`。

---

## 5. 构建与运行

### 5.1 依赖

- Rust 工具链（cargo 1.92+，edition 2024）
- 依赖：`anyhow`、`chrono`、`eframe/egui/egui_extras 0.35`、`rfd`、`walkdir`、`zip 8.6`、`winreg`（Windows only）

### 5.2 构建

```bash
# 开发构建（保留控制台，便于看日志）
cargo build

# 发布构建（Windows GUI 子系统，双击不弹终端）
cargo build --release
```

发布版可执行文件：`target/release/rustzip.exe`

> **关于终端窗口**：`main.rs` 顶部通过
> `#![cfg_attr(all(not(debug_assertions), target_os = "windows"), windows_subsystem = "windows")]`
> 将发布版标记为 GUI 子系统，运行时**不弹出控制台窗口**；debug 构建仍保留控制台方便调试。

### 5.3 运行

```bash
# 直接运行（浏览用户主目录）
target/release/rustzip.exe

# 命令行入口（供右键菜单 / 脚本调用）
rustzip.exe --register-shell          # 注册右键菜单（HKCU）
rustzip.exe --unregister-shell        # 取消右键菜单
rustzip.exe --compress "文件1" "文件2" # 打开压缩对话框并预填源
rustzip.exe --extract-here "a.zip"    # 直接解压到压缩包所在目录
rustzip.exe --extract-to "a.zip"      # 打开解压对话框
rustzip.exe "某个目录"                # 在该目录启动
rustzip.exe "a.zip"                   # 打开压缩包
```

### 5.4 测试

```bash
cargo test
```

测试覆盖：创建/解压往返、压缩级别对比、AES 加密保护、分卷切分与合并、删除条目、CRC 校验。测试使用 `Reporter::new`（无 UI 绑定）独立运行引擎逻辑。

---

## 6. 命令行接口（`cli.rs`）

`Startup` 枚举描述启动意图，`parse()` 解析 `std::env::args()`：

| 参数 | 启动行为 |
| --- | --- |
| （无参数） | `Normal`，浏览用户主目录 |
| `--register-shell` | `RegisterShell` |
| `--unregister-shell` | `UnregisterShell` |
| `--compress <路径>…` | `Compress`，打开压缩对话框 |
| `--extract-here <压缩包>` | `ExtractHere`，直接解压 |
| `--extract-to <压缩包>` | `ExtractTo`，打开解压对话框 |
| `<目录>` | `Browse` |
| `<压缩包>` | `Open` |

---

## 7. Windows 右键菜单集成（`shell.rs`）

写入 `HKEY_CURRENT_USER`（无需管理员权限），注册以下项：

| 注册表路径 | 菜单项 | 命令 |
| --- | --- | --- |
| `*\shell\RustZip.Add` | 添加到压缩文件… | `--compress "%1"` |
| `Directory\shell\RustZip.Add` | 添加到压缩文件… | `--compress "%1"` |
| `SystemFileAssociations\.zip\shell\RustZip.Open` | 用 RustZip 打开 | `"%1"` |
| `...\RustZip.ExtractTo` | 解压到… | `--extract-to "%1"` |
| `...\RustZip.ExtractHere` | 解压到当前文件夹 | `--extract-here "%1"` |
| `Directory\Background\shell\RustZip.Open` | 在此处打开 RustZip | `"%V"` |

注册 / 注销后调用 `SHChangeNotify(SHCNE_ASSOCCHANGED)` 通知资源管理器刷新。

---

## 8. UI 设计与主题（`ui/theme.rs`）

### 8.1 视觉风格

现代浅色主题（类 Windows 11 Fluent）：

- **主色**：靛蓝 `ACCENT`（#3B6FE0），用于强调与主操作。
- **灰阶层次**：`WINDOW_BG` → `PANEL_BG` → `PANEL_SOFT`，卡片化区分层级。
- **圆角**：控件 6px、卡片 9px、窗口 10px。
- **投影**：窗口与浮层带轻阴影。
- **语义色**：`OK_GREEN`（成功）、`WARN`（警告）、`DANGER`（危险）、`LOCK_SOFT`（加密提示）。

布局为卡片式：菜单 / 工具栏 / 地址栏 / 文件列表 / 状态栏各自为独立白色圆角卡片，置于浅灰背景上。

### 8.2 中文字体

`install_fonts` 依次尝试加载系统 CJK 字体（微软雅黑、黑体、等线、PingFang、Noto CJK 等），首个可用的作为中文字体。

### 8.3 图标（`ui/icons.rs`）

所有图标用 `egui::Painter` 手绘（矢量），避免依赖字体表情：文件夹、压缩包、文件、锁、以及工具栏的添加 / 解压 / 测试 / 查看 / 删除 / 信息 / 上一级 / 主目录 / 刷新。

---

## 9. 常见问题与坑

### 9.1 布局错位（`horizontal_centered`）

**坑**：`ui.horizontal_centered` 会分配**整个可用高度并垂直居中**内容。若在顶层直接使用，会导致菜单等被推到窗口中部、其余区域空白。

**修复**：改用 `ui.horizontal`（不分配全高），让各区域按顺序自然排列。

### 9.2 发布版弹终端

见 5.2，需要在 `main.rs` 加 `windows_subsystem` 属性。

### 9.3 分卷打开

分卷必须命名为 `xxx.zip.001` 形式，且从 `.001` 开始连续编号；缺号即停止收集。

### 9.4 大文件

`zip` crate 的条目大小超过 `u32::MAX` 时需 `large_file(true)`（已在 `write_items` 处理）。

### 9.5 删除压缩包内条目

`delete_entries` 使用无损重写，会整体重建压缩包；分卷压缩包**不支持直接删除条目**（`app.rs` 中已拦截提示）。

---

## 10. 扩展指南

- **新增压缩格式**：在 `archive.rs` 增加对应的 `ZipArchive` / 引擎分支，`util::is_archive_path` 扩展可识别扩展名，`util::file_kind` 补充类型描述。
- **新增 UI 对话框**：在 `app/dialogs.rs` 的 `Dialog` 枚举与 `dialogs()` 分发中添加新成员。
- **新增后台任务**：在 `app.rs` 添加 `start_xxx` 方法，用 `task::spawn` 包裹耗时逻辑，返回的 `RunningJob` 交由 `poll_job` 统一处理。
- **新增压缩级别**：在 `Level` 枚举与 `level()` 映射中扩展。
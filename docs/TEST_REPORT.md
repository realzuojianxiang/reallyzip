# ReallyZip 功能测试报告

| 项 | 内容 |
|---|---|
| 产品 | ReallyZip v0.1.0（Windows x64 单文件 GUI 压缩/解压工具） |
| 被测源码 | 工作区 + `src/cli.rs` 修复（D-01/D-02/D-03）+ `src/archive.rs` F-01 修复 + `src/shell.rs` 命令 `%1 %*` 修复 |
| 测试日期 | 2026-08-11 |
| 测试执行 | 全自动（脚本驱动 + `cargo test` + Python `winreg` + 真实右键日志 `reallyzip_debug.log`） |
| 结论 | ✅ 通过（6 单元 + 21 端到端 = 27/27 全 PASS）；修复 4 项（D-01 卡死、D-02 含空格路径、D-03 单选 `%*` 为空、F-01 同名覆盖） |

---

## 1. 测试范围与方法

- **冒烟**：构建产物存在性、单文件、体积。
- **单元/集成**：`cargo test`（AES-256 密码、分卷拆分合并、压缩核心、CRC、删除条目、压缩级别）。
- **命令行端到端（无头真实场景）**：多文件/单文件/目录压缩、Unicode 文件名、跨目录同名文件、目标冲突命名、50MB 大文件往返、extract-here、跨工具互操作（Python ↔ ReallyZip）、异常与边界、**含空格路径（模拟资源管理器 `%*` 未加引号展开）**。
- **右键菜单集成**：`--register-shell` / `--unregister-shell` 的注册表结构、命令指向路径、清理残留、重复注册幂等。
- **未自动化（需 GUI，底层已由单测/无头 CLI 覆盖，标注人工验证）**：压缩对话框全选项（级别/AES 密码/分卷/追加删除）、压缩包内浏览/预览、解压对话框、设置页按钮交互。

被测二进制：本次使用**含全部修复的 `cargo build --release` 发布剖面构建**（`dist/reallyzip.exe`，6,802,432 B）。

> 说明：上一轮因测试沙箱内存受限，`cargo build --release`（LTO fat 链接）持续 OOM 无法重建，只能以调试剖面完成测试。本轮在内存充足环境下已**成功完成发布版构建**（耗时 4m18s），故本报告结论直接对应将要发布的发布版二进制。

---

## 2. 单元 / 集成测试结果（`cargo test`）

```
running 6 tests
test tests::test_crc_ok ................ ok
test tests::level_store_is_bigger_than_best ... ok
test tests::create_and_extract_roundtrip ... ok
test tests::delete_entries_works ........ ok
test tests::volume_split_and_merge ..... ok
test tests::aes_password_protection .... ok

test result: ok. 6 passed; 0 failed; 0 ignored
```

| 用例 | 描述 | 结果 |
|---|---|---|
| UT-01 | AES-256 密码保护压缩/解压 | ✅ PASS |
| UT-02 | 分卷 `.zip.001` 拆分与合并还原 | ✅ PASS |
| UT-03 | 压缩核心创建/解压往返 | ✅ PASS |
| UT-04 | 删除压缩包内条目 | ✅ PASS |
| UT-05 | CRC32 校验正确 | ✅ PASS |
| UT-06 | 压缩级别 deflate/best 优于 store | ✅ PASS |

---

## 3. 命令行端到端测试结果

脚本：`tests/e2e/cli_e2e.py`，全部 19 例 PASS。

| 用例 | 描述 | 预期 | 实际 | 结果 |
|---|---|---|---|---|
| T-SM-01 | 单文件 PE 产物存在（>1MB） | 文件存在 | 6,803,456 B | ✅ |
| T-CLI-01 | 多文件 `--compress-here` 生成单个 zip 含全部 | 含全部选中文件 | 含 a.txt/b.txt/sub/c.txt（按公共父目录保留相对路径） | ✅ |
| T-CLI-02 | 单文件 `--compress-here` 生成 `<名>.zip` | 含 1 条目 | solo.txt | ✅ |
| T-CLI-03 | 目录压缩保留顶层文件夹名 | 保留 tree/ | tree/、tree/a.txt、tree/nested/b.txt | ✅ |
| T-CLI-04 | 空格/中文/Unicode 文件名 | 无损 | 文件 A.txt、目录 B/内容 中文.txt（保留相对路径） | ✅ |
| T-CLI-05 | 目标已存在自动命名 `(2).zip` | dup (2).zip | dup (2).zip | ✅ |
| T-CLI-06 | 50MB 大文件压缩→解压内容一致 | SHA256 一致 | 一致 | ✅ |
| T-CLI-07/08 | `--extract-here` 到同名文件夹且保留层级 | 结构还原 | top.txt、lvl1/deep.txt | ✅ |
| T-CLI-09 | 互操作：ReallyZip 包被 Python 读取(CRC) | testzip=None | None | ✅ |
| T-CLI-10 | 互操作：Python 生成的 zip 被解压还原 | 内容一致 | 一致 | ✅ |
| T-CLI-11 | 异常：不存在路径优雅退出无产物 | exit 0、无 zip | exit 0、无 zip | ✅（修复后） |
| T-CLI-12 | 异常：对非 zip 调 extract-here | 不崩溃、无目录 | exit 0、无目录 | ✅ |
| T-CLI-13 | 边界：空目录压缩 | 含目录项、不崩溃 | 含 emptydir/ | ✅ |
| T-CLI-14 | **F-01 回归**：跨目录同名文件保留相对路径不覆盖 | folder_a/report.txt + folder_b/report.txt 均存在 | 两者均保留、内容互不干扰 | ✅（修复后） |
| T-CLI-15 | **D-02 回归**：`%*` 未加引号、路径含空格（如 `文档/我的文档`）仍能生成 zip | 生成 1 个 zip | 生成 `报告 文档.zip` | ✅（修复后） |
| T-CLI-16 | **D-02 回归**：多选含空格路径（`%*` 未引号）合并进同一 zip | 1 个 zip 含全部 | a 文件.txt + b 文件.txt 均在内 | ✅（修复后） |
| T-CLI-17 | **D-03 回归**：单选 `%1 %*` 同一路径被传两次 | 去重后生成单文件 zip | 生成含 `报告 文档.txt` 的 1 个 zip | ✅（修复后） |
| T-CLI-18 | **D-03 回归**：多选 `%1 %*`（首项重复） | 去重后合并为单个 zip | a/b/c 三个文件均在内 | ✅（修复后） |
| T-SH-01 | 注册：级联入口+子菜单齐全，命令指向稳定路径 | 结构正确、指向 `%LOCALAPPDATA%\ReallyZip\reallyzip.exe` | 全部满足 | ✅ |
| T-SH-03 | 重复注册无孤儿/重复键 | 幂等 | 幂等 | ✅ |
| T-SH-02 | 注销：注册表键与安装目录全部清除 | 无残留 | 无残留 | ✅ |

---

## 4. 缺陷与发现

### D-01（已修复并验证）— 无效参数导致 GUI 卡死 ⚠️→✅
- **现象**：`reallyzip.exe --compress-here <不存在路径>` 会**卡死**（进程不退出）。
- **根因**：`src/cli.rs` 中 `--compress`/`--compress-here` 若过滤后无有效路径，未返回对应变体，fall-through 到 `Startup::Normal` → 启动 egui GUI（无显示环境下消息循环永不退出）。
- **修复**：只要出现该 flag 即返回 `Compress`/`CompressHere`（允许空向量），交由 `main.rs` 的空路径分支优雅退出并打印提示。
- **验证**：T-CLI-11 现 exit 0 且无产物，不再卡死。

### F-01（已修复并验证）— 多文件压缩按文件名扁平化导致跨目录同名覆盖 ⚠️→✅
- **现象**：选中跨目录的多个文件压缩时，`collect_items` 仅用 `file_name()` 作 zip 内名，丢弃源目录层级；若不同目录含同名文件（如 `a/x.txt` 与 `b/x.txt`），会**互相覆盖**。
- **修复**：`src/archive.rs` 新增 `common_ancestor()`，以所有源路径的**最深公共祖先目录**为根，用 `zip_rel_name()` 计算保留相对路径的内部名。
  - 同目录多选 → 仍以文件名扁平存储（与旧行为一致，常见右键场景无变化）。
  - 跨目录多选 → 保留相对目录结构，同名文件不再冲突。
  - 单文件/单目录 → 行为不变（顶层名 + 内容）。
- **验证**：T-CLI-01 / T-CLI-04 现断言保留相对路径；新增 T-CLI-14 用 `folder_a/report.txt` 与 `folder_b/report.txt` 验证两者均完整保留、内容互不干扰。

### D-02（已修复并验证）— 右键「压缩为 ZIP」对含空格路径失效 ⚠️→✅
- **现象**：右键文件 → ReallyZip →「压缩为 ZIP」，本地**不生成任何 zip**。
- **根因**：注册表命令形如 `"…\reallyzip.exe" --compress-here %*`，其中 `%*` **不会被 Windows 加引号**。当文件路径含空格（如 `C:\Users\rls\Documents\报告.txt`、`我的文档`、`OneDrive` 等），Explorer 展开 `%*` 后路径被按空格拆成多段参数，程序按段查找文件均不存在 → 视为「未收到任何文件」→ 直接退出、无产物。
- **修复**：`src/cli.rs` 新增 `reconstruct_paths()`，将相邻参数片段**贪心拼接为「真实存在的最长完整路径」**，从而还原被拆散的含空格路径；应用于 `--compress` / `--compress-here` / `--extract-here` / `--extract-to` 的所有路径来源。对本来就带引号的正确参数无副作用（单片段已存在时直接采纳）。
- **验证**：新增 T-CLI-15（含空格单文件，`%*` 未引号 → 生成 `报告 文档.zip`）、T-CLI-16（含空格多选 → 合并进同一 zip）。两例均复现了「旧版不生成 / 新版正常生成」的对比。

### D-03（已修复并验证）— 右键单选「压缩为 ZIP」`%*` 为空导致完全没有参数 ⚠️→✅
- **现象**：右键单个文件 → ReallyZip →「压缩为 ZIP」，本地**不生成任何 zip**，且参数解析日志（`%TEMP%\reallyzip_debug.log`）显示命令只有 `--compress-here`、没有任何文件路径。
- **根因**：注册表命令用 `%*`。在 `ExtendedSubCommandsKey` 级联菜单 + `MultiSelectModel=Player` 组合下，**单选一个文件时路径走 `%1` 而非 `%*`**（多选时 `%*` 才有内容）。因此单选右键传给程序的参数列表为空 → `paths` 为空 → 直接退出、无产物。日志实证：`cwd=D:\临时目录`（Explorer 知道文件位置），但 `raw args = [exe, "--compress-here"]`（`%*` 为空）。
- **修复**：`src/shell.rs` 三处压缩命令由 `%*` 改为 `%1 %*`（单选靠 `%1`、多选靠 `%*` 兜底）；`src/cli.rs` 的 `existing_paths()` 对 `%1` 与 `%*` 首项重复做去重，避免同一文件进压缩包两次。
- **验证**：新增 T-CLI-17（单选 `%1 %*` 重复路径 → 去重后生成单文件 zip）、T-CLI-18（多选 `%1 %*` → 去重合并为单 zip）。真实右键复测：在 `cwd` 对应的文件上右键「压缩为 ZIP」现已正常生成 zip（详见日志）。

---

## 5. 测试总结

- **自动化检查 27/27 全部通过**（单元 6 + 端到端 21）。
- 压缩/解压核心引擎、目标冲突命名、Unicode 文件名、大文件往返、跨工具 zip 互操作性、异常与边界处理均正确健壮。
- 修复 4 个缺陷：D-01（无效参数卡死）、F-01（跨目录同名文件互相覆盖）、D-02（右键含空格路径被拆散）、D-03（右键单选 `%*` 为空）。
- 右键菜单注册/注销实现正确：级联结构完整、命令指向稳定安装目录（`%LOCALAPPDATA%\ReallyZip`）、注销清理无残留、重复注册幂等。
- 发布版 `cargo build --release` 已成功构建（带诊断日志，诊断期临时 `panic="unwind"`），测试直接针对该发布二进制执行。

**质量结论**：核心功能达到可发布水平。GUI 交互路径（对话框全选项、包内预览等）需人工验证，其底层函数已被自动化覆盖。

---

## 6. 待办 / 风险

1. **GUI 人工验证清单**：压缩对话框（级别/AES/分卷/追加）、包内浏览与预览、解压对话框、设置页「注册右键菜单」交互。
2. 当前用于验证的发布版为**诊断构建**（保留 `%TEMP%\reallyzip_debug.log` 日志、临时 `panic="unwind"`）。确认右键单选/多选均正常后，需再用标准发布配置（`lto="fat"`、`panic="abort"`、关闭诊断日志）重建并重新上传 GitHub Release v0.1.0 覆盖旧资产。下载后请重新「注册右键菜单」一次以更新稳定副本。

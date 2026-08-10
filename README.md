# ReallyZip

[![Release](https://img.shields.io/github/v/release/realzuojianxiang/reallyzip?label=release)](https://github.com/realzuojianxiang/reallyzip/releases/latest)
[![License](https://img.shields.io/github/license/realzuojianxiang/reallyzip)](LICENSE)

用 Rust + egui 编写的图形化压缩 / 解压工具，参考 WinRAR 的体验。现代 Fluent 风格界面，支持 ZIP 的创建、解压、加密、分卷、包内浏览等完整能力。

## 📦 下载

**[⬇️ 下载最新版 reallyzip.exe](https://github.com/realzuojianxiang/reallyzip/releases/latest/download/reallyzip.exe)** · Windows 10/11 x64 · 6.5 MB

单文件绿色运行，无需安装、无运行时依赖。其他版本见 [Releases](https://github.com/realzuojianxiang/reallyzip/releases)。

## ✨ 功能特性

- **创建 / 解压 ZIP**：六档压缩级别（存储 ~ 最大压缩）
- **AES-256 加密**：密码加密与解密
- **分卷压缩**：`.zip.001 / .002 …`（与 7-Zip 兼容），打开时自动合并
- **包内浏览**：不解压直接查看压缩包目录树，预览文本 / 二进制文件
- **追加与删除**：向已有压缩包追加文件（跳过同名）、删除条目（无损重写）
- **完整性测试**：逐条校验 CRC32
- **右键级联菜单**：资源管理器右键收纳为单个「ReallyZip」子菜单，不刷屏
- **拖拽压缩**：把文件拖进窗口即可压缩

## 🚀 快速开始

### 环境要求

- Rust 工具链（cargo 1.92+，edition 2024）

### 构建

```bash
# 发布构建（GUI 程序，运行时不弹终端窗口）
cargo build --release
```

可执行文件：`target/release/reallyzip.exe`，约 **6.5 MB**，单文件绿色运行、无需任何运行时依赖。

<details>
<summary>体积是怎么压到 6.5 MB 的（默认配置是 16 MB）</summary>

| 手段 | 说明 | 省下 |
|---|---|---|
| 渲染后端 wgpu → glow | `eframe` 0.35 默认走 wgpu（DX12/Vulkan 抽象层，体积巨大），改用 glow(OpenGL) | ~6 MB |
| 关闭 accesskit | 去掉屏幕阅读器无障碍层 | ~1 MB |
| 裁剪 zip 编解码 | 只留 Deflate / Deflate64 / AES，去掉 bzip2、zstd、lzma、xz、ppmd | ~1.5 MB |
| LTO fat + codegen-units=1 | 全程序链接时优化 | ~1 MB |
| panic = "abort" | 去掉 unwind 栈展开表 | ~0.3 MB |

代价与回退：

- 极少数无 OpenGL 驱动的环境（部分虚拟机 / 远程桌面）可能黑屏 —— 把 `Cargo.toml` 里 eframe 的 features 换回 `["wgpu", "default_fonts"]` 即可。
- 无法解压用 bzip2 / zstd / lzma / xz 方法压缩的 zip（这些在 zip 里极罕见）—— 在 zip 的 features 里补回对应项即可。

</details>

### 运行

```bash
# 直接运行，浏览用户主目录
target/release/reallyzip.exe

# 命令行入口（供右键菜单 / 脚本）
reallyzip.exe --register-shell             # 注册右键菜单
reallyzip.exe --unregister-shell           # 取消右键菜单
reallyzip.exe --compress "文件1" "文件2"    # 打开压缩对话框
reallyzip.exe --compress-here "文件1"       # 直接压缩为同名 zip，不弹窗
reallyzip.exe --extract-to "a.zip"         # 打开解压对话框
reallyzip.exe --extract-here "a.zip"       # 直接解压到同名文件夹
reallyzip.exe "a.zip"                      # 打开压缩包
```

### 右键菜单

注册后右键只多出一个 **ReallyZip** 入口，展开才是具体动作：

```text
普通文件 / 文件夹          .zip 压缩包
└─ ReallyZip              └─ ReallyZip
   ├─ 添加到压缩文件…          ├─ 用 ReallyZip 打开
   └─ 压缩为 ZIP               ├─ 解压到…
                               ├─ 解压到当前文件夹
                               └─ 添加到压缩文件…
```

用的是 Windows 7+ 的 `ExtendedSubCommandsKey` 级联菜单，全部写在 `HKCU` 下，
不需要管理员权限，也不需要注册 COM 组件。多选文件走 `MultiSelectModel=Player`，
一次只启动一个实例把所有文件压进同一个包。

### 测试

```bash
cargo test
```

## 📦 技术栈

| 组件 | 说明 |
| --- | --- |
| [eframe / egui](https://github.com/emilk/egui) 0.35 | 即时模式 GUI 框架 |
| [zip](https://github.com/zip-rs/zip2) 8.6 | 压缩引擎（Deflate / AES-256） |
| [rfd](https://github.com/PolyMeilex/rfd) | 原生文件对话框 |
| [winreg](https://github.com/gentoo90/winreg) | Windows 注册表（右键菜单） |

## 📚 文档

- **[开发文档](DEVELOPMENT.md)**：架构设计、模块职责、核心实现、构建、扩展指南

## 🗺️ 路线图（可选）

- [ ] 深色模式
- [ ] 更多压缩格式支持（7z / tar / gz）
- [ ] 压缩包内拖拽排序、批量重命名

## 📄 许可证

未指定（内部项目）
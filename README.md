# RustRAR

用 Rust + egui 编写的图形化压缩 / 解压工具，参考 WinRAR 的体验。现代 Fluent 风格界面，支持 ZIP 的创建、解压、加密、分卷、包内浏览等完整能力。

## ✨ 功能特性

- **创建 / 解压 ZIP**：六档压缩级别（存储 ~ 最大压缩）
- **AES-256 加密**：密码加密与解密
- **分卷压缩**：`.zip.001 / .002 …`（与 7-Zip 兼容），打开时自动合并
- **包内浏览**：不解压直接查看压缩包目录树，预览文本 / 二进制文件
- **追加与删除**：向已有压缩包追加文件（跳过同名）、删除条目（无损重写）
- **完整性测试**：逐条校验 CRC32
- **右键菜单集成**：Windows 资源管理器右键直接压缩 / 解压
- **拖拽压缩**：把文件拖进窗口即可压缩

## 🚀 快速开始

### 环境要求

- Rust 工具链（cargo 1.92+，edition 2024）

### 构建

```bash
# 发布构建（GUI 程序，运行时不弹终端窗口）
cargo build --release
```

可执行文件：`target/release/rustrar.exe`

### 运行

```bash
# 直接运行，浏览用户主目录
target/release/rustrar.exe

# 命令行入口（供右键菜单 / 脚本）
rustrar.exe --register-shell          # 注册右键菜单
rustrar.exe --unregister-shell        # 取消右键菜单
rustrar.exe --compress "文件1" "文件2" # 打开压缩对话框
rustrar.exe "a.zip"                   # 打开压缩包
```

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
# FastView

<div align="center">

[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)
[![Version](https://img.shields.io/badge/version-0.1.1--dev-blue.svg)](https://github.com/wsyqn6/fastview/releases)
[![Rust](https://img.shields.io/badge/Rust-1.70+-orange.svg)](https://www.rust-lang.org)
[![Platform](https://img.shields.io/badge/Platform-Windows%20%7C%20Linux%20%7C%20macOS-blue.svg)](https://github.com/)

<!-- TODO: Replace 'yourusername' with your actual GitHub username before publishing -->

**A fast, lightweight, and modern image viewer built with Rust**

> ⚠️ **Development Preview**: This project is in active development. Features and APIs may change.

[English](#english) | [中文](#中文)

</div>

---

## English

### 🚀 Overview

FastView is a minimalist image viewer designed for speed and simplicity. Built with Rust and Eframe (egui), it delivers instant startup, minimal resource usage, and a clean modern interface.

**Core Philosophy**: Do one thing well - view images fast.

### ✨ Features

- ⚡ **Lightning Fast** - Startup in < 1 second, smooth performance
- 💾 **Tiny Footprint** - ~5MB executable, < 20MB memory usage
- 🖼️ **Wide Format Support** - JPEG, PNG, GIF, WebP, BMP, TIFF, ICO, AVIF
- 🔄 **EXIF Auto-Rotation** - Automatically corrects photo orientation
- 🎯 **Smart Zoom** - Fit, Fill, Original, and custom zoom modes
- 🖱️ **Drag & Drop** - Simply drag images into the window
- 🌐 **Cross-Platform** - Windows, Linux, macOS
- 🎨 **Modern UI** - Clean interface powered by egui
- 🔧 **Configurable** - Language settings, cache management

### 📸 Screenshots

*(Add screenshots here)*

### 🛠️ Installation

#### From Source

```bash
# Clone the repository
git clone https://github.com/wsyqn6/fastview.git
cd fastview

# Build in release mode
cargo build --release

# Run the application
./target/release/fastview
```

#### Pre-built Binaries

Download the latest release from the [Releases page](https://github.com/wsyqn6/fastview/releases).

### ⌨️ Keyboard Shortcuts

| Action | Key |
|--------|-----|
| Previous Image | `←` |
| Next Image | `→` |
| Zoom In | `+` |
| Zoom Out | `-` |
| Rotate Left | `r` |
| Rotate Right | `R` |
| Fit to Window | `0` |
| Fill Window | `2` |
| Original Size | `1` |
| Fullscreen | `f` |
| Borderless Mode | `v` |
| Drag Mode | `Space` |
| Exit Fullscreen | `Esc` |
| Show Shortcuts | `H` / `?` |

### 🏗️ Tech Stack

- **GUI Framework**: [Eframe](https://github.com/emilk/egui) (egui) - Immediate mode GUI
- **Image Processing**: [image-rs](https://github.com/image-rs/image) - Pure Rust image library
- **File Dialog**: [rfd](https://github.com/PolyMeilex/rfd) - Native file dialogs
- **EXIF Handling**: [kamadak-exif](https://github.com/kamadak/exif-rs) - EXIF metadata parsing
- **Configuration**: serde + dirs - Minimal dependencies

### 📂 Project Structure

```
fastview/
├── src/
│   ├── main.rs      # Application entry point
│   ├── app.rs       # Main application logic and UI
│   ├── types.rs     # Type definitions
│   └── fonts.rs     # Font loading (Chinese support)
├── Cargo.toml       # Rust package manifest
├── LICENSE          # MIT License
└── README.md        # This file
```

### 🤝 Contributing

Contributions are welcome! Please feel free to submit a Pull Request.

1. Fork the repository
2. Create your feature branch (`git checkout -b feature/amazing-feature`)
3. Commit your changes (`git commit -m 'Add some amazing feature'`)
4. Push to the branch (`git push origin feature/amazing-feature`)
5. Open a Pull Request

### 📄 License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.

### 🙏 Acknowledgments

- [egui](https://github.com/emilk/egui) - Amazing immediate mode GUI framework
- [image-rs](https://github.com/image-rs/image) - Excellent image processing library
- The Rust community for incredible tools and libraries

---

## 中文

> ⚠️ **开发预览版**：本项目正在积极开发中，功能和 API 可能会发生变化。

### 🚀 项目简介

FastView 是一款极简主义图片查看器，专注于速度和简洁。使用 Rust 和 Eframe (egui) 构建，提供即时启动、最小资源占用和清爽的现代界面。

**核心理念**：专注做好一件事 - 快速查看图片。

### ✨ 功能特性

- ⚡ **极速启动** - 启动时间 < 1 秒，流畅运行
- 💾 **体积小巧** - 可执行文件约 5MB，内存占用 < 20MB
- 🖼️ **格式全面** - 支持 JPEG、PNG、GIF、WebP、BMP、TIFF、ICO、AVIF
- 🔄 **自动旋转** - 根据 EXIF 信息自动校正照片方向
- 🎯 **智能缩放** - 适应窗口、填充窗口、原始尺寸和自定义缩放
- 🖱️ **拖拽打开** - 直接拖拽图片到窗口即可打开
- 🌐 **跨平台** - Windows、Linux、macOS
- 🎨 **现代界面** - 基于 egui 的简洁界面
- 🔧 **可配置** - 语言设置、缓存管理

### 📸 截图展示

*(在此添加截图)*

### 🛠️ 安装方法

#### 从源码编译

```bash
# 克隆仓库
git clone https://github.com/wsyqn6/fastview.git
cd fastview

# 发布模式编译
cargo build --release

# 运行程序
./target/release/fastview
```

#### 预编译版本

从 [Releases 页面](https://github.com/wsyqn6/fastview/releases) 下载最新版本。

### ⌨️ 快捷键

| 功能 | 按键 |
|------|------|
| 上一张 | `←` |
| 下一张 | `→` |
| 放大 | `+` |
| 缩小 | `-` |
| 逆时针旋转 | `r` |
| 顺时针旋转 | `R` |
| 适应窗口 | `0` |
| 填充窗口 | `2` |
| 原始尺寸 | `1` |
| 全屏 | `f` |
| 无边框模式 | `v` |
| 拖动模式 | `Space` |
| 退出全屏 | `Esc` |
| 显示快捷键 | `H` / `?` |

### 🏗️ 技术栈

- **GUI 框架**: [Eframe](https://github.com/emilk/egui) (egui) - 即时模式 GUI
- **图片处理**: [image-rs](https://github.com/image-rs/image) - 纯 Rust 图片库
- **文件对话框**: [rfd](https://github.com/PolyMeilex/rfd) - 原生文件对话框
- **EXIF 处理**: [kamadak-exif](https://github.com/kamadak/exif-rs) - EXIF 元数据解析
- **配置管理**: serde + dirs - 最小化依赖

### 📂 项目结构

```
fastview/
├── src/
│   ├── main.rs      # 程序入口
│   ├── app.rs       # 主应用逻辑和 UI
│   ├── types.rs     # 类型定义
│   └── fonts.rs     # 字体加载（中文支持）
├── Cargo.toml       # Rust 包配置
├── LICENSE          # MIT 许可证
└── README.md        # 本文件
```

### 🤝 贡献指南

欢迎贡献！请随时提交 Pull Request。

1. Fork 本仓库
2. 创建特性分支 (`git checkout -b feature/amazing-feature`)
3. 提交更改 (`git commit -m 'Add some amazing feature'`)
4. 推送到分支 (`git push origin feature/amazing-feature`)
5. 开启 Pull Request

### 📄 开源协议

本项目采用 MIT 许可证 - 详见 [LICENSE](LICENSE) 文件。

### 🙏 致谢

- [egui](https://github.com/emilk/egui) - 出色的即时模式 GUI 框架
- [image-rs](https://github.com/image-rs/image) - 优秀的图片处理库
- Rust 社区提供的强大工具和库

---

<div align="center">

Made with ❤️ using Rust

</div>

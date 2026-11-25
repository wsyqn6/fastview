# FastView

<div align="center">

[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)
[![Version](https://img.shields.io/github/v/tag/wsyqn6/fastview)](https://github.com/wsyqn6/fastview/releases)
[![Website](https://img.shields.io/badge/Website-wsyqn6.github.io/fastview-00d4ff)](https://wsyqn6.github.io/fastview)
[![Rust](https://img.shields.io/badge/Rust-1.88+-orange.svg)](https://www.rust-lang.org)
[![Platform](https://img.shields.io/badge/Platform-Windows-blue.svg)](https://www.microsoft.com/windows)
[![CI](https://github.com/wsyqn6/fastview/actions/workflows/ci.yml/badge.svg)](https://github.com/wsyqn6/fastview/actions)


**A fast, lightweight, and modern image viewer built with Rust**

> ⚠️ **Development Preview**: This project is in active development. Features and APIs may change.

[English](#english) | [中文](#中文)

</div>

---

## English

### 🚀 Overview

FastView is a minimalist image viewer built with Rust and gpui-unofficial. On Windows it uses WIC DCT scaling for a fast first frame, then fills in detail on demand, with a self-drawn titlebar and native backdrop materials.

**Core Philosophy**: Do one thing well — view images fast.

### ✨ Features

- ⚡ **Fast First Frame** - WIC 1/8 → 1/2ⁿ preview before any full decode
- 🖼️ **Formats** - JPEG, PNG, WebP, GIF, BMP, TIFF
- 🔄 **EXIF Orientation** - Read and normalize rotation
- 🎯 **Zoom & Pan** - Fit, Original, Fill, wheel zoom, drag to pan
- 🗂️ **Folder & ZIP** - Natural-sorted browsing, shell thumbnails, plain ZIP entries
- 🪟 **Windows Native Look** - Mica/Acrylic, self-drawn titlebar, adjustable opacity
- 🎨 **Polished UI** - Photo info, minimap, settings panel, Chinese/English text
- 💾 **Bounded Cache** - Byte-budgeted texture LRU with prefetch

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

# Run the application (Windows)
.\target\release\fastview.exe
```

#### Pre-built Binaries

Download the latest release from the [Releases page](https://github.com/wsyqn6/fastview/releases).

### ⌨️ Keyboard Shortcuts

| Action | Key |
|--------|-----|
| Previous / Next | `←` / `→` |
| Zoom In / Out | `+` / `-` |
| Rotate CW / CCW | `R` / `Shift+R` |
| Fit / Original / Fill | `0` / `1` / `2` |
| Fullscreen (exit with `Esc`) | `F` / `F11` |
| Borderless Mode | `V` |
| Status Bar / Thumbnails | `S` / `T` |
| Photo Info | `I` |
| Settings | `Ctrl+,` |
| Open File | `Ctrl+O` |
| Menus | `Alt+F/V/S/H` |
| Show Shortcuts | `H` / `?` |

### 🏗️ Tech Stack

- **GUI**: gpui-unofficial 1.28.1 with Windows DirectComposition
- **Decode**: WIC DCT scaling + image 0.25 fallback
- **EXIF**: kamadak-exif
- **Thumbnails**: Windows shell image factory
- **Config**: serde + dirs (`%APPDATA%\fastview\gpui-settings.json`)

### 📂 Project Structure

```
src/
├── main.rs      # 入口，Windows 模块按 cfg 门控
├── app/         # 视图、浮层、快捷键、标题栏、设置
├── decode/      # WIC 缩放选档与 image 兜底
├── cache.rs     # 按字节预算的纹理 LRU
├── viewer.rs    # 缩放 / 平移 / 旋转状态
├── folder.rs    # 目录扫描与自然排序
├── shell.rs     # 系统缩略图（Windows）
├── win.rs       # DWM / 材质 / 窗口钩子（Windows）
├── metadata.rs  # EXIF 与文件信息
├── text.rs      # 中英文案
└── archive.rs   # ZIP 只读解压
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

- [gpui-unofficial](https://crates.io/crates/gpui-unofficial) - GPU-accelerated UI framework
- [image-rs](https://github.com/image-rs/image) - Image decoding fallback
- Windows WIC / shell - Fast scaled decode and cached thumbnails

---

## 中文

> ⚠️ **开发预览版**：本项目正在积极开发中，功能和 API 可能会发生变化。

### 🚀 项目简介

FastView 是一款极简图片查看器，基于 Rust 与 gpui-unofficial。Windows 上用 WIC DCT 缩放先出首帧，再按需补清晰，配合自绘标题栏与系统材质。

**核心理念**：专注做好一件事 —— 快速查看图片。

### ✨ 功能特性

- ⚡ **快速首帧** - WIC 1/8 → 1/2ⁿ 预览，无需先全解码
- 🖼️ **格式** - JPEG、PNG、WebP、GIF、BMP、TIFF
- 🔄 **EXIF 方向** - 读取并归一化旋转
- 🎯 **缩放平移** - 适应 / 原始 / 填充、滚轮缩放、拖拽平移
- 🗂️ **目录与 ZIP** - 自然排序翻页、系统缩略图、普通 ZIP 直读
- 🪟 **Windows 原生观感** - 云母 / 亚克力、自绘标题栏、透明度可调
- 🎨 **精致界面** - 照片信息、minimap、设置面板、中英文案
- 💾 **有界缓存** - 按字节预算的纹理 LRU 与预取

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

# 运行程序（Windows）
.\target\release\fastview.exe
```

#### 预编译版本

从 [Releases 页面](https://github.com/wsyqn6/fastview/releases) 下载最新版本。

### ⌨️ 快捷键

| 功能 | 按键 |
|------|------|
| 上一张 / 下一张 | `←` / `→` |
| 放大 / 缩小 | `+` / `-` |
| 顺时针 / 逆时针旋转 | `R` / `Shift+R` |
| 适应 / 原始 / 填充 | `0` / `1` / `2` |
| 全屏（`Esc` 退出） | `F` / `F11` |
| 无边框模式 | `V` |
| 状态栏 / 缩略图 | `S` / `T` |
| 照片信息 | `I` |
| 设置 | `Ctrl+,` |
| 打开文件 | `Ctrl+O` |
| 展开菜单 | `Alt+F/V/S/H` |
| 快捷键说明 | `H` / `?` |

### 🏗️ 技术栈

- **GUI**：gpui-unofficial 1.28.1 + Windows DirectComposition
- **解码**：WIC DCT 缩放 + image 0.25 兜底
- **EXIF**：kamadak-exif
- **缩略图**：Windows shell 缩略图缓存
- **配置**：serde + dirs（`%APPDATA%\fastview\gpui-settings.json`）

### 📂 项目结构

```
src/
├── main.rs      # 入口，Windows 模块按 cfg 门控
├── app/         # 视图、浮层、快捷键、标题栏、设置
├── decode/      # WIC 缩放选档与 image 兜底
├── cache.rs     # 按字节预算的纹理 LRU
├── viewer.rs    # 缩放 / 平移 / 旋转状态
├── folder.rs    # 目录扫描与自然排序
├── shell.rs     # 系统缩略图（Windows）
├── win.rs       # DWM / 材质 / 窗口钩子（Windows）
├── metadata.rs  # EXIF 与文件信息
├── text.rs      # 中英文案
└── archive.rs   # ZIP 只读解压
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

- [gpui-unofficial](https://crates.io/crates/gpui-unofficial) - GPU 加速 UI 框架
- [image-rs](https://github.com/image-rs/image) - 解码兜底
- Windows WIC / shell - 快速缩放解码与缩略图缓存

---

<div align="center">

Made with ❤️ using Rust

</div>

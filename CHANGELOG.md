# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.4.0] - 2026-11-25

### Added
- 单实例：已有进程接收新打开的图片路径，不再重复启动
- gpui 查看器：WIC DCT 分级预览、按字节预算的纹理 LRU 与预取
- 系统缩略图条、目录翻页与只读 ZIP 直读
- 自绘标题栏 / 菜单、云母 / 亚克力材质与透明度调节、中英文案
- 照片信息浮层、minimap、设置面板与完整快捷键
- 启动时检查并按需静默注册为受支持图片格式的“打开方式”候选（不接管默认程序）

### Changed
- 首图解码与 gpui 启动并行，窗口就绪后直接复用预解码结果
- 以 gpui 实现替换 egui；旧 egui 线归档至 `archive/egui`
- 配置迁移至 `%APPDATA%\fastview\gpui-settings.json`
- CI / Release 改为 Windows-only，并修复 tag 版本号注入
- 「获取更新」改为打开 GitHub 发布页，不自动下载或替换程序

### Fixed
- 旋转先复用当前纹理，后台再补目标档
- 缓存命中直接换纹理，避免重复解码

## [0.3.1] - 2026-05-10

### Changed
- Removed test_update_server binary configuration from Cargo.toml
- Updated auto-update UI integration with improved dialog handling

### Fixed
- Resolved clippy warnings for unnecessary casts
- Fixed code formatting issues in updater module

## [0.3.0] - 2026-04-26

### Added
- Image resource reuse optimization to eliminate duplicate decoding
- GenerateThumbnailFromCache command for cache-based thumbnail generation
- Graceful fallback mechanism when thumbnail cache miss occurs

### Changed
- Optimized thumbnail loading to visible range only (significant performance improvement)
- Standardized thumbnail sizes to 100px for navigation and bottom bar
- Increased prefetch priority to improve cache hit rate
- Removed preload restriction for better cache performance
- Optimized GitHub Actions CI workflow configuration

### Fixed
- Resolved clippy manual_div_ceil warning in example code
- Implemented graceful error handling for thumbnail cache operations

## [0.2.8] - 2026-04-25

### Added
- Drag and drop support for opening images
- Automatic EXIF orientation correction
- Chinese language support with proper font rendering
- Modular code structure (types, fonts, app, main)

### Fixed
- Image zoom jumping issue when switching modes
- Chinese character display problems
- Compilation errors in initial setup

### Changed
- Migrated from Tauri to pure Rust + Eframe
- Improved image loading with EXIF support
- Enhanced zoom mode transitions

## [0.1.1] - 2026-04-07

### Initial Development Release

#### Features
- Fast image viewing with instant startup (< 1 second)
- Support for 8+ image formats (JPEG, PNG, GIF, WebP, BMP, TIFF, ICO, AVIF)
- Multiple zoom modes: Fit, Fill, Original, Custom
- Image rotation (90° clockwise/counterclockwise)
- Fullscreen mode
- Drag mode for panning large images
- Thumbnail preview for zoomed images
- Keyboard shortcuts for all major actions
- Settings persistence (language, cache size)
- Cross-platform support (Windows, Linux, macOS)
- Modern UI powered by egui
- Image caching for better performance
- Status bar with image information

#### Technical
- Built with Rust for safety and performance
- Minimal dependencies (~5MB release binary)
- Low memory footprint (< 20MB)
- Configurable cache management
- Bilingual support (English/Chinese)

---

## Version Guidelines

### Version Numbering

- **MAJOR** version for incompatible API changes
- **MINOR** version for backwards-compatible functionality additions
- **PATCH** version for backwards-compatible bug fixes

### Change Categories

- **Added** - New features
- **Changed** - Changes in existing functionality
- **Deprecated** - Soon-to-be removed features
- **Removed** - Removed features
- **Fixed** - Bug fixes
- **Security** - Security improvements

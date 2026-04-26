# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

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

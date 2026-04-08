use eframe::egui;
use lru::LruCache;
use std::path::PathBuf;
use std::sync::Arc;

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum Language {
    Chinese,
    English,
}

impl Default for Language {
    fn default() -> Self {
        if let Ok(lang) = std::env::var("LANG") {
            if lang.starts_with("zh") || lang.starts_with("zh_CN") || lang.starts_with("zh_TW") {
                return Language::Chinese;
            }
        }
        Language::English
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Settings {
    pub language: Language,
    pub max_cache_size: usize,
    pub show_status_bar: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            language: Language::default(),
            max_cache_size: 10,
            show_status_bar: true,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub enum TextKey {
    // Menu titles
    MenuFile,
    MenuView,
    MenuSettings,
    MenuHelp,

    // File menu
    OpenFile,
    Exit,

    // View menu
    FitToWindow,
    OriginalSize,
    FillWindow,
    ZoomIn,
    ZoomOut,
    RotateClockwise,
    RotateCounterClockwise,
    ToggleFullscreen,
    ToggleBorderless,

    // Help menu
    ShortcutsHelp,
    AboutFastView,
    CheckForUpdates,

    // About dialog
    Version,
    GitHub,
    OK,
    AppDescription,

    // Status
    Fit,
    Fill,
    Original,

    // Shortcuts window
    Navigation,
    ZoomAndView,
    Rotation,
    System,
    PreviousNext,
    ZoomInOut,
    RotateLeft,
    RotateRight,
    DragMode,
    ExitFullscreen,
    ShowHideShortcuts,
    ClickToOpen,

    // Settings
    General,
    Language,
    Cache,
    MaxCacheSize,
    ShowStatusBar,
    ResetSettings,
    Chinese,
    English,
}

impl TextKey {
    pub fn text(self, lang: Language) -> &'static str {
        match lang {
            Language::Chinese => match self {
                TextKey::MenuFile => "文件",
                TextKey::MenuView => "查看",
                TextKey::MenuSettings => "设置",
                TextKey::MenuHelp => "帮助",
                TextKey::OpenFile => "打开文件...",
                TextKey::Exit => "退出",
                TextKey::FitToWindow => "适应窗口",
                TextKey::OriginalSize => "原始尺寸",
                TextKey::FillWindow => "填充窗口",
                TextKey::ZoomIn => "放大",
                TextKey::ZoomOut => "缩小",
                TextKey::RotateClockwise => "顺时针旋转",
                TextKey::RotateCounterClockwise => "逆时针旋转",
                TextKey::ToggleFullscreen => "切换全屏",
                TextKey::ToggleBorderless => "无边框模式",
                TextKey::ShortcutsHelp => "快捷键",
                TextKey::AboutFastView => "关于 FastView",
                TextKey::CheckForUpdates => "检查更新...",
                TextKey::Version => "版本",
                TextKey::GitHub => "GitHub",
                TextKey::OK => "确定",
                TextKey::AppDescription => "一个轻量级图片查看器",
                TextKey::Navigation => "导航",
                TextKey::ZoomAndView => "缩放与视图",
                TextKey::Rotation => "旋转",
                TextKey::System => "系统",
                TextKey::Fit => "适应",
                TextKey::Fill => "填充",
                TextKey::Original => "原始",
                TextKey::PreviousNext => "上一张/下一张",
                TextKey::ZoomInOut => "放大/缩小",
                TextKey::RotateLeft => "逆时针旋转",
                TextKey::RotateRight => "顺时针旋转",
                TextKey::DragMode => "拖动模式（按住空格）",
                TextKey::ExitFullscreen => "退出全屏",
                TextKey::ShowHideShortcuts => "显示/隐藏快捷键",
                TextKey::ClickToOpen => "点击打开\n或拖拽图片",
                TextKey::General => "通用",
                TextKey::Language => "语言",
                TextKey::Cache => "缓存",
                TextKey::MaxCacheSize => "最大缓存数量",
                TextKey::ShowStatusBar => "显示状态栏",
                TextKey::ResetSettings => "重置设置",
                TextKey::Chinese => "中文",
                TextKey::English => "英文",
            },
            Language::English => match self {
                TextKey::MenuFile => "File",
                TextKey::MenuView => "View",
                TextKey::MenuSettings => "Settings",
                TextKey::MenuHelp => "Help",
                TextKey::OpenFile => "Open File...",
                TextKey::Exit => "Exit",
                TextKey::FitToWindow => "Fit to Window",
                TextKey::OriginalSize => "Original Size",
                TextKey::FillWindow => "Fill Window",
                TextKey::ZoomIn => "Zoom In",
                TextKey::ZoomOut => "Zoom Out",
                TextKey::RotateClockwise => "Rotate Clockwise",
                TextKey::RotateCounterClockwise => "Rotate Counter-Clockwise",
                TextKey::ToggleFullscreen => "Toggle Fullscreen",
                TextKey::ToggleBorderless => "Borderless Mode",
                TextKey::ShortcutsHelp => "Shortcuts",
                TextKey::AboutFastView => "About FastView",
                TextKey::CheckForUpdates => "Check for Updates...",
                TextKey::Version => "Version",
                TextKey::GitHub => "GitHub",
                TextKey::OK => "OK",
                TextKey::AppDescription => "A lightweight image viewer",
                TextKey::Navigation => "Navigation",
                TextKey::ZoomAndView => "Zoom & View",
                TextKey::Rotation => "Rotation",
                TextKey::System => "System",
                TextKey::Fit => "Fit",
                TextKey::Fill => "Fill",
                TextKey::Original => "Original",
                TextKey::PreviousNext => "Previous/Next",
                TextKey::ZoomInOut => "Zoom In/Out",
                TextKey::RotateLeft => "Rotate Left",
                TextKey::RotateRight => "Rotate Right",
                TextKey::DragMode => "Drag Mode (hold Space)",
                TextKey::ExitFullscreen => "Exit Fullscreen",
                TextKey::ShowHideShortcuts => "Show/Hide Shortcuts",
                TextKey::ClickToOpen => "Click to open\nor drag & drop images",
                TextKey::General => "General",
                TextKey::Language => "Language",
                TextKey::Cache => "Cache",
                TextKey::MaxCacheSize => "Max Cache Size",
                TextKey::ShowStatusBar => "Show Status Bar",
                TextKey::ResetSettings => "Reset Settings",
                TextKey::Chinese => "Chinese",
                TextKey::English => "English",
            },
        }
    }
}

#[derive(PartialEq, Clone, Debug)]
pub enum ZoomMode {
    Fit,
    Fill,
    Original,
    Custom,
}

/// 缓存条目类型(支持双层缓存)
#[derive(Clone)]
#[allow(dead_code)] // 预留变体和方法,用于未来扩展
pub enum CacheEntry {
    /// 完整图片缓存(主纹理 + 缩略图)
    FullImage(Arc<CachedImage>),

    /// 仅缩略图缓存(用于预加载的相邻图片)
    ThumbnailOnly {
        thumb_texture: egui::TextureHandle,
        image_size: egui::Vec2,
    },
}

#[allow(dead_code)] // 预留方法,用于未来扩展
impl CacheEntry {
    /// 获取主纹理(如果有)
    pub fn main_texture(&self) -> Option<&egui::TextureHandle> {
        match self {
            CacheEntry::FullImage(cached) => Some(&cached.texture),
            CacheEntry::ThumbnailOnly { .. } => None,
        }
    }

    /// 获取缩略图纹理
    pub fn thumbnail_texture(&self) -> &egui::TextureHandle {
        match self {
            CacheEntry::FullImage(cached) => &cached.thumbnail_texture,
            CacheEntry::ThumbnailOnly { thumb_texture, .. } => thumb_texture,
        }
    }

    /// 获取图片尺寸
    pub fn image_size(&self) -> egui::Vec2 {
        match self {
            CacheEntry::FullImage(cached) => cached.image_size,
            CacheEntry::ThumbnailOnly { image_size, .. } => *image_size,
        }
    }

    /// 估算内存占用(字节)
    pub fn estimated_memory_bytes(&self) -> usize {
        match self {
            CacheEntry::FullImage(cached) => cached.estimated_memory_bytes(),
            CacheEntry::ThumbnailOnly { .. } => {
                // 缩略图固定150x150 RGBA
                150 * 150 * 4
            }
        }
    }

    /// 判断是否为完整图片
    pub fn is_full_image(&self) -> bool {
        matches!(self, CacheEntry::FullImage(_))
    }
}

#[derive(Clone)]
pub struct CachedImage {
    pub texture: egui::TextureHandle,
    pub thumbnail_texture: egui::TextureHandle,
    pub image_size: egui::Vec2,
}

impl CachedImage {
    /// 估算内存占用（字节）
    pub fn estimated_memory_bytes(&self) -> usize {
        // 主纹理 + 缩略图文理 (RGBA, 4 bytes per pixel)
        let main_texture_bytes = (self.image_size.x * self.image_size.y * 4.0) as usize;
        let thumb_texture_bytes = 200 * 200 * 4; // 缩略图固定大小
        main_texture_bytes + thumb_texture_bytes
    }
}

pub type ImageCache = Arc<std::sync::Mutex<LruCache<PathBuf, CacheEntry>>>;

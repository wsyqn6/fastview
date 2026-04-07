use eframe::egui;
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
    // Menu
    Settings,
    File,
    OpenFile,
    Exit,
    View,
    Fullscreen,
    Shortcuts,
    // Status
    Fit,
    Fill,
    Original,
    // Shortcuts window
    PreviousNext,
    ZoomInOut,
    FitToWindow,
    OriginalSize,
    FillWindow,
    RotateLeft,
    RotateRight,
    ToggleFullscreen,
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
                TextKey::Settings => "设置",
                TextKey::File => "文件",
                TextKey::OpenFile => "打开文件",
                TextKey::Exit => "退出",
                TextKey::View => "视图",
                TextKey::Fullscreen => "全屏",
                TextKey::Shortcuts => "快捷键",
                TextKey::Fit => "适应",
                TextKey::Fill => "填充",
                TextKey::Original => "原始",
                TextKey::PreviousNext => "上一张/下一张",
                TextKey::ZoomInOut => "放大/缩小",
                TextKey::FitToWindow => "适应窗口",
                TextKey::OriginalSize => "原始尺寸",
                TextKey::FillWindow => "填充窗口",
                TextKey::RotateLeft => "逆时针旋转",
                TextKey::RotateRight => "顺时针旋转",
                TextKey::ToggleFullscreen => "切换全屏",
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
                TextKey::Settings => "Settings",
                TextKey::File => "File",
                TextKey::OpenFile => "Open File",
                TextKey::Exit => "Exit",
                TextKey::View => "View",
                TextKey::Fullscreen => "Fullscreen",
                TextKey::Shortcuts => "Shortcuts",
                TextKey::Fit => "Fit",
                TextKey::Fill => "Fill",
                TextKey::Original => "Original",
                TextKey::PreviousNext => "Previous/Next",
                TextKey::ZoomInOut => "Zoom In/Out",
                TextKey::FitToWindow => "Fit to Window",
                TextKey::OriginalSize => "Original Size",
                TextKey::FillWindow => "Fill Window",
                TextKey::RotateLeft => "Rotate Left",
                TextKey::RotateRight => "Rotate Right",
                TextKey::ToggleFullscreen => "Toggle Fullscreen",
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

#[derive(PartialEq, Clone)]
pub enum ZoomMode {
    Fit,
    Fill,
    Original,
    Custom,
}

#[derive(Clone)]
pub struct CachedImage {
    pub texture: egui::TextureHandle,
    pub thumbnail_texture: egui::TextureHandle,
    pub image_size: egui::Vec2,
}

pub type ImageCache = Arc<std::sync::Mutex<std::collections::HashMap<PathBuf, Arc<CachedImage>>>>;

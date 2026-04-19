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
        if let Ok(lang) = std::env::var("LANG")
            && (lang.starts_with("zh") || lang.starts_with("zh_CN") || lang.starts_with("zh_TW"))
        {
            return Language::Chinese;
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

/// 解码后的图片数据（线程安全共享）
pub struct DecodedImage {
    /// RGBA 像素数据
    pub data: Vec<u8>,
    /// 宽度、高度
    pub width: u32,
    pub height: u32,
}

/// 分块图片数据
#[derive(Clone)]
pub struct TiledImage {
    /// 原始图片尺寸
    pub width: u32,
    pub height: u32,
    /// 缩略图数据（用于背景显示）
    pub thumbnail: Arc<DecodedImage>,
    /// 分块信息
    pub tiles: Vec<TileInfo>,
    /// 块大小（默认1024）
    pub tile_size: u32,
    /// 列数
    pub cols: u32,
    /// 行数
    pub rows: u32,
}

/// 单个图片块信息
#[derive(Clone, Debug)]
pub struct TileInfo {
    /// 块的坐标（列，行）
    pub col: u32,
    pub row: u32,
    /// 块在原始图片中的位置和尺寸
    #[allow(dead_code)]
    pub x: u32,
    #[allow(dead_code)]
    pub y: u32,
    pub width: u32,
    pub height: u32,
    /// 是否已加载到纹理
    #[allow(dead_code)]
    pub loaded: bool,
}

#[derive(PartialEq, Clone, Debug)]
pub enum ZoomMode {
    Fit,
    Fill,
    Original,
    Custom,
}

/// 缓存条目类型
#[derive(Clone)]
pub enum CacheEntry {
    /// 解码后的图片数据
    Decoded(Arc<DecodedImage>),
    /// 分块图片元数据（不包含像素数据，像素数据单独管理）
    TiledMeta(Arc<TiledImage>),
}

impl CacheEntry {
    /// 估算内存占用(字节)
    pub fn estimated_memory_bytes(&self) -> usize {
        match self {
            CacheEntry::Decoded(img) => (img.width * img.height * 4) as usize,
            CacheEntry::TiledMeta(tiled) => {
                // 只计算缩略图的内存，块数据单独管理
                (tiled.thumbnail.width * tiled.thumbnail.height * 4) as usize
            }
        }
    }
}

pub type ImageCache = Arc<std::sync::Mutex<LruCache<PathBuf, CacheEntry>>>;

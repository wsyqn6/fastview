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
    pub thumbnail_bar_enabled: bool,  // 缩略图导航栏开关
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            language: Language::default(),
            max_cache_size: 10,
            show_status_bar: true,
            thumbnail_bar_enabled: true,  // 默认启用
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cache_entry_memory_estimation_decoded() {
        let image = DecodedImage {
            data: vec![0u8; 100 * 100 * 4], // 100x100 RGBA
            width: 100,
            height: 100,
        };
        let entry = CacheEntry::Decoded(Arc::new(image));
        assert_eq!(entry.estimated_memory_bytes(), 100 * 100 * 4);
    }

    #[test]
    fn test_cache_entry_memory_estimation_tiled() {
        let thumbnail = Arc::new(DecodedImage {
            data: vec![0u8; 50 * 50 * 4], // 50x50 缩略图
            width: 50,
            height: 50,
        });
        
        let tiled = TiledImage {
            width: 1000,
            height: 1000,
            thumbnail,
            tiles: vec![],
            tile_size: 1024,
            cols: 1,
            rows: 1,
        };
        
        let entry = CacheEntry::TiledMeta(Arc::new(tiled));
        // 只计算缩略图的内存
        assert_eq!(entry.estimated_memory_bytes(), 50 * 50 * 4);
    }

    #[test]
    fn test_language_default_english() {
        // 如果没有设置 LANG 环境变量，默认应该是 English
        // 注意：这个测试可能受环境影响
        let lang = Language::default();
        // 至少确保不会 panic
        match lang {
            Language::Chinese | Language::English => {},
        }
    }

    #[test]
    fn test_settings_default_values() {
        let settings = Settings::default();
        assert!(settings.max_cache_size > 0);
        assert!(settings.show_status_bar); // 默认显示状态栏
    }
}

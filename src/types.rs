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

/// 解码后的图片数据（线程安全共享）
pub struct DecodedImage {
    /// RGBA 像素数据
    pub data: Vec<u8>,
    /// 宽度、高度
    pub width: u32,
    pub height: u32,
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
}

impl CacheEntry {
    /// 估算内存占用(字节)
    pub fn estimated_memory_bytes(&self) -> usize {
        match self {
            CacheEntry::Decoded(img) => (img.width * img.height * 4) as usize,
        }
    }
}


pub type ImageCache = Arc<std::sync::Mutex<LruCache<PathBuf, CacheEntry>>>;

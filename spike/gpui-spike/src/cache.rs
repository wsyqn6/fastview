//! 已解码图片的 LRU 缓存
//!
//! 缓存的是**已上传的纹理**（`Arc<RenderImage>`）：命中时不解码、不重新上传，
//! 下一帧直接绘制。代价是纹理不能随 `RenderImage` 销毁自动回收，
//! 淘汰时必须把 `Arc` 交回调用方去 `window.drop_image`。
//!
//! 这里刻意不碰 `Window`：回收责任转成返回值，逻辑才能单测。

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use gpui::RenderImage;

use crate::decode::ImageMeta;

/// 单张图缓存的所有档位
struct Cached {
    meta: ImageMeta,
    /// denom（1/2ⁿ 的 n）-> 纹理
    levels: HashMap<u32, Level>,
    last_used: u64,
}

struct Level {
    texture: Arc<RenderImage>,
    bytes: usize,
}

pub struct ImageCache {
    entries: HashMap<PathBuf, Cached>,
    budget: usize,
    bytes: usize,
    tick: u64,
}

impl ImageCache {
    pub fn new(budget: usize) -> Self {
        Self {
            entries: HashMap::new(),
            budget,
            bytes: 0,
            tick: 0,
        }
    }

    /// 已知元信息时省掉一次 probe
    pub fn meta(&self, path: &Path) -> Option<ImageMeta> {
        self.entries.get(path).map(|cached| cached.meta)
    }

    pub fn contains(&self, path: &Path) -> bool {
        self.entries.contains_key(path)
    }

    /// 查指定档位；命中会刷新 LRU
    pub fn get(&mut self, path: &Path, denom: u32) -> Option<Arc<RenderImage>> {
        self.tick += 1;
        let tick = self.tick;
        let cached = self.entries.get_mut(path)?;
        cached.last_used = tick;
        cached
            .levels
            .get(&denom)
            .map(|level| Arc::clone(&level.texture))
    }

    /// 写入一个档位，返回需要回收的纹理
    ///
    /// `keep` 是当前正在显示的图，淘汰时永不选中它。
    pub fn insert(
        &mut self,
        path: PathBuf,
        meta: ImageMeta,
        denom: u32,
        texture: Arc<RenderImage>,
        keep: &Path,
    ) -> Vec<Arc<RenderImage>> {
        self.tick += 1;

        let bytes = texture_bytes(&texture);
        let tick = self.tick;
        let cached = self.entries.entry(path).or_insert_with(|| Cached {
            meta,
            levels: HashMap::new(),
            last_used: 0,
        });
        cached.meta = meta;
        cached.last_used = tick;

        // 同档位替换：旧纹理也要交回去回收
        let mut dropped = Vec::new();
        if let Some(old) = cached.levels.insert(denom, Level { texture, bytes }) {
            self.bytes = self.bytes.saturating_sub(old.bytes);
            dropped.push(old.texture);
        }
        self.bytes += bytes;

        dropped.extend(self.evict(keep));
        dropped
    }

    /// 淘汰到预算以内，跳过 `keep`
    fn evict(&mut self, keep: &Path) -> Vec<Arc<RenderImage>> {
        if self.bytes <= self.budget {
            return Vec::new();
        }

        let mut victims: Vec<(PathBuf, u64)> = self
            .entries
            .iter()
            .filter(|(path, _)| path.as_path() != keep)
            .map(|(path, cached)| (path.clone(), cached.last_used))
            .collect();
        victims.sort_by_key(|(_, last_used)| *last_used);

        let mut dropped = Vec::new();
        for (path, _) in victims {
            if self.bytes <= self.budget {
                break;
            }
            let Some(cached) = self.entries.remove(&path) else {
                continue;
            };
            for (_, level) in cached.levels {
                self.bytes = self.bytes.saturating_sub(level.bytes);
                dropped.push(level.texture);
            }
        }
        dropped
    }
}

/// `RenderImage` 的像素字节数（gpui 统一按 RGBA/BGRA 4 字节存）
fn texture_bytes(texture: &RenderImage) -> usize {
    let size = texture.size(0);
    (size.width.0.max(0) as usize) * (size.height.0.max(0) as usize) * 4
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::decode::ImageMeta;
    use gpui::RenderImage;
    use image::RgbaImage;

    fn meta(width: u32, height: u32) -> ImageMeta {
        ImageMeta {
            storage: (width, height),
            orientation: image::metadata::Orientation::NoTransforms,
        }
    }

    /// 造一张指定尺寸的纹理（内容无所谓，只算字节）
    fn texture(width: u32, height: u32) -> Arc<RenderImage> {
        let buffer = RgbaImage::from_raw(width, height, vec![0; (width * height * 4) as usize])
            .expect("尺寸与缓冲一致");
        Arc::new(RenderImage::new(vec![image::Frame::new(buffer)]))
    }

    #[test]
    fn hit_returns_same_texture() {
        let mut cache = ImageCache::new(64 * 1024 * 1024);
        let path = PathBuf::from("a.jpg");
        let texture = texture(64, 64);

        assert!(cache.get(&path, 2).is_none());
        cache.insert(path.clone(), meta(256, 256), 2, Arc::clone(&texture), &path);

        let hit = cache.get(&path, 2).expect("应当命中");
        assert!(Arc::ptr_eq(&hit, &texture));
        // 其它档位互不影响
        assert!(cache.get(&path, 1).is_none());
    }

    #[test]
    fn evicts_least_recently_used_and_keeps_current() {
        // 预算只够 2 张 64×64（各 16 KB）
        let mut cache = ImageCache::new(40 * 1024);
        let a = PathBuf::from("a.jpg");
        let b = PathBuf::from("b.jpg");
        let c = PathBuf::from("c.jpg");

        cache.insert(a.clone(), meta(64, 64), 1, texture(64, 64), &a);
        cache.insert(b.clone(), meta(64, 64), 1, texture(64, 64), &b);
        // 摸一下 a，让 b 变成最久未用
        assert!(cache.get(&a, 1).is_some());

        let dropped = cache.insert(c.clone(), meta(64, 64), 1, texture(64, 64), &c);

        assert_eq!(dropped.len(), 1, "应当淘汰一张");
        assert!(cache.contains(&a), "刚用过的 a 应保留");
        assert!(cache.contains(&c), "新插入的 c 必须保留");
        assert!(!cache.contains(&b), "最久未用的 b 应被淘汰");
    }

    #[test]
    fn never_evicts_the_kept_path() {
        // 预算连一张都放不下
        let mut cache = ImageCache::new(1024);
        let current = PathBuf::from("current.jpg");

        let dropped = cache.insert(
            current.clone(),
            meta(64, 64),
            1,
            texture(64, 64),
            &current,
        );

        assert!(cache.contains(&current), "当前显示的图不能被淘汰");
        // 超预算时其它项会被清空，但 keep 保留
        assert!(dropped.is_empty());
    }

    #[test]
    fn replacing_same_denom_returns_old_texture() {
        let mut cache = ImageCache::new(64 * 1024 * 1024);
        let path = PathBuf::from("a.jpg");
        let first = texture(32, 32);

        cache.insert(path.clone(), meta(64, 64), 1, first, &path);
        let dropped = cache.insert(path.clone(), meta(64, 64), 1, texture(32, 32), &path);

        assert_eq!(dropped.len(), 1, "旧纹理需要回收，否则 atlas 泄漏");
    }

    #[test]
    fn sizes_match_render_image() {
        let texture = texture(100, 50);
        assert_eq!(texture_bytes(&texture), 100 * 50 * 4);
    }
}

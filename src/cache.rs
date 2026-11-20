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

use crate::decode::{ImageMeta, Rotation};

/// 单张图缓存的所有档位
struct Cached {
    meta: ImageMeta,
    /// (用户旋转, denom) -> 纹理；不同角度各存一份
    levels: HashMap<(Rotation, u32), Level>,
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

    /// 当前占用 / 预算 / 图片条目数，供调试日志输出
    pub fn usage(&self) -> (usize, usize, usize) {
        (self.bytes, self.budget, self.entries.len())
    }

    /// 查指定档位；命中会刷新 LRU
    pub fn get(&mut self, path: &Path, rotation: Rotation, denom: u32) -> Option<Arc<RenderImage>> {
        self.tick += 1;
        let tick = self.tick;
        let cached = self.entries.get_mut(path)?;
        cached.last_used = tick;
        cached
            .levels
            .get(&(rotation, denom))
            .map(|level| Arc::clone(&level.texture))
    }

    /// 写入一个档位，返回需要回收的纹理
    ///
    /// `keep` 是当前正在显示的图，淘汰时永不选中它。
    /// 非 0° 旋转档只留最新角度：旋转可即时重建，存四个角度会吃满预算。
    pub fn insert(
        &mut self,
        path: PathBuf,
        meta: ImageMeta,
        rotation: Rotation,
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

        let mut dropped = Vec::new();
        // 淘汰同图的其它旋转角度，0° 基准保留（切图/预取仍按 0° 命中）
        if rotation != Rotation::None {
            let stale: Vec<(Rotation, u32)> = cached
                .levels
                .keys()
                .filter(|(r, _)| *r != Rotation::None && *r != rotation)
                .copied()
                .collect();
            for key in stale {
                if let Some(level) = cached.levels.remove(&key) {
                    self.bytes = self.bytes.saturating_sub(level.bytes);
                    dropped.push(level.texture);
                }
            }
        }

        // 同档位替换：旧纹理也要交回去回收
        if let Some(old) = cached
            .levels
            .insert((rotation, denom), Level { texture, bytes })
        {
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
            scalable: true,
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

        assert!(cache.get(&path, Rotation::None, 2).is_none());
        cache.insert(
            path.clone(),
            meta(256, 256),
            Rotation::None,
            2,
            Arc::clone(&texture),
            &path,
        );

        let hit = cache.get(&path, Rotation::None, 2).expect("应当命中");
        assert!(Arc::ptr_eq(&hit, &texture));
        // 其它档位与角度互不影响
        assert!(cache.get(&path, Rotation::None, 1).is_none());
        assert!(cache.get(&path, Rotation::Cw90, 2).is_none());
    }

    #[test]
    fn evicts_least_recently_used_and_keeps_current() {
        // 预算只够 2 张 64×64（各 16 KB）
        let mut cache = ImageCache::new(40 * 1024);
        let a = PathBuf::from("a.jpg");
        let b = PathBuf::from("b.jpg");
        let c = PathBuf::from("c.jpg");

        cache.insert(a.clone(), meta(64, 64), Rotation::None, 1, texture(64, 64), &a);
        cache.insert(b.clone(), meta(64, 64), Rotation::None, 1, texture(64, 64), &b);
        // 摸一下 a，让 b 变成最久未用
        assert!(cache.get(&a, Rotation::None, 1).is_some());

        let dropped = cache.insert(
            c.clone(),
            meta(64, 64),
            Rotation::None,
            1,
            texture(64, 64),
            &c,
        );

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
            Rotation::None,
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

        cache.insert(path.clone(), meta(64, 64), Rotation::None, 1, first, &path);
        let dropped = cache.insert(
            path.clone(),
            meta(64, 64),
            Rotation::None,
            1,
            texture(32, 32),
            &path,
        );

        assert_eq!(dropped.len(), 1, "旧纹理需要回收，否则 atlas 泄漏");
    }

    #[test]
    fn sizes_match_render_image() {
        let texture = texture(100, 50);
        assert_eq!(texture_bytes(&texture), 100 * 50 * 4);
    }

    #[test]
    fn rotation_keeps_base_and_drops_old_angle() {
        let mut cache = ImageCache::new(64 * 1024 * 1024);
        let path = PathBuf::from("a.jpg");
        let m = meta(64, 64);

        cache.insert(path.clone(), m, Rotation::None, 2, texture(32, 32), &path);
        cache.insert(path.clone(), m, Rotation::Cw90, 2, texture(32, 32), &path);
        // 换到 180°：90° 应被淘汰，0° 基准保留
        let dropped = cache.insert(
            path.clone(),
            m,
            Rotation::Cw180,
            2,
            texture(32, 32),
            &path,
        );

        assert_eq!(dropped.len(), 1, "旧旋转角度需回收");
        assert!(cache.get(&path, Rotation::None, 2).is_some(), "0° 基准保留");
        assert!(cache.get(&path, Rotation::Cw90, 2).is_none(), "90° 被淘汰");
        assert!(cache.get(&path, Rotation::Cw180, 2).is_some(), "新角度在缓存");
    }
}

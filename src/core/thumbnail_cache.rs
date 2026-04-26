use crate::core::loader::{LoadCommand, LoadResult};
use crate::core::types::{CacheEntry, ImageCache};
use crate::log_error;
use crate::utils::{lock_or_recover, to_non_zero_usize};
use eframe::egui;
use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::mpsc::Sender;

/// 统一的缩略图缓存管理器
///
/// 作为所有缩略图请求的唯一入口，确保：
/// - 同一张图片的缩略图只生成一次
/// - 自动从 image_cache 复用已解码的图片数据
/// - 统一管理纹理生命周期和淘汰策略
pub struct ThumbnailCache {
    /// 纹理缓存 (path -> texture)
    textures: lru::LruCache<PathBuf, egui::TextureHandle>,
    /// 待处理的请求 (避免重复请求)
    pending: HashSet<PathBuf>,
    /// 失败的记录 (避免重复尝试)
    failed: HashSet<PathBuf>,
}

impl ThumbnailCache {
    pub fn new() -> Self {
        Self {
            textures: lru::LruCache::new(to_non_zero_usize(30, 30)),
            pending: HashSet::new(),
            failed: HashSet::new(),
        }
    }

    /// 获取或创建缩略图纹理
    ///
    /// # 流程
    /// 1. 检查 textures 缓存 → 命中则返回
    /// 2. 检查 pending / failed → 跳过
    /// 3. 检查 image_cache 是否有解码数据：
    ///    - 有：立即在 UI 线程生成并缓存
    ///    - 无：发送异步请求到后台线程
    /// 4. 返回 Option<TextureHandle>
    pub fn get_or_create(
        &mut self,
        path: &PathBuf,
        ctx: &egui::Context,
        image_cache: &ImageCache,
        cmd_tx: &Option<Sender<LoadCommand>>,
    ) -> Option<egui::TextureHandle> {
        // 1. 检查缓存
        if let Some(texture) = self.textures.get(path) {
            return Some(texture.clone());
        }

        // 2. 检查是否在待处理或失败列表中
        if self.pending.contains(path) || self.failed.contains(path) {
            return None;
        }

        // 3. 标记为待处理
        self.pending.insert(path.clone());

        // 4. 尝试从 image_cache 立即生成
        if let Some(texture) = self.try_generate_from_cache(path, ctx, image_cache) {
            self.pending.remove(path);
            self.textures.put(path.clone(), texture.clone());
            return Some(texture);
        }

        // 5. 缓存未命中，发送异步请求
        if let Some(tx) = cmd_tx {
            debug_log!("[THUMB_CACHE] 请求异步生成缩略图: {:?}", path.file_name());
            let _ = tx.send(LoadCommand::GenerateThumbnailFromCache {
                path: path.clone(),
                size: 100,
                priority: crate::core::loader::LoadPriority::Low,
            });
        }

        None
    }

    /// 尝试从 image_cache 立即生成缩略图（同步）
    fn try_generate_from_cache(
        &self,
        path: &PathBuf,
        ctx: &egui::Context,
        image_cache: &ImageCache,
    ) -> Option<egui::TextureHandle> {
        let mut cache_guard = lock_or_recover(image_cache);
        let cached = cache_guard.get(path)?;

        match cached {
            CacheEntry::Decoded(image) => {
                use image::imageops::resize;

                // 计算缩略图尺寸（保持宽高比，统一为100px）
                let max_thumb_size = 100;
                let scale = (max_thumb_size as f32 / image.width.max(image.height) as f32).min(1.0);
                let thumb_w = (image.width as f32 * scale) as u32;
                let thumb_h = (image.height as f32 * scale) as u32;

                // 从原始像素数据创建 RgbaImage
                let img =
                    image::RgbaImage::from_raw(image.width, image.height, image.data.clone())?;

                // 生成缩略图（使用 Nearest 快速算法，速度优先）
                let thumb_img =
                    resize(&img, thumb_w, thumb_h, image::imageops::FilterType::Nearest);

                // 创建纹理
                let color_image = egui::ColorImage::from_rgba_unmultiplied(
                    [thumb_w as usize, thumb_h as usize],
                    thumb_img.as_raw(),
                );
                let texture = ctx.load_texture(
                    format!("thumb_{:?}", path.file_name()),
                    color_image,
                    egui::TextureOptions::LINEAR,
                );

                Some(texture)
            }
            CacheEntry::TiledMeta(tiled) => {
                // 对于分块图片，直接使用已有的缩略图
                let thumb_image = &tiled.thumbnail;
                let color_image = egui::ColorImage::from_rgba_unmultiplied(
                    [thumb_image.width as usize, thumb_image.height as usize],
                    &thumb_image.data,
                );
                let texture = ctx.load_texture(
                    format!("thumb_{:?}", path.file_name()),
                    color_image,
                    egui::TextureOptions::LINEAR,
                );

                Some(texture)
            }
        }
    }

    /// 处理后台线程返回的缩略图结果
    pub fn process_result(&mut self, result: &LoadResult, ctx: &egui::Context) {
        match result {
            LoadResult::ThumbnailReady { path, image } => {
                self.pending.remove(path);

                // 创建纹理
                let color_image = egui::ColorImage::from_rgba_unmultiplied(
                    [image.width as usize, image.height as usize],
                    &image.data,
                );
                let texture = ctx.load_texture(
                    format!("thumb_{:?}", path.file_name()),
                    color_image,
                    egui::TextureOptions::LINEAR,
                );

                debug_log!(
                    "[THUMB_CACHE] 缩略图纹理创建完成: {:?} ({}x{})",
                    path.file_name(),
                    image.width,
                    image.height
                );

                self.textures.put(path.clone(), texture);
            }
            LoadResult::ThumbnailFailed { path, error } => {
                self.pending.remove(path);
                self.failed.insert(path.clone());

                log_error!("缩略图生成失败 {:?}: {}", path.file_name(), error);
            }
            _ => {}
        }
    }

    /// 清除指定路径的缓存
    pub fn invalidate(&mut self, path: &PathBuf) {
        self.textures.pop(path);
        self.pending.remove(path);
        self.failed.remove(path);
    }

    /// 清除所有缓存
    pub fn clear(&mut self) {
        self.textures.clear();
        self.pending.clear();
        self.failed.clear();
    }

    /// 获取缓存中的纹理数量
    pub fn len(&self) -> usize {
        self.textures.len()
    }

    /// 检查缓存是否为空
    pub fn is_empty(&self) -> bool {
        self.textures.is_empty()
    }

    // 内部方法供 ThumbnailBar 使用
    pub(crate) fn contains_texture(&self, path: &PathBuf) -> bool {
        self.textures.contains(path)
    }

    pub(crate) fn is_pending(&self, path: &PathBuf) -> bool {
        self.pending.contains(path)
    }

    pub(crate) fn is_failed(&self, path: &PathBuf) -> bool {
        self.failed.contains(path)
    }

    pub(crate) fn mark_pending(&mut self, path: PathBuf) {
        self.pending.insert(path);
    }

    pub(crate) fn mark_ready(&mut self, path: &PathBuf) {
        self.pending.remove(path);
        self.failed.remove(path);
    }

    pub(crate) fn mark_failed(&mut self, path: &PathBuf) {
        self.pending.remove(path);
        self.failed.insert(path.clone());
    }

    pub(crate) fn add_texture(&mut self, path: PathBuf, texture: egui::TextureHandle) {
        self.textures.put(path, texture);
    }

    pub(crate) fn get_texture(&mut self, path: &PathBuf) -> Option<egui::TextureHandle> {
        self.textures.get(path).cloned()
    }
}

impl Default for ThumbnailCache {
    fn default() -> Self {
        Self::new()
    }
}

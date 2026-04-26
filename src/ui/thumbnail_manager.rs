/// 缩略图管理器 - 处理缩略图的生成、缓存和结果处理
use eframe::egui;
use std::cell::Cell;
use std::path::PathBuf;

use crate::core::loader::{LoadCommand, LoadResult};
use crate::core::thumbnail::ThumbnailBar;

/// 缩略图管理器
pub struct ThumbnailManager {
    /// 缩略图栏 UI 组件
    pub bar: ThumbnailBar,
}

impl Default for ThumbnailManager {
    fn default() -> Self {
        Self::new()
    }
}

impl ThumbnailManager {
    pub fn new() -> Self {
        Self {
            bar: ThumbnailBar::new(),
        }
    }

    /// 处理缩略图加载结果
    pub fn process_result(&mut self, result: &LoadResult, ctx: &egui::Context) {
        match result {
            LoadResult::ThumbnailReady { path, image } => {
                #[cfg(debug_assertions)]
                eprintln!(
                    "[THUMB] Creating texture for {:?} ({}x{}, data_size={})",
                    path.file_name(),
                    image.width,
                    image.height,
                    image.data.len()
                );

                // 验证数据
                let expected_size = (image.width * image.height * 4) as usize;
                if image.data.len() != expected_size {
                    #[cfg(debug_assertions)]
                    eprintln!(
                        "[THUMB] WARNING: Data size mismatch! Expected {}, got {}",
                        expected_size,
                        image.data.len()
                    );
                }

                // 创建纹理
                let color_image = egui::ColorImage::from_rgba_unmultiplied(
                    [image.width as usize, image.height as usize],
                    &image.data,
                );

                #[cfg(debug_assertions)]
                eprintln!("[THUMB] ColorImage created: {:?}", color_image.size);

                let texture = ctx.load_texture(
                    format!("thumb_{:?}", path.file_name()),
                    color_image,
                    egui::TextureOptions::LINEAR,
                );

                #[cfg(debug_assertions)]
                eprintln!("[THUMB] Texture created, size: {:?}", texture.size_vec2());

                // 添加到缓存
                self.bar.add_texture(path.clone(), texture);
                self.bar.mark_ready(path);

                #[cfg(debug_assertions)]
                eprintln!(
                    "[THUMB] Added to cache, total textures: {}",
                    self.bar.cache.len()
                );
            }
            LoadResult::ThumbnailFailed { path, error: _ } => {
                #[cfg(debug_assertions)]
                eprintln!("[THUMB] Failed: {:?}", path.file_name());
                self.bar.mark_failed(path);
            }
            _ => {} // 其他结果类型不处理
        }
    }

    /// 请求生成当前图片周围的缩略图
    pub fn request_surrounding_thumbnails(
        &mut self,
        current_images: &[PathBuf],
        current_index: usize,
        cmd_tx: &Option<std::sync::mpsc::Sender<LoadCommand>>,
        range: usize,
    ) {
        if current_images.is_empty() {
            return;
        }

        let start = current_index.saturating_sub(range);
        let end = (current_index + range).min(current_images.len() - 1);

        for idx in start..=end {
            if let Some(path) = current_images.get(idx)
                && self.bar.should_generate(path)
            {
                self.bar.request_generation(path, cmd_tx);
            }
        }
    }

    /// 渲染缩略图栏，返回被点击的索引（如果有）
    pub fn render(
        &mut self,
        ui: &mut egui::Ui,
        ctx: &egui::Context,
        current_images: &[PathBuf],
        current_index: usize,
        cmd_tx: &Option<std::sync::mpsc::Sender<LoadCommand>>,
    ) -> Option<usize> {
        let clicked = Cell::new(None);
        self.bar.render(
            ui,
            ctx,
            current_images,
            current_index,
            |index| {
                clicked.set(Some(index));
            },
            cmd_tx,
        );
        clicked.into_inner()
    }

    /// 切换显示/隐藏
    pub fn toggle(&mut self) {
        self.bar.toggle();
    }

    /// 检查是否可见
    pub fn is_visible(&self) -> bool {
        self.bar.visible
    }

    /// 获取缩略图缓存的可变引用（供 nav_thumbnail 使用）
    pub fn cache_mut(&mut self) -> &mut crate::core::thumbnail_cache::ThumbnailCache {
        &mut self.bar.cache
    }
}

//! 纹理复用池
//!
//! 管理 egui 纹理对象的生命周期，避免频繁分配/释放 GPU 资源

use eframe::egui;
use std::sync::atomic::{AtomicU64, Ordering};

/// 全局纹理 ID 计数器
static NEXT_TEXTURE_ID: AtomicU64 = AtomicU64::new(1000);

/// 纹理复用池
pub struct TexturePool {
    pool: Vec<egui::TextureHandle>,
    max_size: usize,
}

impl TexturePool {
    /// 创建新的纹理池
    pub fn new(max_size: usize) -> Self {
        Self {
            pool: Vec::with_capacity(max_size),
            max_size,
        }
    }

    /// 从池中获取纹理（如果池为空则创建新纹理）
    pub fn acquire(&mut self, ctx: &egui::Context) -> egui::TextureHandle {
        if let Some(texture) = self.pool.pop() {
            texture
        } else {
            // 创建新纹理（使用唯一 ID 字符串）
            let id = NEXT_TEXTURE_ID.fetch_add(1, Ordering::Relaxed);
            let texture_id = format!("texture_pool_{}", id);
            let empty_image = egui::ColorImage::from_rgba_unmultiplied([1, 1], &[0, 0, 0, 0]);
            ctx.load_texture(&texture_id, empty_image, egui::TextureOptions::LINEAR)
        }
    }

    /// 释放纹理回池中（如果池已满则丢弃）
    pub fn release(&mut self, texture: egui::TextureHandle) {
        if self.pool.len() < self.max_size {
            self.pool.push(texture);
        }
        // 超出容量则丢弃（egui 会自动管理生命周期）
    }

    /// 清空池（释放所有纹理）
    pub fn clear(&mut self) {
        self.pool.clear();
    }

    /// 获取池中当前纹理数量
    pub fn len(&self) -> usize {
        self.pool.len()
    }

    /// 检查池是否为空
    pub fn is_empty(&self) -> bool {
        self.pool.is_empty()
    }
}

use eframe::egui;
use std::path::PathBuf;
use std::sync::Arc;

use crate::app::{FastViewApp, elapsed_ms};
use crate::core::types::{CacheEntry, ImageCache};
use crate::core::{LoadCommand, LoadPriority, ZoomMode};

use crate::utils::lock_or_recover;

/// 加载图片（优先检查缓存）
pub fn load_image(
    app: &mut FastViewApp,
    path: &PathBuf,
    ctx: &egui::Context,
) -> Result<(), String> {
    // 1. 优先检查缓存
    if let Some(cached) = {
        let mut cache_guard = lock_or_recover(&app.image_cache);
        cache_guard.get(path).cloned()
    } {
        // 缓存命中,立即应用
        apply_cached_entry(app, cached, path, ctx);
        return Ok(());
    }

    // 2. 缓存未命中,异步加载
    load_image_async(app, path, ctx);
    Ok(())
}

/// 异步加载图片（使用专用后台线程）
pub fn load_image_async(app: &mut FastViewApp, path: &PathBuf, _ctx: &egui::Context) {
    debug_log!(
        "[{:.3}s] [CACHE] 请求加载: {:?}",
        elapsed_ms() as f64 / 1000.0,
        path.file_name()
    );

    // 1. 立即设置路径
    app.current_path = Some(path.clone());
    app.file_size = std::fs::metadata(path).map(|m| m.len()).unwrap_or(0);

    // 2. 清除旧纹理，避免显示上一张图片
    let old_texture = app.texture.take();
    drop(old_texture);

    // 3. 先获取图片尺寸以决定是否使用分块加载
    if let Some(ref tx) = app.cmd_tx {
        // 首先尝试创建分块图片元数据（会读取尺寸并生成缩略图）
        debug_log!(
            "[{:.3}s] [CACHE] 发送分块图片创建请求",
            elapsed_ms() as f64 / 1000.0
        );
        let _ = tx.send(LoadCommand::CreateTiledImage {
            path: path.clone(),
            priority: LoadPriority::Critical,
        });
    }
}

/// 应用缓存条目
pub fn apply_cached_entry(
    app: &mut FastViewApp,
    entry: CacheEntry,
    path: &PathBuf,
    ctx: &egui::Context,
) {
    debug_log!(
        "[{:.3}s] [CACHE] 缓存命中: {:?}",
        elapsed_ms() as f64 / 1000.0,
        path.file_name()
    );

    app.current_path = Some(path.clone());
    app.file_size = std::fs::metadata(path).map(|m| m.len()).unwrap_or(0);

    match entry {
        CacheEntry::Decoded(image) => {
            // 从解码数据创建纹理（使用唯一ID避免冲突）
            let image_size = egui::vec2(image.width as f32, image.height as f32);
            let texture_id = format!("image_{:?}", path.file_name());
            let color_image = egui::ColorImage::from_rgba_unmultiplied(
                [image.width as usize, image.height as usize],
                &image.data,
            );
            let texture = ctx.load_texture(&texture_id, color_image, egui::TextureOptions::LINEAR);

            debug_log!(
                "[{:.3}s] [CACHE] 缓存纹理创建完成: {}",
                elapsed_ms() as f64 / 1000.0,
                texture_id
            );

            // 显示图片
            app.texture = Some(texture);
            app.image_size = image_size;
            app.tiled_image = None; // 清除分块图片
            app.tile_textures.clear();

            app.zoom_mode = ZoomMode::Fit;
            app.zoom = 1.0;
            app.rotation = 0.0;
            app.image_offset = egui::Vec2::ZERO;

            // 将解码后的数据重新放回缓存（因为 entry 被移动了）
            {
                let mut cache_guard = lock_or_recover(&app.image_cache);
                cache_guard.put(path.clone(), CacheEntry::Decoded(image));
            }
        }
        CacheEntry::TiledMeta(tiled) => {
            // 处理分块图片
            let image_size = egui::vec2(tiled.width as f32, tiled.height as f32);

            // 创建缩略图文理作为背景
            let thumb_texture_id = format!("thumb_{:?}", path.file_name());
            let thumb_color_image = egui::ColorImage::from_rgba_unmultiplied(
                [
                    tiled.thumbnail.width as usize,
                    tiled.thumbnail.height as usize,
                ],
                &tiled.thumbnail.data,
            );
            let thumb_texture = ctx.load_texture(
                &thumb_texture_id,
                thumb_color_image,
                egui::TextureOptions::LINEAR,
            );

            // 设置缩略图为当前纹理
            app.texture = Some(thumb_texture);
            app.image_size = image_size;
            app.tiled_image = Some(tiled.clone());
            app.tile_textures.clear();

            app.zoom_mode = ZoomMode::Fit;
            app.zoom = 1.0;
            app.rotation = 0.0;
            app.image_offset = egui::Vec2::ZERO;

            // 将分块图片数据重新放回缓存
            {
                let mut cache_guard = lock_or_recover(&app.image_cache);
                cache_guard.put(path.clone(), CacheEntry::TiledMeta(tiled));
            }

            // 请求加载可见区域的块
            crate::operation::tile_renderer::request_visible_tiles(app, ctx);
        }
    }

    // 更新目录列表
    crate::operation::navigation::update_directory_list(app, path);

    // 激进的内存清理：如果缓存超过5张，移除最旧的
    if let Ok(mut cache) = app.image_cache.lock() {
        while cache.len() > 5 {
            cache.pop_lru();
        }
    }
}

/// 内存检查和淘汰（如果超出限制则移除最旧条目）
pub fn evict_if_needed(
    _app: &FastViewApp,
    cache: &mut lru::LruCache<PathBuf, CacheEntry>,
    new_entry_bytes: usize,
) {
    let max_memory = 150 * 1024 * 1024; // 150MB (提高限制以容纳更多预加载图片)

    loop {
        let current_memory: usize = cache
            .iter()
            .map(|(_, entry)| entry.estimated_memory_bytes())
            .sum();

        // 如果超出限制，淘汰最旧的条目
        if current_memory + new_entry_bytes > max_memory {
            if let Some((oldest_path, oldest_entry)) = cache.pop_lru() {
                let freed_bytes = oldest_entry.estimated_memory_bytes();
                debug_log!(
                    "[EVICT] Removed {:?} (freed {} MB)",
                    oldest_path.file_name(),
                    freed_bytes as f64 / 1024.0 / 1024.0
                );
            } else {
                break; // 缓存已空
            }
        } else {
            break; // 内存充足
        }
    }
}

/// 获取或创建导航缩略图纹理
pub fn get_or_create_nav_thumbnail(
    app: &mut FastViewApp,
    ui: &mut egui::Ui,
) -> Option<egui::TextureHandle> {
    // 检查是否已有缓存的缩略图纹理
    if let Some((cached_path, texture)) = &app.nav_thumbnail
        && let Some(ref current_path) = app.current_path
        && cached_path == current_path
    {
        return Some(texture.clone());
    }

    // 没有缓存或路径变化，从图片缓存中获取数据生成缩略图
    if let Some(ref path) = app.current_path {
        // 尝试从缓存中获取图片数据
        if let Some(cached) = {
            let mut cache_guard = lock_or_recover(&app.image_cache);
            cache_guard.get(path).cloned()
        } {
            match cached {
                CacheEntry::Decoded(image) => {
                    use image::imageops::thumbnail;

                    // 计算缩略图尺寸（保持宽高比）
                    let max_thumb_size = 120;
                    let scale =
                        (max_thumb_size as f32 / image.width.max(image.height) as f32).min(1.0);
                    let thumb_w = (image.width as f32 * scale) as u32;
                    let thumb_h = (image.height as f32 * scale) as u32;

                    // 从原始像素数据创建 RgbaImage
                    if let Some(img) =
                        image::RgbaImage::from_raw(image.width, image.height, image.data.clone())
                    {
                        // 生成缩略图（非常快，因为只是缩放操作）
                        let thumb_img = thumbnail(&img, thumb_w, thumb_h);

                        // 创建纹理
                        let color_image = egui::ColorImage::from_rgba_unmultiplied(
                            [thumb_w as usize, thumb_h as usize],
                            thumb_img.as_raw(),
                        );
                        let texture = ui.ctx().load_texture(
                            "nav_thumbnail",
                            color_image,
                            egui::TextureOptions::LINEAR,
                        );

                        // 缓存纹理
                        app.nav_thumbnail = Some((path.clone(), texture.clone()));
                        return Some(texture);
                    }
                }
                CacheEntry::TiledMeta(tiled) => {
                    // 对于分块图片，直接使用缩略图
                    let thumb_image = &tiled.thumbnail;
                    let color_image = egui::ColorImage::from_rgba_unmultiplied(
                        [thumb_image.width as usize, thumb_image.height as usize],
                        &thumb_image.data,
                    );
                    let texture = ui.ctx().load_texture(
                        "nav_thumbnail",
                        color_image,
                        egui::TextureOptions::LINEAR,
                    );

                    // 缓存纹理
                    app.nav_thumbnail = Some((path.clone(), texture.clone()));
                    return Some(texture);
                }
            }
        }
    }
    None
}

/// 尝试从缓存快速生成缩略图（供后台线程调用前的预检查）
pub fn try_generate_thumbnail_from_cache(
    image_cache: &ImageCache,
    path: &PathBuf,
    size: u32,
) -> Option<Arc<crate::core::types::DecodedImage>> {
    let mut cache_guard = lock_or_recover(image_cache);
    if let Some(cached) = cache_guard.get(path) {
        match cached {
            CacheEntry::Decoded(image) => {
                use image::imageops::thumbnail;
                
                // 从缓存数据快速缩放
                if let Some(img) = image::RgbaImage::from_raw(
                    image.width, 
                    image.height, 
                    image.data.clone()
                ) {
                    let thumb_img = thumbnail(&img, size, size);
                    let width = thumb_img.width();
                    let height = thumb_img.height();
                    let data = thumb_img.into_raw();
                    return Some(Arc::new(crate::core::types::DecodedImage {
                        data,
                        width,
                        height,
                    }));
                }
            }
            CacheEntry::TiledMeta(tiled) => {
                // 直接返回已有的缩略图
                return Some(tiled.thumbnail.clone());
            }
        }
    }
    None
}

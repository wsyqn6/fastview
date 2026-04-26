use eframe::egui;
use std::path::PathBuf;
use std::sync::Arc;

use crate::app::{DirectoryCache, FastViewApp, elapsed_ms};

use crate::core::{CacheEntry, LoadResult};
use crate::utils::lock_or_recover;

/// 处理异步加载结果
pub fn handle_load_results(app: &mut FastViewApp, ui: &mut egui::Ui) -> (bool, Option<PathBuf>) {
    let mut needs_prefetch = false;
    let mut path_for_dir_update: Option<PathBuf> = None;
    let mut results_processed = 0;

    if let Some(rx) = &app.result_rx {
        // 收集所有待处理的结果
        let mut pending_results = Vec::new();
        let recv_start = std::time::Instant::now();
        while let Ok(result) = rx.try_recv() {
            pending_results.push(result);
        }
        let recv_duration = recv_start.elapsed();
        if !pending_results.is_empty() {
            debug_log!(
                "[{:.3}s] [APP] 收集了 {} 个结果 (耗时 {}ms)",
                elapsed_ms() as f64 / 1000.0,
                pending_results.len(),
                recv_duration.as_millis()
            );
        }

        // 按优先级排序：当前图片优先，其他按接收顺序
        if !pending_results.is_empty() {
            let current_path = app.current_path.clone();
            pending_results.sort_by(|a, b| {
                let a_is_current = match a {
                    LoadResult::ImageReady { path, .. } => current_path.as_ref() == Some(path),
                    _ => false,
                };
                let b_is_current = match b {
                    LoadResult::ImageReady { path, .. } => current_path.as_ref() == Some(path),
                    _ => false,
                };

                // 当前图片排前面
                match (a_is_current, b_is_current) {
                    (true, false) => std::cmp::Ordering::Less,
                    (false, true) => std::cmp::Ordering::Greater,
                    _ => std::cmp::Ordering::Equal,
                }
            });
        }

        // 处理排序后的结果
        for result in pending_results {
            results_processed += 1;
            match result {
                LoadResult::ImageReady { path, image } => {
                    handle_image_ready(app, ui, path.clone(), image, &mut needs_prefetch);
                    path_for_dir_update = Some(path);
                }
                LoadResult::DirectoryScanned { images } => {
                    handle_directory_scanned(app, ui, images, &mut needs_prefetch);
                }
                LoadResult::TiledImageMetaReady { path, tiled_image } => {
                    handle_tiled_meta_ready(
                        app,
                        ui,
                        path.clone(),
                        tiled_image,
                        &mut needs_prefetch,
                    );
                    path_for_dir_update = Some(path);
                }
                LoadResult::TileReady {
                    path,
                    col,
                    row,
                    data,
                    width,
                    height,
                    ..
                } => {
                    handle_tile_ready(app, ui, path, col, row, data, width, height);
                }
                LoadResult::Error { path, error } => {
                    handle_load_error(app, ui, path, error);
                }
                LoadResult::ThumbnailReady { path, image } => {
                    #[cfg(debug_assertions)]
                    eprintln!("[APP] Received thumbnail result for {:?}", path.file_name());

                    let result_ref = LoadResult::ThumbnailReady {
                        path: path.clone(),
                        image: image.clone(),
                    };
                    app.thumbnail_mgr.process_result(&result_ref, ui.ctx());
                }
                LoadResult::ThumbnailFailed { path, error } => {
                    let result_ref = LoadResult::ThumbnailFailed {
                        path: path.clone(),
                        error: error.clone(),
                    };
                    app.thumbnail_mgr.process_result(&result_ref, ui.ctx());
                }
            }
        }
    }

    if results_processed > 0 {
        debug_log!(
            "[{:.3}s] [APP] 处理了 {} 个结果",
            elapsed_ms() as f64 / 1000.0,
            results_processed
        );
    }

    (needs_prefetch, path_for_dir_update)
}

/// 处理图片加载完成
#[allow(unused_variables)]
fn handle_image_ready(
    app: &mut FastViewApp,
    ui: &mut egui::Ui,
    path: PathBuf,
    image: Arc<crate::core::types::DecodedImage>,
    needs_prefetch: &mut bool,
) {
    debug_log!(
        "[{:.3}s] [APP] 收到图片: {:?} ({}x{})",
        elapsed_ms() as f64 / 1000.0,
        path.file_name(),
        image.width,
        image.height
    );

    let is_current = app.current_path.as_ref() == Some(&path);

    // 只有当前图片才立即创建纹理
    if is_current {
        // 创建纹理（使用唯一ID避免冲突）
        let image_size = egui::vec2(image.width as f32, image.height as f32);
        let texture_id = format!("image_{:?}", path.file_name());

        // 创建主纹理
        let color_image = egui::ColorImage::from_rgba_unmultiplied(
            [image.width as usize, image.height as usize],
            &image.data,
        );
        let texture = ui
            .ctx()
            .load_texture(&texture_id, color_image, egui::TextureOptions::LINEAR);

        debug_log!(
            "[{:.3}s] [APP] 纹理创建完成: {}",
            elapsed_ms() as f64 / 1000.0,
            texture_id
        );

        // 在设置新纹理前，显式清除旧纹理以释放内存
        let old_texture = app.texture.take();
        drop(old_texture); // 立即释放

        // 更新纹理和尺寸
        app.texture = Some(texture);
        app.image_size = image_size;

        // 重置缩放模式
        debug_log!("[{:.3}s] [APP] 重置缩放模式", elapsed_ms() as f64 / 1000.0);
        app.zoom_mode = crate::core::ZoomMode::Fit;
        app.zoom = 1.0;
        app.rotation = 0.0;
        app.image_offset = egui::Vec2::ZERO;

        // 将解码后的数据放入缓存（供导航缩略图使用）
        {
            let mut cache_guard = lock_or_recover(&app.image_cache);
            let memory_bytes = (image.width * image.height * 4) as usize;
            app.evict_if_needed(&mut cache_guard, memory_bytes);
            cache_guard.put(path.clone(), CacheEntry::Decoded(image));
        }

        // 注意：不在这里触发预加载，等待目录扫描完成后再触发
        // 这样可以确保 current_images 已经有数据

        ui.ctx().request_repaint();
    } else {
        // 预加载的图片：只存入缓存，不创建纹理
        debug_log!(
            "[{:.3}s] [APP] 缓存预加载图片: {:?}",
            elapsed_ms() as f64 / 1000.0,
            path.file_name()
        );

        let mut cache_guard = lock_or_recover(&app.image_cache);
        let memory_bytes = (image.width * image.height * 4) as usize;

        // 内存检查和淘汰
        app.evict_if_needed(&mut cache_guard, memory_bytes);

        cache_guard.put(path, CacheEntry::Decoded(image));
    }
}

/// 处理目录扫描完成
fn handle_directory_scanned(
    app: &mut FastViewApp,
    ui: &mut egui::Ui,
    images: Vec<PathBuf>,
    needs_prefetch: &mut bool,
) {
    debug_log!(
        "[{:.3}s] [APP] 目录扫描完成: {} 张图片",
        elapsed_ms() as f64 / 1000.0,
        images.len()
    );

    if !images.is_empty() {
        // 更新目录缓存
        app.dir_cache = Some(DirectoryCache {
            images: images.clone(),
        });

        // 如果当前有路径，找到它在列表中的位置
        if let Some(ref current_path) = app.current_path {
            if let Some(pos) = images.iter().position(|p| p == current_path) {
                debug_log!(
                    "[{:.3}s] [APP] 找到当前图片位置: {}",
                    elapsed_ms() as f64 / 1000.0,
                    pos
                );
                app.current_images = images;
                app.current_index = pos;

                // 目录扫描完成后，触发预加载（此时 current_images 已有数据）
                *needs_prefetch = true;
                debug_log!(
                    "[{:.3}s] [EVENTS] 设置 needs_prefetch=true (目录扫描完成)",
                    elapsed_ms() as f64 / 1000.0
                );

                // 注意：不需要手动请求缩略图，render() 会自动请求可见范围内的缩略图
            } else {
                debug_log!(
                    "[{:.3}s] [APP] 警告：当前图片不在扫描结果中",
                    elapsed_ms() as f64 / 1000.0
                );
            }
        } else {
            debug_log!(
                "[{:.3}s] [APP] 警告：current_path为空",
                elapsed_ms() as f64 / 1000.0
            );
        }

        ui.ctx().request_repaint();
    }
}

/// 处理分块图片元数据就绪
#[allow(unused_variables)]
fn handle_tiled_meta_ready(
    app: &mut FastViewApp,
    ui: &mut egui::Ui,
    path: PathBuf,
    tiled_image: Arc<crate::core::types::TiledImage>,
    needs_prefetch: &mut bool,
) {
    debug_log!(
        "[{:.3}s] [APP] 收到分块图片元数据: {:?} ({}x{})",
        elapsed_ms() as f64 / 1000.0,
        path.file_name(),
        tiled_image.width,
        tiled_image.height
    );

    let is_current = app.current_path.as_ref() == Some(&path);

    if is_current {
        // 将分块图片元数据存入缓存
        {
            let mut cache_guard = lock_or_recover(&app.image_cache);
            let memory_bytes =
                (tiled_image.thumbnail.width * tiled_image.thumbnail.height * 4) as usize;
            app.evict_if_needed(&mut cache_guard, memory_bytes);
            cache_guard.put(path.clone(), CacheEntry::TiledMeta(tiled_image.clone()));
        }

        // 应用缓存条目（会创建缩略图纹理并请求加载可见块）
        app.apply_cached_entry(CacheEntry::TiledMeta(tiled_image), &path, ui.ctx());

        // 注意：不在这里触发预加载，等待目录扫描完成后再触发

        ui.ctx().request_repaint();
    }
}

/// 处理块加载完成
fn handle_tile_ready(
    app: &mut FastViewApp,
    ui: &mut egui::Ui,
    path: PathBuf,
    col: u32,
    row: u32,
    data: Vec<u8>,
    width: u32,
    height: u32,
) {
    debug_log!(
        "[{:.3}s] [APP] 收到块: {:?} ({},{}) - {}x{}",
        elapsed_ms() as f64 / 1000.0,
        path.file_name(),
        col,
        row,
        width,
        height
    );

    let is_current = app.current_path.as_ref() == Some(&path);

    if is_current {
        // 创建块纹理
        let texture_id = format!("tile_{:?}_{}_{}", path.file_name(), col, row);
        let color_image =
            egui::ColorImage::from_rgba_unmultiplied([width as usize, height as usize], &data);
        let texture = ui
            .ctx()
            .load_texture(&texture_id, color_image, egui::TextureOptions::LINEAR);

        // 存储纹理
        app.tile_textures.insert((col, row), texture);

        ui.ctx().request_repaint();
    }
}

/// 处理加载错误
fn handle_load_error(app: &mut FastViewApp, ui: &mut egui::Ui, path: PathBuf, error: String) {
    debug_log!(
        "[{:.3}s] [APP] 加载失败: {:?}, error={}",
        elapsed_ms() as f64 / 1000.0,
        path.file_name(),
        &error
    );

    // 如果是当前图片，显示错误
    if app.current_path.as_ref() == Some(&path) {
        let filename = path
            .file_name()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_default();
        app.load_error = Some(format!("Failed to load {}: {}", filename, error));
        ui.ctx().request_repaint();
    }
}

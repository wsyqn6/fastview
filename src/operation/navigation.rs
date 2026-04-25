use eframe::egui;
use std::path::PathBuf;

use crate::app::{FastViewApp, elapsed_ms};
use crate::core::{LoadCommand, LoadPriority};

use crate::utils::lock_or_recover;

/// 切换到上一张图片
pub fn prev_image(app: &mut FastViewApp, ctx: &egui::Context) {
    // 只有一张图片或没有图片时，不执行切换
    if app.current_images.len() <= 1 {
        return;
    }

    debug_log!(
        "[{:.3}s] [NAV] 上一张: current_images={}, current_index={}",
        elapsed_ms() as f64 / 1000.0,
        app.current_images.len(),
        app.current_index
    );

    // 计算新索引
    let new_index = if app.current_index > 0 {
        app.current_index - 1
    } else {
        app.current_images.len() - 1
    };

    // 如果索引没变化，不执行切换
    if new_index == app.current_index {
        return;
    }

    app.current_index = new_index;
    let path = app.current_images[app.current_index].clone();
    debug_log!(
        "[{:.3}s] [NAV] 切换到: {:?}",
        elapsed_ms() as f64 / 1000.0,
        path.file_name()
    );

    // 调用缓存管理器加载图片
    crate::operation::cache_manager::load_image(app, &path, ctx).ok();

    // 请求生成周围的缩略图
    let cmd_tx = app.cmd_tx.clone();
    app.thumbnail_mgr.request_surrounding_thumbnails(
        &app.current_images,
        app.current_index,
        &cmd_tx,
        5, // 前后各5张
    );
}

/// 切换到下一张图片
pub fn next_image(app: &mut FastViewApp, ctx: &egui::Context) {
    // 只有一张图片或没有图片时，不执行切换
    if app.current_images.len() <= 1 {
        return;
    }

    debug_log!(
        "[{:.3}s] [NAV] 下一张: current_images={}, current_index={}",
        elapsed_ms() as f64 / 1000.0,
        app.current_images.len(),
        app.current_index
    );

    // 计算新索引
    let new_index = if app.current_index < app.current_images.len() - 1 {
        app.current_index + 1
    } else {
        0
    };

    // 如果索引没变化，不执行切换
    if new_index == app.current_index {
        return;
    }

    app.current_index = new_index;
    let path = app.current_images[app.current_index].clone();
    debug_log!(
        "[{:.3}s] [NAV] 切换到: {:?}",
        elapsed_ms() as f64 / 1000.0,
        path.file_name()
    );

    // 调用缓存管理器加载图片
    crate::operation::cache_manager::load_image(app, &path, ctx).ok();

    // 请求生成周围的缩略图
    let cmd_tx = app.cmd_tx.clone();
    app.thumbnail_mgr.request_surrounding_thumbnails(
        &app.current_images,
        app.current_index,
        &cmd_tx,
        5, // 前后各5张
    );
}

/// 更新目录列表（异步扫描，仅首次加载时执行）
pub fn update_directory_list(app: &mut FastViewApp, path: &PathBuf) {
    // 检查是否需要重新扫描目录
    let need_rescan = if let Some(dir_cache) = &app.dir_cache {
        // 如果缓存存在，检查当前图片是否在缓存的目录中
        if let Some(pos) = dir_cache.images.iter().position(|p| p == path) {
            debug_log!(
                "[{:.3}s] [NAV] 使用目录缓存: {} 张图片",
                elapsed_ms() as f64 / 1000.0,
                dir_cache.images.len()
            );

            app.current_images = dir_cache.images.clone();
            app.current_index = pos;
            debug_log!(
                "[{:.3}s] [NAV] 从缓存恢复位置: {}",
                elapsed_ms() as f64 / 1000.0,
                pos
            );
            return; // 缓存命中，直接返回
        } else {
            // 当前图片不在缓存的目录中，说明切换到了新目录
            debug_log!(
                "[{:.3}s] [NAV] 检测到目录变化，清除旧缓存",
                elapsed_ms() as f64 / 1000.0
            );
            true // 需要重新扫描
        }
    } else {
        true // 首次打开，需要扫描
    };

    // 需要扫描目录
    if need_rescan && let Some(parent) = path.parent() {
        debug_log!(
            "[{:.3}s] [NAV] 触发目录扫描: {:?}",
            elapsed_ms() as f64 / 1000.0,
            parent
        );

        // 清除旧缓存
        app.dir_cache = None;

        // 发送扫描命令到后台线程
        if let Some(ref tx) = app.cmd_tx {
            let _ = tx.send(LoadCommand::ScanDirectory {
                dir_path: parent.to_path_buf(),
            });
        }
        // 注意：此时不设置 current_images，等待扫描结果返回后再更新
    }
}

/// 预加载相邻图片（智能方向性预加载）
pub fn preload_adjacent_images(app: &mut FastViewApp) {
    if app.current_images.is_empty() || app.current_index >= app.current_images.len() {
        return;
    }

    // 检查当前图片是否已在缓存中（已加载完成）
    let current_loaded = {
        if let Some(ref path) = app.current_path {
            let cache_guard = lock_or_recover(&app.image_cache);
            cache_guard.contains(path)
        } else {
            false
        }
    };

    // 只有当前图片加载完成后才预加载
    if !current_loaded {
        debug_log!(
            "[{:.3}s] [NAV] 跳过预加载：当前图片尚未加载完成",
            elapsed_ms() as f64 / 1000.0
        );
        return;
    }

    let mut to_prefetch = Vec::new();

    // 策略：优先预加载下一张，其次是上两张，避免加载已看过的
    let next_idx = app.current_index + 1;
    let next2_idx = app.current_index + 2;
    let next3_idx = app.current_index + 3;

    // 检查缓存，只预加载未缓存的图片
    let cache_guard = lock_or_recover(&app.image_cache);

    // 1. 预加载下一张（最高优先级）
    if next_idx < app.current_images.len() {
        let path = &app.current_images[next_idx];
        if !cache_guard.contains(path) {
            to_prefetch.push(path.clone());
        } else {
            debug_log!(
                "[{:.3}s] [NAV] 跳过预加载（已缓存）: {:?}",
                elapsed_ms() as f64 / 1000.0,
                path.file_name()
            );
        }
    }

    // 2. 预加载下两张（次高优先级）
    if next2_idx < app.current_images.len() {
        let path = &app.current_images[next2_idx];
        if !cache_guard.contains(path) {
            to_prefetch.push(path.clone());
        } else {
            debug_log!(
                "[{:.3}s] [NAV] 跳过预加载（已缓存）: {:?}",
                elapsed_ms() as f64 / 1000.0,
                path.file_name()
            );
        }
    }

    // 3. 预加载下三张（可选）
    if next3_idx < app.current_images.len() {
        let path = &app.current_images[next3_idx];
        if !cache_guard.contains(path) {
            to_prefetch.push(path.clone());
        } else {
            debug_log!(
                "[{:.3}s] [NAV] 跳过预加载（已缓存）: {:?}",
                elapsed_ms() as f64 / 1000.0,
                path.file_name()
            );
        }
    }

    drop(cache_guard); // 释放锁

    // 发送预加载命令到后台线程
    if !to_prefetch.is_empty() {
        debug_log!(
            "[{:.3}s] [NAV] 预加载 {} 张图片",
            elapsed_ms() as f64 / 1000.0,
            to_prefetch.len()
        );

        if let Some(ref tx) = app.cmd_tx {
            let _ = tx.send(LoadCommand::Prefetch {
                paths: to_prefetch,
                priority: LoadPriority::Low,
            });
        }
    } else {
        debug_log!(
            "[{:.3}s] [NAV] 无需预加载：所有相邻图片已缓存",
            elapsed_ms() as f64 / 1000.0
        );
    }
}

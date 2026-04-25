use eframe::egui;

use crate::app::{FastViewApp, elapsed_ms};
use crate::core::{LoadCommand, LoadPriority, ZoomMode};

/// 请求加载可见区域的块
pub fn request_visible_tiles(app: &mut FastViewApp, ctx: &egui::Context) {
    if let Some(ref tiled) = app.tiled_image
        && let Some(ref path) = app.current_path
    {
        // 计算当前可见区域对应的块
        let available = ctx.content_rect().size();

        // 根据当前缩放和偏移计算可见区域
        let mut size = egui::vec2(tiled.width as f32, tiled.height as f32);
        match app.zoom_mode {
            ZoomMode::Fit => {
                let scale_x = available.x / size.x;
                let scale_y = available.y / size.y;
                let scale = scale_x.min(scale_y);
                size *= scale;
            }
            ZoomMode::Fill => {
                let scale = (available.x / size.x).max(available.y / size.y);
                size *= scale;
            }
            ZoomMode::Original => {
                // 原始尺寸
            }
            ZoomMode::Custom => {
                size *= app.zoom;
            }
        }

        // 计算可见区域在原始图片中的位置
        // image_rect 的中心是 available/2 + image_offset
        // 所以 image_rect.min = (available - size) / 2 + image_offset
        let rect_min_x = (available.x - size.x) / 2.0 + app.image_offset.x;
        let rect_min_y = (available.y - size.y) / 2.0 + app.image_offset.y;

        // 可见区域占整个图片的比例
        let view_left_ratio = (-rect_min_x / size.x).clamp(0.0, 1.0);
        let view_top_ratio = (-rect_min_y / size.y).clamp(0.0, 1.0);
        let view_right_ratio = ((-rect_min_x + available.x) / size.x).clamp(0.0, 1.0);
        let view_bottom_ratio = ((-rect_min_y + available.y) / size.y).clamp(0.0, 1.0);

        // 转换到原始图片坐标
        let view_left = view_left_ratio * tiled.width as f32;
        let view_top = view_top_ratio * tiled.height as f32;
        let view_right = view_right_ratio * tiled.width as f32;
        let view_bottom = view_bottom_ratio * tiled.height as f32;

        // 计算需要加载的块范围
        let start_col = (view_left / tiled.tile_size as f32) as u32;
        let end_col = ((view_right / tiled.tile_size as f32).ceil() as u32).min(tiled.cols);
        let start_row = (view_top / tiled.tile_size as f32) as u32;
        let end_row = ((view_bottom / tiled.tile_size as f32).ceil() as u32).min(tiled.rows);

        // 请求加载这些块
        if let Some(ref tx) = app.cmd_tx {
            for row in start_row..end_row {
                for col in start_col..end_col {
                    // 检查是否已经加载
                    if !app.tile_textures.contains_key(&(col, row)) {
                        let _ = tx.send(LoadCommand::LoadTile {
                            path: path.clone(),
                            col,
                            row,
                            priority: LoadPriority::High,
                        });
                    }
                }
            }
        }
    }
}

/// 渲染已加载的块
pub fn render_tiles(
    app: &FastViewApp,
    ui: &mut egui::Ui,
    image_rect: egui::Rect,
    display_size: egui::Vec2,
    original_size: egui::Vec2,
    _available: egui::Vec2,
    rotation: f32,
) {
    if let Some(ref tiled) = app.tiled_image {
        // 只在第一次渲染或有新块时输出日志
        static mut LAST_TILE_COUNT: usize = 0;
        unsafe {
            if app.tile_textures.len() != LAST_TILE_COUNT {
                debug_log!(
                    "[{:.3}s] [RENDER] 渲染 {} 个块, image_rect={:?}, display_size={:?}",
                    elapsed_ms() as f64 / 1000.0,
                    app.tile_textures.len(),
                    image_rect,
                    display_size
                );
                LAST_TILE_COUNT = app.tile_textures.len();
            }
        }

        // 遍历所有已加载的块纹理并渲染
        for ((col, row), texture) in &app.tile_textures {
            // 找到对应的块信息
            if let Some(tile_info) = tiled.tiles.iter().find(|t| t.col == *col && t.row == *row) {
                // 计算缩放比例：显示尺寸 / 原始图片尺寸
                let scale_x = display_size.x / original_size.x;
                let scale_y = display_size.y / original_size.y;

                // 块在原始图片中的位置，转换到显示坐标
                let tile_display_x = tile_info.x as f32 * scale_x;
                let tile_display_y = tile_info.y as f32 * scale_y;
                let tile_display_w = tile_info.width as f32 * scale_x;
                let tile_display_h = tile_info.height as f32 * scale_y;

                // image_rect 已经是考虑了 offset 后的矩形
                // 所以块的绝对位置应该是 image_rect 的左上角 + 块在图片内的相对位置
                let tile_rect = egui::Rect::from_min_size(
                    image_rect.min + egui::vec2(tile_display_x, tile_display_y),
                    egui::vec2(tile_display_w, tile_display_h),
                );

                // 渲染块纹理，应用相同的旋转
                let mut tile_image =
                    egui::Image::new((texture.id(), egui::vec2(tile_display_w, tile_display_h)));
                if rotation != 0.0 {
                    let angle_rad = rotation * std::f32::consts::TAU / 360.0;
                    tile_image = tile_image.rotate(angle_rad, egui::Vec2::splat(0.5));
                }
                ui.put(tile_rect, tile_image);
            }
        }
    }
}

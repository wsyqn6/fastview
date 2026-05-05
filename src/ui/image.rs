use eframe::egui;

use crate::app::FastViewApp;
use crate::core::ZoomMode;

/// 渲染中央图片显示区域
pub fn render_image_area(app: &mut FastViewApp, ui: &mut egui::Ui) {
    // CentralPanel：深色背景突出图片
    egui::CentralPanel::default()
        .frame(egui::Frame::NONE.fill(egui::Color32::from_rgba_unmultiplied(30, 30, 30, 255)))
        .show_inside(ui, |ui| {
            // 处理拖拽文件
            handle_dragged_files(app, ui);

            let available = ui.available_size();

            // 先检查是否有纹理，避免借用冲突
            let has_texture = app.texture.is_some();

            if has_texture {
                render_main_image(app, ui, available);
            } else if app.tiled_image.is_some() {
                // 分块图片但没有主纹理时，仍然需要处理导航
                render_tiled_placeholder(app, ui, available);
            } else {
                // 没有图片时显示提示
                render_empty_state(app, ui, available);
            }
        });
}

/// 处理拖拽的文件
fn handle_dragged_files(app: &mut FastViewApp, ui: &mut egui::Ui) {
    if ui.ctx().input(|i| !i.raw.dropped_files.is_empty()) {
        let files = ui.ctx().input(|i| i.raw.dropped_files.clone());
        for file in files {
            if let Some(path) = file.path
                && path.is_file()
                && let Some(ext) = path.extension()
                && let Some(ext_str) = ext.to_str()
            {
                let ext_lower = ext_str.to_lowercase();
                if [
                    "jpg", "jpeg", "png", "gif", "webp", "bmp", "tiff", "tif", "ico", "avif",
                ]
                .contains(&ext_lower.as_str())
                {
                    app.load_image(&path, ui.ctx()).ok();
                    break;
                }
            }
        }
    }
}

/// 渲染主图片
fn render_main_image(app: &mut FastViewApp, ui: &mut egui::Ui, available: egui::Vec2) {
    // 在函数内部获取 texture，避免借用冲突
    let Some(ref texture) = app.texture else {
        return;
    };

    let mut size = app.image_size;

    // 计算缩放
    match app.zoom_mode {
        ZoomMode::Fit => {
            let scale_x = available.x / size.x;
            let scale_y = available.y / size.y;
            let scale = scale_x.min(scale_y);
            size *= scale;
            app.current_scale = scale;
        }
        ZoomMode::Fill => {
            let scale = (available.x / size.x).max(available.y / size.y);
            size *= scale;
            app.current_scale = scale;
        }
        ZoomMode::Original => {
            size = app.image_size;
            app.current_scale = 1.0;
        }
        ZoomMode::Custom => {
            size *= app.zoom;
            app.current_scale = app.zoom;
        }
    }

    // 计算图片位置
    let center = egui::Pos2::new(available.x / 2.0, available.y / 2.0);
    let rect = egui::Rect::from_center_size(center + app.image_offset, size);
    let ui_offset = ui.cursor().min;
    let absolute_rect = rect.translate(ui_offset.to_vec2());

    // 渲染图片
    let mut image = egui::Image::new((texture.id(), size));
    if app.rotation != 0.0 {
        let angle_rad = app.rotation * std::f32::consts::TAU / 360.0;
        image = image.rotate(angle_rad, egui::Vec2::splat(0.5));
    }

    // 如果有过渡动画，绘制旧图（渐隐）和新图（渐显）
    if let Some(ref prev_tex) = app.previous_texture {
        let alpha = app.transition_alpha.clamp(0.0, 1.0);

        // 绘制旧图（底层，渐隐）
        let old_alpha = (1.0 - alpha).clamp(0.0, 1.0);
        if old_alpha > 0.01 {
            // 避免绘制几乎不可见的图像
            let mut prev_image = egui::Image::new((prev_tex.id(), size));
            if app.rotation != 0.0 {
                let angle_rad = app.rotation * std::f32::consts::TAU / 360.0;
                prev_image = prev_image.rotate(angle_rad, egui::Vec2::splat(0.5));
            }
            let tint_old =
                egui::Color32::from_rgba_unmultiplied(255, 255, 255, (old_alpha * 255.0) as u8);
            prev_image = prev_image.tint(tint_old);
            ui.put(absolute_rect, prev_image);
        }

        // 绘制新图（顶层，渐显）
        if alpha < 1.0 {
            let tint_new =
                egui::Color32::from_rgba_unmultiplied(255, 255, 255, (alpha * 255.0) as u8);
            image = image.tint(tint_new);
        }
    }

    ui.put(absolute_rect, image);

    // 动画结束后清理旧纹理
    if app.transition_alpha >= 1.0 && app.previous_texture.is_some() {
        app.previous_texture = None;
    }

    // 如果是分块图片，渲染已加载的块
    if let Some(ref tiled) = app.tiled_image {
        let original_size = egui::vec2(tiled.width as f32, tiled.height as f32);
        app.render_tiles(
            ui,
            absolute_rect,
            size,
            original_size,
            available,
            app.rotation,
        );
    }

    // 检查是否需要显示导航缩略图
    let need_navigation = size.x > available.x || size.y > available.y;

    // 拖动模式处理
    if app.is_drag_mode && need_navigation {
        handle_drag_mode(app, ui);
    }

    // 显示缩略图导航
    if need_navigation && let Some(thumb_tex) = app.get_or_create_nav_thumbnail(ui) {
        render_thumbnail_navigator(app, ui, thumb_tex, size, available);
    }
}

/// 处理拖动模式
fn handle_drag_mode(app: &mut FastViewApp, ui: &mut egui::Ui) {
    let is_pressed = ui
        .ctx()
        .input(|i| i.pointer.button_down(egui::PointerButton::Primary));

    let cursor_icon = if is_pressed {
        egui::CursorIcon::Grabbing
    } else {
        egui::CursorIcon::Grab
    };

    ui.ctx().set_cursor_icon(cursor_icon);

    if is_pressed {
        if !app.pointer_down {
            app.pointer_down = true;
        } else {
            let delta = ui.ctx().input(|i| i.pointer.delta());
            app.image_offset += delta;
        }
    } else {
        app.pointer_down = false;
    }
}

/// 渲染缩略图导航器
fn render_thumbnail_navigator(
    app: &FastViewApp,
    ui: &mut egui::Ui,
    thumb_tex: egui::TextureHandle,
    display_size: egui::Vec2,
    available: egui::Vec2,
) {
    let img_ratio = app.image_size.x / app.image_size.y;

    // 缩略图尺寸：保持宽高比，统一为100px（与底部缩略图一致）
    let max_thumb_size = 100.0;
    let (thumb_w, thumb_h) = if img_ratio > 1.0 {
        (max_thumb_size, max_thumb_size / img_ratio)
    } else {
        (max_thumb_size * img_ratio, max_thumb_size)
    };
    let thumb_size = egui::vec2(thumb_w, thumb_h);

    // 使用 Area 创建悬浮缩略图
    egui::Area::new(egui::Id::new("thumbnail_navigator"))
        .anchor(egui::Align2::RIGHT_BOTTOM, [-24.0, -24.0])
        .show(ui.ctx(), |ui| {
            let mut thumb_image = egui::Image::new((thumb_tex.id(), thumb_size));
            if app.rotation != 0.0 {
                let angle_rad = app.rotation * std::f32::consts::TAU / 360.0;
                thumb_image = thumb_image.rotate(angle_rad, egui::Vec2::splat(0.5));
            }
            let response = ui.add(thumb_image);

            // 绘制视口指示器（红框）
            draw_viewport_indicator(
                app,
                ui,
                response.rect.center(),
                thumb_size,
                display_size,
                available,
            );
        });
}

/// 绘制视口指示器
fn draw_viewport_indicator(
    app: &FastViewApp,
    ui: &mut egui::Ui,
    thumb_center: egui::Pos2,
    thumb_size: egui::Vec2,
    display_size: egui::Vec2,
    available: egui::Vec2,
) {
    // 可视区域占图片的比例
    let view_portion_x = (available.x / display_size.x).min(1.0);
    let view_portion_y = (available.y / display_size.y).min(1.0);

    // 红框的大小
    let view_rect_w = thumb_size.x * view_portion_x;
    let view_rect_h = thumb_size.y * view_portion_y;

    // 当前滚动位置的相对偏移
    let offset_ratio_x = (-app.image_offset.x / display_size.x + 0.5).clamp(0.0, 1.0);
    let offset_ratio_y = (-app.image_offset.y / display_size.y + 0.5).clamp(0.0, 1.0);

    // 红框的左上角位置（相对于缩略图中心）
    let unrotated_view_x = (thumb_size.x - view_rect_w) * offset_ratio_x - thumb_size.x / 2.0;
    let unrotated_view_y = (thumb_size.y - view_rect_h) * offset_ratio_y - thumb_size.y / 2.0;

    // 如果缩略图旋转了，需要将红框的四个角点旋转相同的角度
    if app.rotation != 0.0 {
        draw_rotated_viewport_rect(
            ui,
            thumb_center,
            unrotated_view_x,
            unrotated_view_y,
            view_rect_w,
            view_rect_h,
            app.rotation,
        );
    } else {
        // 未旋转时直接绘制矩形
        let rect_min = thumb_center + egui::vec2(unrotated_view_x, unrotated_view_y);
        let rect = egui::Rect::from_min_size(rect_min, egui::vec2(view_rect_w, view_rect_h));

        ui.painter().rect_stroke(
            rect,
            0.0,
            egui::Stroke::new(2.0, egui::Color32::from_rgba_unmultiplied(255, 80, 80, 230)),
            egui::StrokeKind::Outside,
        );
    }
}

/// 绘制旋转后的视口矩形
fn draw_rotated_viewport_rect(
    ui: &mut egui::Ui,
    thumb_center: egui::Pos2,
    unrotated_view_x: f32,
    unrotated_view_y: f32,
    view_rect_w: f32,
    view_rect_h: f32,
    rotation: f32,
) {
    let angle_rad = rotation * std::f32::consts::TAU / 360.0;
    let cos_a = angle_rad.cos();
    let sin_a = angle_rad.sin();

    // 红框的四个角点（相对于缩略图中心）
    let corners = [
        egui::vec2(unrotated_view_x, unrotated_view_y),
        egui::vec2(unrotated_view_x + view_rect_w, unrotated_view_y),
        egui::vec2(
            unrotated_view_x + view_rect_w,
            unrotated_view_y + view_rect_h,
        ),
        egui::vec2(unrotated_view_x, unrotated_view_y + view_rect_h),
    ];

    // 旋转四个角点
    let rotated_corners: Vec<egui::Pos2> = corners
        .iter()
        .map(|p| {
            let rx = p.x * cos_a - p.y * sin_a;
            let ry = p.x * sin_a + p.y * cos_a;
            thumb_center + egui::vec2(rx, ry)
        })
        .collect();

    // 绘制旋转后的红框（四条线段）
    for i in 0..4 {
        let start = rotated_corners[i];
        let end = rotated_corners[(i + 1) % 4];

        ui.painter().line_segment(
            [start, end],
            egui::Stroke::new(2.0, egui::Color32::from_rgba_unmultiplied(255, 80, 80, 230)),
        );
    }
}

/// 渲染分块图片占位符（当没有主纹理时）
fn render_tiled_placeholder(_app: &FastViewApp, _ui: &mut egui::Ui, _available: egui::Vec2) {
    // 分块图片会在 render_tiles 中处理
    // 这里可以添加加载中的提示
}

/// 渲染空状态
fn render_empty_state(app: &FastViewApp, ui: &mut egui::Ui, available: egui::Vec2) {
    if let Some(error) = &app.load_error {
        // 错误状态：显示友好提示框
        ui.with_layout(egui::Layout::top_down(egui::Align::Center), |ui| {
            ui.add_space(available.y / 3.0);

            egui::Frame::popup(ui.style())
                .fill(egui::Color32::from_rgb(255, 200, 200))
                .show(ui, |ui| {
                    ui.vertical(|ui| {
                        ui.label(
                            egui::RichText::new("⚠️ 加载失败")
                                .size(14.0)
                                .color(egui::Color32::RED),
                        );
                        ui.add_space(4.0);
                        ui.label(
                            egui::RichText::new(error)
                                .size(12.0)
                                .color(egui::Color32::DARK_RED),
                        );
                        ui.add_space(8.0);

                        ui.horizontal(|ui| {
                            if ui.button("🔄 重试").clicked() {
                                // 需要可变引用，这里只能标记，在外部处理
                                ui.ctx().data_mut(|data| {
                                    data.insert_temp(egui::Id::new("retry_load"), true);
                                });
                            }
                            if ui.button("❌ 关闭").clicked() {
                                ui.ctx().data_mut(|data| {
                                    data.insert_temp(egui::Id::new("clear_error"), true);
                                });
                            }
                        });
                    });
                });
        });
    } else {
        // 正常空状态
        ui.with_layout(egui::Layout::top_down(egui::Align::Center), |ui| {
            ui.add_space(available.y / 3.0);
            ui.label(
                egui::RichText::new("拖放图片或按 Ctrl+O 打开")
                    .size(14.0)
                    .color(egui::Color32::GRAY),
            );
        });
    }
}

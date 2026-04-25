use eframe::egui;

use crate::core::{TextKey, ZoomMode};
use crate::FastViewApp;

/// 渲染状态栏（悬浮半透明设计）
pub fn render_status_bar(app: &mut FastViewApp, ui: &mut egui::Ui) {
    // 显示条件：设置开启 + 非全屏模式
    if !app.settings.show_status_bar || app.is_fullscreen {
        return;
    }

    let screen_rect = ui.ctx().content_rect();

    egui::Area::new(egui::Id::new("floating_status_bar"))
        .anchor(egui::Align2::CENTER_BOTTOM, [0.0, -12.0]) // 底部居中，距离底部12px
        .show(ui.ctx(), |ui| {
            let visuals = &ui.ctx().global_style().visuals;

            // 毛玻璃背景效果
            let bg_color = visuals.panel_fill.gamma_multiply(0.7);
            let border_color = visuals.window_stroke.color.gamma_multiply(0.3);

            egui::Frame::NONE
                .fill(bg_color)
                .stroke(egui::Stroke::new(1.0, border_color))
                .corner_radius(10.0)
                .shadow(egui::epaint::Shadow {
                    offset: [0, 2],
                    blur: 12,
                    spread: 0,
                    color: egui::Color32::BLACK.gamma_multiply(0.15),
                })
                .inner_margin(egui::Margin::symmetric(14, 8)) // 左右14px, 上下8px
                .show(ui, |ui| {
                    // 计算最大宽度限制（避免过宽）
                    let max_width = (screen_rect.width() * 0.9).min(800.0);
                    ui.set_max_width(max_width);

                    // 使用 horizontal 布局，内容垂直居中对齐
                    ui.with_layout(
                        egui::Layout::left_to_right(egui::Align::Center),
                        |ui| {
                            render_status_content(ui, visuals, app);
                        },
                    );
                });
        });
}

/// 渲染状态栏内容
pub fn render_status_content(ui: &mut egui::Ui, visuals: &egui::Visuals, app: &FastViewApp) {
    // 辅助函数：添加分隔符（带间距）
    let separator = |ui: &mut egui::Ui| {
        ui.add_space(8.0);
        ui.separator();
        ui.add_space(8.0);
    };

    // 文件名（12号加粗，最大宽度200px，超出截断）
    if let Some(ref path) = app.current_path {
        let filename = path
            .file_name()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_default();

        ui.scope(|ui| {
            ui.set_max_width(200.0);
            let response = ui.add(
                egui::Label::new(egui::RichText::new(&filename).strong().size(12.0)).truncate(),
            );
            // 确保悬浮时鼠标图标不变
            if response.hovered() {
                ui.ctx().set_cursor_icon(egui::CursorIcon::Default);
            }
        });

        separator(ui);

        // 图片尺寸（等宽字体）
        ui.label(
            egui::RichText::new(format!(
                "{}×{}",
                app.image_size.x as u32, app.image_size.y as u32
            ))
            .family(egui::FontFamily::Monospace)
            .size(10.0)
            .color(visuals.weak_text_color()),
        );

        separator(ui);
    }

    // 图片索引
    if !app.current_images.is_empty() {
        ui.label(
            egui::RichText::new(format!(
                "{}/{}",
                app.current_index + 1,
                app.current_images.len()
            ))
            .size(10.0),
        );
        separator(ui);
    }

    // 缩放模式（仅 Custom 模式根据比例显示颜色）
    let zoom_text = match app.zoom_mode {
        ZoomMode::Fit => app.t(TextKey::Fit).to_string(),
        ZoomMode::Fill => app.t(TextKey::Fill).to_string(),
        ZoomMode::Original => app.t(TextKey::Original).to_string(),
        ZoomMode::Custom => format!("{}%", (app.zoom * 100.0) as u32),
    };

    let zoom_color = if app.zoom_mode == ZoomMode::Custom {
        if app.zoom > 1.0 {
            egui::Color32::from_rgb(255, 140, 0)
        } else if app.zoom < 1.0 {
            egui::Color32::from_rgb(100, 149, 237)
        } else {
            visuals.weak_text_color()
        }
    } else {
        visuals.weak_text_color()
    };

    ui.label(egui::RichText::new(&zoom_text).size(10.0).color(zoom_color));

    // 旋转角度
    if app.rotation != 0.0 {
        separator(ui);
        ui.label(
            egui::RichText::new(format!("{}°", app.rotation as u32))
                .size(10.0)
                .color(visuals.weak_text_color()),
        );
    }

    // 文件大小
    if app.file_size > 0 {
        separator(ui);
        ui.label(
            egui::RichText::new(app.format_file_size(app.file_size))
                .size(10.0)
                .family(egui::FontFamily::Monospace)
                .color(visuals.weak_text_color()),
        );
    }
}

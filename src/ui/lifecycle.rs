use eframe::egui;
use std::path::PathBuf;

use crate::app::FastViewApp;

/// 处理全屏模式下的UI自动隐藏逻辑
pub fn handle_fullscreen_ui(app: &mut FastViewApp, ui: &mut egui::Ui) {
    if app.is_fullscreen {
        let pointer_delta = ui.input(|i| i.pointer.delta());
        let any_motion = pointer_delta != egui::Vec2::ZERO || ui.input(|i| i.pointer.any_pressed());

        if any_motion {
            app.last_mouse_move = std::time::Instant::now();
            ui.ctx()
                .send_viewport_cmd(egui::ViewportCommand::CursorVisible(true));
        } else {
            let elapsed = app.last_mouse_move.elapsed().as_secs_f32();
            if elapsed > 3.0 {
                ui.ctx()
                    .send_viewport_cmd(egui::ViewportCommand::CursorVisible(false));
            }
        }
    } else {
        // 非全屏模式下确保光标始终可见
        ui.ctx()
            .send_viewport_cmd(egui::ViewportCommand::CursorVisible(true));
    }
}

/// 处理异步加载后的后续操作（目录更新、预加载）
pub fn handle_post_load_operations(
    app: &mut FastViewApp,
    _ui: &mut egui::Ui,
    needs_prefetch: bool,
    path_for_dir_update: Option<PathBuf>,
) {
    // 更新目录列表（在借用结束后）
    if let Some(ref path) = path_for_dir_update {
        app.update_directory_list(path);
    }

    // 预加载相邻图片（在借用结束后）
    if needs_prefetch {
        app.preload_adjacent_images();
    }
}

/// 处理缩略图导航栏的渲染和点击事件
pub fn handle_thumbnail_navigation(app: &mut FastViewApp, ui: &mut egui::Ui) {
    // 渲染缩略图导航栏
    let clicked_index = {
        let ctx = ui.ctx().clone();
        let current_images = app.current_images.clone();
        let current_index = app.current_index;
        let cmd_tx = app.cmd_tx.clone();

        app.thumbnail_mgr
            .render(ui, &ctx, &current_images, current_index, &cmd_tx)
    };

    // 处理点击事件
    if let Some(index) = clicked_index
        && index < app.current_images.len()
        && index != app.current_index
    {
        app.current_index = index;
        let path = app.current_images[index].clone();
        let ctx = ui.ctx().clone();
        let _ = app.load_image(&path, &ctx);
    }
}

/// 检查是否需要持续重绘（有待处理的加载任务）
pub fn check_needs_repaint(app: &FastViewApp, ui: &mut egui::Ui) {
    if app.current_path.is_some() && app.texture.is_none() {
        ui.ctx().request_repaint();
    }
}

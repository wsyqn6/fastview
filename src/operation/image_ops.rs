use eframe::egui;

use crate::app::FastViewApp;
use crate::core::ZoomMode;

/// 放大图片
pub fn zoom_in(app: &mut FastViewApp, current_scale: f32) {
    // 如果当前不是 Custom 模式，先切换到 Custom 并使用当前缩放比例
    if app.zoom_mode != ZoomMode::Custom {
        app.zoom = current_scale;
        app.zoom_mode = ZoomMode::Custom;
    }
    app.zoom *= 1.2;
}

/// 缩小图片
pub fn zoom_out(app: &mut FastViewApp, current_scale: f32) {
    // 如果当前不是 Custom 模式，先切换到 Custom 并使用当前缩放比例
    if app.zoom_mode != ZoomMode::Custom {
        app.zoom = current_scale;
        app.zoom_mode = ZoomMode::Custom;
    }
    app.zoom /= 1.2;
    if app.zoom < 0.01 {
        app.zoom = 0.01;
    }
}

/// 向左旋转90度
pub fn rotate_left(app: &mut FastViewApp) {
    app.rotation -= 90.0;
    if app.rotation < 0.0 {
        app.rotation += 360.0;
    }
}

/// 向右旋转90度
pub fn rotate_right(app: &mut FastViewApp) {
    app.rotation += 90.0;
    if app.rotation >= 360.0 {
        app.rotation -= 360.0;
    }
}

/// 切换全屏模式
pub fn toggle_fullscreen(app: &mut FastViewApp, ctx: &egui::Context) {
    app.is_fullscreen = !app.is_fullscreen;
    if app.is_fullscreen {
        app.is_ui_visible = false;
        // 立即隐藏光标
        ctx.send_viewport_cmd(egui::ViewportCommand::CursorVisible(false));
    } else {
        app.is_ui_visible = true;
        ctx.send_viewport_cmd(egui::ViewportCommand::CursorVisible(true));
    }
    ctx.send_viewport_cmd(egui::ViewportCommand::Fullscreen(app.is_fullscreen));
}

/// 切换无边框模式
pub fn toggle_borderless(app: &mut FastViewApp, ctx: &egui::Context) {
    app.is_borderless = !app.is_borderless;
    ctx.send_viewport_cmd(egui::ViewportCommand::Decorations(!app.is_borderless));
}

/// 切换状态栏显示
pub fn toggle_status_bar(app: &mut FastViewApp) {
    app.settings.show_status_bar = !app.settings.show_status_bar;
    app.save_settings();
}

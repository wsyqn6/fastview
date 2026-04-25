use eframe::egui;

use crate::app::FastViewApp;
use crate::core::{TextKey, ZoomMode};

/// 渲染菜单栏
pub fn render_menu_bar(app: &mut FastViewApp, ui: &mut egui::Ui) {
    // 菜单栏显示逻辑：仅在全屏模式下不显示，其他情况根据无边框设置决定
    let should_show_menu = !app.is_fullscreen && !app.is_borderless;

    if !should_show_menu {
        return;
    }

    // 传统菜单栏（类似 Windows 原生应用）
    egui::Panel::top("menu_bar")
        .exact_size(24.0)
        .show_inside(ui, |ui| {
            ui.horizontal(|ui| {
                render_file_menu(app, ui);
                render_view_menu(app, ui);
                render_settings_button(app, ui);
                render_help_menu(app, ui);
            });
        });
}

/// 渲染文件菜单
fn render_file_menu(app: &mut FastViewApp, ui: &mut egui::Ui) {
    ui.menu_button(app.t(TextKey::MenuFile), |ui| {
        if ui.button(app.t(TextKey::OpenFile)).clicked() {
            if let Some(path) = rfd::FileDialog::new()
                .add_filter(
                    "Images",
                    &[
                        "jpg", "jpeg", "png", "gif", "webp", "bmp", "tiff", "tif", "ico", "avif",
                    ],
                )
                .pick_file()
            {
                app.load_image(&path, ui.ctx()).ok();
            }
            ui.close();
        }
        ui.separator();
        if ui.button(app.t(TextKey::Exit)).clicked() {
            ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
            ui.close();
        }
    });
}

/// 渲染查看菜单
fn render_view_menu(app: &mut FastViewApp, ui: &mut egui::Ui) {
    ui.menu_button(app.t(TextKey::MenuView), |ui| {
        // 缩放模式
        if ui.button(app.t(TextKey::FitToWindow)).clicked() {
            app.zoom_mode = ZoomMode::Fit;
            app.image_offset = egui::Vec2::ZERO;
            ui.close();
        }
        if ui.button(app.t(TextKey::OriginalSize)).clicked() {
            app.zoom_mode = ZoomMode::Original;
            app.image_offset = egui::Vec2::ZERO;
            ui.close();
        }
        if ui.button(app.t(TextKey::FillWindow)).clicked() {
            app.zoom_mode = ZoomMode::Fill;
            app.image_offset = egui::Vec2::ZERO;
            ui.close();
        }
        ui.separator();

        // 缩放操作
        if ui.button(app.t(TextKey::ZoomIn)).clicked() {
            app.zoom_in(app.current_scale);
            ui.close();
        }
        if ui.button(app.t(TextKey::ZoomOut)).clicked() {
            app.zoom_out(app.current_scale);
            ui.close();
        }
        ui.separator();

        // 旋转
        if ui.button(app.t(TextKey::RotateClockwise)).clicked() {
            app.rotate_right();
            ui.close();
        }
        if ui.button(app.t(TextKey::RotateCounterClockwise)).clicked() {
            app.rotate_left();
            ui.close();
        }
        ui.separator();

        // 全屏
        if ui.button(app.t(TextKey::ToggleFullscreen)).clicked() {
            app.toggle_fullscreen(ui.ctx());
            ui.close();
        }

        // 无边框模式
        if ui.button(app.t(TextKey::ToggleBorderless)).clicked() {
            app.toggle_borderless(ui.ctx());
            ui.close();
        }
    });
}

/// 渲染设置按钮
fn render_settings_button(app: &mut FastViewApp, ui: &mut egui::Ui) {
    if ui.button(app.t(TextKey::MenuSettings)).clicked() {
        app.show_settings = true;
        app.window_stack.push(crate::app::WindowType::Settings);
    }
}

/// 渲染帮助菜单
fn render_help_menu(app: &mut FastViewApp, ui: &mut egui::Ui) {
    ui.menu_button(app.t(TextKey::MenuHelp), |ui| {
        if ui.button(app.t(TextKey::ShortcutsHelp)).clicked() {
            app.show_shortcuts = !app.show_shortcuts;
            if app.show_shortcuts {
                app.window_stack.push(crate::app::WindowType::Shortcuts);
            } else {
                app.window_stack
                    .retain(|w| w != &crate::app::WindowType::Shortcuts);
            }
            ui.close();
        }
        if ui.button(app.t(TextKey::AboutFastView)).clicked() {
            app.show_about = true;
            app.window_stack.push(crate::app::WindowType::About);
            ui.close();
        }
        ui.separator();

        // 检查更新（禁用状态，预留接口）
        ui.add_enabled_ui(false, |ui| {
            let _ = ui.button(app.t(TextKey::CheckForUpdates));
        });
    });
}

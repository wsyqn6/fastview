use eframe::egui;

use crate::app::{WindowType, elapsed_ms};

use crate::app::FastViewApp;
use crate::core::ZoomMode;

/// 处理键盘事件
pub fn handle_keyboard_events(app: &mut FastViewApp, ui: &mut egui::Ui) {
    for event in ui.ctx().input(|i| i.events.clone()) {
        if let egui::Event::Key {
            key,
            pressed,
            modifiers,
            ..
        } = event
        {
            if pressed {
                match key {
                    egui::Key::ArrowLeft => {
                        debug_log!(
                            "[{:.3}s] [APP] 检测到左箭头键",
                            elapsed_ms() as f64 / 1000.0
                        );
                        app.prev_image(ui.ctx());
                    }
                    egui::Key::ArrowRight => {
                        debug_log!(
                            "[{:.3}s] [APP] 检测到右箭头键",
                            elapsed_ms() as f64 / 1000.0
                        );
                        app.next_image(ui.ctx());
                    }
                    egui::Key::Equals | egui::Key::Plus => {
                        app.zoom_in(app.current_scale);
                    }
                    egui::Key::Minus => {
                        app.zoom_out(app.current_scale);
                    }
                    egui::Key::Num0 => {
                        app.zoom_mode = ZoomMode::Fit;
                        app.image_offset = egui::Vec2::ZERO;
                    }
                    egui::Key::Num1 => {
                        app.zoom_mode = ZoomMode::Original;
                        app.image_offset = egui::Vec2::ZERO;
                    }
                    egui::Key::Num2 => {
                        app.zoom_mode = ZoomMode::Fill;
                        app.image_offset = egui::Vec2::ZERO;
                    }
                    egui::Key::R if modifiers.shift => {
                        app.rotate_left();
                    }
                    egui::Key::R => {
                        app.rotate_right();
                    }
                    egui::Key::F => {
                        app.toggle_fullscreen(ui.ctx());
                    }
                    egui::Key::V => {
                        app.toggle_borderless(ui.ctx());
                    }
                    egui::Key::S => {
                        app.toggle_status_bar();
                    }
                    egui::Key::H => {
                        toggle_shortcuts_window(app);
                    }
                    egui::Key::T => {
                        app.thumbnail_mgr.toggle();
                    }
                    egui::Key::Space => {
                        app.is_drag_mode = true;
                    }
                    egui::Key::Escape => {
                        handle_escape_key(app, ui);
                    }
                    _ => {}
                }
            }

            // 松开空格键时退出拖动模式
            if !pressed && key == egui::Key::Space {
                app.is_drag_mode = false;
            }
        }

        // 处理 ? 键 (Shift+/)
        if let egui::Event::Text(text) = event
            && text == "?"
        {
            toggle_shortcuts_window(app);
        }
    }
}

/// 切换快捷键窗口
fn toggle_shortcuts_window(app: &mut FastViewApp) {
    app.show_shortcuts = !app.show_shortcuts;
    if app.show_shortcuts {
        app.window_stack.push(WindowType::Shortcuts);
    } else {
        app.window_stack.retain(|w| w != &WindowType::Shortcuts);
    }
}

/// 处理 Escape 键
fn handle_escape_key(app: &mut FastViewApp, ui: &mut egui::Ui) {
    // 后开先关原则：关闭最后打开的窗口
    if let Some(window_type) = app.window_stack.pop() {
        match window_type {
            WindowType::Shortcuts => app.show_shortcuts = false,
            WindowType::Settings => app.show_settings = false,
            WindowType::About => app.show_about = false,
        }
    }
    // 如果没有打开的窗口，则退出全屏或直接退出程序
    else if app.is_fullscreen {
        app.toggle_fullscreen(ui.ctx());
    } else {
        // 直接退出程序
        ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
    }
}

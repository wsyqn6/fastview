use eframe::egui;

use crate::core::{Language, Settings, TextKey};
use crate::FastViewApp;

/// 渲染所有对话框（设置、快捷键、关于）
pub fn render_dialogs(app: &mut FastViewApp, ui: &mut egui::Ui) {
    render_settings_dialog(app, ui);
    render_shortcuts_dialog(app, ui);
    render_about_dialog(app, ui);
}

/// 渲染设置对话框
fn render_settings_dialog(app: &mut FastViewApp, ui: &mut egui::Ui) {
    if !app.show_settings {
        return;
    }

    // Get all text outside to avoid borrowing issues
    let lang = app.settings.language;
    let settings_text = app.t(TextKey::MenuSettings);
    let general_text = TextKey::General.text(lang);
    let language_text = TextKey::Language.text(lang);
    let chinese_text = TextKey::Chinese.text(lang);
    let english_text = TextKey::English.text(lang);
    let cache_text = TextKey::Cache.text(lang);
    let max_cache_text = TextKey::MaxCacheSize.text(lang);
    let show_status_text = TextKey::ShowStatusBar.text(lang);
    let reset_text = TextKey::ResetSettings.text(lang);

    // Need to capture these outside the closure
    let current_lang = app.settings.language;
    let current_max_cache = app.settings.max_cache_size;
    let current_show_status = app.settings.show_status_bar;

    // Settings window - 卡片式设计，无标题栏
    egui::Window::new(settings_text)
        .open(&mut app.show_settings)
        .title_bar(false) // 移除标题栏
        .resizable(false)
        .collapsible(false)
        .fixed_size([320.0, 240.0])
        .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0]) // 居中显示
        .frame(egui::Frame::window(&ui.ctx().global_style())) // 使用窗口样式
        .show(ui.ctx(), |ui: &mut egui::Ui| {
            let mut temp_lang = current_lang;
            let mut temp_max_cache = current_max_cache;
            let mut temp_show_status = current_show_status;

            ui.heading(general_text);
            ui.label(language_text);
            ui.horizontal(|ui: &mut egui::Ui| {
                ui.radio_value(&mut temp_lang, Language::Chinese, chinese_text);
                ui.radio_value(&mut temp_lang, Language::English, english_text);
            });

            if temp_lang != current_lang {
                app.settings.language = temp_lang;
            }

            ui.separator();
            ui.heading(cache_text);
            let slider = egui::Slider::new(&mut temp_max_cache, 3..=30).text(max_cache_text);
            ui.add(slider);

            if temp_max_cache != current_max_cache {
                app.settings.max_cache_size = temp_max_cache;
            }

            ui.separator();
            ui.checkbox(&mut temp_show_status, show_status_text);

            if temp_show_status != current_show_status {
                app.settings.show_status_bar = temp_show_status;
            }

            ui.separator();
            if ui.button(reset_text).clicked() {
                app.settings = Settings::default();
            }
        });

    // Auto save on changes
    app.save_settings();
}

/// 渲染快捷键对话框
fn render_shortcuts_dialog(app: &mut FastViewApp, ui: &mut egui::Ui) {
    if !app.show_shortcuts {
        return;
    }

    let lang = app.settings.language;
    let title = app.t(TextKey::ShortcutsHelp);

    // 提前获取所有翻译文本，避免在闭包中借用 self
    let navigation_text = TextKey::Navigation.text(lang);
    let zoom_view_text = TextKey::ZoomAndView.text(lang);
    let rotation_text = TextKey::Rotation.text(lang);
    let system_text = TextKey::System.text(lang);

    egui::Window::new(title)
        .open(&mut app.show_shortcuts)
        .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
        .collapsible(false)
        .resizable(false)
        .fixed_size([480.0, 360.0])
        .title_bar(true)
        .show(ui.ctx(), |ui| {
            let visuals = &ui.ctx().global_style().visuals;
            ui.add_space(8.0);

            // 辅助函数：创建更具质感的键盘按键样式
            let key_badge = |ui: &mut egui::Ui, key: &str| {
                let bg_color = visuals.widgets.noninteractive.bg_fill;
                let stroke_color = visuals.widgets.noninteractive.bg_stroke.color;
                let text_color = visuals.text_color();

                egui::Frame::NONE
                    .fill(bg_color)
                    .stroke(egui::Stroke::new(1.0, stroke_color))
                    .shadow(egui::epaint::Shadow {
                        offset: [0, 2],
                        blur: 0,
                        spread: 0,
                        color: stroke_color.gamma_multiply(0.5),
                    })
                    .corner_radius(4.0)
                    .inner_margin(egui::Margin::symmetric(6, 2))
                    .show(ui, |ui| {
                        ui.label(
                            egui::RichText::new(key)
                                .family(egui::FontFamily::Monospace)
                                .size(11.0)
                                .color(text_color),
                        );
                    });
            };

            // 辅助函数：创建快捷键行
            let shortcut_row = |ui: &mut egui::Ui, keys: &[&str], desc: &str| {
                ui.horizontal(|ui| {
                    ui.add_space(4.0);
                    for (i, key) in keys.iter().enumerate() {
                        if i > 0 {
                            ui.label(egui::RichText::new("+").size(10.0).weak());
                        }
                        key_badge(ui, key);
                    }
                    ui.add_space(8.0);
                    ui.label(
                        egui::RichText::new(desc)
                            .size(11.0)
                            .color(visuals.weak_text_color()),
                    );
                });
                ui.add_space(2.0);
            };

            // 分组标题
            let section_title = |ui: &mut egui::Ui, title: &str| {
                ui.add_space(6.0);
                ui.label(
                    egui::RichText::new(title)
                        .size(11.0)
                        .strong()
                        .color(visuals.text_color()),
                );
                ui.add_space(2.0);
            };

            // 双列布局
            ui.columns(2, |columns| {
                // 左列
                columns[0].with_layout(egui::Layout::top_down(egui::Align::LEFT), |ui| {
                    section_title(ui, navigation_text);
                    shortcut_row(ui, &["←", "→"], TextKey::PreviousNext.text(lang));
                    shortcut_row(ui, &["Space"], TextKey::DragMode.text(lang));

                    ui.add_space(8.0);
                    section_title(ui, zoom_view_text);
                    shortcut_row(ui, &["+", "-"], TextKey::ZoomInOut.text(lang));
                    shortcut_row(ui, &["0"], TextKey::FitToWindow.text(lang));
                    shortcut_row(ui, &["1"], TextKey::OriginalSize.text(lang));
                    shortcut_row(ui, &["2"], TextKey::FillWindow.text(lang));
                });

                // 右列
                columns[1].with_layout(egui::Layout::top_down(egui::Align::LEFT), |ui| {
                    section_title(ui, rotation_text);
                    shortcut_row(ui, &["R"], TextKey::RotateLeft.text(lang));
                    shortcut_row(ui, &["Shift", "R"], TextKey::RotateRight.text(lang));

                    ui.add_space(8.0);
                    section_title(ui, system_text);
                    shortcut_row(ui, &["F"], TextKey::ToggleFullscreen.text(lang));
                    shortcut_row(ui, &["V"], TextKey::ToggleBorderless.text(lang));
                    shortcut_row(ui, &["S"], TextKey::ToggleStatusBar.text(lang));
                    shortcut_row(ui, &["Esc"], TextKey::ExitFullscreen.text(lang));
                    shortcut_row(ui, &["H", "?"], TextKey::ShowHideShortcuts.text(lang));
                });
            });

            ui.add_space(4.0);
        });
}

/// 渲染关于对话框
fn render_about_dialog(app: &mut FastViewApp, ui: &mut egui::Ui) {
    if !app.show_about {
        return;
    }

    let version = app.get_version();
    let title = app.t(TextKey::AboutFastView);
    let version_label = app.t(TextKey::Version);
    let github_label = app.t(TextKey::GitHub);
    let ok_text = app.t(TextKey::OK);
    let description = app.t(TextKey::AppDescription);

    egui::Window::new(title)
        .open(&mut app.show_about)
        .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
        .collapsible(false)
        .resizable(false)
        .fixed_size([320.0, 200.0])
        .show(ui.ctx(), |ui| {
            ui.vertical_centered(|ui| {
                ui.add_space(20.0);
                ui.heading("FastView");
                ui.label(format!("{} {}", version_label, version));
                ui.add_space(10.0);
                ui.label(description);
                ui.add_space(10.0);
                ui.hyperlink_to(
                    format!("{}: https://github.com/wsyqn6/fastview", github_label),
                    "https://github.com/wsyqn6/fastview",
                );
                ui.add_space(20.0);
                // 点击确定按钮关闭窗口（通过 open 参数自动处理）
                let _ = ui.button(ok_text);
            });
        });
}

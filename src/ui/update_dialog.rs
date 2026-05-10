//! 更新相关对话框 UI

use eframe::egui;

use crate::app::FastViewApp;
use crate::core::{TextKey, updater::UpdateStatus};

/// 渲染更新相关对话框
pub fn render_update_dialogs(app: &mut FastViewApp, ui: &mut egui::Ui) {
    render_checking_dialog(app, ui);
    render_update_available_dialog(app, ui);
    render_downloading_dialog(app, ui);
    render_download_complete_dialog(app, ui);
    render_error_dialog(app, ui);
}

/// 检查中对话框
fn render_checking_dialog(app: &mut FastViewApp, ui: &mut egui::Ui) {
    if app.update_status != UpdateStatus::Checking {
        return;
    }

    let checking_text = app.t(TextKey::CheckForUpdates);

    egui::Window::new(checking_text)
        .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
        .collapsible(false)
        .resizable(false)
        .fixed_size([300.0, 120.0])
        .title_bar(false)
        .show(ui.ctx(), |ui| {
            ui.vertical_centered(|ui| {
                ui.add_space(20.0);
                ui.spinner();
                ui.add_space(10.0);
                ui.label(checking_text);
            });
        });
}

/// 发现更新对话框
fn render_update_available_dialog(app: &mut FastViewApp, ui: &mut egui::Ui) {
    let update_info = match &app.update_status {
        UpdateStatus::UpdateAvailable { version, changelog, asset } => {
            (version.clone(), changelog.clone(), asset.clone())
        }
        _ => return,
    };

    let (version, changelog, _asset) = update_info;
    let title = app.t(TextKey::UpdateAvailable);
    let current_version_label = app.t(TextKey::CurrentVersion);
    let latest_version_label = app.t(TextKey::LatestVersion);
    let download_text = app.t(TextKey::DownloadAndUpdate);
    let cancel_text = app.t(TextKey::Cancel);
    let current_version = app.get_version();

    let mut should_close = false;
    let mut should_download = false;

    egui::Window::new(title)
        .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
        .collapsible(false)
        .resizable(true)
        .default_size([450.0, 350.0])
        .show(ui.ctx(), |ui| {
            ui.vertical(|ui| {
                // 版本信息
                ui.horizontal(|ui| {
                    ui.label(format!("{}: {}", current_version_label, current_version));
                    ui.add_space(20.0);
                    ui.label(egui::RichText::new(format!("{}: {}", latest_version_label, version)).strong());
                });
                
                ui.separator();

                // 更新内容
                ui.label(egui::RichText::new("What's New").strong());
                ui.add_space(4.0);
                
                egui::ScrollArea::vertical()
                    .max_height(180.0)
                    .show(ui, |ui| {
                        ui.label(
                            egui::RichText::new(&changelog)
                                .size(11.0)
                                .color(ui.visuals().text_color())
                        );
                    });

                ui.add_space(10.0);
                ui.separator();
                ui.add_space(10.0);

                // 按钮
                ui.horizontal(|ui| {
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.button(download_text).clicked() {
                            should_download = true;
                        }
                        if ui.button(cancel_text).clicked() {
                            should_close = true;
                        }
                    });
                });
            });
        });

    if should_close {
        app.update_status = UpdateStatus::NotChecked;
    }

    if should_download {
        app.start_update_download();
    }
}

/// 下载进度对话框
fn render_downloading_dialog(app: &mut FastViewApp, ui: &mut egui::Ui) {
    let progress = match &app.update_status {
        UpdateStatus::Downloading(p) => *p,
        _ => return,
    };

    let downloading_text = app.t(TextKey::Downloading);
    let cancel_text = app.t(TextKey::Cancel);

    let mut should_cancel = false;

    egui::Window::new(downloading_text)
        .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
        .collapsible(false)
        .resizable(false)
        .fixed_size([350.0, 150.0])
        .show(ui.ctx(), |ui| {
            ui.vertical_centered(|ui| {
                ui.add_space(20.0);
                ui.label(downloading_text);
                ui.add_space(10.0);
                
                // 进度条
                ui.add(
                    egui::ProgressBar::new(progress)
                        .show_percentage()
                        .animate(true)
                );
                
                ui.add_space(20.0);
                
                if ui.button(cancel_text).clicked() {
                    should_cancel = true;
                }
            });
        });

    if should_cancel {
        app.cancel_update_download();
    }
}

/// 下载完成对话框
fn render_download_complete_dialog(app: &mut FastViewApp, ui: &mut egui::Ui) {
    if !matches!(&app.update_status, UpdateStatus::DownloadComplete(_)) {
        return;
    }

    let complete_text = app.t(TextKey::DownloadComplete);
    let restart_text = app.t(TextKey::RestartNow);
    let remind_text = app.t(TextKey::RemindLater);

    let mut should_restart = false;
    let mut should_remind = false;

    egui::Window::new(complete_text)
        .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
        .collapsible(false)
        .resizable(false)
        .fixed_size([350.0, 150.0])
        .show(ui.ctx(), |ui| {
            ui.vertical_centered(|ui| {
                ui.add_space(20.0);
                ui.label(complete_text);
                ui.add_space(20.0);
                
                ui.horizontal(|ui| {
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.button(restart_text).clicked() {
                            should_restart = true;
                        }
                        if ui.button(remind_text).clicked() {
                            should_remind = true;
                        }
                    });
                });
            });
        });

    if should_restart {
        app.execute_update();
    }

    if should_remind {
        app.update_status = UpdateStatus::NotChecked;
    }
}

/// 错误对话框
fn render_error_dialog(app: &mut FastViewApp, ui: &mut egui::Ui) {
    let error_msg = match &app.update_status {
        UpdateStatus::Error(msg) => msg.clone(),
        _ => return,
    };

    let ok_text = app.t(TextKey::OK);

    let mut should_close = false;

    egui::Window::new("Error")
        .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
        .collapsible(false)
        .resizable(false)
        .fixed_size([350.0, 150.0])
        .show(ui.ctx(), |ui| {
            ui.vertical_centered(|ui| {
                ui.add_space(20.0);
                ui.colored_label(egui::Color32::RED, &error_msg);
                ui.add_space(20.0);
                
                if ui.button(ok_text).clicked() {
                    should_close = true;
                }
            });
        });

    if should_close {
        app.update_status = UpdateStatus::NotChecked;
    }
}

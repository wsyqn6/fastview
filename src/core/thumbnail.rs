use crate::core::loader::{LoadCommand, LoadPriority};
use crate::utils::to_non_zero_usize;
/// 缩略图导航栏模块
use eframe::egui;
use std::collections::HashSet;
use std::path::PathBuf;
use std::time::Instant;

pub struct ThumbnailBar {
    pub visible: bool,
    pub textures: lru::LruCache<PathBuf, egui::TextureHandle>,
    pub pending: HashSet<PathBuf>,
    pub failed: HashSet<PathBuf>,
    pub last_interaction: Instant,
}

impl Default for ThumbnailBar {
    fn default() -> Self {
        Self::new()
    }
}

impl ThumbnailBar {
    pub fn new() -> Self {
        Self {
            visible: false,
            textures: lru::LruCache::new(to_non_zero_usize(30, 30)),
            pending: HashSet::new(),
            failed: HashSet::new(),
            last_interaction: Instant::now(),
        }
    }

    pub fn toggle(&mut self) {
        let was_visible = self.visible;
        self.visible = !self.visible;
        if self.visible {
            self.last_interaction = Instant::now();
            #[cfg(debug_assertions)]
            eprintln!("[THUMB] Shown");
        } else if was_visible {
            #[cfg(debug_assertions)]
            eprintln!("[THUMB] Hidden");
        }
    }

    pub fn check_auto_hide(&mut self) -> bool {
        if self.visible && self.last_interaction.elapsed().as_secs() >= 2 {
            self.visible = false;
            return true;
        }
        false
    }

    pub fn record_interaction(&mut self) {
        self.last_interaction = Instant::now();
    }

    pub fn should_generate(&self, path: &PathBuf) -> bool {
        !self.textures.contains(path) && !self.pending.contains(path) && !self.failed.contains(path)
    }

    pub fn mark_pending(&mut self, path: PathBuf) {
        self.pending.insert(path);
    }

    pub fn mark_ready(&mut self, path: &PathBuf) {
        self.pending.remove(path);
        self.failed.remove(path);
    }

    pub fn mark_failed(&mut self, path: &PathBuf) {
        self.pending.remove(path);
        self.failed.insert(path.clone());
    }

    pub fn add_texture(&mut self, path: PathBuf, texture: egui::TextureHandle) {
        self.textures.put(path, texture);
    }

    pub fn get_texture(&mut self, path: &PathBuf) -> Option<egui::TextureHandle> {
        self.textures.get(path).cloned()
    }

    pub fn request_generation(
        &mut self,
        path: &PathBuf,
        cmd_tx: &Option<std::sync::mpsc::Sender<LoadCommand>>,
    ) {
        if !self.should_generate(path) {
            return;
        }
        self.mark_pending(path.clone());
        if let Some(tx) = cmd_tx {
            // 使用新的命令类型，后台线程会自动检查缓存
            debug_log!(
                "[THUMB] 请求生成缩略图: {:?}",
                path.file_name()
            );
            let _ = tx.send(LoadCommand::GenerateThumbnailFromCache {
                path: path.clone(),
                size: 100,
                priority: LoadPriority::Low,
            });
        }
    }

    pub fn render(
        &mut self,
        ui: &mut egui::Ui,
        ctx: &egui::Context,
        current_images: &[PathBuf],
        current_index: usize,
        on_image_select: impl Fn(usize),
        cmd_tx: &Option<std::sync::mpsc::Sender<LoadCommand>>,
    ) {
        if !self.visible || current_images.is_empty() {
            return;
        }

        self.check_auto_hide();

        let screen_rect = ctx.content_rect();
        let bar_height = 120.0;
        let bar_width = (screen_rect.width() * 0.9).min(1600.0);
        let bar_x = (screen_rect.width() - bar_width) / 2.0;
        let bar_rect = egui::Rect::from_min_size(
            egui::Pos2::new(bar_x, screen_rect.bottom() - bar_height - 8.0),
            egui::Vec2::new(bar_width, bar_height),
        );

        // 深色半透明背景 (与主界面一致)
        let bg_color = egui::Color32::from_rgba_unmultiplied(30, 30, 30, 245);
        ui.painter().rect_filled(bar_rect, 10.0, bg_color);

        // 顶部细线分隔
        let top_line_rect = egui::Rect::from_min_size(
            egui::Pos2::new(bar_rect.left(), bar_rect.top()),
            egui::Vec2::new(bar_rect.width(), 1.0),
        );
        ui.painter().rect_filled(
            top_line_rect,
            0.0,
            egui::Color32::from_rgba_unmultiplied(255, 255, 255, 25),
        );

        // 渲染列表
        self.render_list(
            ui,
            bar_rect,
            current_images,
            current_index,
            on_image_select,
            cmd_tx,
        );
    }

    fn render_list(
        &mut self,
        ui: &mut egui::Ui,
        bar_rect: egui::Rect,
        current_images: &[PathBuf],
        current_index: usize,
        on_image_select: impl Fn(usize),
        cmd_tx: &Option<std::sync::mpsc::Sender<LoadCommand>>,
    ) {
        let thumb_size = 90.0; // 正方形容器尺寸
        let spacing = 10.0;
        let padding = 20.0;
        let visible_width = bar_rect.width() - (padding * 2.0);

        // 计算滚动偏移，使当前图片居中
        let target_offset = if current_index < current_images.len() {
            let center_pos = current_index as f32 * (thumb_size + spacing);
            (center_pos - visible_width / 2.0).max(0.0)
        } else {
            0.0
        };

        // 计算可见范围
        let start_idx = (target_offset / (thumb_size + spacing)) as usize;
        let end_idx = ((target_offset + visible_width) / (thumb_size + spacing)) as usize + 2;
        let end_idx = end_idx.min(current_images.len());

        #[allow(clippy::needless_range_loop)]
        for idx in start_idx..end_idx {
            let path = &current_images[idx];
            let x = bar_rect.left() + padding + idx as f32 * (thumb_size + spacing) - target_offset;
            let y = bar_rect.top() + (bar_rect.height() - thumb_size) / 2.0; // 垂直居中

            let container_rect = egui::Rect::from_min_size(
                egui::Pos2::new(x, y),
                egui::Vec2::new(thumb_size, thumb_size),
            );

            self.render_single_thumbnail(
                ui,
                path,
                idx,
                idx == current_index,
                container_rect,
                &on_image_select,
            );

            if self.should_generate(path) {
                self.request_generation(path, cmd_tx);
            }
        }
    }

    fn render_single_thumbnail(
        &mut self,
        ui: &mut egui::Ui,
        path: &PathBuf,
        index: usize,
        is_current: bool,
        container_rect: egui::Rect,
        on_image_select: &impl Fn(usize),
    ) {
        if let Some(texture) = self.get_texture(path) {
            // 计算保持宽高比的显示区域 (letterbox)
            let tex_size = texture.size_vec2();
            let tex_aspect = tex_size.x / tex_size.y;
            let container_aspect = container_rect.width() / container_rect.height();

            let (display_rect, uv_rect) = if tex_aspect > container_aspect {
                // 纹理更宽，以宽度为准
                let display_height = container_rect.width() / tex_aspect;
                let display_y =
                    container_rect.top() + (container_rect.height() - display_height) / 2.0;
                let display_rect = egui::Rect::from_min_size(
                    egui::Pos2::new(container_rect.left(), display_y),
                    egui::Vec2::new(container_rect.width(), display_height),
                );
                (
                    display_rect,
                    egui::Rect::from_min_max(egui::Pos2::ZERO, egui::Pos2::new(1.0, 1.0)),
                )
            } else {
                // 纹理更高，以高度为准
                let display_width = container_rect.height() * tex_aspect;
                let display_x =
                    container_rect.left() + (container_rect.width() - display_width) / 2.0;
                let display_rect = egui::Rect::from_min_size(
                    egui::Pos2::new(display_x, container_rect.top()),
                    egui::Vec2::new(display_width, container_rect.height()),
                );
                (
                    display_rect,
                    egui::Rect::from_min_max(egui::Pos2::ZERO, egui::Pos2::new(1.0, 1.0)),
                )
            };

            // 绘制背景 (深色，与主界面一致)
            ui.painter().rect_filled(
                container_rect,
                6.0,
                egui::Color32::from_rgba_unmultiplied(40, 40, 40, 200),
            );

            // 绘制缩略图
            ui.painter()
                .image(texture.id(), display_rect, uv_rect, egui::Color32::WHITE);

            // 边框样式
            let (border_color, border_width) = if is_current {
                (egui::Color32::from_rgb(59, 130, 246), 2.5) // 蓝色高亮
            } else if ui.rect_contains_pointer(container_rect) {
                (
                    egui::Color32::from_rgba_unmultiplied(255, 255, 255, 150),
                    2.0,
                ) // 悬停白色
            } else {
                (
                    egui::Color32::from_rgba_unmultiplied(255, 255, 255, 40),
                    1.0,
                ) // 默认淡白
            };

            // 绘制边框
            ui.painter().rect_stroke(
                container_rect,
                6.0,
                egui::Stroke::new(border_width, border_color),
                egui::StrokeKind::Inside,
            );

            // 当前选中：添加外发光效果
            if is_current {
                for i in 1..=2 {
                    let expand_amount = i as f32 * 2.0;
                    let alpha = (3 - i) as u8 * 40;
                    ui.painter().rect_stroke(
                        container_rect.expand(expand_amount),
                        6.0,
                        egui::Stroke::new(
                            1.5,
                            egui::Color32::from_rgba_unmultiplied(59, 130, 246, alpha),
                        ),
                        egui::StrokeKind::Outside,
                    );
                }
            }

            // 点击检测
            let response = ui.interact(
                container_rect,
                egui::Id::new(("thumb", index)),
                egui::Sense::click(),
            );
            if response.clicked() {
                (on_image_select)(index);
                self.record_interaction();
            }

            // 悬停效果
            if response.hovered() {
                self.record_interaction();
            }
        } else {
            self.render_placeholder(ui, container_rect, is_current);
        }
    }

    fn render_placeholder(&self, ui: &mut egui::Ui, container_rect: egui::Rect, is_current: bool) {
        // 深色背景 (与主界面一致)
        let bg_color = egui::Color32::from_rgba_unmultiplied(40, 40, 40, 200);
        ui.painter().rect_filled(container_rect, 6.0, bg_color);

        // 边框
        let border_color = if is_current {
            egui::Color32::from_rgb(59, 130, 246)
        } else {
            egui::Color32::from_rgba_unmultiplied(255, 255, 255, 40)
        };

        ui.painter().rect_stroke(
            container_rect,
            6.0,
            egui::Stroke::new(if is_current { 2.0 } else { 1.0 }, border_color),
            egui::StrokeKind::Inside,
        );

        // 加载动画：三个点
        let center = container_rect.center();
        let dot_size = 4.0;
        let spacing = 8.0;

        for i in 0..3 {
            let x = center.x - spacing + (i as f32 * spacing);
            let y = center.y;
            let dot_rect = egui::Rect::from_center_size(
                egui::Pos2::new(x, y),
                egui::Vec2::new(dot_size, dot_size),
            );
            ui.painter().circle_filled(
                dot_rect.center(),
                dot_size / 2.0,
                egui::Color32::from_rgba_unmultiplied(255, 255, 255, 150),
            );
        }
    }
}

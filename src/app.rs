use eframe::egui;
use image::GenericImageView;
use std::path::PathBuf;
use std::sync::Arc;

use crate::fonts::setup_fonts;
use crate::types::{CachedImage, ImageCache, Language, Settings, TextKey, ZoomMode};

/// 读取图片的 EXIF 方向信息
fn read_exif_orientation(path: &PathBuf) -> image::metadata::Orientation {
    use std::fs::File;
    use std::io::BufReader;

    // 尝试读取 EXIF 数据
    if let Ok(file) = File::open(path) {
        let mut bufreader = BufReader::new(&file);
        let exifreader = exif::Reader::new();

        if let Ok(exif) = exifreader.read_from_container(&mut bufreader) {
            // 查找 Orientation 标签
            if let Some(orientation_field) =
                exif.get_field(exif::Tag::Orientation, exif::In::PRIMARY)
            {
                // 解析方向值
                if let exif::Value::Short(ref shorts) = orientation_field.value {
                    if let Some(&orientation_value) = shorts.first() {
                        // EXIF 方向值 (1-8) 转换为 image crate 的 Orientation
                        return match orientation_value {
                            1 => image::metadata::Orientation::NoTransforms,
                            2 => image::metadata::Orientation::FlipHorizontal,
                            3 => image::metadata::Orientation::Rotate180,
                            4 => image::metadata::Orientation::FlipVertical,
                            5 => {
                                // Rotate 90 CW + Flip Horizontal = Rotate 270 CW
                                image::metadata::Orientation::Rotate270
                            }
                            6 => image::metadata::Orientation::Rotate90,
                            7 => {
                                // Rotate 270 CW + Flip Horizontal = Rotate 90 CW
                                image::metadata::Orientation::Rotate90
                            }
                            8 => image::metadata::Orientation::Rotate270,
                            _ => image::metadata::Orientation::NoTransforms,
                        };
                    }
                }
            }
        }
    }

    // 默认无转换
    image::metadata::Orientation::NoTransforms
}

pub struct FastViewApp {
    pub texture: Option<egui::TextureHandle>,
    pub thumbnail_texture: Option<egui::TextureHandle>,
    pub zoom: f32,
    pub rotation: f32,
    pub zoom_mode: ZoomMode,
    pub current_images: Vec<PathBuf>,
    pub current_index: usize,
    pub image_size: egui::Vec2,
    pub show_shortcuts: bool,
    pub is_drag_mode: bool,
    pub image_offset: egui::Vec2,
    pub pointer_down: bool,
    pub current_path: Option<PathBuf>,
    pub is_fullscreen: bool,
    pub current_scale: f32,
    pub image_cache: ImageCache,
    pub settings: Settings,
    pub show_settings: bool,
}

impl Default for FastViewApp {
    fn default() -> Self {
        Self {
            texture: None,
            thumbnail_texture: None,
            zoom: 1.0,
            rotation: 0.0,
            zoom_mode: ZoomMode::Fit,
            current_images: Vec::new(),
            current_index: 0,
            image_size: egui::Vec2::ZERO,
            show_shortcuts: false,
            is_drag_mode: false,
            image_offset: egui::Vec2::ZERO,
            pointer_down: false,
            current_path: None,
            is_fullscreen: false,
            current_scale: 1.0,
            image_cache: Arc::new(std::sync::Mutex::new(std::collections::HashMap::new())),
            settings: Settings::default(),
            show_settings: false,
        }
    }
}

impl FastViewApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        setup_fonts(cc);

        let mut app = Self::default();
        app.load_settings();
        app
    }

    fn load_settings(&mut self) {
        if let Some(config_dir) = dirs::config_dir() {
            let config_path = config_dir.join("fastview").join("settings.json");
            if config_path.exists() {
                if let Ok(content) = std::fs::read_to_string(&config_path) {
                    if let Ok(settings) = serde_json::from_str(&content) {
                        self.settings = settings;
                    }
                }
            }
        }
    }

    pub fn save_settings(&self) {
        if let Some(config_dir) = dirs::config_dir() {
            let config_path = config_dir.join("fastview").join("settings.json");
            if let Some(parent) = config_path.parent() {
                std::fs::create_dir_all(parent).ok();
            }
            if let Ok(content) = serde_json::to_string_pretty(&self.settings) {
                std::fs::write(&config_path, content).ok();
            }
        }
    }

    fn t(&self, key: TextKey) -> &'static str {
        key.text(self.settings.language)
    }

    pub fn load_image(&mut self, path: &PathBuf, ctx: &egui::Context) -> Result<(), String> {
        // 加载图片
        let img = image::open(path).map_err(|e| format!("Failed to open image: {}", e))?;

        // 读取 EXIF 方向信息
        let orientation = read_exif_orientation(path);

        // 应用方向转换
        let mut dynamic_img = img;
        dynamic_img.apply_orientation(orientation);

        let (width, height) = dynamic_img.dimensions();
        let image_size = egui::vec2(width as f32, height as f32);

        let color_image = egui::ColorImage::from_rgba_unmultiplied(
            [width as usize, height as usize],
            dynamic_img.to_rgba8().as_raw(),
        );

        let texture = ctx.load_texture("image", color_image, egui::TextureOptions::LINEAR);

        let thumb_size = 200;
        let thumb_img = dynamic_img.thumbnail(thumb_size, thumb_size);
        let (tw, th) = thumb_img.dimensions();
        let thumb_color_image = egui::ColorImage::from_rgba_unmultiplied(
            [tw as usize, th as usize],
            thumb_img.to_rgba8().as_raw(),
        );
        let thumbnail_texture =
            ctx.load_texture("thumbnail", thumb_color_image, egui::TextureOptions::LINEAR);

        let cached = Arc::new(CachedImage {
            texture: texture.clone(),
            thumbnail_texture: thumbnail_texture.clone(),
            image_size,
        });

        {
            let mut cache_guard = self.image_cache.lock().unwrap();
            cache_guard.insert(path.clone(), cached);
        }

        self.texture = Some(texture);
        self.thumbnail_texture = Some(thumbnail_texture);
        self.image_size = image_size;
        self.current_path = Some(path.clone());
        self.zoom_mode = ZoomMode::Fit;
        self.zoom = 1.0;
        self.rotation = 0.0;
        self.image_offset = egui::Vec2::ZERO;

        self.cleanup_cache();

        if let Some(parent) = path.parent() {
            let mut images: Vec<PathBuf> = parent
                .read_dir()
                .ok()
                .into_iter()
                .flat_map(|entries| entries.filter_map(|e| e.ok()))
                .map(|entry| entry.path())
                .filter(|p| {
                    p.is_file()
                        && p.extension()
                            .and_then(|ext| ext.to_str())
                            .map(|ext| {
                                matches!(
                                    ext.to_lowercase().as_str(),
                                    "jpg"
                                        | "jpeg"
                                        | "png"
                                        | "gif"
                                        | "webp"
                                        | "bmp"
                                        | "tiff"
                                        | "tif"
                                        | "ico"
                                        | "avif"
                                )
                            })
                            .unwrap_or(false)
                })
                .collect();
            images.sort();

            if let Some(pos) = images.iter().position(|p| p == path) {
                self.current_images = images;
                self.current_index = pos;
            }
        }

        Ok(())
    }

    pub fn prev_image(&mut self, ctx: &egui::Context) {
        if !self.current_images.is_empty() {
            if self.current_index > 0 {
                self.current_index -= 1;
            } else {
                self.current_index = self.current_images.len() - 1;
            }
            let path = self.current_images[self.current_index].clone();
            self.load_image(&path, ctx).ok();
        }
    }

    pub fn next_image(&mut self, ctx: &egui::Context) {
        if !self.current_images.is_empty() {
            if self.current_index < self.current_images.len() - 1 {
                self.current_index += 1;
            } else {
                self.current_index = 0;
            }
            let path = self.current_images[self.current_index].clone();
            self.load_image(&path, ctx).ok();
        }
    }

    pub fn zoom_in(&mut self, current_scale: f32) {
        // 如果当前不是 Custom 模式，先切换到 Custom 并使用当前缩放比例
        if self.zoom_mode != ZoomMode::Custom {
            self.zoom = current_scale;
            self.zoom_mode = ZoomMode::Custom;
        }
        self.zoom *= 1.2;
    }

    pub fn zoom_out(&mut self, current_scale: f32) {
        // 如果当前不是 Custom 模式，先切换到 Custom 并使用当前缩放比例
        if self.zoom_mode != ZoomMode::Custom {
            self.zoom = current_scale;
            self.zoom_mode = ZoomMode::Custom;
        }
        self.zoom /= 1.2;
        if self.zoom < 0.01 {
            self.zoom = 0.01;
        }
    }

    pub fn rotate_left(&mut self) {
        self.rotation -= 90.0;
        if self.rotation < 0.0 {
            self.rotation += 360.0;
        }
    }

    pub fn rotate_right(&mut self) {
        self.rotation += 90.0;
        if self.rotation >= 360.0 {
            self.rotation -= 360.0;
        }
    }

    fn cleanup_cache(&mut self) {
        let mut cache_guard = self.image_cache.lock().unwrap();
        if cache_guard.len() > self.settings.max_cache_size {
            let current_path = self.current_path.as_ref();
            cache_guard.retain(|path, _| Some(path) == current_path);
        }
    }

    pub fn toggle_fullscreen(&mut self, ctx: &egui::Context) {
        self.is_fullscreen = !self.is_fullscreen;
        ctx.send_viewport_cmd(egui::ViewportCommand::Fullscreen(self.is_fullscreen));
    }
}

impl eframe::App for FastViewApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // 全屏时隐藏菜单栏和状态栏，提升沉浸体验
        if !self.is_fullscreen {
            // 传统菜单栏（类似 Windows 原生应用）
            egui::TopBottomPanel::top("menu_bar")
                .exact_height(24.0)
                .show(ctx, |ui| {
                    ui.horizontal(|ui| {
                        let lang = self.settings.language;

                        // 文件菜单
                        ui.menu_button(TextKey::File.text(lang), |ui| {
                            if ui.button(TextKey::OpenFile.text(lang)).clicked() {
                                if let Some(path) = rfd::FileDialog::new()
                                    .add_filter(
                                        "Images",
                                        &[
                                            "jpg", "jpeg", "png", "gif", "webp", "bmp", "tiff",
                                            "tif", "ico", "avif",
                                        ],
                                    )
                                    .pick_file()
                                {
                                    self.load_image(&path, ctx).ok();
                                }
                                ui.close();
                            }
                            ui.separator();
                            if ui.button(TextKey::Exit.text(lang)).clicked() {
                                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                                ui.close();
                            }
                        });

                        // 视图菜单
                        ui.menu_button(TextKey::View.text(lang), |ui| {
                            if ui.button(TextKey::Fullscreen.text(lang)).clicked() {
                                self.toggle_fullscreen(ctx);
                                ui.close();
                            }
                            ui.separator();
                            ui.checkbox(&mut self.show_shortcuts, TextKey::Shortcuts.text(lang));
                            ui.checkbox(&mut self.show_settings, TextKey::Settings.text(lang));
                        });
                    });
                });
        }

        // 检查当前图片是否已经有高清缓存版本，如果有就更新显示
        if let Some(current_path) = &self.current_path {
            let need_update = {
                let cache_guard = self.image_cache.lock().unwrap();
                cache_guard.get(current_path).cloned()
            };

            if let Some(cached) = need_update {
                if self.texture.as_ref().map(|t| t.id()) != Some(cached.texture.id()) {
                    self.texture = Some(cached.texture.clone());
                    self.thumbnail_texture = Some(cached.thumbnail_texture.clone());
                    self.image_size = cached.image_size;
                }
            }
        }

        // Status bar - 玻璃态设计
        if self.settings.show_status_bar && !self.is_fullscreen {
            egui::TopBottomPanel::bottom("status_bar")
                .exact_height(32.0)
                .show(ctx, |ui| {
                    // 自定义背景：半透明玻璃效果
                    let visuals = &ctx.style().visuals;
                    let bg_color = visuals.panel_fill.gamma_multiply(0.85);
                    ui.painter()
                        .rect_filled(ui.available_rect_before_wrap(), 0.0, bg_color);

                    ui.horizontal(|ui| {
                        ui.add_space(12.0);

                        // 文件名（加粗）
                        if let Some(ref path) = self.current_path {
                            let filename = path
                                .file_name()
                                .map(|s| s.to_string_lossy())
                                .unwrap_or_default();

                            ui.label(egui::RichText::new(filename).strong().size(12.0));

                            // 自定义分隔符
                            ui.add_space(8.0);
                            ui.separator();
                            ui.add_space(8.0);

                            // 图片尺寸（等宽字体）
                            ui.label(
                                egui::RichText::new(format!(
                                    "{}×{}",
                                    self.image_size.x as u32, self.image_size.y as u32
                                ))
                                .family(egui::FontFamily::Monospace)
                                .size(11.0)
                                .color(visuals.weak_text_color()),
                            );

                            ui.add_space(8.0);
                            ui.separator();
                            ui.add_space(8.0);
                        }

                        // 图片索引
                        if !self.current_images.is_empty() {
                            ui.label(
                                egui::RichText::new(format!(
                                    "{}/{}",
                                    self.current_index + 1,
                                    self.current_images.len()
                                ))
                                .size(11.0),
                            );
                            ui.add_space(8.0);
                            ui.separator();
                            ui.add_space(8.0);
                        }

                        // 缩放模式（使用徽章样式）
                        let zoom_text = match self.zoom_mode {
                            ZoomMode::Fit => self.t(TextKey::Fit).to_string(),
                            ZoomMode::Fill => self.t(TextKey::Fill).to_string(),
                            ZoomMode::Original => self.t(TextKey::Original).to_string(),
                            ZoomMode::Custom => format!("{}%", (self.zoom * 100.0) as u32),
                        };

                        let zoom_color = if self.zoom_mode == ZoomMode::Custom {
                            visuals.selection.bg_fill
                        } else {
                            visuals.weak_text_color()
                        };

                        ui.label(egui::RichText::new(zoom_text).size(11.0).color(zoom_color));

                        // 旋转角度
                        if self.rotation != 0.0 {
                            ui.add_space(8.0);
                            ui.separator();
                            ui.add_space(8.0);
                            ui.label(
                                egui::RichText::new(format!("{}°", self.rotation as u32))
                                    .size(11.0)
                                    .color(visuals.weak_text_color()),
                            );
                        }

                        ui.add_space(12.0);
                    });
                });
        }

        // CentralPanel：保留背景色，移除内边距
        egui::CentralPanel::default()
            .frame(
                egui::Frame::NONE.fill(ctx.style().visuals.panel_fill), // 使用面板背景色
            )
            .show(ctx, |ui| {
                // 处理拖拽文件
                if ui.ctx().input(|i| !i.raw.dropped_files.is_empty()) {
                    let files = ui.ctx().input(|i| i.raw.dropped_files.clone());
                    for file in files {
                        if let Some(path) = file.path {
                            if path.is_file() {
                                if let Some(ext) = path.extension() {
                                    if let Some(ext_str) = ext.to_str() {
                                        let ext_lower = ext_str.to_lowercase();
                                        if [
                                            "jpg", "jpeg", "png", "gif", "webp", "bmp", "tiff",
                                            "tif", "ico", "avif",
                                        ]
                                        .contains(&ext_lower.as_str())
                                        {
                                            self.load_image(&path, ctx).ok();
                                            break;
                                        }
                                    }
                                }
                            }
                        }
                    }
                }

                let available = ui.available_size();

                if let Some(ref texture) = self.texture {
                    let mut size = self.image_size;

                    match self.zoom_mode {
                        ZoomMode::Fit => {
                            // Fit 模式：保持比例，完全适应可用空间
                            let scale_x = available.x / size.x;
                            let scale_y = available.y / size.y;
                            let scale = scale_x.min(scale_y);
                            size *= scale;
                            self.current_scale = scale;
                        }
                        ZoomMode::Fill => {
                            let scale = (available.x / size.x).max(available.y / size.y);
                            size *= scale;
                            self.current_scale = scale;
                        }
                        ZoomMode::Original => {
                            size = self.image_size;
                            self.current_scale = 1.0;
                        }
                        ZoomMode::Custom => {
                            size *= self.zoom;
                            self.current_scale = self.zoom;
                        }
                    }

                    // 使用之前获取的 available，避免重复调用导致的不一致
                    let center = egui::Pos2::new(available.x / 2.0, available.y / 2.0);
                    let rect = egui::Rect::from_center_size(center + self.image_offset, size);

                    // 获取 ui 的实际偏移量
                    let ui_offset = ui.cursor().min;
                    let absolute_rect = rect.translate(ui_offset.to_vec2());

                    let mut image = egui::Image::new((texture.id(), size));
                    if self.rotation != 0.0 {
                        let angle_rad = self.rotation * std::f32::consts::TAU / 360.0;
                        image = image.rotate(angle_rad, egui::Vec2::splat(0.5));
                    }

                    ui.put(absolute_rect, image);

                    // 缩略图导航：当图片大于可视区域时显示
                    let need_navigation = size.x > available.x || size.y > available.y;

                    // 拖动模式：按住空格键或鼠标中键时启用
                    if self.is_drag_mode && need_navigation {
                        ui.ctx().set_cursor_icon(egui::CursorIcon::Move);
                        if ctx.input(|i| i.pointer.button_down(egui::PointerButton::Primary)) {
                            if !self.pointer_down {
                                self.pointer_down = true;
                            } else {
                                let delta = ctx.input(|i| i.pointer.delta());
                                self.image_offset += delta;
                            }
                        } else {
                            self.pointer_down = false;
                        }
                    }

                    // 显示缩略图导航（只要需要导航就显示，不依赖拖动模式）
                    if need_navigation {
                        if let Some(ref thumb_tex) = self.thumbnail_texture {
                            let img_ratio = self.image_size.x / self.image_size.y;
                            let (thumb_w, thumb_h) = if img_ratio > 1.0 {
                                (160.0, 160.0 / img_ratio)
                            } else {
                                (120.0 * img_ratio, 120.0)
                            };
                            let thumb_size = egui::vec2(thumb_w, thumb_h);
                            let screen_rect = ui.ctx().available_rect();
                            let thumb_pos = egui::Pos2::new(
                                screen_rect.right() - thumb_size.x - 10.0,
                                screen_rect.bottom() - thumb_size.y - 10.0,
                            );
                            let thumb_rect = egui::Rect::from_min_size(thumb_pos, thumb_size);

                            let mut thumb_image = egui::Image::new((thumb_tex.id(), thumb_size));
                            if self.rotation != 0.0 {
                                let angle_rad = self.rotation * std::f32::consts::TAU / 360.0;
                                thumb_image = thumb_image.rotate(angle_rad, egui::Vec2::splat(0.5));
                            }
                            ui.put(thumb_rect, thumb_image);

                            // 计算红框：映射可视区域到缩略图
                            // 1. 使用已经计算好的缩放后图片尺寸（考虑旋转）
                            let (scaled_w, scaled_h) = if self.rotation % 180.0 == 0.0 {
                                (size.x, size.y)
                            } else {
                                (size.y, size.x)
                            };

                            // 2. 计算图片左上角相对于可视区域中心的位置
                            let image_left = center.x + self.image_offset.x - size.x / 2.0;
                            let image_top = center.y + self.image_offset.y - size.y / 2.0;

                            // 3. 计算可视区域在图片上的相对位置 (0.0 - 1.0)
                            let view_ratio_x = if scaled_w > available.x {
                                (-image_left / (scaled_w - available.x)).clamp(0.0, 1.0)
                            } else {
                                0.5 // 图片完全显示，居中
                            };
                            let view_ratio_y = if scaled_h > available.y {
                                (-image_top / (scaled_h - available.y)).clamp(0.0, 1.0)
                            } else {
                                0.5 // 图片完全显示，居中
                            };

                            // 4. 计算红框大小（可视区域占图片的比例）
                            let view_rect_w =
                                (thumb_size.x * available.x / scaled_w).min(thumb_size.x);
                            let view_rect_h =
                                (thumb_size.y * available.y / scaled_h).min(thumb_size.y);

                            // 5. 计算红框位置
                            let view_rect_x =
                                thumb_pos.x + (thumb_size.x - view_rect_w) * view_ratio_x;
                            let view_rect_y =
                                thumb_pos.y + (thumb_size.y - view_rect_h) * view_ratio_y;

                            ui.painter().rect_stroke(
                                egui::Rect::from_min_size(
                                    egui::Pos2::new(view_rect_x, view_rect_y),
                                    egui::vec2(view_rect_w, view_rect_h),
                                ),
                                2.0,
                                egui::Stroke::new(2.0, egui::Color32::RED),
                                egui::StrokeKind::Inside,
                            );
                        }
                    }
                } else {
                    egui::Area::new(egui::Id::new("welcome_area"))
                        .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
                        .show(ctx, |ui| {
                            let response = ui.label(self.t(TextKey::ClickToOpen));
                            if response.clicked() {
                                if let Some(path) = rfd::FileDialog::new()
                                    .add_filter(
                                        "Images",
                                        &[
                                            "jpg", "jpeg", "png", "gif", "webp", "bmp", "tiff",
                                            "tif", "ico", "avif",
                                        ],
                                    )
                                    .pick_file()
                                {
                                    self.load_image(&path, ctx).ok();
                                }
                            }
                        });
                }
            });

        for event in ctx.input(|i| i.events.clone()) {
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
                            self.prev_image(ctx);
                        }
                        egui::Key::ArrowRight => {
                            self.next_image(ctx);
                        }
                        egui::Key::Equals | egui::Key::Plus => {
                            self.zoom_in(self.current_scale);
                        }
                        egui::Key::Minus => {
                            self.zoom_out(self.current_scale);
                        }
                        egui::Key::Num0 => {
                            self.zoom_mode = ZoomMode::Fit;
                            self.image_offset = egui::Vec2::ZERO;
                        }
                        egui::Key::Num1 => {
                            self.zoom_mode = ZoomMode::Original;
                            self.image_offset = egui::Vec2::ZERO;
                        }
                        egui::Key::Num2 => {
                            self.zoom_mode = ZoomMode::Fill;
                            self.image_offset = egui::Vec2::ZERO;
                        }
                        egui::Key::R if modifiers.shift => {
                            self.rotate_left();
                        }
                        egui::Key::R => {
                            self.rotate_right();
                        }
                        egui::Key::F => {
                            self.toggle_fullscreen(ctx);
                        }
                        egui::Key::H => {
                            self.show_shortcuts = !self.show_shortcuts;
                        }
                        egui::Key::Space => {
                            self.is_drag_mode = true;
                        }
                        egui::Key::Escape => {
                            if self.is_fullscreen {
                                // 全屏状态下：退出全屏
                                self.toggle_fullscreen(ctx);
                            } else {
                                // 非全屏状态下：关闭程序
                                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                            }
                            // 同时关闭弹窗
                            if self.show_shortcuts {
                                self.show_shortcuts = false;
                            }
                            if self.show_settings {
                                self.show_settings = false;
                            }
                        }
                        _ => {}
                    }
                }
                if !pressed && key == egui::Key::Space {
                    self.is_drag_mode = false;
                }
            }
        }

        // Settings window
        if self.show_settings {
            // Get all text outside to avoid borrowing issues
            let lang = self.settings.language;
            let settings_text = TextKey::Settings.text(lang);
            let general_text = TextKey::General.text(lang);
            let language_text = TextKey::Language.text(lang);
            let chinese_text = TextKey::Chinese.text(lang);
            let english_text = TextKey::English.text(lang);
            let cache_text = TextKey::Cache.text(lang);
            let max_cache_text = TextKey::MaxCacheSize.text(lang);
            let show_status_text = TextKey::ShowStatusBar.text(lang);
            let reset_text = TextKey::ResetSettings.text(lang);

            // Need to capture these outside the closure
            let current_lang = self.settings.language;
            let current_max_cache = self.settings.max_cache_size;
            let current_show_status = self.settings.show_status_bar;


            egui::Window::new(settings_text)
                .open(&mut self.show_settings)
                .resizable(false)
                .fixed_size([320.0, 220.0])
                .default_pos(egui::Pos2::new(
                    ctx.available_rect().center().x - 160.0,
                    ctx.available_rect().center().y - 110.0,
                ))
                .show(ctx, |ui: &mut egui::Ui| {
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
                        self.settings.language = temp_lang;
                    }

                    ui.separator();
                    ui.heading(cache_text);
                    let slider =
                        egui::Slider::new(&mut temp_max_cache, 3..=30).text(max_cache_text);
                    ui.add(slider);

                    if temp_max_cache != current_max_cache {
                        self.settings.max_cache_size = temp_max_cache;
                    }

                    ui.separator();
                    ui.checkbox(&mut temp_show_status, show_status_text);

                    if temp_show_status != current_show_status {
                        self.settings.show_status_bar = temp_show_status;
                    }

                    ui.separator();
                    if ui.button(reset_text).clicked() {
                        self.settings = Settings::default();
                    }
                });

            // Auto save on changes
            self.save_settings();
        }

        // Shortcuts window - 卡片式设计
        if self.show_shortcuts {
            let lang = self.settings.language;
            let title = TextKey::Shortcuts.text(lang);

            egui::Window::new(title)
                .open(&mut self.show_shortcuts)
                .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
                .collapsible(false)
                .resizable(false)
                .fixed_size([380.0, 420.0])
                .title_bar(true)
                .show(ctx, |ui| {
                    let visuals = &ctx.style().visuals;
                    ui.add_space(4.0);

                    // 辅助函数：创建键盘按键样式
                    let key_badge = |ui: &mut egui::Ui, key: &str| {
                        let button_color = visuals.selection.bg_fill.gamma_multiply(0.15);
                        let border_color = visuals.selection.bg_fill.gamma_multiply(0.3);

                        egui::Frame::NONE
                            .fill(button_color)
                            .stroke(egui::Stroke::new(1.0, border_color))
                            .corner_radius(6.0)
                            .inner_margin(egui::Margin::symmetric(8, 3))
                            .show(ui, |ui| {
                                ui.label(
                                    egui::RichText::new(key)
                                        .family(egui::FontFamily::Monospace)
                                        .size(11.0)
                                        .strong(),
                                );
                            });
                    };

                    // 辅助函数：创建快捷键行
                    let shortcut_row = |ui: &mut egui::Ui, keys: &[&str], desc: &str| {
                        ui.horizontal(|ui| {
                            ui.add_space(8.0);
                            for (i, key) in keys.iter().enumerate() {
                                if i > 0 {
                                    ui.add_space(2.0);
                                }
                                key_badge(ui, key);
                            }
                            ui.add_space(12.0);
                            ui.label(
                                egui::RichText::new(desc)
                                    .size(12.0)
                                    .color(visuals.weak_text_color()),
                            );
                        });
                        ui.add_space(2.0);
                    };

                    // 分组标题
                    let section_title = |ui: &mut egui::Ui, title: &str| {
                        ui.add_space(8.0);
                        ui.label(
                            egui::RichText::new(title.to_uppercase())
                                .size(10.0)
                                .color(visuals.weak_text_color()),
                        );
                        ui.add_space(4.0);
                    };

                    // 导航部分
                    section_title(ui, "Navigation");
                    shortcut_row(ui, &["←", "→"], TextKey::PreviousNext.text(lang));
                    shortcut_row(ui, &["Space"], TextKey::DragMode.text(lang));

                    ui.separator();

                    // 缩放部分
                    section_title(ui, "Zoom & View");
                    shortcut_row(ui, &["+", "-"], TextKey::ZoomInOut.text(lang));
                    shortcut_row(ui, &["0"], TextKey::FitToWindow.text(lang));
                    shortcut_row(ui, &["1"], TextKey::OriginalSize.text(lang));
                    shortcut_row(ui, &["2"], TextKey::FillWindow.text(lang));

                    ui.separator();

                    // 旋转部分
                    section_title(ui, "Rotation");
                    shortcut_row(ui, &["R"], TextKey::RotateLeft.text(lang));
                    shortcut_row(ui, &["Shift", "R"], TextKey::RotateRight.text(lang));

                    ui.separator();

                    // 系统部分
                    section_title(ui, "System");
                    shortcut_row(ui, &["F"], TextKey::ToggleFullscreen.text(lang));
                    shortcut_row(ui, &["Esc"], TextKey::ExitFullscreen.text(lang));
                    shortcut_row(ui, &["H", "?"], TextKey::ShowHideShortcuts.text(lang));

                    ui.add_space(8.0);
                });
        }
    }
}

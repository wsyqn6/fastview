use eframe::egui;
use image::GenericImageView;
use std::path::PathBuf;
use std::sync::Arc;

use crate::types::{CachedImage, ImageCache, Language, Settings, TextKey, ZoomMode};
use crate::fonts::setup_fonts;

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
            if let Some(orientation_field) = exif.get_field(exif::Tag::Orientation, exif::In::PRIMARY) {
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
        let img = image::open(path)
            .map_err(|e| format!("Failed to open image: {}", e))?;
        
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
        
        let texture = ctx.load_texture(
            "image",
            color_image,
            egui::TextureOptions::LINEAR,
        );
        
        let thumb_size = 200;
        let thumb_img = dynamic_img.thumbnail(thumb_size, thumb_size);
        let (tw, th) = thumb_img.dimensions();
        let thumb_color_image = egui::ColorImage::from_rgba_unmultiplied(
            [tw as usize, th as usize],
            thumb_img.to_rgba8().as_raw(),
        );
        let thumbnail_texture = ctx.load_texture(
            "thumbnail",
            thumb_color_image,
            egui::TextureOptions::LINEAR,
        );
        
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
                                    "jpg" | "jpeg" | "png" | "gif" | "webp" | "bmp" | "tiff"
                                        | "tif" | "ico" | "avif"
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
}

impl eframe::App for FastViewApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Top menu bar
        egui::TopBottomPanel::top("menu_bar").show(ctx, |ui| {
            let lang = self.settings.language;
            egui::menu::bar(ui, |ui| {
                // File menu
                let file_text = TextKey::File.text(lang);
                let open_text = TextKey::OpenFile.text(lang);
                let exit_text = TextKey::Exit.text(lang);
                ui.menu_button(file_text, |ui: &mut egui::Ui| {
                    if ui.button(open_text).clicked() {
                        if let Some(path) = rfd::FileDialog::new()
                            .add_filter(
                                "Images",
                                &[
                                    "jpg", "jpeg", "png", "gif", "webp", "bmp", "tiff", "tif",
                                    "ico", "avif",
                                ],
                            )
                            .pick_file()
                        {
                            self.load_image(&path, ctx).ok();
                        }
                        ui.close();
                    }
                    if ui.button(exit_text).clicked() {
                        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                        ui.close();
                    }
                });

                // View menu
                let view_text = TextKey::View.text(lang);
                let fullscreen_text = TextKey::Fullscreen.text(lang);
                let shortcuts_text = TextKey::Shortcuts.text(lang);
                let settings_text = TextKey::Settings.text(lang);
                ui.menu_button(view_text, |ui: &mut egui::Ui| {
                    ui.checkbox(&mut self.is_fullscreen, fullscreen_text);
                    ui.separator();
                    ui.checkbox(&mut self.show_shortcuts, shortcuts_text);
                    ui.checkbox(&mut self.show_settings, settings_text);
                });

                // Window controls (right aligned)
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    // Close button (red on hover)
                    let mut close_button = egui::Button::new("×");
                    close_button = close_button.frame(true);
                    let close_response = ui.add(close_button);
                    if close_response.hovered() {
                        ui.painter().rect_filled(
                            close_response.rect,
                            0.0,
                            egui::Color32::from_rgb(232, 17, 35),
                        );
                    }
                    if close_response.clicked() {
                        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                    }

                    // Maximize button
                    let is_maximized = ctx.viewport_for(ctx.viewport_id(), |vp| vp.builder.maximized).unwrap_or(false);
                    let max_icon = if is_maximized { "◌" } else { "□" };
                    let mut max_button = egui::Button::new(max_icon);
                    max_button = max_button.frame(true);
                    let max_response = ui.add(max_button);
                    if max_response.hovered() {
                        ui.painter().rect_filled(
                            max_response.rect,
                            0.0,
                            egui::Color32::from_rgb(66, 133, 244),
                        );
                    }
                    if max_response.clicked() {
                        ctx.send_viewport_cmd(egui::ViewportCommand::Maximized(!is_maximized));
                    }

                    // Minimize button
                    let mut min_button = egui::Button::new("−");
                    min_button = min_button.frame(true);
                    let min_response = ui.add(min_button);
                    if min_response.hovered() {
                        ui.painter().rect_filled(
                            min_response.rect,
                            0.0,
                            egui::Color32::from_rgb(66, 133, 244),
                        );
                    }
                    if min_response.clicked() {
                        ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(true));
                    }
                });
            });
        });

        // 检查当前图片是否已经有高清缓存版本，如果有就更新显示
        if let Some(current_path) = &self.current_path {
            let need_update = {
                let cache_guard = self.image_cache.lock().unwrap();
                if let Some(cached) = cache_guard.get(current_path) {
                    // 如果当前纹理不是高清版本，需要更新
                    Some(cached.clone())
                } else {
                    None
                }
            };

            if let Some(cached) = need_update {
                if self.texture.as_ref().map(|t| t.id()) != Some(cached.texture.id()) {
                    self.texture = Some(cached.texture.clone());
                    self.thumbnail_texture = Some(cached.thumbnail_texture.clone());
                    self.image_size = cached.image_size;
                }
            }
        }
        
        // Status bar controlled by settings
        if self.settings.show_status_bar {
            egui::TopBottomPanel::bottom("status_bar")
                .default_height(28.0)
                .show(ctx, |ui| {
                    ui.horizontal(|ui| {
                        ui.spacing();
                        if let Some(ref path) = self.current_path {
                            let filename = path
                                .file_name()
                                .map(|s| s.to_string_lossy())
                                .unwrap_or_default();
                            ui.label(filename);
                            ui.separator();
                            ui.label(format!(
                                "{}x{}",
                                self.image_size.x as u32, self.image_size.y as u32
                            ));
                            ui.separator();
                        }
                        if !self.current_images.is_empty() {
                            ui.label(format!(
                                "{}/{}",
                                self.current_index + 1,
                                self.current_images.len()
                            ));
                            ui.separator();
                        }
                        let zoom_text = match self.zoom_mode {
                            ZoomMode::Fit => self.t(TextKey::Fit).to_string(),
                            ZoomMode::Fill => self.t(TextKey::Fill).to_string(),
                            ZoomMode::Original => self.t(TextKey::Original).to_string(),
                            ZoomMode::Custom => format!("{}%", (self.zoom * 100.0) as u32),
                        };
                        ui.label(zoom_text);
                        if self.rotation != 0.0 {
                            ui.separator();
                            ui.label(format!("{}°", self.rotation as u32));
                        }
                    });
                });
        }

        egui::CentralPanel::default().show(ctx, |ui| {
            // 处理拖拽文件
            if ui.ctx().input(|i| !i.raw.dropped_files.is_empty()) {
                let files = ui.ctx().input(|i| i.raw.dropped_files.clone());
                for file in files {
                    if let Some(path) = file.path {
                        if path.is_file() {
                            if let Some(ext) = path.extension() {
                                if let Some(ext_str) = ext.to_str() {
                                    let ext_lower = ext_str.to_lowercase();
                                    if ["jpg", "jpeg", "png", "gif", "webp", "bmp", "tiff", "tif", "ico", "avif"]
                                        .contains(&ext_lower.as_str()) {
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
                        let scale = (available.x / size.x).min(available.y / size.y).min(1.0);
                        size = size * scale;
                        self.current_scale = scale;
                    }
                    ZoomMode::Fill => {
                        let scale = (available.x / size.x).max(available.y / size.y);
                        size = size * scale;
                        self.current_scale = scale;
                    }
                    ZoomMode::Original => {
                        size = self.image_size;
                        self.current_scale = 1.0;
                    }
                    ZoomMode::Custom => {
                        size = size * self.zoom;
                        self.current_scale = self.zoom;
                    }
                }

                let avail = ui.available_size();
                let center = egui::Pos2::new(avail.x / 2.0, avail.y / 2.0);
                let rect = egui::Rect::from_center_size(center + self.image_offset, size);

                let mut image = egui::Image::new((texture.id(), size));
                if self.rotation != 0.0 {
                    let angle_rad = self.rotation * std::f32::consts::TAU / 360.0;
                    image = image.rotate(angle_rad, egui::Vec2::splat(0.5));
                }
                ui.put(rect, image);

                let show_thumbnail = self.is_drag_mode || size.x > avail.x || size.y > avail.y;

                if self.is_drag_mode && show_thumbnail {
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

                if show_thumbnail {
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

                        let (scaled_w, scaled_h) = match self.rotation % 180.0 {
                            0.0 => (self.image_size.x * self.zoom, self.image_size.y * self.zoom),
                            _ => (self.image_size.y * self.zoom, self.image_size.x * self.zoom),
                        };

                        let view_w = avail.x / scaled_w;
                        let view_h = avail.y / scaled_h;
                        let view_rect_w = thumb_size.x * view_w;
                        let view_rect_h = thumb_size.y * view_h;
                        let view_rect_x = thumb_pos.x
                            + (thumb_size.x - view_rect_w)
                                * (0.5 - self.image_offset.x / (scaled_w - avail.x))
                                    .max(0.0)
                                    .min(0.5);
                        let view_rect_y = thumb_pos.y
                            + (thumb_size.y - view_rect_h)
                                * (0.5 - self.image_offset.y / (scaled_h - avail.y))
                                    .max(0.0)
                                    .min(0.5);
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
                                        "jpg", "jpeg", "png", "gif", "webp", "bmp", "tiff", "tif",
                                        "ico", "avif",
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
                            self.is_fullscreen = !self.is_fullscreen;
                        }
                        egui::Key::H => {
                            self.show_shortcuts = !self.show_shortcuts;
                        }
                        egui::Key::Space => {
                            self.is_drag_mode = true;
                        }
                        egui::Key::Escape => {
                            if self.is_fullscreen {
                                self.is_fullscreen = false;
                            }
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

            let settings_text = settings_text;
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

        // Shortcuts window
        if self.show_shortcuts {
            let lang = self.settings.language;
            let title = TextKey::Shortcuts.text(lang);
            let prev_next = TextKey::PreviousNext.text(lang);
            let zoom = TextKey::ZoomInOut.text(lang);
            let fit = TextKey::FitToWindow.text(lang);
            let original = TextKey::OriginalSize.text(lang);
            let fill = TextKey::FillWindow.text(lang);
            let rot_left = TextKey::RotateLeft.text(lang);
            let rot_right = TextKey::RotateRight.text(lang);
            let fullscreen = TextKey::ToggleFullscreen.text(lang);
            let drag = TextKey::DragMode.text(lang);
            let exit_full = TextKey::ExitFullscreen.text(lang);
            let show_hide = TextKey::ShowHideShortcuts.text(lang);
            let file_open = format!(
                "{}: {}",
                TextKey::File.text(lang),
                TextKey::OpenFile.text(lang)
            );

            egui::Window::new(title)
                .open(&mut self.show_shortcuts)
                .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
                .collapsible(false)
                .resizable(false)
                .fixed_size([320.0, 320.0])
                .show(ctx, |ui| {
                    ui.label(format!("←/→: {}", prev_next));
                    ui.label(format!("+/-: {}", zoom));
                    ui.label(format!("0: {}", fit));
                    ui.label(format!("1: {}", original));
                    ui.label(format!("2: {}", fill));
                    ui.label(format!("r: {}", rot_left));
                    ui.label(format!("Shift+R: {}", rot_right));
                    ui.label(format!("f: {}", fullscreen));
                    ui.label(format!("Space: {}", drag));
                    ui.label(format!("Esc: {}", exit_full));
                    ui.label(format!("H/?: {}", show_hide));
                    ui.label(file_open);
                });
        }
    }
}

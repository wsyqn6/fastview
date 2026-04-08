use eframe::egui;
use image::GenericImageView;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::mpsc::{self, Receiver};

#[derive(Debug, Clone, PartialEq)]
enum WindowType {
    Shortcuts,
    Settings,
    About,
}

#[derive(Clone)]
struct DirectoryCache {
    path: PathBuf,
    images: Vec<PathBuf>,
    timestamp: std::time::Instant,
}

/// 后台解码后的图片数据
struct DecodedImageData {
    path: PathBuf,
    rgba_data: Vec<u8>,
    width: u32,
    height: u32,
    thumbnail_rgba: Vec<u8>,
    thumb_width: u32,
    thumb_height: u32,
}

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
    pub is_borderless: bool,  // 无边框模式
    pub current_scale: f32,
    pub image_cache: ImageCache,
    pub settings: Settings,
    pub show_settings: bool,
    pub file_size: u64,   // 文件大小（字节）
    pub show_about: bool, // 控制"关于"对话框显示

    // 窗口打开顺序栈，用于ESC键后开先关逻辑
    window_stack: Vec<WindowType>,
    
    // 目录缓存，避免频繁扫描文件系统
    dir_cache: Option<DirectoryCache>,
    
    // 异步加载通道
    image_rx: Option<Receiver<DecodedImageData>>,
}

impl Default for FastViewApp {
    fn default() -> Self {
        use lru::LruCache;
        
        let (_tx, rx) = mpsc::channel();
        
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
            is_borderless: false,
            current_scale: 1.0,
            image_cache: Arc::new(std::sync::Mutex::new(LruCache::new(10.try_into().unwrap()))),
            settings: Settings::default(),
            show_settings: false,
            file_size: 0,
            show_about: false,
            window_stack: Vec::new(),
            dir_cache: None,
            image_rx: Some(rx),
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

    pub fn save_settings(&mut self) {
        if let Some(config_dir) = dirs::config_dir() {
            let config_path = config_dir.join("fastview").join("settings.json");
            if let Some(parent) = config_path.parent() {
                std::fs::create_dir_all(parent).ok();
            }
            if let Ok(content) = serde_json::to_string_pretty(&self.settings) {
                std::fs::write(&config_path, content).ok();
            }
        }
        
        // 更新 LRU 缓存大小
        let new_capacity = self.settings.max_cache_size;
        if let Ok(mut cache) = self.image_cache.lock() {
            // LRU 不支持动态调整大小，需要重建
            let mut new_cache = lru::LruCache::new(new_capacity.try_into().unwrap());
            // 保留最近的项目
            for (key, value) in cache.iter() {
                new_cache.put(key.clone(), value.clone());
            }
            *cache = new_cache;
        }
    }

    fn t(&self, key: TextKey) -> &'static str {
        key.text(self.settings.language)
    }

    /// 获取应用版本号
    fn get_version(&self) -> &'static str {
        env!("CARGO_PKG_VERSION")
    }

    // 格式化文件大小
    fn format_file_size(&self, bytes: u64) -> String {
        if bytes < 1024 {
            format!("{} B", bytes)
        } else if bytes < 1024 * 1024 {
            format!("{:.1} KB", bytes as f64 / 1024.0)
        } else if bytes < 1024 * 1024 * 1024 {
            format!("{:.1} MB", bytes as f64 / (1024.0 * 1024.0))
        } else {
            format!("{:.1} GB", bytes as f64 / (1024.0 * 1024.0 * 1024.0))
        }
    }

    pub fn load_image(&mut self, path: &PathBuf, ctx: &egui::Context) -> Result<(), String> {
        // 优化1: 优先检查缓存，避免重复解码
        if let Some(cached) = {
            let mut cache_guard = self.image_cache.lock().unwrap();
            cache_guard.get(path).cloned()
        } {
            // 缓存命中，直接应用（微秒级）
            self.apply_cached_image(cached, path);
            return Ok(());
        }
        
        // 缓存未命中，启动异步加载
        self.load_image_async(path, ctx);
        Ok(())
    }
    
    /// 应用已缓存的图片
    fn apply_cached_image(&mut self, cached: Arc<CachedImage>, path: &PathBuf) {
        self.texture = Some(cached.texture.clone());
        self.thumbnail_texture = Some(cached.thumbnail_texture.clone());
        self.image_size = cached.image_size;
        self.current_path = Some(path.clone());
        self.zoom_mode = ZoomMode::Fit;
        self.zoom = 1.0;
        self.rotation = 0.0;
        self.image_offset = egui::Vec2::ZERO;
        self.file_size = std::fs::metadata(path).map(|m| m.len()).unwrap_or(0);
        
        // 更新目录列表（如果需要）
        self.update_directory_list(path);
    }
    
    /// 异步加载图片（渐进式加载）
    fn load_image_async(&mut self, path: &PathBuf, ctx: &egui::Context) {
        // 1. 立即设置当前路径，显示加载状态
        self.current_path = Some(path.clone());
        self.file_size = std::fs::metadata(path).map(|m| m.len()).unwrap_or(0);
        
        // 2. 尝试快速生成缩略图用于即时显示
        if let Some(thumb_tex) = self.generate_quick_thumbnail(path, ctx) {
            self.thumbnail_texture = Some(thumb_tex);
            ctx.request_repaint();
        }
        
        // 3. 启动后台线程解码图片
        let path_clone = path.clone();
        
        // 创建新的通道用于接收解码结果
        let (tx, rx) = mpsc::channel();
        self.image_rx = Some(rx);
        
        std::thread::spawn(move || {
            if let Ok(decoded) = Self::decode_image_raw(&path_clone) {
                let _ = tx.send(decoded);
            }
        });
        
        // 4. 更新目录列表
        self.update_directory_list(path);
    }
    
    /// 在后台线程解码图片（只解码，不创建纹理）
    fn decode_image_raw(path: &PathBuf) -> Result<DecodedImageData, String> {
        // 加载图片
        let img = image::open(path).map_err(|e| format!("Failed to open: {}", e))?;
        
        // 读取 EXIF 方向信息
        let orientation = read_exif_orientation(path);
        
        // 应用方向转换
        let mut dynamic_img = img;
        dynamic_img.apply_orientation(orientation);
        
        let (width, height) = dynamic_img.dimensions();
        let rgba_data = dynamic_img.to_rgba8().into_raw();
        
        // 生成缩略图
        let thumb_size = 200;
        let thumb_img = dynamic_img.thumbnail(thumb_size, thumb_size);
        let (thumb_width, thumb_height) = thumb_img.dimensions();
        let thumbnail_rgba = thumb_img.to_rgba8().into_raw();
        
        Ok(DecodedImageData {
            path: path.clone(),
            rgba_data,
            width,
            height,
            thumbnail_rgba,
            thumb_width,
            thumb_height,
        })
    }
    
    /// 在主线程从解码数据创建纹理
    fn create_texture_from_decoded(&mut self, decoded: DecodedImageData, ctx: &egui::Context) {
        let image_size = egui::vec2(decoded.width as f32, decoded.height as f32);
        
        // 创建主纹理
        let color_image = egui::ColorImage::from_rgba_unmultiplied(
            [decoded.width as usize, decoded.height as usize],
            &decoded.rgba_data,
        );
        let texture = ctx.load_texture("image", color_image, egui::TextureOptions::LINEAR);
        
        // 创建缩略图纹理
        let thumb_color_image = egui::ColorImage::from_rgba_unmultiplied(
            [decoded.thumb_width as usize, decoded.thumb_height as usize],
            &decoded.thumbnail_rgba,
        );
        let thumbnail_texture = ctx.load_texture("thumbnail", thumb_color_image, egui::TextureOptions::LINEAR);
        
        let cached = Arc::new(CachedImage {
            texture: texture.clone(),
            thumbnail_texture: thumbnail_texture.clone(),
            image_size,
        });
        
        // 存入缓存
        {
            let mut cache_guard = self.image_cache.lock().unwrap();
            cache_guard.put(decoded.path.clone(), cached);
        }
        
        // 应用到当前显示
        self.texture = Some(texture);
        self.thumbnail_texture = Some(thumbnail_texture);
        self.image_size = image_size;
        self.zoom_mode = ZoomMode::Fit;
        self.zoom = 1.0;
        self.rotation = 0.0;
        self.image_offset = egui::Vec2::ZERO;
        
        // 预加载相邻图片
        self.preload_adjacent_images(ctx);
    }
    
    /// 快速生成缩略图（用于即时显示）
    fn generate_quick_thumbnail(&self, path: &PathBuf, ctx: &egui::Context) -> Option<egui::TextureHandle> {
        // 尝试快速读取并生成小尺寸缩略图
        if let Ok(img) = image::open(path) {
            let thumb_size = 100; // 更小的尺寸用于快速显示
            let thumb_img = img.thumbnail(thumb_size, thumb_size);
            let (tw, th) = thumb_img.dimensions();
            
            let thumb_color_image = egui::ColorImage::from_rgba_unmultiplied(
                [tw as usize, th as usize],
                thumb_img.to_rgba8().as_raw(),
            );
            
            Some(ctx.load_texture("quick_thumb", thumb_color_image, egui::TextureOptions::LINEAR))
        } else {
            None
        }
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
    
    /// 更新目录列表（带缓存优化）
    fn update_directory_list(&mut self, path: &PathBuf) {
        if let Some(parent) = path.parent() {
            // 检查目录缓存是否有效（5秒内不重新扫描）
            let need_rescan = if let Some(dir_cache) = &self.dir_cache {
                dir_cache.path != parent || dir_cache.timestamp.elapsed().as_secs() >= 5
            } else {
                true
            };
            
            if need_rescan {
                // 执行扫描并缓存结果
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
                
                // 更新缓存
                self.dir_cache = Some(DirectoryCache {
                    path: parent.to_path_buf(),
                    images: images.clone(),
                    timestamp: std::time::Instant::now(),
                });
                
                // 更新当前图片列表
                if let Some(pos) = images.iter().position(|p| p == path) {
                    self.current_images = images;
                    self.current_index = pos;
                }
            } else {
                // 使用缓存的目录列表
                if let Some(dir_cache) = &self.dir_cache {
                    if let Some(pos) = dir_cache.images.iter().position(|p| p == path) {
                        self.current_images = dir_cache.images.clone();
                        self.current_index = pos;
                    }
                }
            }
        }
    }
    
    /// 预加载相邻图片（优化切换速度）
    fn preload_adjacent_images(&mut self, _ctx: &egui::Context) {
        if self.current_images.is_empty() {
            return;
        }
        
        let current_idx = self.current_index;
        let total = self.current_images.len();
        
        // 预加载前后各一张图片
        for offset in [-1isize, 1] {
            let next_idx = if offset < 0 {
                if current_idx > 0 { current_idx - 1 } else { total - 1 }
            } else {
                if current_idx < total - 1 { current_idx + 1 } else { 0 }
            };
            
            let next_path = self.current_images[next_idx].clone();
            
            // 检查是否已在缓存中
            let already_cached = {
                let cache = self.image_cache.lock().unwrap();
                cache.contains(&next_path)
            };
            
            if !already_cached {
                // 异步预加载（只解码，触发操作系统文件缓存）
                
                std::thread::spawn(move || {
                    if let Ok(img) = image::open(&next_path) {
                        let orientation = read_exif_orientation(&next_path);
                        let mut dynamic_img = img;
                        dynamic_img.apply_orientation(orientation);
                        
                        let (_width, _height) = dynamic_img.dimensions();
                        let _rgba_data = dynamic_img.to_rgba8().into_raw();
                        
                        // 生成缩略图
                        let thumb = dynamic_img.thumbnail(200, 200);
                        let _thumb_rgba = thumb.to_rgba8().into_raw();
                        
                        // 注意：由于纹理创建需要 egui::Context，我们不能在后台线程创建
                        // 所以这里只预解码数据，当用户切换到此图片时会从磁盘快速加载
                        // 这是一个折中方案，避免了复杂的跨线程纹理传递
                        
                        // 记录预加载完成（可选：可以添加一个标志位）
                        eprintln!("[PRELOAD] Decoded: {:?}", next_path.file_name());
                    }
                });
            }
        }
    }

    pub fn toggle_fullscreen(&mut self, ctx: &egui::Context) {
        self.is_fullscreen = !self.is_fullscreen;
        ctx.send_viewport_cmd(egui::ViewportCommand::Fullscreen(self.is_fullscreen));
    }

    pub fn toggle_borderless(&mut self, ctx: &egui::Context) {
        self.is_borderless = !self.is_borderless;
        ctx.send_viewport_cmd(egui::ViewportCommand::Decorations(!self.is_borderless));
    }
}

impl eframe::App for FastViewApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        // 全屏或无边框模式时隐藏菜单栏，提升沉浸体验
        if !self.is_fullscreen && !self.is_borderless {
            // 传统菜单栏（类似 Windows 原生应用）
            egui::Panel::top("menu_bar")
                .exact_size(24.0)
                .show_inside(ui, |ui| {
                    ui.horizontal(|ui| {
                        // 文件菜单
                        ui.menu_button(self.t(TextKey::MenuFile), |ui| {
                            if ui.button(self.t(TextKey::OpenFile)).clicked() {
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
                                    self.load_image(&path, ui.ctx()).ok();
                                }
                                ui.close();
                            }
                            ui.separator();
                            if ui.button(self.t(TextKey::Exit)).clicked() {
                                ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
                                ui.close();
                            }
                        });

                        // 查看菜单
                        ui.menu_button(self.t(TextKey::MenuView), |ui| {
                            // 缩放模式
                            if ui.button(self.t(TextKey::FitToWindow)).clicked() {
                                self.zoom_mode = ZoomMode::Fit;
                                self.image_offset = egui::Vec2::ZERO;
                                ui.close();
                            }
                            if ui.button(self.t(TextKey::OriginalSize)).clicked() {
                                self.zoom_mode = ZoomMode::Original;
                                self.image_offset = egui::Vec2::ZERO;
                                ui.close();
                            }
                            if ui.button(self.t(TextKey::FillWindow)).clicked() {
                                self.zoom_mode = ZoomMode::Fill;
                                self.image_offset = egui::Vec2::ZERO;
                                ui.close();
                            }
                            ui.separator();
                            // 缩放操作
                            if ui.button(self.t(TextKey::ZoomIn)).clicked() {
                                self.zoom_in(self.current_scale);
                                ui.close();
                            }
                            if ui.button(self.t(TextKey::ZoomOut)).clicked() {
                                self.zoom_out(self.current_scale);
                                ui.close();
                            }
                            ui.separator();
                            // 旋转
                            if ui.button(self.t(TextKey::RotateClockwise)).clicked() {
                                self.rotate_right();
                                ui.close();
                            }
                            if ui.button(self.t(TextKey::RotateCounterClockwise)).clicked() {
                                self.rotate_left();
                                ui.close();
                            }
                            ui.separator();
                            // 全屏
                            if ui.button(self.t(TextKey::ToggleFullscreen)).clicked() {
                                self.toggle_fullscreen(ui.ctx());
                                ui.close();
                            }
                            // 无边框模式
                            if ui.button(self.t(TextKey::ToggleBorderless)).clicked() {
                                self.toggle_borderless(ui.ctx());
                                ui.close();
                            }
                        });

                        // 设置按钮（直接点击打开，无需下拉）
                        if ui.button(self.t(TextKey::MenuSettings)).clicked() {
                            self.show_settings = true;
                            self.window_stack.push(WindowType::Settings);
                        }

                        // 帮助菜单
                        ui.menu_button(self.t(TextKey::MenuHelp), |ui| {
                            if ui.button(self.t(TextKey::ShortcutsHelp)).clicked() {
                                self.show_shortcuts = !self.show_shortcuts;
                                if self.show_shortcuts {
                                    self.window_stack.push(WindowType::Shortcuts);
                                } else {
                                    // 如果关闭了快捷键窗口，从栈中移除
                                    self.window_stack.retain(|w| w != &WindowType::Shortcuts);
                                }
                                ui.close();
                            }
                            if ui.button(self.t(TextKey::AboutFastView)).clicked() {
                                self.show_about = true;
                                self.window_stack.push(WindowType::About);
                                ui.close();
                            }
                            ui.separator();
                            // 检查更新（禁用状态，预留接口）
                            ui.add_enabled_ui(false, |ui| {
                                let _ = ui.button(self.t(TextKey::CheckForUpdates));
                            });
                        });
                    });
                });
        }

        // 检查异步加载完成的图片
        if let Some(rx) = &self.image_rx {
            if let Ok(decoded) = rx.try_recv() {
                self.create_texture_from_decoded(decoded, ui.ctx());
                ui.ctx().request_repaint();
            }
        }

        // 检查当前图片是否已经有高清缓存版本，如果有就更新显示
        if let Some(current_path) = &self.current_path {
            let need_update = {
                let mut cache_guard = self.image_cache.lock().unwrap();
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

        // Status bar - 悬浮半透明设计
        if self.settings.show_status_bar && !self.is_fullscreen {
            let screen_rect = ui.ctx().content_rect();
            let status_height = 28.0;

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
                        .inner_margin(egui::Margin::symmetric(14, 3)) // 左右14px, 上下3px
                        .show(ui, |ui| {
                            // 计算最大宽度限制（避免过宽）
                            let max_width = (screen_rect.width() * 0.9).min(800.0);
                            ui.set_max_width(max_width);
                            ui.set_min_height(status_height - 12.0);

                            // 使用 horizontal 布局，内容自适应宽度
                            // Area 已通过 anchor 居中，所以内容会自动居中显示
                            ui.horizontal(|ui| {
                                render_status_content(ui, visuals, self);
                            });
                        });
                });
        }

        // CentralPanel：深色背景突出图片
        egui::CentralPanel::default()
            .frame(
                egui::Frame::NONE.fill(egui::Color32::from_rgba_unmultiplied(30, 30, 30, 255)), // 深灰色背景
            )
            .show_inside(ui, |ui| {
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
                                            self.load_image(&path, ui.ctx()).ok();
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
                        let is_pressed = ui.ctx().input(|i| i.pointer.button_down(egui::PointerButton::Primary));
                        
                        // 注意：在 Windows 上，Grab/Grabbing 光标可能不被原生支持
                        // 如果图标没变，可以尝试以下替代方案：
                        // 1. AllScroll - 四向箭头（当前默认）
                        // 2. Grab/Grabbing - 抓手图标（可能需要自定义光标文件）
                        // 3. Move - 移动图标
                        
                        let cursor_icon = if is_pressed {
                            egui::CursorIcon::Grabbing  // 按下时的闭手图标
                        } else {
                            egui::CursorIcon::Grab      // 未按下时的抓手图标
                        };
                        
                        ui.ctx().set_cursor_icon(cursor_icon);
                        
                        if is_pressed {
                            if !self.pointer_down {
                                self.pointer_down = true;
                            } else {
                                let delta = ui.ctx().input(|i| i.pointer.delta());
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
                            let screen_rect = ui.ctx().content_rect();
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
                        .show(ui.ctx(), |ui| {
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
                                    self.load_image(&path, ui.ctx()).ok();
                                }
                            }
                        });
                }
            });

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
                            self.prev_image(ui.ctx());
                        }
                        egui::Key::ArrowRight => {
                            self.next_image(ui.ctx());
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
                            self.toggle_fullscreen(ui.ctx());
                        }
                        egui::Key::V => {
                            self.toggle_borderless(ui.ctx());
                        }
                        egui::Key::H => {
                            self.show_shortcuts = !self.show_shortcuts;
                            if self.show_shortcuts {
                                self.window_stack.push(WindowType::Shortcuts);
                            } else {
                                // 如果关闭了快捷键窗口，从栈中移除
                                self.window_stack.retain(|w| w != &WindowType::Shortcuts);
                            }
                        }
                        egui::Key::Space => {
                            self.is_drag_mode = true;
                        }
                        egui::Key::Escape => {
                            // 后开先关原则：关闭最后打开的窗口
                            if let Some(window_type) = self.window_stack.pop() {
                                match window_type {
                                    WindowType::Shortcuts => self.show_shortcuts = false,
                                    WindowType::Settings => self.show_settings = false,
                                    WindowType::About => self.show_about = false,
                                }
                            }
                            // 如果没有打开的窗口，则退出全屏或直接退出程序
                            else if self.is_fullscreen {
                                self.toggle_fullscreen(ui.ctx());
                            } else {
                                // 直接退出程序
                                ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
                            }
                        }
                        _ => {}
                    }
                }
                if !pressed && key == egui::Key::Space {
                    self.is_drag_mode = false;
                }
            }
            
            // 处理 ? 键 (Shift+/)
            if let egui::Event::Text(text) = event {
                if text == "?" {
                    self.show_shortcuts = !self.show_shortcuts;
                    if self.show_shortcuts {
                        self.window_stack.push(WindowType::Shortcuts);
                    } else {
                        self.window_stack.retain(|w| w != &WindowType::Shortcuts);
                    }
                }
            }
        }

        // Settings window
        if self.show_settings {
            // Get all text outside to avoid borrowing issues
            let lang = self.settings.language;
            let settings_text = self.t(TextKey::MenuSettings);
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

            // Settings window - 卡片式设计，无标题栏
            egui::Window::new(settings_text)
                .open(&mut self.show_settings)
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
            let title = self.t(TextKey::ShortcutsHelp);

            // 提前获取所有翻译文本，避免在闭包中借用 self
            let navigation_text = TextKey::Navigation.text(lang);
            let zoom_view_text = TextKey::ZoomAndView.text(lang);
            let rotation_text = TextKey::Rotation.text(lang);
            let system_text = TextKey::System.text(lang);

            egui::Window::new(title)
                .open(&mut self.show_shortcuts)
                .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
                .collapsible(false)
                .resizable(false)
                .fixed_size([380.0, 420.0])
                .title_bar(true)
                .show(ui.ctx(), |ui| {
                    let visuals = &ui.ctx().global_style().visuals;
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
                    section_title(ui, navigation_text);
                    shortcut_row(ui, &["←", "→"], TextKey::PreviousNext.text(lang));
                    shortcut_row(ui, &["Space"], TextKey::DragMode.text(lang));

                    ui.separator();

                    // 缩放部分
                    section_title(ui, zoom_view_text);
                    shortcut_row(ui, &["+", "-"], TextKey::ZoomInOut.text(lang));
                    shortcut_row(ui, &["0"], TextKey::FitToWindow.text(lang));
                    shortcut_row(ui, &["1"], TextKey::OriginalSize.text(lang));
                    shortcut_row(ui, &["2"], TextKey::FillWindow.text(lang));

                    ui.separator();

                    // 旋转部分
                    section_title(ui, rotation_text);
                    shortcut_row(ui, &["R"], TextKey::RotateLeft.text(lang));
                    shortcut_row(ui, &["Shift", "R"], TextKey::RotateRight.text(lang));

                    ui.separator();

                    // 系统部分
                    section_title(ui, system_text);
                    shortcut_row(ui, &["F"], TextKey::ToggleFullscreen.text(lang));
                    shortcut_row(ui, &["V"], TextKey::ToggleBorderless.text(lang));
                    shortcut_row(ui, &["Esc"], TextKey::ExitFullscreen.text(lang));
                    shortcut_row(ui, &["H", "?"], TextKey::ShowHideShortcuts.text(lang));

                    ui.add_space(8.0);
                });
        }

        // About dialog
        if self.show_about {
            let version = self.get_version();
            let title = self.t(TextKey::AboutFastView);
            let version_label = self.t(TextKey::Version);
            let github_label = self.t(TextKey::GitHub);
            let ok_text = self.t(TextKey::OK);
            let description = self.t(TextKey::AppDescription);

            egui::Window::new(title)
                .open(&mut self.show_about)
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
    }
}

/// 渲染状态栏内容的辅助函数
fn render_status_content(ui: &mut egui::Ui, visuals: &egui::Visuals, app: &FastViewApp) {
    // 文件名（加粗，带最大宽度限制）
    if let Some(ref path) = app.current_path {
        let filename = path
            .file_name()
            .map(|s| s.to_string_lossy())
            .unwrap_or_default();

        // 限制文件名最大宽度为 150px，超出部分显示省略号
        ui.add_sized(
            [150.0, 16.0],
            egui::Label::new(egui::RichText::new(filename).strong().size(12.0)).truncate(),
        );

        // 自定义分隔符
        ui.add_space(8.0);
        ui.separator();
        ui.add_space(8.0);

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

        ui.add_space(8.0);
        ui.separator();
        ui.add_space(8.0);
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
        ui.add_space(8.0);
        ui.separator();
        ui.add_space(8.0);
    }

    // 缩放模式（使用徽章样式）
    let zoom_text = match app.zoom_mode {
        ZoomMode::Fit => app.t(TextKey::Fit).to_string(),
        ZoomMode::Fill => app.t(TextKey::Fill).to_string(),
        ZoomMode::Original => app.t(TextKey::Original).to_string(),
        ZoomMode::Custom => format!("{}%", (app.zoom * 100.0) as u32),
    };

    // 根据缩放模式使用不同的视觉样式
    if app.zoom_mode == ZoomMode::Custom {
        // Custom 模式：使用醒目的橙色/金色徽章
        let badge_bg = egui::Color32::from_rgb(255, 165, 0).gamma_multiply(0.2); // 橙色背景 20% 透明度
        let badge_text = egui::Color32::from_rgb(255, 140, 0); // 深橙色文字
        egui::Frame::NONE
            .fill(badge_bg)
            .corner_radius(4.0)
            .inner_margin(egui::Margin::symmetric(6, 2))
            .show(ui, |ui| {
                ui.label(
                    egui::RichText::new(&zoom_text)
                        .size(10.0)
                        .strong() // 加粗增强可读性
                        .color(badge_text),
                );
            });
    } else {
        // 标准模式：使用弱文本颜色
        ui.label(
            egui::RichText::new(&zoom_text)
                .size(10.0)
                .color(visuals.weak_text_color()),
        );
    }

    // 旋转角度
    if app.rotation != 0.0 {
        ui.add_space(8.0);
        ui.separator();
        ui.add_space(8.0);
        ui.label(
            egui::RichText::new(format!("{}°", app.rotation as u32))
                .size(10.0)
                .color(visuals.weak_text_color()),
        );
    }

    // 文件大小显示
    if app.file_size > 0 {
        ui.add_space(8.0);
        ui.separator();
        ui.add_space(8.0);

        let size_text = app.format_file_size(app.file_size);
        ui.label(
            egui::RichText::new(size_text)
                .size(10.0)
                .family(egui::FontFamily::Monospace)
                .color(visuals.weak_text_color()),
        );
    }
    
    // 缓存信息（开发阶段显示，便于调试）
    if let Ok(cache) = app.image_cache.lock() {
        let cache_count = cache.len();
        if cache_count > 0 {
            ui.add_space(8.0);
            ui.separator();
            ui.add_space(8.0);
            
            // 计算总内存占用
            let total_memory: usize = cache.iter()
                .map(|(_, entry)| entry.estimated_memory_bytes())
                .sum();
            
            let memory_mb = total_memory as f64 / (1024.0 * 1024.0);
            let cache_text = format!("缓存: {} ({:.1}MB)", cache_count, memory_mb);
            ui.label(
                egui::RichText::new(cache_text)
                    .size(10.0)
                    .color(visuals.weak_text_color()),
            );
        }
    }
}

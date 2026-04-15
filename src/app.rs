use eframe::egui;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;

use crate::fonts::setup_fonts;
use crate::i18n::TextKey;
use crate::loader::{ImageLoader, LoadCommand, LoadPriority, LoadResult};
use crate::types::{CacheEntry, ImageCache, Language, Settings, ZoomMode};

// 全局启动时间（用于相对时间日志）
static START_TIME: once_cell::sync::Lazy<Instant> = once_cell::sync::Lazy::new(Instant::now);

fn elapsed_ms() -> u64 {
    START_TIME.elapsed().as_millis() as u64
}

#[derive(Debug, Clone, PartialEq)]
enum WindowType {
    Shortcuts,
    Settings,
    About,
}

#[derive(Clone)]
struct DirectoryCache {
    images: Vec<PathBuf>,
}

pub struct FastViewApp {
    pub texture: Option<egui::TextureHandle>,
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
    pub is_borderless: bool, // 无边框模式
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

    // 新的加载器相关字段
    cmd_tx: Option<std::sync::mpsc::Sender<LoadCommand>>,
    result_rx: Option<std::sync::mpsc::Receiver<LoadResult>>,
    loader_handle: Option<std::thread::JoinHandle<()>>,

    // UI 自动隐藏逻辑
    last_mouse_move: std::time::Instant,
    is_ui_visible: bool, // 控制全屏下菜单栏和状态栏的可见性
}

impl Default for FastViewApp {
    fn default() -> Self {
        use lru::LruCache;

        Self {
            texture: None,
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
            image_cache: Arc::new(std::sync::Mutex::new(LruCache::new(5.try_into().unwrap()))),
            settings: Settings::default(),
            show_settings: false,
            file_size: 0,
            show_about: false,
            window_stack: Vec::new(),
            dir_cache: None,
            cmd_tx: None,
            result_rx: None,
            loader_handle: None,
            last_mouse_move: Instant::now(),
            is_ui_visible: true,
        }
    }
}

impl FastViewApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        setup_fonts(cc);

        let mut app = Self::default();
        app.load_settings();
        app.start_loader();
        app
    }

    fn start_loader(&mut self) {
        let (result_tx, result_rx) = std::sync::mpsc::channel();
        let (loader, cmd_tx) = ImageLoader::new(result_tx);

        self.loader_handle = Some(std::thread::spawn(move || {
            loader.run();
        }));
        self.cmd_tx = Some(cmd_tx);
        self.result_rx = Some(result_rx);
    }

    fn load_settings(&mut self) {
        if let Some(config_dir) = dirs::config_dir() {
            let config_path = config_dir.join("fastview").join("settings.json");
            if config_path.exists()
                && let Ok(content) = std::fs::read_to_string(&config_path)
                && let Ok(settings) = serde_json::from_str(&content)
            {
                self.settings = settings;
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
            format!("{:.0} KB", bytes as f64 / 1024.0)
        } else if bytes < 1024 * 1024 * 1024 {
            format!("{:.1} MB", bytes as f64 / (1024.0 * 1024.0))
        } else {
            format!("{:.1} GB", bytes as f64 / (1024.0 * 1024.0 * 1024.0))
        }
    }



    /// 按需生成导航缩略图纹理
    fn get_or_create_nav_thumbnail(&mut self, ui: &mut egui::Ui) -> Option<egui::TextureHandle> {
        // 检查是否已有缓存的缩略图纹理（通过当前路径判断）
        if let Some(ref path) = self.current_path {
            // 尝试从缓存中获取图片数据
            if let Some(cached) = {
                let mut cache_guard = self.image_cache.lock().unwrap();
                cache_guard.get(path).cloned()
            } {
                let crate::types::CacheEntry::Decoded(image) = cached;
                use image::imageops::thumbnail;

                // 缩略图最大尺寸
                let max_thumb_size = 120;

                // 计算缩略图尺寸（保持宽高比）
                let scale = (max_thumb_size as f32 / image.width.max(image.height) as f32).min(1.0);
                let thumb_w = (image.width as f32 * scale) as u32;
                let thumb_h = (image.height as f32 * scale) as u32;

                // 生成缩略图
                if let Some(img) = image::RgbaImage::from_raw(image.width, image.height, image.data.clone()) {
                    let thumb_img = thumbnail(&img, thumb_w, thumb_h);

                    // 创建纹理
                    let thumb_texture_id = format!("nav_thumb_{:?}", path.file_name());
                    let color_image = egui::ColorImage::from_rgba_unmultiplied(
                        [thumb_w as usize, thumb_h as usize],
                        thumb_img.as_raw(),
                    );
                    return Some(ui.ctx().load_texture(&thumb_texture_id, color_image, egui::TextureOptions::LINEAR));
                }
            }
        }
        None
    }

    pub fn load_image(&mut self, path: &PathBuf, ctx: &egui::Context) -> Result<(), String> {
        // 1. 优先检查缓存
        if let Some(cached) = {
            let mut cache_guard = self.image_cache.lock().unwrap();
            cache_guard.get(path).cloned()
        } {
            // 缓存命中,立即应用
            self.apply_cached_entry(cached, path, ctx);
            return Ok(());
        }

        // 2. 缓存未命中,异步加载
        self.load_image_async(path, ctx);
        Ok(())
    }

    /// 应用缓存条目
    fn apply_cached_entry(&mut self, entry: CacheEntry, path: &PathBuf, ctx: &egui::Context) {
        eprintln!(
            "[{:.3}s] [APP] 缓存命中: {:?}",
            elapsed_ms() as f64 / 1000.0,
            path.file_name()
        );

        self.current_path = Some(path.clone());
        self.file_size = std::fs::metadata(path).map(|m| m.len()).unwrap_or(0);

        let CacheEntry::Decoded(image) = entry;

        // 从解码数据创建纹理（使用唯一ID避免冲突）
        let image_size = egui::vec2(image.width as f32, image.height as f32);
        let texture_id = format!("image_{:?}", path.file_name());
        let color_image = egui::ColorImage::from_rgba_unmultiplied(
            [image.width as usize, image.height as usize],
            &image.data,
        );
        let texture = ctx.load_texture(&texture_id, color_image, egui::TextureOptions::LINEAR);

        eprintln!(
            "[{:.3}s] [APP] 缓存纹理创建完成: {}",
            elapsed_ms() as f64 / 1000.0,
            texture_id
        );

        // 显示图片
        self.texture = Some(texture);
        self.image_size = image_size;

        self.zoom_mode = ZoomMode::Fit;
        self.zoom = 1.0;
        self.rotation = 0.0;
        self.image_offset = egui::Vec2::ZERO;

        // 更新目录列表
        self.update_directory_list(path);

        // 激进的内存清理：如果缓存超过5张，移除最旧的
        if let Ok(mut cache) = self.image_cache.lock() {
            while cache.len() > 5 {
                cache.pop_lru();
            }
        }
    }

    /// 异步加载图片(使用专用后台线程)
    fn load_image_async(&mut self, path: &PathBuf, _ctx: &egui::Context) {
        eprintln!(
            "[{:.3}s] [APP] 请求加载: {:?}",
            elapsed_ms() as f64 / 1000.0,
            path.file_name()
        );

        // 1. 立即设置路径
        self.current_path = Some(path.clone());
        self.file_size = std::fs::metadata(path).map(|m| m.len()).unwrap_or(0);

        // 2. 清除旧纹理，避免显示上一张图片
        let old_texture = self.texture.take();
        drop(old_texture);

        // 3. 发送高清图加载请求（直接加载完整尺寸）
        if let Some(ref tx) = self.cmd_tx {
            eprintln!(
                "[{:.3}s] [APP] 发送加载请求 (高清)",
                elapsed_ms() as f64 / 1000.0
            );
            let _ = tx.send(LoadCommand::Load {
                path: path.clone(),
                priority: LoadPriority::Critical,
            });
        }
    }

    pub fn prev_image(&mut self, ctx: &egui::Context) {
        // 只有一张图片或没有图片时，不执行切换
        if self.current_images.len() <= 1 {
            return;
        }

        eprintln!(
            "[{:.3}s] [APP] 上一张: current_images={}, current_index={}",
            elapsed_ms() as f64 / 1000.0,
            self.current_images.len(),
            self.current_index
        );

        // 计算新索引
        let new_index = if self.current_index > 0 {
            self.current_index - 1
        } else {
            self.current_images.len() - 1
        };

        // 如果索引没变化，不执行切换
        if new_index == self.current_index {
            return;
        }

        self.current_index = new_index;
        let path = self.current_images[self.current_index].clone();
        eprintln!(
            "[{:.3}s] [APP] 切换到: {:?}",
            elapsed_ms() as f64 / 1000.0,
            path.file_name()
        );
        self.load_image(&path, ctx).ok();
    }

    pub fn next_image(&mut self, ctx: &egui::Context) {
        // 只有一张图片或没有图片时，不执行切换
        if self.current_images.len() <= 1 {
            return;
        }

        eprintln!(
            "[{:.3}s] [APP] 下一张: current_images={}, current_index={}",
            elapsed_ms() as f64 / 1000.0,
            self.current_images.len(),
            self.current_index
        );

        // 计算新索引
        let new_index = if self.current_index < self.current_images.len() - 1 {
            self.current_index + 1
        } else {
            0
        };

        // 如果索引没变化，不执行切换
        if new_index == self.current_index {
            return;
        }

        self.current_index = new_index;
        let path = self.current_images[self.current_index].clone();
        eprintln!(
            "[{:.3}s] [APP] 切换到: {:?}",
            elapsed_ms() as f64 / 1000.0,
            path.file_name()
        );
        self.load_image(&path, ctx).ok();
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

    /// 更新目录列表(异步扫描，仅首次加载时执行)
    fn update_directory_list(&mut self, path: &PathBuf) {
        // 检查是否需要重新扫描目录
        let need_rescan = if let Some(dir_cache) = &self.dir_cache {
            // 如果缓存存在，检查当前图片是否在缓存的目录中
            if let Some(pos) = dir_cache.images.iter().position(|p| p == path) {
                eprintln!(
                    "[{:.3}s] [APP] 使用目录缓存: {} 张图片",
                    elapsed_ms() as f64 / 1000.0,
                    dir_cache.images.len()
                );

                self.current_images = dir_cache.images.clone();
                self.current_index = pos;
                eprintln!(
                    "[{:.3}s] [APP] 从缓存恢复位置: {}",
                    elapsed_ms() as f64 / 1000.0,
                    pos
                );
                return; // 缓存命中，直接返回
            } else {
                // 当前图片不在缓存的目录中，说明切换到了新目录
                eprintln!(
                    "[{:.3}s] [APP] 检测到目录变化，清除旧缓存",
                    elapsed_ms() as f64 / 1000.0
                );
                true // 需要重新扫描
            }
        } else {
            true // 首次打开，需要扫描
        };

        // 需要扫描目录
        if need_rescan {
            if let Some(parent) = path.parent() {
                eprintln!(
                    "[{:.3}s] [APP] 触发目录扫描: {:?}",
                    elapsed_ms() as f64 / 1000.0,
                    parent
                );

                // 清除旧缓存
                self.dir_cache = None;
                
                // 发送扫描命令到后台线程
                if let Some(ref tx) = self.cmd_tx {
                    let _ = tx.send(LoadCommand::ScanDirectory {
                        dir_path: parent.to_path_buf(),
                    });
                }
                // 注意：此时不设置 current_images，等待扫描结果返回后再更新
            }
        }
    }

    /// 预加载相邻图片(智能方向性预加载)
    fn preload_adjacent_images(&mut self, _ctx: &egui::Context) {
        if self.current_images.is_empty() || self.current_index >= self.current_images.len() {
            return;
        }

        // 检查当前图片是否已在缓存中（已加载完成）
        let current_loaded = {
            if let Some(ref path) = self.current_path {
                let cache_guard = self.image_cache.lock().unwrap();
                cache_guard.contains(path)
            } else {
                false
            }
        };

        // 只有当前图片加载完成后才预加载
        if !current_loaded {
            eprintln!(
                "[{:.3}s] [APP] 跳过预加载：当前图片尚未加载完成",
                elapsed_ms() as f64 / 1000.0
            );
            return;
        }

        let mut to_prefetch = Vec::new();

        // 策略：优先预加载下一张，其次是上两张，避免加载已看过的
        let next_idx = self.current_index + 1;
        let next2_idx = self.current_index + 2;
        let next3_idx = self.current_index + 3;

        // 检查缓存，只预加载未缓存的图片
        let cache_guard = self.image_cache.lock().unwrap();

        // 1. 预加载下一张（最高优先级）
        if next_idx < self.current_images.len() {
            let path = &self.current_images[next_idx];
            if !cache_guard.contains(path) {
                to_prefetch.push(path.clone());
            } else {
                eprintln!(
                    "[{:.3}s] [APP] 跳过预加载（已缓存）: {:?}",
                    elapsed_ms() as f64 / 1000.0,
                    path.file_name()
                );
            }
        }

        // 2. 预加载下两张（次高优先级）
        if next2_idx < self.current_images.len() {
            let path = &self.current_images[next2_idx];
            if !cache_guard.contains(path) {
                to_prefetch.push(path.clone());
            } else {
                eprintln!(
                    "[{:.3}s] [APP] 跳过预加载（已缓存）: {:?}",
                    elapsed_ms() as f64 / 1000.0,
                    path.file_name()
                );
            }
        }

        // 3. 预加载下三张（可选）
        if next3_idx < self.current_images.len() {
            let path = &self.current_images[next3_idx];
            if !cache_guard.contains(path) {
                to_prefetch.push(path.clone());
            } else {
                eprintln!(
                    "[{:.3}s] [APP] 跳过预加载（已缓存）: {:?}",
                    elapsed_ms() as f64 / 1000.0,
                    path.file_name()
                );
            }
        }

        drop(cache_guard); // 释放锁

        // 发送预加载命令到后台线程
        if !to_prefetch.is_empty() {
            eprintln!(
                "[{:.3}s] [APP] 预加载 {} 张图片",
                elapsed_ms() as f64 / 1000.0,
                to_prefetch.len()
            );

            if let Some(ref tx) = self.cmd_tx {
                let _ = tx.send(LoadCommand::Prefetch {
                    paths: to_prefetch,
                    priority: LoadPriority::Low,
                });
            }
        } else {
            eprintln!(
                "[{:.3}s] [APP] 无需预加载：所有相邻图片已缓存",
                elapsed_ms() as f64 / 1000.0
            );
        }
    }

    pub fn toggle_fullscreen(&mut self, ctx: &egui::Context) {
        self.is_fullscreen = !self.is_fullscreen;
        if self.is_fullscreen {
            self.is_ui_visible = false;
            // 立即隐藏光标
            ctx.send_viewport_cmd(egui::ViewportCommand::CursorVisible(false));
        } else {
            self.is_ui_visible = true;
            ctx.send_viewport_cmd(egui::ViewportCommand::CursorVisible(true));
        }
        ctx.send_viewport_cmd(egui::ViewportCommand::Fullscreen(self.is_fullscreen));
    }

    pub fn toggle_borderless(&mut self, ctx: &egui::Context) {
        self.is_borderless = !self.is_borderless;
        ctx.send_viewport_cmd(egui::ViewportCommand::Decorations(!self.is_borderless));
    }

    pub fn toggle_status_bar(&mut self) {
        self.settings.show_status_bar = !self.settings.show_status_bar;
        self.save_settings();
    }

    /// 内存检查和淘汰(如果超出限制则移除最旧条目)
    fn evict_if_needed(
        &self,
        cache: &mut lru::LruCache<PathBuf, CacheEntry>,
        new_entry_bytes: usize,
    ) {
        let max_memory = 150 * 1024 * 1024; // 150MB (提高限制以容纳更多预加载图片)

        loop {
            let current_memory: usize = cache
                .iter()
                .map(|(_, entry)| entry.estimated_memory_bytes())
                .sum();

            // 如果超出限制，淘汰最旧的条目
            if current_memory + new_entry_bytes > max_memory {
                if let Some((oldest_path, oldest_entry)) = cache.pop_lru() {
                    let freed_bytes = oldest_entry.estimated_memory_bytes();
                    eprintln!(
                        "[EVICT] Removed {:?} (freed {} MB)",
                        oldest_path.file_name(),
                        freed_bytes as f64 / 1024.0 / 1024.0
                    );
                } else {
                    break; // 缓存已空
                }
            } else {
                break; // 内存充足
            }
        }
    }
}

impl eframe::App for FastViewApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        // 全屏模式下的 UI 自动隐藏逻辑
        if self.is_fullscreen {
            let pointer_delta = ui.input(|i| i.pointer.delta());
            let any_motion =
                pointer_delta != egui::Vec2::ZERO || ui.input(|i| i.pointer.any_pressed());

            if any_motion {
                self.last_mouse_move = Instant::now();
                ui.ctx()
                    .send_viewport_cmd(egui::ViewportCommand::CursorVisible(true));
            } else {
                let elapsed = self.last_mouse_move.elapsed().as_secs_f32();
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

        // 菜单栏显示逻辑：仅在全屏模式下不显示，其他情况根据无边框设置决定
        let should_show_menu = !self.is_fullscreen && !self.is_borderless;

        if should_show_menu {
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

        // 检查异步加载完成的图片(统一处理)
        let mut needs_prefetch = false;
        let mut path_for_dir_update: Option<PathBuf> = None;
        let mut results_processed = 0;

        if let Some(rx) = &self.result_rx {
            // 收集所有待处理的结果
            let mut pending_results = Vec::new();
            let recv_start = Instant::now();
            while let Ok(result) = rx.try_recv() {
                pending_results.push(result);
            }
            let recv_duration = recv_start.elapsed();
            if !pending_results.is_empty() {
                eprintln!(
                    "[{:.3}s] [APP] 收集了 {} 个结果 (耗时 {}ms)",
                    elapsed_ms() as f64 / 1000.0,
                    pending_results.len(),
                    recv_duration.as_millis()
                );
            }

            // 按优先级排序：当前图片优先，其他按接收顺序
            if !pending_results.is_empty() {
                let current_path = self.current_path.clone();
                pending_results.sort_by(|a, b| {
                    let a_is_current = match a {
                        LoadResult::ImageReady { path, .. } => current_path.as_ref() == Some(path),
                        _ => false,
                    };
                    let b_is_current = match b {
                        LoadResult::ImageReady { path, .. } => current_path.as_ref() == Some(path),
                        _ => false,
                    };

                    // 当前图片排前面
                    match (a_is_current, b_is_current) {
                        (true, false) => std::cmp::Ordering::Less,
                        (false, true) => std::cmp::Ordering::Greater,
                        _ => std::cmp::Ordering::Equal,
                    }
                });
            }

            // 处理排序后的结果
            for result in pending_results {
                results_processed += 1;
                match result {
                    LoadResult::ImageReady { path, image } => {
                        eprintln!(
                            "[{:.3}s] [APP] 收到图片: {:?} ({}x{})",
                            elapsed_ms() as f64 / 1000.0,
                            path.file_name(),
                            image.width,
                            image.height
                        );

                        let is_current = self.current_path.as_ref() == Some(&path);

                        // 只有当前图片才立即创建纹理
                        if is_current {
                            // 创建纹理（使用唯一ID避免冲突）
                            let image_size = egui::vec2(image.width as f32, image.height as f32);
                            let texture_id = format!("image_{:?}", path.file_name());

                            // 创建主纹理
                            let color_image = egui::ColorImage::from_rgba_unmultiplied(
                                [image.width as usize, image.height as usize],
                                &image.data,
                            );
                            let texture = ui.ctx().load_texture(
                                &texture_id,
                                color_image,
                                egui::TextureOptions::LINEAR,
                            );

                            eprintln!(
                                "[{:.3}s] [APP] 纹理创建完成: {}",
                                elapsed_ms() as f64 / 1000.0,
                                texture_id
                            );

                            // 在设置新纹理前，显式清除旧纹理以释放内存
                            let old_texture = self.texture.take();
                            drop(old_texture); // 立即释放

                            // 更新纹理和尺寸
                            self.texture = Some(texture);
                            self.image_size = image_size;

                            // 重置缩放模式
                            eprintln!("[{:.3}s] [APP] 重置缩放模式", elapsed_ms() as f64 / 1000.0);
                            self.zoom_mode = ZoomMode::Fit;
                            self.zoom = 1.0;
                            self.rotation = 0.0;
                            self.image_offset = egui::Vec2::ZERO;

                            // 触发预加载（在借用结束后）
                            needs_prefetch = true;

                            // 记录需要更新目录的路径
                            path_for_dir_update = Some(path.clone());
                        } else {
                            // 预加载的图片：只存入缓存，不创建纹理
                            eprintln!(
                                "[{:.3}s] [APP] 缓存预加载图片: {:?}",
                                elapsed_ms() as f64 / 1000.0,
                                path.file_name()
                            );

                            let mut cache_guard = self.image_cache.lock().unwrap();
                            let memory_bytes = (image.width * image.height * 4) as usize;

                            // 内存检查和淘汰
                            self.evict_if_needed(&mut cache_guard, memory_bytes);

                            cache_guard.put(path.clone(), CacheEntry::Decoded(image));
                        }

                        ui.ctx().request_repaint();
                    }
                    LoadResult::DirectoryScanned { images } => {
                        // 目录扫描完成，更新缓存和列表
                        eprintln!(
                            "[{:.3}s] [APP] 目录扫描完成: {} 张图片",
                            elapsed_ms() as f64 / 1000.0,
                            images.len()
                        );

                        if !images.is_empty() {
                            // 更新目录缓存
                            self.dir_cache = Some(DirectoryCache {
                                images: images.clone(),
                            });

                            // 如果当前有路径，找到它在列表中的位置
                            if let Some(ref current_path) = self.current_path {
                                if let Some(pos) = images.iter().position(|p| p == current_path) {
                                    eprintln!(
                                        "[{:.3}s] [APP] 找到当前图片位置: {}",
                                        elapsed_ms() as f64 / 1000.0,
                                        pos
                                    );
                                    self.current_images = images;
                                    self.current_index = pos;
                                } else {
                                    eprintln!(
                                        "[{:.3}s] [APP] 警告：当前图片不在扫描结果中",
                                        elapsed_ms() as f64 / 1000.0
                                    );
                                }
                            } else {
                                eprintln!(
                                    "[{:.3}s] [APP] 警告：current_path为空",
                                    elapsed_ms() as f64 / 1000.0
                                );
                            }

                            ui.ctx().request_repaint();
                        }
                    }
                    _ => {} // 忽略其他结果类型
                }
            }
        }

        if results_processed > 0 {
            eprintln!(
                "[{:.3}s] [APP] 处理了 {} 个结果",
                elapsed_ms() as f64 / 1000.0,
                results_processed
            );
        }

        // 更新目录列表（在借用结束后）
        if let Some(ref path) = path_for_dir_update {
            self.update_directory_list(path);
        }

        // 预加载相邻图片（在借用结束后）
        if needs_prefetch {
            self.preload_adjacent_images(ui.ctx());
        }

        // Status bar - 悬浮半透明设计
        // 显示条件：设置开启 + 非全屏模式
        if self.settings.show_status_bar && !self.is_fullscreen {
            let screen_rect = ui.ctx().content_rect();

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
                        .inner_margin(egui::Margin::symmetric(14, 8)) // 左右14px, 上下8px（提供足够垂直空间）
                        .show(ui, |ui| {
                            // 计算最大宽度限制（避免过宽）
                            let max_width = (screen_rect.width() * 0.9).min(800.0);
                            ui.set_max_width(max_width);

                            // 使用 horizontal 布局，内容垂直居中对齐
                            ui.with_layout(
                                egui::Layout::left_to_right(egui::Align::Center),
                                |ui| {
                                    render_status_content(ui, visuals, self);
                                },
                            );
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
                        if let Some(path) = file.path
                            && path.is_file()
                            && let Some(ext) = path.extension()
                            && let Some(ext_str) = ext.to_str()
                        {
                            let ext_lower = ext_str.to_lowercase();
                            if [
                                "jpg", "jpeg", "png", "gif", "webp", "bmp", "tiff", "tif", "ico",
                                "avif",
                            ]
                            .contains(&ext_lower.as_str())
                            {
                                self.load_image(&path, ui.ctx()).ok();
                                break;
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
                        let is_pressed = ui
                            .ctx()
                            .input(|i| i.pointer.button_down(egui::PointerButton::Primary));

                        // 注意：在 Windows 上，Grab/Grabbing 光标可能不被原生支持
                        // 如果图标没变，可以尝试以下替代方案：
                        // 1. AllScroll - 四向箭头（当前默认）
                        // 2. Grab/Grabbing - 抓手图标（可能需要自定义光标文件）
                        // 3. Move - 移动图标

                        let cursor_icon = if is_pressed {
                            egui::CursorIcon::Grabbing // 按下时的闭手图标
                        } else {
                            egui::CursorIcon::Grab // 未按下时的抓手图标
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

                    // 显示缩略图导航（按需生成）
                    if need_navigation {
                        if let Some(thumb_tex) = self.get_or_create_nav_thumbnail(ui) {
                        let img_ratio = self.image_size.x / self.image_size.y;

                        // 缩略图尺寸：保持宽高比，最大边120px
                        let max_thumb_size = 120.0;
                        let (thumb_w, thumb_h) = if img_ratio > 1.0 {
                            (max_thumb_size, max_thumb_size / img_ratio)
                        } else {
                            (max_thumb_size * img_ratio, max_thumb_size)
                        };
                        let thumb_size = egui::vec2(thumb_w, thumb_h);

                        // 使用 Area 创建悬浮缩略图
                        egui::Area::new(egui::Id::new("thumbnail_navigator"))
                            .anchor(egui::Align2::RIGHT_BOTTOM, [-16.0, -16.0]) // 右下角，距离边缘16px
                            .show(ui.ctx(), |ui| {
                                // 绘制缩略图并获取其位置
                                let mut thumb_image =
                                    egui::Image::new((thumb_tex.id(), thumb_size));
                                if self.rotation != 0.0 {
                                    let angle_rad = self.rotation * std::f32::consts::TAU / 360.0;
                                    thumb_image =
                                        thumb_image.rotate(angle_rad, egui::Vec2::splat(0.5));
                                }
                                let response = ui.add(thumb_image);

                                // 获取缩略图的中心位置
                                let thumb_center = response.rect.center();

                                // 计算视口指示器在未旋转缩略图上的位置和大小
                                // 可视区域占图片的比例
                                let view_portion_x = (available.x / size.x).min(1.0);
                                let view_portion_y = (available.y / size.y).min(1.0);

                                // 在未旋转的缩略图上，红框的大小
                                let view_rect_w = thumb_size.x * view_portion_x;
                                let view_rect_h = thumb_size.y * view_portion_y;

                                // 计算当前滚动位置的相对偏移
                                let offset_ratio_x =
                                    (-self.image_offset.x / size.x + 0.5).clamp(0.0, 1.0);
                                let offset_ratio_y =
                                    (-self.image_offset.y / size.y + 0.5).clamp(0.0, 1.0);

                                // 在未旋转的缩略图上，红框的左上角位置（相对于缩略图中心）
                                let unrotated_view_x = (thumb_size.x - view_rect_w)
                                    * offset_ratio_x
                                    - thumb_size.x / 2.0;
                                let unrotated_view_y = (thumb_size.y - view_rect_h)
                                    * offset_ratio_y
                                    - thumb_size.y / 2.0;

                                // 如果缩略图旋转了，需要将红框的四个角点旋转相同的角度
                                if self.rotation != 0.0 {
                                    let angle_rad = self.rotation * std::f32::consts::TAU / 360.0;
                                    let cos_a = angle_rad.cos();
                                    let sin_a = angle_rad.sin();

                                    // 红框的四个角点（相对于缩略图中心）
                                    let corners = [
                                        egui::vec2(unrotated_view_x, unrotated_view_y),
                                        egui::vec2(
                                            unrotated_view_x + view_rect_w,
                                            unrotated_view_y,
                                        ),
                                        egui::vec2(
                                            unrotated_view_x + view_rect_w,
                                            unrotated_view_y + view_rect_h,
                                        ),
                                        egui::vec2(
                                            unrotated_view_x,
                                            unrotated_view_y + view_rect_h,
                                        ),
                                    ];

                                    // 旋转四个角点
                                    let rotated_corners: Vec<egui::Pos2> = corners
                                        .iter()
                                        .map(|p| {
                                            let rx = p.x * cos_a - p.y * sin_a;
                                            let ry = p.x * sin_a + p.y * cos_a;
                                            thumb_center + egui::vec2(rx, ry)
                                        })
                                        .collect();

                                    // 绘制旋转后的红框（四条线段）
                                    for i in 0..4 {
                                        let start = rotated_corners[i];
                                        let end = rotated_corners[(i + 1) % 4];

                                        // 外层红色线条
                                        ui.painter().line_segment(
                                            [start, end],
                                            egui::Stroke::new(
                                                2.0,
                                                egui::Color32::from_rgba_unmultiplied(
                                                    255, 80, 80, 230,
                                                ),
                                            ),
                                        );

                                        // 内层白色线条（稍微向内收缩）
                                        let dir = (end - start).normalized();
                                        let perp = egui::vec2(-dir.y, dir.x); // 垂直方向
                                        let inner_start = start.to_vec2() + perp * 0.5;
                                        let inner_end = end.to_vec2() + perp * 0.5;
                                        ui.painter().line_segment(
                                            [inner_start.to_pos2(), inner_end.to_pos2()],
                                            egui::Stroke::new(
                                                1.0,
                                                egui::Color32::WHITE.gamma_multiply(0.8),
                                            ),
                                        );
                                    }
                                } else {
                                    // 未旋转时，直接绘制矩形
                                    let view_rect_x = thumb_center.x + unrotated_view_x;
                                    let view_rect_y = thumb_center.y + unrotated_view_y;

                                    let indicator_rect = egui::Rect::from_min_size(
                                        egui::Pos2::new(view_rect_x, view_rect_y),
                                        egui::vec2(view_rect_w, view_rect_h),
                                    );

                                    // 外层红色边框
                                    ui.painter().rect_stroke(
                                        indicator_rect,
                                        2.0,
                                        egui::Stroke::new(
                                            2.0,
                                            egui::Color32::from_rgba_unmultiplied(255, 80, 80, 230),
                                        ),
                                        egui::StrokeKind::Inside,
                                    );

                                    // 内层白色边框
                                    ui.painter().rect_stroke(
                                        indicator_rect.shrink(1.0),
                                        1.0,
                                        egui::Stroke::new(
                                            1.0,
                                            egui::Color32::WHITE.gamma_multiply(0.8),
                                        ),
                                        egui::StrokeKind::Inside,
                                    );
                                }
                            });
                        }
                    }
                } else if self.current_path.is_some() {
                    // Loading 状态：保持深色背景，不显示任何内容
                } else {
                    // 真正的初始状态:没有加载任何图片
                    egui::Area::new(egui::Id::new("welcome_area"))
                        .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
                        .show(ui.ctx(), |ui| {
                            let response = ui.label(self.t(TextKey::ClickToOpen));
                            if response.clicked()
                                && let Some(path) = rfd::FileDialog::new()
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
                            eprintln!(
                                "[{:.3}s] [APP] 检测到左箭头键",
                                elapsed_ms() as f64 / 1000.0
                            );
                            self.prev_image(ui.ctx());
                        }
                        egui::Key::ArrowRight => {
                            eprintln!(
                                "[{:.3}s] [APP] 检测到右箭头键",
                                elapsed_ms() as f64 / 1000.0
                            );
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
                        egui::Key::S => {
                            self.toggle_status_bar();
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
            if let egui::Event::Text(text) = event
                && text == "?"
            {
                self.show_shortcuts = !self.show_shortcuts;
                if self.show_shortcuts {
                    self.window_stack.push(WindowType::Shortcuts);
                } else {
                    self.window_stack.retain(|w| w != &WindowType::Shortcuts);
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
                .fixed_size([480.0, 360.0]) // 稍微加宽，高度更紧凑
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

        // 如果有待处理的加载任务，持续请求重绘以确保及时接收结果
        if self.current_path.is_some() && self.texture.is_none() {
            ui.ctx().request_repaint();
        }
    }
}

/// 渲染状态栏内容的辅助函数
fn render_status_content(ui: &mut egui::Ui, visuals: &egui::Visuals, app: &FastViewApp) {
    // 辅助函数：添加分隔符（带间距）
    let separator = |ui: &mut egui::Ui| {
        ui.add_space(8.0);
        ui.separator();
        ui.add_space(8.0);
    };

    // 文件名（12号加粗，最大宽度200px，超出截断）
    if let Some(ref path) = app.current_path {
        let filename = path
            .file_name()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_default();

        ui.scope(|ui| {
            ui.set_max_width(200.0);
            let response = ui.add(
                egui::Label::new(egui::RichText::new(&filename).strong().size(12.0)).truncate(),
            );
            // 确保悬浮时鼠标图标不变
            if response.hovered() {
                ui.ctx().set_cursor_icon(egui::CursorIcon::Default);
            }
        });

        separator(ui);

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

        separator(ui);
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
        separator(ui);
    }

    // 缩放模式（仅 Custom 模式根据比例显示颜色）
    let zoom_text = match app.zoom_mode {
        ZoomMode::Fit => app.t(TextKey::Fit).to_string(),
        ZoomMode::Fill => app.t(TextKey::Fill).to_string(),
        ZoomMode::Original => app.t(TextKey::Original).to_string(),
        ZoomMode::Custom => format!("{}%", (app.zoom * 100.0) as u32),
    };

    let zoom_color = if app.zoom_mode == ZoomMode::Custom {
        if app.zoom > 1.0 {
            egui::Color32::from_rgb(255, 140, 0)
        } else if app.zoom < 1.0 {
            egui::Color32::from_rgb(100, 149, 237)
        } else {
            visuals.weak_text_color()
        }
    } else {
        visuals.weak_text_color()
    };

    ui.label(egui::RichText::new(&zoom_text).size(10.0).color(zoom_color));

    // 旋转角度
    if app.rotation != 0.0 {
        separator(ui);
        ui.label(
            egui::RichText::new(format!("{}°", app.rotation as u32))
                .size(10.0)
                .color(visuals.weak_text_color()),
        );
    }

    // 文件大小
    if app.file_size > 0 {
        separator(ui);
        ui.label(
            egui::RichText::new(app.format_file_size(app.file_size))
                .size(10.0)
                .family(egui::FontFamily::Monospace)
                .color(visuals.weak_text_color()),
        );
    }
}

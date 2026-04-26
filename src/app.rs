use eframe::egui;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;

use crate::core::loader::ImageLoader;
use crate::core::types::CacheEntry;
use crate::core::{ImageCache, LoadCommand, LoadResult, Settings, TextKey, TiledImage, ZoomMode};
use crate::handler;
use crate::handler::keyboard;
use crate::ui::dialogs;
use crate::ui::fonts::{setup_minimal_fonts, start_async_font_loader};
use crate::ui::{render_menu_bar, render_status_content};

use crate::operation;
use crate::ui::lifecycle;
use crate::ui::thumbnail_manager::ThumbnailManager;
use crate::{log_error, log_warn};

// 全局启动时间（用于相对时间日志）
static START_TIME: once_cell::sync::Lazy<Instant> = once_cell::sync::Lazy::new(Instant::now);

pub(crate) fn elapsed_ms() -> u64 {
    START_TIME.elapsed().as_millis() as u64
}

/// 从磁盘加载配置 (用于并行初始化)
fn load_settings_from_disk() -> Settings {
    if let Some(config_dir) = dirs::config_dir() {
        let config_path = config_dir.join("fastview").join("settings.json");
        if config_path.exists() {
            match std::fs::read_to_string(&config_path) {
                Ok(content) => match serde_json::from_str(&content) {
                    Ok(settings) => {
                        return settings;
                    }
                    Err(e) => {
                        log_warn!("Failed to parse settings file, using defaults: {}", e);
                    }
                },
                Err(e) => {
                    log_warn!("Failed to read settings file: {}", e);
                }
            }
        }
    }
    Settings::default()
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum WindowType {
    Shortcuts,
    Settings,
    About,
}

#[derive(Clone)]
pub(crate) struct DirectoryCache {
    pub(crate) images: Vec<PathBuf>,
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
    pub file_size: u64,             // 文件大小（字节）
    pub show_about: bool,           // 控制“关于”对话框显示
    pub load_error: Option<String>, // 加载错误信息

    // 窗口打开顺序栈，用于ESC键后开先关逻辑
    pub(crate) window_stack: Vec<WindowType>,

    // 目录缓存，避免频繁扫描文件系统
    pub(crate) dir_cache: Option<DirectoryCache>,

    // 新的加载器相关字段
    pub(crate) cmd_tx: Option<std::sync::mpsc::Sender<LoadCommand>>,
    pub(crate) result_rx: Option<std::sync::mpsc::Receiver<LoadResult>>,
    loader_handle: Option<std::thread::JoinHandle<()>>,

    // UI 自动隐藏逻辑
    pub(crate) last_mouse_move: std::time::Instant,
    pub(crate) is_ui_visible: bool, // 控制全屏下菜单栏和状态栏的可见性

    // 分块图片相关
    pub(crate) tiled_image: Option<Arc<TiledImage>>,
    pub(crate) tile_textures: std::collections::HashMap<(u32, u32), egui::TextureHandle>, // 已加载的块纹理

    // 缩略图管理器
    pub(crate) thumbnail_mgr: ThumbnailManager,
}

impl Default for FastViewApp {
    fn default() -> Self {
        use crate::utils::to_non_zero_usize;
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
            image_cache: Arc::new(std::sync::Mutex::new(LruCache::new(to_non_zero_usize(
                5, 10,
            )))),
            settings: Settings::default(),
            show_settings: false,
            file_size: 0,
            show_about: false,
            load_error: None,
            window_stack: Vec::new(),
            dir_cache: None,
            cmd_tx: None,
            result_rx: None,
            loader_handle: None,
            last_mouse_move: Instant::now(),
            is_ui_visible: true,
            tiled_image: None,
            tile_textures: std::collections::HashMap::new(),

            thumbnail_mgr: ThumbnailManager::new(),
        }
    }
}

impl FastViewApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        let t0 = Instant::now();

        // Phase 1: 立即设置最小字体 (无 I/O, <5ms)
        setup_minimal_fonts(cc);
        perf_log!("Minimal fonts setup", t0);

        // Phase 2: 并行启动: 字体加载 + 配置读取
        let font_ctx = cc.egui_ctx.clone();
        let font_handle = std::thread::spawn(move || {
            start_async_font_loader(font_ctx);
        });

        let settings_handle = std::thread::spawn(load_settings_from_disk);

        perf_log!("Parallel tasks started", t0);

        // Phase 3: 等待配置就绪 (字体在后台继续)
        let settings = settings_handle.join().unwrap_or_else(|_| {
            log_warn!("Settings thread panicked, using defaults");
            Settings::default()
        });
        perf_log!("Settings loaded", t0);

        // Phase 4: 创建应用实例
        let mut app = Self::default();
        app.settings = settings;

        // 应用无边框模式设置
        if app.settings.borderless_mode {
            cc.egui_ctx
                .send_viewport_cmd(egui::ViewportCommand::Decorations(false));
            app.is_borderless = true;
        }

        app.start_loader();

        // Phase 5: 字体线程 detach (在后台完成后自动应用)
        std::mem::forget(font_handle);

        perf_log!("App initialized", t0);
        app
    }

    fn start_loader(&mut self) {
        let (result_tx, result_rx) = std::sync::mpsc::channel();
        let (mut loader, cmd_tx) = ImageLoader::new(result_tx);

        // 设置主缓存引用，让后台线程可以复用已解码数据
        loader.set_image_cache(self.image_cache.clone());

        self.loader_handle = Some(std::thread::spawn(move || {
            loader.run();
        }));
        self.cmd_tx = Some(cmd_tx);
        self.result_rx = Some(result_rx);
    }

    pub fn save_settings(&mut self) {
        if let Some(config_dir) = dirs::config_dir() {
            let config_path = config_dir.join("fastview").join("settings.json");
            if let Some(parent) = config_path.parent()
                && let Err(e) = std::fs::create_dir_all(parent)
            {
                log_error!("Failed to create config directory: {}", e);
                return;
            }
            match serde_json::to_string_pretty(&self.settings) {
                Ok(content) => {
                    if let Err(e) = std::fs::write(&config_path, content) {
                        log_error!("Failed to save settings: {}", e);
                    }
                }
                Err(e) => {
                    log_error!("Failed to serialize settings: {}", e);
                }
            }
        }

        // 更新 LRU 缓存大小（固定为10）
        let new_capacity = 10;
        if let Ok(mut cache) = self.image_cache.lock() {
            use crate::utils::to_non_zero_usize;
            // LRU 不支持动态调整大小，需要重建
            let mut new_cache = lru::LruCache::new(to_non_zero_usize(new_capacity, 10));
            // 保留最近的项目
            for (key, value) in cache.iter() {
                new_cache.put(key.clone(), value.clone());
            }
            *cache = new_cache;
        }
    }

    pub(crate) fn t(&self, key: TextKey) -> &'static str {
        key.text(self.settings.language)
    }

    /// 获取应用版本号
    pub(crate) fn get_version(&self) -> &'static str {
        env!("CARGO_PKG_VERSION")
    }

    // 格式化文件大小
    pub(crate) fn format_file_size(&self, bytes: u64) -> String {
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
    pub(crate) fn get_or_create_nav_thumbnail(
        &mut self,
        ui: &mut egui::Ui,
    ) -> Option<egui::TextureHandle> {
        operation::cache_manager::get_or_create_nav_thumbnail(self, ui)
    }

    pub fn load_image(&mut self, path: &PathBuf, ctx: &egui::Context) -> Result<(), String> {
        operation::cache_manager::load_image(self, path, ctx)
    }

    /// 应用缓存条目
    pub(crate) fn apply_cached_entry(
        &mut self,
        entry: CacheEntry,
        path: &PathBuf,
        ctx: &egui::Context,
    ) {
        operation::cache_manager::apply_cached_entry(self, entry, path, ctx);
    }

    /// 异步加载图片（使用专用后台线程）
    fn load_image_async(&mut self, path: &PathBuf, ctx: &egui::Context) {
        operation::cache_manager::load_image_async(self, path, ctx);
    }

    pub fn prev_image(&mut self, ctx: &egui::Context) {
        operation::navigation::prev_image(self, ctx);
    }

    pub fn next_image(&mut self, ctx: &egui::Context) {
        operation::navigation::next_image(self, ctx);
    }

    pub fn zoom_in(&mut self, current_scale: f32) {
        operation::image_ops::zoom_in(self, current_scale);
    }

    pub fn zoom_out(&mut self, current_scale: f32) {
        operation::image_ops::zoom_out(self, current_scale);
    }

    pub fn rotate_left(&mut self) {
        operation::image_ops::rotate_left(self);
    }

    pub fn rotate_right(&mut self) {
        operation::image_ops::rotate_right(self);
    }

    pub(crate) fn update_directory_list(&mut self, path: &PathBuf) {
        operation::navigation::update_directory_list(self, path);
    }

    /// 预加载相邻图片（智能方向性预加载）
    pub(crate) fn preload_adjacent_images(&mut self) {
        operation::navigation::preload_adjacent_images(self);
    }

    /// 请求加载可见区域的块
    fn request_visible_tiles(&mut self, ctx: &egui::Context) {
        operation::tile_renderer::request_visible_tiles(self, ctx);
    }

    /// 渲染已加载的块
    pub(crate) fn render_tiles(
        &self,
        ui: &mut egui::Ui,
        image_rect: egui::Rect,
        display_size: egui::Vec2,
        original_size: egui::Vec2,
        available: egui::Vec2,
        rotation: f32,
    ) {
        operation::tile_renderer::render_tiles(
            self,
            ui,
            image_rect,
            display_size,
            original_size,
            available,
            rotation,
        );
    }

    pub fn toggle_fullscreen(&mut self, ctx: &egui::Context) {
        operation::image_ops::toggle_fullscreen(self, ctx);
    }

    pub fn toggle_borderless(&mut self, ctx: &egui::Context) {
        operation::image_ops::toggle_borderless(self, ctx);
    }

    pub fn toggle_status_bar(&mut self) {
        operation::image_ops::toggle_status_bar(self);
    }

    /// 内存检查和淘汰（如果超出限制则移除最旧条目）
    pub(crate) fn evict_if_needed(
        &self,
        cache: &mut lru::LruCache<PathBuf, CacheEntry>,
        new_entry_bytes: usize,
    ) {
        operation::cache_manager::evict_if_needed(self, cache, new_entry_bytes);
    }
}

impl Drop for FastViewApp {
    fn drop(&mut self) {
        // 优雅关闭后台加载器线程
        if let Some(handle) = self.loader_handle.take() {
            debug_log!("[APP] Shutting down image loader thread...");

            // 注意:当前 loader 没有 Shutdown 命令,线程会在 channel 关闭后自动退出
            // 这里我们简单地 detach,让线程自然结束
            drop(handle);

            debug_log!("[APP] Loader thread detached");
        }

        // 清理纹理资源
        self.texture = None;
        self.tile_textures.clear();

        debug_log!("[APP] FastViewApp resources cleaned up");
    }
}

impl eframe::App for FastViewApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        // 处理全屏UI自动隐藏
        lifecycle::handle_fullscreen_ui(self, ui);

        // 渲染菜单栏
        render_menu_bar(self, ui);

        // 处理异步加载结果
        let (needs_prefetch, path_for_dir_update) = handler::events::handle_load_results(self, ui);

        // 处理预加载和目录更新
        lifecycle::handle_post_load_operations(self, ui, needs_prefetch, path_for_dir_update);

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

                    // 如果是分块图片，渲染已加载的块
                    if let Some(ref tiled) = self.tiled_image {
                        // 对于分块图片，我们需要使用原始图片尺寸来计算缩放比例
                        let original_size = egui::vec2(tiled.width as f32, tiled.height as f32);
                        self.render_tiles(
                            ui,
                            absolute_rect,
                            size,
                            original_size,
                            available,
                            self.rotation,
                        );
                    }

                    // 检查是否需要显示导航缩略图
                    // 条件：当前显示的图片尺寸大于可视区域（说明无法完整显示）
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
                    if need_navigation && let Some(thumb_tex) = self.get_or_create_nav_thumbnail(ui)
                    {
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
                            .anchor(egui::Align2::RIGHT_BOTTOM, [-24.0, -24.0]) // 右下角，距离边缘24px
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
                } else if self.current_path.is_some() {
                    // Loading 状态或错误状态
                    if let Some(ref error_msg) = self.load_error {
                        // 显示错误信息
                        let current_path = self.current_path.clone();
                        let error_message = error_msg.clone(); // 克隆以避免借用冲突
                        egui::Area::new(egui::Id::new("error_message"))
                            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
                            .show(ui.ctx(), |ui| {
                                ui.vertical_centered(|ui| {
                                    ui.add_space(40.0);
                                    ui.label(
                                        egui::RichText::new("❌ Load Error")
                                            .size(16.0)
                                            .color(egui::Color32::RED),
                                    );
                                    ui.add_space(10.0);
                                    ui.label(
                                        egui::RichText::new(&error_message)
                                            .size(12.0)
                                            .color(egui::Color32::WHITE),
                                    );
                                    ui.add_space(20.0);
                                    if ui.button("Retry").clicked() {
                                        // 重新加载当前图片
                                        if let Some(ref path) = current_path {
                                            self.load_error = None;
                                            self.load_image(path, ui.ctx()).ok();
                                        }
                                    }
                                });
                            });
                    }
                    // 否则保持深色背景，不显示任何内容
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

        // 处理键盘事件
        keyboard::handle_keyboard_events(self, ui);

        // 渲染对话框（设置、快捷键、关于）
        dialogs::render_dialogs(self, ui);

        // 检查是否需要持续重绘
        lifecycle::check_needs_repaint(self, ui);

        // 处理缩略图导航栏
        lifecycle::handle_thumbnail_navigation(self, ui);
    }
}

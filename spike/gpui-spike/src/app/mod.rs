//! 应用状态与每帧编排
//!
//! 这里只保留 `Spike` 的状态、构造和 `render` 骨架，具体实现按职责分在子模块：
//!
//! - `theme`：配色与界面尺寸
//! - `settings`：配置文件读写
//! - `widgets`：基础绘制件（窗口控制图标、窗口按钮、助记符文本）
//! - `titlebar` / `menu_layer` / `pane` / `thumbs` / `settings_panel`：界面分块
//! - `load`：图片加载、缓存与预取
//! - `keys`：快捷键与菜单命令
//! - `boot`：窗口创建与事件循环入口

mod boot;
mod help_panel;
mod keys;
mod load;
mod menu_layer;
mod pane;
mod settings;
mod settings_panel;
mod status;
pub(crate) mod theme;
mod thumbs;
mod titlebar;
mod widgets;

use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::Arc;
use std::time::Instant;

use gpui::{
    AnyElement, Bounds, Context, ExternalPaths, FileDropEvent, FocusHandle, KeyDownEvent,
    ModifiersChangedEvent, MouseButton, MouseMoveEvent, Pixels, Point, RenderImage, ScrollHandle,
    Size, Subscription, Window, div, prelude::*, px, size,
};

use self::settings::Settings;
use self::theme::{Palette, THUMB_BAR_HEIGHT, TITLEBAR_HEIGHT};
#[cfg(windows)]
use crate::win;
use crate::{cache, folder, viewer};

pub use boot::run;

/// 图片纹理缓存预算：够存 6~8 张 4K 的 1/1，或 20 张以上的 1/2
const IMAGE_BUDGET: usize = 192 * 1024 * 1024;

/// 应用状态
pub struct Spike {
    /// 原生 HWND（0 表示不可用），用于设置 DWM 边框 / 圆角
    hwnd: isize,
    started_at: Instant,
    first_frame_logged: bool,
    /// 系统深浅色变化时通知重绘（首帧后注册）
    _appearance_sub: Option<Subscription>,
    /// 窗口真正显示时 DWM 会重建框架，需要再压一次边框设置
    _visibility_sub: Option<Subscription>,
    /// 当前图片与查看状态
    image: Option<viewer::LoadedImage>,
    view: viewer::ViewState,
    loading: bool,
    error: Option<String>,
    /// 加载序号：后发起的请求覆盖先前的，避免旧结果回写
    load_id: u64,
    /// 旋转序号：连续旋转时只保留最后一次的结果
    rotate_id: u64,
    /// 正在补的更清晰档位（1/denom），None 表示空闲
    sharpen: Option<u32>,
    /// 补过但失败的档位，避免每帧重试
    sharpen_failed: Option<u32>,
    /// 同目录图片列表：翻页与缩略图条共用
    folder: Option<folder::Folder>,
    /// 缩略图纹理，键为文件路径
    thumbs: HashMap<PathBuf, Arc<RenderImage>>,
    /// 目录 generation：换目录后丢弃旧的缩略图结果
    thumb_gen: u64,
    /// 缩略图条是否显示
    show_thumbs: bool,
    /// 缩略图条横向滚动位置
    thumb_scroll: ScrollHandle,
    /// 缩略图条已滚动定位到哪一项，避免每帧重滚
    thumb_scrolled: Option<usize>,
    /// 已解码纹理的 LRU 缓存：切回看过的图不再重解
    cache: cache::ImageCache,
    /// 正在预取的图，避免重复提交
    prefetching: HashSet<PathBuf>,
    /// 拖拽平移的上一帧位置
    drag_last: Option<Point<Pixels>>,
    /// 键盘快捷键
    focus_handle: FocusHandle,
    /// 上一次的窗口状态（最大化、全屏）：变化后 DWM 会重算框架，需要重设边框
    window_state: (bool, bool),
    /// 当前展开的顶层菜单（索引对应 `menu::MENUS`）
    open_menu: Option<usize>,
    /// 下拉里键盘选中的条目（`spec.items` 的索引）
    menu_selected: Option<usize>,
    /// 顶层菜单项在窗口坐标里的位置（prepaint 每帧更新，用于定位下拉与切换）
    menu_rects: Rc<RefCell<Vec<Bounds<Pixels>>>>,
    /// Alt 是否按住：按住才显示助记符下划线（同系统菜单）
    alt_held: bool,
    /// 窗口失焦时收起菜单（原生菜单同行为）
    _activation_sub: Option<Subscription>,
    /// 用户配置：主题 / 材质 / 透明度
    settings: Settings,
    /// 设置面板是否展开
    settings_open: bool,
    /// 快捷键说明浮层是否展开
    help_open: bool,
    /// 正在拖动透明度滑杆
    settings_drag: bool,
    /// 滑杆轨道的窗口坐标（prepaint 每帧更新）
    slider_bounds: Rc<RefCell<Option<Bounds<Pixels>>>>,
    /// 系统强调色（`0xRRGGBB`）
    accent: u32,
    /// 有文件拖到窗口上：整窗高亮
    drop_hover: bool,
    /// 菜单展开序号：变化时重放淡入动画
    menu_seq: u64,
    /// 状态栏是否常显（`S` 切换）；否则无操作后自动淡出
    status_pinned: bool,
    /// 状态栏当前目标可见性
    status_shown: bool,
    /// 状态栏动画序号：显隐切换时自增，用于重放动画
    status_seq: u64,
    /// 状态栏自动淡出的截止时间
    status_deadline: Option<Instant>,
    /// 状态栏定时器代数：作废旧的等待任务
    status_timer_gen: u64,
}

impl Spike {
    fn new(hwnd: isize, started_at: Instant, settings: Settings, cx: &mut Context<Self>) -> Self {
        #[cfg(windows)]
        let accent = win::system_accent().unwrap_or(theme::DEFAULT_ACCENT);
        #[cfg(not(windows))]
        let accent = theme::DEFAULT_ACCENT;

        Self {
            hwnd,
            started_at,
            first_frame_logged: false,
            _appearance_sub: None,
            _visibility_sub: None,
            image: None,
            view: viewer::ViewState::default(),
            loading: false,
            error: None,
            load_id: 0,
            rotate_id: 0,
            sharpen: None,
            sharpen_failed: None,
            folder: None,
            thumbs: HashMap::new(),
            thumb_gen: 0,
            show_thumbs: false,
            thumb_scroll: ScrollHandle::new(),
            thumb_scrolled: None,
            cache: cache::ImageCache::new(IMAGE_BUDGET),
            prefetching: HashSet::new(),
            drag_last: None,
            focus_handle: cx.focus_handle(),
            window_state: (false, false),
            open_menu: None,
            menu_selected: None,
            menu_rects: Rc::new(RefCell::new(Vec::new())),
            alt_held: false,
            _activation_sub: None,
            settings,
            settings_open: false,
            help_open: false,
            settings_drag: false,
            slider_bounds: Rc::new(RefCell::new(None)),
            accent,
            drop_hover: false,
            menu_seq: 0,
            status_pinned: false,
            status_shown: true,
            status_seq: 0,
            status_deadline: None,
            status_timer_gen: 0,
        }
    }

    /// 视口（标题栏以下、缩略图条以上）尺寸
    fn view_size(&self, window: &Window) -> Size<Pixels> {
        let viewport = window.viewport_size();
        let height: f32 = viewport.height.into();
        let bar = if self.show_thumbs {
            THUMB_BAR_HEIGHT
        } else {
            0.0
        };
        size(
            viewport.width,
            px((height - TITLEBAR_HEIGHT - bar).max(0.0)),
        )
    }

    /// 当前图片的逻辑尺寸（无图时用 1×1 占位，避免除零）
    fn base_size(&self, window: &Window) -> Size<Pixels> {
        self.image
            .as_ref()
            .map(|image| image.logical_size(window.scale_factor()))
            .unwrap_or_else(|| size(px(1.0), px(1.0)))
    }

    /// 视口宽度对应的设备像素（解码选档用）
    fn need_px(&self, window: &Window) -> u32 {
        let width: f32 = self.view_size(window).width.into();
        (width * window.scale_factor()).round().max(1.0) as u32
    }

    /// 首帧日志 + 窗口状态变化时重设窗口外观
    ///
    /// 窗口真正显示之后 DWM 会重建一次框架，创建时设的边框与材质会被还原，
    /// 表现就是标题栏顶部冒出一条 1px 亮线、材质丢失。
    fn sync_window(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let first_frame = !self.first_frame_logged;
        if first_frame {
            self.first_frame_logged = true;
            eprintln!(
                "[SPIKE] first frame after {} ms",
                self.started_at.elapsed().as_millis()
            );
        }

        let state = (window.is_maximized(), window.is_fullscreen());
        if first_frame || self.window_state != state {
            self.window_state = state;
            self.apply_window_style(window, cx);
        }
    }

    /// 是否使用暗色主题（配置优先，否则跟随系统）
    fn is_dark(&self, window: &Window) -> bool {
        theme::is_dark(self.settings.theme, window.appearance())
    }

    /// 是否真的使用透明材质（非 Windows 一律实色）
    fn translucent(&self) -> bool {
        translucent(&self.settings)
    }

    /// 当前配色：含背景透明度与系统强调色
    fn palette(&self, window: &Window) -> Palette {
        let alpha = if self.translucent() {
            self.settings.alpha()
        } else {
            1.0
        };
        Palette::new(self.is_dark(window), alpha, self.accent)
    }

    /// 重新读取系统强调色；变化时返回 true
    fn refresh_accent(&mut self) -> bool {
        #[cfg(windows)]
        let next = win::system_accent().unwrap_or(theme::DEFAULT_ACCENT);
        #[cfg(not(windows))]
        let next = theme::DEFAULT_ACCENT;

        let changed = next != self.accent;
        self.accent = next;
        changed
    }

    /// 应用 DWM 边框 / 暗色模式 / 背景材质
    fn apply_window_style(&self, window: &mut Window, cx: &mut Context<Self>) {
        if self.hwnd == 0 {
            return;
        }
        let hwnd = self.hwnd as *mut core::ffi::c_void;
        let dark = self.is_dark(window);
        let palette = self.palette(window);
        let translucent = self.translucent();
        let backdrop = translucent && self.settings.uses_backdrop();
        let border_rgb = palette.titlebar_rgb;
        #[cfg(windows)]
        let kind = backdrop_kind(&self.settings);

        window.defer(cx, move |_, _| {
            #[cfg(windows)]
            {
                win::apply_window_frame(hwnd, border_rgb, backdrop);
                win::apply_dark_mode(hwnd, dark);
                let supported = win::apply_backdrop(hwnd, kind);
                // 22621 之前没有系统背景：用 Win10 老式亚克力；拖动时会自动降级
                win::set_legacy_accent(hwnd, backdrop && !supported);
            }
            #[cfg(not(windows))]
            {
                let _ = (hwnd, dark, border_rgb, backdrop, window);
            }
        });
    }

    /// 一次性窗口钩子：深浅色 / 可见性 / 失焦
    fn ensure_window_hooks(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self._appearance_sub.is_none() {
            self._appearance_sub = Some(cx.observe_window_appearance(window, |this, window, cx| {
                this.refresh_accent();
                this.apply_window_style(window, cx);
                cx.notify();
            }));
        }

        if self._visibility_sub.is_none() {
            self._visibility_sub =
                Some(cx.observe_window_visibility(window, |this, _, window, cx| {
                    this.apply_window_style(window, cx);
                }));
        }

        // 窗口失焦时收起菜单并清掉 Alt 状态（系统菜单同行为）
        if self._activation_sub.is_none() {
            self._activation_sub =
                Some(cx.observe_window_activation(window, |this, window, cx| {
                    if window.is_window_active() {
                        // 激活后系统强调色可能已变；只有真的变了才重设窗口外观，
                        // 避免每次切换焦点都触发一次 DWM 框架重建
                        if this.refresh_accent() {
                            this.apply_window_style(window, cx);
                            cx.notify();
                        }
                        return;
                    }
                    // Alt+Tab 时松开 Alt 收不到事件，必须自己复位下划线
                    let alt_reset = std::mem::take(&mut this.alt_held);
                    let menu_closed = this.close_menu();
                    // 切走时鼠标抬起事件收不到，滑杆拖动要收尾并写盘
                    let drag_reset = std::mem::take(&mut this.settings_drag);
                    if drag_reset {
                        this.settings.save();
                    }
                    // 拖文件时切走窗口可能收不到 drop exit，兜底清掉高亮
                    let drop_reset = std::mem::take(&mut this.drop_hover);
                    if alt_reset || menu_closed || drag_reset || drop_reset {
                        cx.notify();
                    }
                }));
        }
    }
}

impl Render for Spike {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.sync_window(window, cx);
        // 缩放/窗口尺寸变化后，当前档位可能已不够清晰
        self.schedule_sharpen(window, cx);
        self.ensure_window_hooks(window, cx);

        let palette = self.palette(window);
        let titlebar = self.render_titlebar(window, cx);
        let menu_layer = self.render_menu_layer(window, cx);
        let settings_panel = self.render_settings_panel(window, cx);
        let help_panel = self.render_help_panel(window, cx);
        let thumb_bar = self.show_thumbs.then(|| self.render_thumbnails(window, cx));
        let focus_handle = self.focus_handle.clone();
        let drop_hover = self.drop_hover;

        div()
            .id("app-root")
            .size_full()
            .relative()
            .flex()
            .flex_col()
            .track_focus(&focus_handle)
            .on_key_down(cx.listener(|this, event: &KeyDownEvent, window, cx| {
                this.handle_key(event, window, cx);
            }))
            // 助记符下划线跟着 Alt 的按下/松开走
            .on_modifiers_changed(cx.listener(|this, event: &ModifiersChangedEvent, _, cx| {
                if this.alt_held != event.modifiers.alt {
                    this.alt_held = event.modifiers.alt;
                    cx.notify();
                }
            }))
            // 外部文件拖入：整窗接管，松开即打开
            .on_drag_move::<ExternalPaths>(cx.listener(|this, _, _, cx| {
                if !this.drop_hover {
                    this.drop_hover = true;
                    cx.notify();
                }
            }))
            .on_drop::<ExternalPaths>(cx.listener(|this, paths: &ExternalPaths, window, cx| {
                if std::mem::take(&mut this.drop_hover) {
                    cx.notify();
                }
                if let Some(path) = paths.0.first() {
                    this.open_path(path.clone(), window, cx);
                }
            }))
            .on_file_drop_exit(cx.listener(|this, _: &FileDropEvent, _, cx| {
                if std::mem::take(&mut this.drop_hover) {
                    cx.notify();
                }
            }))
            // 透明度滑杆的拖动由根元素接管：鼠标移出滑杆也不会断
            .on_mouse_move(cx.listener(|this, event: &MouseMoveEvent, window, cx| {
                this.touch_status(cx);
                if this.settings_drag && event.dragging() {
                    this.set_opacity_at(event.position, window, cx);
                }
            }))
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(|this, _, _, cx| {
                    this.finish_opacity_drag(cx);
                    // 文件拖拽结束时 exit 事件可能丢失，鼠标抬起兜底复位高亮
                    if std::mem::take(&mut this.drop_hover) {
                        cx.notify();
                    }
                }),
            )
            .child(titlebar)
            .child(self.render_viewer(window, cx))
            .when_some(thumb_bar, |this, bar| this.child(bar))
            .when(drop_hover, |this| this.child(drop_overlay(&palette)))
            .when_some(menu_layer, |this, layer| this.child(layer))
            .when_some(settings_panel, |this, panel| this.child(panel))
            .when_some(help_panel, |this, panel| this.child(panel))
    }
}

/// 文件拖到窗口上时的高亮框与提示
fn drop_overlay(palette: &Palette) -> AnyElement {
    div()
        .absolute()
        .inset_0()
        .flex()
        .items_center()
        .justify_center()
        .child(
            div()
                .absolute()
                .inset(px(6.0))
                .rounded(px(10.0))
                .border_2()
                .border_color(palette.accent.opacity(0.85))
                .bg(palette.accent.opacity(0.05)),
        )
        .child(
            div()
                .px(px(14.0))
                .py(px(8.0))
                .rounded(px(8.0))
                .bg(palette.card_bg)
                .border_1()
                .border_color(palette.card_border)
                .text_size(px(12.0))
                .text_color(palette.text)
                .child("松开以打开图片"),
        )
        .into_any_element()
}

/// 配置里的材质 → DWM 材质；100% 不透明或未选材质时不请求 backdrop
#[cfg(windows)]
fn backdrop_kind(settings: &Settings) -> win::Backdrop {
    if !translucent(settings) || !settings.uses_backdrop() {
        return win::Backdrop::None;
    }
    match settings.material {
        settings::Material::Acrylic => win::Backdrop::Acrylic,
        settings::Material::Mica => win::Backdrop::Mica,
        settings::Material::None => win::Backdrop::None,
    }
}

/// 自绘背景层能否透明：Windows、不透明度 < 100%、且 DComp 没被排查开关关掉
fn translucent(settings: &Settings) -> bool {
    cfg!(windows)
        && settings.translucent()
        && std::env::var_os("GPUI_DISABLE_DIRECT_COMPOSITION").is_none()
}

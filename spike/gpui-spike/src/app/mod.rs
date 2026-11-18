//! 应用状态与每帧编排
//!
//! 这里只保留 `Spike` 的状态、构造和 `render` 骨架，具体实现按职责分在子模块：
//!
//! - `theme`：配色与界面尺寸
//! - `widgets`：基础绘制件（窗口控制图标、窗口按钮、助记符文本）
//! - `titlebar` / `menu_layer` / `pane` / `thumbs`：四块界面
//! - `load`：图片加载、缓存与预取
//! - `keys`：快捷键与菜单命令
//! - `boot`：窗口创建与事件循环入口

mod boot;
mod keys;
mod load;
mod menu_layer;
mod pane;
mod theme;
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
    Bounds, Context, FocusHandle, KeyDownEvent, ModifiersChangedEvent, Pixels, Point, RenderImage,
    ScrollHandle, Size, Subscription, Window, div, prelude::*, px, size,
};

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
}

impl Spike {
    fn new(hwnd: isize, started_at: Instant, cx: &mut Context<Self>) -> Self {
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

    /// 首帧日志 + 窗口状态变化时重设 DWM 边框
    ///
    /// 窗口真正显示之后 DWM 会重建一次框架，创建时设的边框颜色会被还原，
    /// 表现就是标题栏顶部冒出一条 1px 亮线。
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
            #[cfg(windows)]
            self.refresh_window_frame(window, cx);
        }
    }

    /// 把 DWM 边框画成标题栏底色
    #[cfg(windows)]
    fn refresh_window_frame(&self, window: &mut Window, cx: &mut Context<Self>) {
        if self.hwnd == 0 {
            return;
        }
        let hwnd = self.hwnd as *mut core::ffi::c_void;
        let border_rgb = Palette::for_appearance(window.appearance()).titlebar_rgb;
        window.defer(cx, move |_, _| win::apply_window_frame(hwnd, border_rgb));
    }

    /// 一次性窗口钩子：深浅色 / 可见性 / 失焦
    fn ensure_window_hooks(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self._appearance_sub.is_none() {
            self._appearance_sub = Some(cx.observe_window_appearance(window, |this, window, cx| {
                #[cfg(windows)]
                this.refresh_window_frame(window, cx);
                cx.notify();
            }));
        }

        if self._visibility_sub.is_none() {
            self._visibility_sub =
                Some(cx.observe_window_visibility(window, |this, _, window, cx| {
                    #[cfg(windows)]
                    this.refresh_window_frame(window, cx);
                }));
        }

        // 窗口失焦时收起菜单并清掉 Alt 状态（系统菜单同行为）
        if self._activation_sub.is_none() {
            self._activation_sub =
                Some(cx.observe_window_activation(window, |this, window, cx| {
                    if window.is_window_active() {
                        return;
                    }
                    // Alt+Tab 时松开 Alt 收不到事件，必须自己复位下划线
                    let alt_reset = std::mem::take(&mut this.alt_held);
                    let menu_closed = this.close_menu();
                    if alt_reset || menu_closed {
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

        let titlebar = self.render_titlebar(window, cx);
        let menu_layer = self.render_menu_layer(window, cx);
        let palette = Palette::for_appearance(window.appearance());
        let thumb_bar = self
            .show_thumbs
            .then(|| self.render_thumbnails(&palette, cx));
        let focus_handle = self.focus_handle.clone();

        div()
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
            .child(titlebar)
            .child(self.render_viewer(window, cx))
            .when_some(thumb_bar, |this, bar| this.child(bar))
            .when_some(menu_layer, |this, layer| this.child(layer))
    }
}

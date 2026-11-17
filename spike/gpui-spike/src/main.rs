//! FastView gpui 骨架（spike）
//!
//! 首次落地 Windows 体验的两块地基：
//! 1. 现代窗口 UI：自绘标题栏，菜单与窗口按钮都在标题栏内，不额外占一行（同 VS Code），
//!    并保留 DWM 原生动画、拖动分屏、贴靠布局；
//! 2. 菜单：自绘入口 + `TrackPopupMenuEx` 原生下拉，命令回到 gpui 处理。
//!
//! 内容区仍是占位，只用来观察材质与窗口行为。

#[cfg(windows)]
mod menu;
#[cfg(windows)]
mod win;

use std::time::Instant;

use gpui::{
    AnyElement, App, Bounds, ClickEvent, Context, MouseButton, SharedString, TitlebarOptions,
    Window, WindowBackgroundAppearance, WindowBounds, WindowControlArea, WindowOptions, div,
    prelude::*, px, rgb, size,
};
use gpui_platform::application;

const TITLEBAR_HEIGHT: f32 = 32.0;
/// Win11 标准窗口按钮宽度
const WINDOW_BUTTON_WIDTH: f32 = 46.0;
/// 标题栏遮罩：只留淡淡的材质感，保证菜单文字可读
const TITLEBAR_TINT: f32 = 0.20;
/// 内容区遮罩：主体要"实"，材质的通透感只属于标题栏
const CONTENT_TINT: f32 = 0.72;

struct Spike {
    /// 原生 HWND（0 表示不可用），用于弹出系统菜单
    hwnd: isize,
    backdrop: WindowBackgroundAppearance,
    last_cmd: SharedString,
    started_at: Instant,
    first_frame_logged: bool,
}

/// 窗口控制图标：用几何图形自绘
///
/// 图标字体（Segoe Fluent Icons）在 gpui 里匹配不到，会渲染成方框，
/// 所以这里全部用 div 拼出来，不依赖任何字体。
fn icon_minimize() -> impl IntoElement {
    div().w(px(10.0)).h(px(1.0)).bg(rgb(0xffffff))
}

fn icon_maximize() -> impl IntoElement {
    div()
        .w(px(10.0))
        .h(px(10.0))
        .border_1()
        .border_color(rgb(0xffffff))
}

fn icon_restore() -> impl IntoElement {
    div()
        .relative()
        .w(px(10.0))
        .h(px(10.0))
        // 后面那个窗口只露出两条边
        .child(
            div()
                .absolute()
                .left(px(2.0))
                .top(px(0.0))
                .w(px(8.0))
                .h(px(8.0))
                .border_t(px(1.0))
                .border_r(px(1.0))
                .border_color(rgb(0xffffff)),
        )
        .child(
            div()
                .absolute()
                .left(px(0.0))
                .top(px(2.0))
                .w(px(8.0))
                .h(px(8.0))
                .border_1()
                .border_color(rgb(0xffffff)),
        )
}

fn icon_close() -> impl IntoElement {
    div()
        .text_size(px(15.0))
        .text_color(rgb(0xffffff))
        .child("×")
}

/// Win11 风格的窗口控制按钮
///
/// 交给系统处理：`window_control_area` 会让 `WM_NCHITTEST` 返回 HTMINBUTTON /
/// HTMAXBUTTON / HTCLOSE，由 `DefWindowProc` 执行最小化、最大化（含 Snap Layouts）与关闭，
/// 因此这里不挂 on_click，避免和系统行为重复。
fn window_button(
    id: &'static str,
    icon: AnyElement,
    danger: bool,
    area: WindowControlArea,
) -> impl IntoElement {
    div()
        .id(id)
        .w(px(WINDOW_BUTTON_WIDTH))
        .h_full()
        .flex()
        .items_center()
        .justify_center()
        .window_control_area(area)
        .hover(move |style| {
            if danger {
                style.bg(rgb(0xc42b1c))
            } else {
                style.bg(gpui::hsla(0.0, 0.0, 1.0, 0.06))
            }
        })
        .active(|style| style.bg(gpui::hsla(0.0, 0.0, 1.0, 0.10)))
        .child(icon)
}

impl Spike {
    fn backdrop_label(&self) -> &'static str {
        match self.backdrop {
            WindowBackgroundAppearance::Opaque => "Opaque",
            WindowBackgroundAppearance::Transparent => "Transparent",
            WindowBackgroundAppearance::Blurred => "Blurred (acrylic)",
            WindowBackgroundAppearance::MicaBackdrop => "Mica",
            WindowBackgroundAppearance::MicaAltBackdrop => "Mica Alt",
        }
    }

    /// 标题栏：左边菜单、右边窗口按钮，空白处可拖拽 / 双击最大化
    fn render_titlebar(&self, window: &Window, cx: &mut Context<Self>) -> impl IntoElement {
        let is_maximized = window.is_maximized();

        let mut bar = div()
            .id("titlebar")
            .flex()
            .flex_row()
            .items_center()
            .h(px(TITLEBAR_HEIGHT))
            // 标题栏比主体更透（Mica 主要体现在这里），与主体靠底色明度区分，不加分隔线
            .bg(gpui::hsla(0.0, 0.0, 0.0, TITLEBAR_TINT))
            // 左边缘留白也可拖动（需要显式 id 才会生成命中盒）
            .child(
                div()
                    .id("titlebar-drag-left")
                    .w(px(6.0))
                    .h_full()
                    .window_control_area(WindowControlArea::Drag),
            );

        #[cfg(windows)]
        for (index, spec) in menu::MENUS.iter().enumerate() {
            bar = bar.child(
                div()
                    .id(("top-menu", index))
                    .h_full()
                    .px_3()
                    .flex()
                    .items_center()
                    .rounded_sm()
                    .text_size(px(12.0))
                    .text_color(rgb(0xffffff))
                    .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                    .hover(|style| style.bg(gpui::hsla(0.0, 0.0, 1.0, 0.10)))
                    .on_click(cx.listener(move |this, event: &ClickEvent, window, cx| {
                        this.open_menu(index, event, window, cx);
                    }))
                    .child(spec.title),
            );
        }

        // 空白区即拖拽区：拖动分屏、双击最大化都交给系统（HTCAPTION）
        bar.child(
            div()
                .id("titlebar-drag")
                .flex_1()
                .h_full()
                .window_control_area(WindowControlArea::Drag),
        )
        .child(window_button(
            "win-min",
            icon_minimize().into_any_element(),
            false,
            WindowControlArea::Min,
        ))
        .child(window_button(
            "win-max",
            if is_maximized {
                icon_restore().into_any_element()
            } else {
                icon_maximize().into_any_element()
            },
            false,
            WindowControlArea::Max,
        ))
        .child(window_button(
            "win-close",
            icon_close().into_any_element(),
            true,
            WindowControlArea::Close,
        ))
    }

    /// 弹出原生下拉菜单（DComp 下唯一可用的系统菜单方案）
    #[cfg(windows)]
    fn open_menu(
        &mut self,
        index: usize,
        event: &ClickEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let scale = window.scale_factor();
        let position = event.position();
        let x: f32 = position.x.into();
        self.pop_menu_at(index, x, TITLEBAR_HEIGHT * scale, window, cx);
    }

    /// 在客户区 (x, y) 处为第 index 个菜单弹出原生下拉（逻辑像素）
    #[cfg(windows)]
    fn pop_menu_at(
        &mut self,
        index: usize,
        x: f32,
        y: f32,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(spec) = menu::MENUS.get(index) else {
            return;
        };

        // 顶层命令按钮（无下拉）直接执行
        if spec.items.is_empty() {
            self.dispatch(menu::Cmd::Settings, window, cx);
            return;
        }
        if self.hwnd == 0 {
            return;
        }

        if let Some(cmd) =
            menu::popup_at_client(self.hwnd as *mut core::ffi::c_void, x, y, spec.items)
        {
            self.dispatch(cmd, window, cx);
        }
    }

    /// 菜单命令落地（后续这里会长成真正的 app action）
    #[cfg(windows)]
    fn dispatch(&mut self, cmd: menu::Cmd, window: &mut Window, cx: &mut Context<Self>) {
        use menu::Cmd;

        let backdrop = match cmd {
            Cmd::Exit => {
                window.defer(cx, |window, _| window.remove_window());
                return;
            }
            Cmd::BackdropOpaque => Some(WindowBackgroundAppearance::Opaque),
            Cmd::BackdropMica => Some(WindowBackgroundAppearance::MicaBackdrop),
            Cmd::BackdropMicaAlt => Some(WindowBackgroundAppearance::MicaAltBackdrop),
            Cmd::BackdropBlurred => Some(WindowBackgroundAppearance::Blurred),
            Cmd::BackdropTransparent => Some(WindowBackgroundAppearance::Transparent),
            _ => None,
        };

        if let Some(backdrop) = backdrop {
            self.backdrop = backdrop;
            window.set_background_appearance(backdrop);
        }

        self.last_cmd = format!("{cmd:?}").into();
        cx.notify();
    }
}

impl Render for Spike {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // 启动到首帧的耗时（gpui 侧冷启动基线）
        if !self.first_frame_logged {
            self.first_frame_logged = true;
            eprintln!(
                "[SPIKE] first frame after {} ms",
                self.started_at.elapsed().as_millis()
            );
        }

        let titlebar = self.render_titlebar(window, cx);

        div()
            .size_full()
            .flex()
            .flex_col()
            .child(titlebar)
            .child(
                div()
                    .flex_1()
                    .flex()
                    .flex_col()
                    .items_center()
                    .justify_center()
                    .gap_3()
                    // 主体用更实的底色，避免整窗过于通透
                    .bg(gpui::hsla(0.0, 0.0, 0.0, CONTENT_TINT))
                    .child(
                        div()
                            .text_2xl()
                            .text_color(rgb(0xffffff))
                            .child("FastView · gpui spike"),
                    )
                    // 半透明卡片：肉眼确认背景材质是否透出来
                    .child(
                        div()
                            .w(px(460.0))
                            .h(px(170.0))
                            .rounded_xl()
                            .bg(gpui::hsla(0.0, 0.0, 1.0, 0.10))
                            .border_1()
                            .border_color(gpui::hsla(0.0, 0.0, 1.0, 0.20))
                            .flex()
                            .flex_col()
                            .items_center()
                            .justify_center()
                            .gap_2()
                            .child(div().text_sm().text_color(rgb(0xffffff)).child(format!(
                                "backdrop: {}",
                                self.backdrop_label()
                            )))
                            .child(
                                div()
                                    .text_sm()
                                    .text_color(gpui::hsla(0.0, 0.0, 1.0, 0.7))
                                    .child(format!("last command: {}", self.last_cmd)),
                            ),
                    ),
            )
    }
}

/// 取窗口原生句柄（Windows）
fn native_handle(window: &Window) -> isize {
    #[cfg(windows)]
    {
        use raw_window_handle::{HasWindowHandle, RawWindowHandle};
        // 显式走 trait：gpui 自身也有同名的 `window_handle()` 返回 AnyWindowHandle
        if let Ok(RawWindowHandle::Win32(win32)) =
            HasWindowHandle::window_handle(window).map(|handle| handle.as_raw())
        {
            return win32.hwnd.get();
        }
        0
    }
    #[cfg(not(windows))]
    {
        let _ = window;
        0
    }
}

fn main() {
    let started_at = Instant::now();
    let backdrop = WindowBackgroundAppearance::MicaBackdrop;

    application().run(move |cx: &mut App| {
        let options = WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(Bounds::centered(
                None,
                size(px(960.0), px(640.0)),
                cx,
            ))),
            // 自绘标题栏：菜单与窗口按钮都放进标题栏，避免多占一行
            titlebar: Some(TitlebarOptions {
                title: Some("FastView".into()),
                appears_transparent: true,
                traffic_light_position: None,
            }),
            window_background: backdrop,
            window_min_size: Some(size(px(420.0), px(320.0))),
            ..Default::default()
        };

        cx.open_window(options, move |window, cx| {
            let hwnd = native_handle(window);
            #[cfg(windows)]
            if hwnd != 0 {
                // 去掉 DWM 的 1px 窗口边框，避免自绘标题栏顶部出现亮线
                win::hide_window_border(hwnd as *mut core::ffi::c_void);
            }
            cx.new(|_| Spike {
                hwnd,
                backdrop,
                last_cmd: "—".into(),
                started_at,
                first_frame_logged: false,
            })
        })
        .expect("failed to open window");

        cx.activate(true);
    });
}

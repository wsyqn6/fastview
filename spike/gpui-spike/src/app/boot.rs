//! 启动装配：窗口选项、首次 DWM 边框、命令行传入的图片

use std::path::PathBuf;
use std::time::Instant;

use gpui::{
    App, Bounds, TitlebarOptions, Window, WindowBackgroundAppearance, WindowBounds, WindowOptions,
    prelude::*, px, size,
};
use gpui_platform::application;

use super::Spike;
use super::theme::Palette;
#[cfg(windows)]
use crate::win;

/// 创建主窗口并运行事件循环
pub fn run() {
    let started_at = Instant::now();
    // 实色方案：标题栏与内容区各自不透明，靠明度差区分（同 VS Code / Windows Terminal）。
    // `Opaque` 下渲染器用白色清屏，但整块客户区都被我们的实色覆盖，不会露出清屏色。
    let backdrop = WindowBackgroundAppearance::Opaque;
    let initial_path = std::env::args_os().nth(1).map(PathBuf::from);

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
                // 把 DWM 的 1px 边框画成标题栏底色，避免自绘标题栏顶部出现亮线
                let border_rgb = Palette::for_appearance(window.appearance()).titlebar_rgb;
                win::apply_window_frame(hwnd as *mut core::ffi::c_void, border_rgb);
            }

            let view = cx.new(|cx| Spike::new(hwnd, started_at, cx));

            // 让内容区直接接收键盘快捷键
            let handle = view.read(cx).focus_handle.clone();
            window.focus(&handle, cx);

            // 命令行传入的图片在首帧后后台解码，避免阻塞启动
            if let Some(path) = initial_path.clone() {
                let weak = view.downgrade();
                window.defer(cx, move |window, cx| {
                    weak.update(cx, |this, cx| this.open_path(path, window, cx))
                        .ok();
                });
            }

            view
        })
        .expect("failed to open window");

        cx.activate(true);
    });
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

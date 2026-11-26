//! 启动装配：窗口选项、首次 DWM 外观、命令行传入的图片

use std::path::PathBuf;
use std::time::Instant;

use gpui::{
    App, Bounds, TitlebarOptions, Window, WindowBackgroundAppearance, WindowBounds, WindowOptions,
    prelude::*, px, size,
};
use gpui_platform::application;

use super::Spike;
use super::settings::Settings;
use super::theme;
#[cfg(windows)]
use crate::win;

/// 启动阶段按初始窗口尺寸估算解码宽度；放大一点避免复用时分辨率不够
fn speculative_need_px() -> u32 {
    #[cfg(windows)]
    let scale = unsafe { windows_sys::Win32::UI::HiDpi::GetDpiForSystem() } as f32 / 96.0;
    #[cfg(not(windows))]
    let scale = 1.0;
    (960.0 * scale * 1.25).round().max(1.0) as u32
}

/// 创建主窗口并运行事件循环
pub fn run() {
    let started_at = Instant::now();
    crate::spike_log!("[SPIKE] launch pid={}", std::process::id());
    // 配置在开窗之前读：材质与透明度决定首帧用透明还是实色背景
    let settings = Settings::load();
    let after_settings = Instant::now();
    // 材质只有 Windows 有系统背景可透；其他平台与排查模式下退化为实色
    let translucent = super::translucent(&settings);
    let initial_path = std::env::args_os().nth(1).map(PathBuf::from);

    #[cfg(windows)]
    let first_instance = crate::single_instance::acquire();
    #[cfg(windows)]
    if !first_instance {
        let paths = std::env::args_os()
            .skip(1)
            .map(PathBuf::from)
            .collect::<Vec<_>>();
        if crate::single_instance::forward(&paths) {
            crate::spike_log!("[SPIKE] forwarded to existing instance");
            return;
        }
    }
    #[cfg(windows)]
    let (incoming_tx, incoming_rx) = std::sync::mpsc::channel::<Vec<PathBuf>>();

    // application() 构造 gpui 平台（WindowsPlatform / DirectWrite），
    // run() 才启动消息循环并回调；分开计时才能定位 app 阶段的真实归属
    let app = application();
    let after_app_new = Instant::now();

    // 首图解码与 gpui 事件循环/建窗并行；窗口就绪后按尺寸匹配复用
    let (preload_tx, preload_rx) = std::sync::mpsc::channel::<super::load::Preloaded>();
    let preload_spec = initial_path.clone().and_then(|path| {
        matches!(
            crate::folder::input_kind(&path),
            crate::folder::InputKind::Image
        )
        .then_some((path, speculative_need_px()))
    });
    if let Some((path, need_px)) = &preload_spec {
        let tx = preload_tx.clone();
        let path = path.clone();
        let need_px = *need_px;
        app.background_executor()
            .spawn(async move {
                let result = super::load::decode_task(
                    path.clone(),
                    need_px,
                    crate::decode::Rotation::None,
                    None,
                )
                .await;
                let _ = tx.send(super::load::Preloaded {
                    path,
                    need_px,
                    result,
                });
            })
            .detach();
    }

    app.run(move |cx: &mut App| {
        let after_app = Instant::now();
        #[cfg(windows)]
        if first_instance {
            let _ = crate::single_instance::install(incoming_tx);
        }
        // 静默注册“打开方式”候选；不接管默认程序，失败只记日志
        #[cfg(windows)]
        cx.background_executor()
            .spawn(async { crate::assoc::register_silent(); })
            .detach();
        let centered = Bounds::centered(None, size(px(960.0), px(640.0)), cx);
        let after_bounds = Instant::now();
        let options = WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(centered)),
            // 自绘标题栏：菜单与窗口按钮都放进标题栏，避免多占一行
            titlebar: Some(TitlebarOptions {
                title: Some("FastView".into()),
                appears_transparent: true,
                traffic_light_position: None,
            }),
            // gpui 在创建时按窗口样式测量 border_offset；这里先让它量到 0，
            // 建窗后由 win.rs 再加回 WS_THICKFRAME 并接管客户区，避免多出一圈未绘制边缘。
            is_resizable: false,
            // 材质模式必须非 Opaque：渲染器才会清成透明，DWM 背景透出来。
            // 实色模式用 Opaque，subpixel 文字抗锯齿也能恢复。
            window_background: if translucent {
                WindowBackgroundAppearance::Transparent
            } else {
                WindowBackgroundAppearance::Opaque
            },
            window_min_size: Some(size(px(420.0), px(320.0))),
            ..Default::default()
        };

        cx.open_window(options, move |window, cx| {
            let after_window = Instant::now();
            let hwnd = native_handle(window);
            #[cfg(windows)]
            if hwnd != 0 {
                let raw = hwnd as *mut core::ffi::c_void;
                let dark = theme::is_dark(settings.theme, window.appearance());
                let accent = win::system_accent().unwrap_or(theme::DEFAULT_ACCENT);
                let border_rgb =
                    theme::Palette::new(dark, settings.alpha(), accent).titlebar_rgb;
                let backdrop = translucent && settings.uses_backdrop();
                // 先试无边框；Win10 失败时回退成标题栏底色，避免露出原生亮边
                win::apply_window_frame(raw, border_rgb, backdrop);
                win::apply_window_icon(raw);
                win::apply_dark_mode(raw, dark);
                let supported = win::apply_backdrop(raw, super::backdrop_kind(&settings));
                // 22621 之前没有系统背景材质：直接设老式亚克力；窗口创建时已是
                // `Transparent`，渲染器仍会清成透明，这里只补系统背景模糊。
                win::set_legacy_accent(raw, backdrop && !supported);
                // 去 native frame、保留边缘缩放，并在拖动时降级模糊。
                win::install_window_hooks(raw);
            }
            let after_native = Instant::now();

            let view = cx.new(|cx| Spike::new(hwnd, started_at, settings, cx));
            let after_view = Instant::now();
            crate::spike_log!(
                "[SPIKE] boot {} ms: settings {} · app_new {} · app_run {} · bounds {} · open_window {} · native {} · view {}",
                after_view.duration_since(started_at).as_millis(),
                after_settings.duration_since(started_at).as_millis(),
                after_app_new.duration_since(after_settings).as_millis(),
                after_app.duration_since(after_app_new).as_millis(),
                after_bounds.duration_since(after_app).as_millis(),
                after_window.duration_since(after_bounds).as_millis(),
                after_native.duration_since(after_window).as_millis(),
                after_view.duration_since(after_native).as_millis(),
            );

            if let Ok(preloaded) = preload_rx.try_recv() {
                view.update(cx, |this, _| this.set_preload(preloaded));
            }

            // 让内容区直接接收键盘快捷键
            let handle = view.read(cx).focus_handle.clone();
            window.focus(&handle, cx);

            #[cfg(windows)]
            if first_instance {
                let receiver = std::sync::Arc::new(std::sync::Mutex::new(Some(incoming_rx)));
                let weak = view.downgrade();
                cx.spawn(async move |cx| loop {
                    let incoming = {
                        let guard = receiver
                            .lock()
                            .unwrap_or_else(|poisoned| poisoned.into_inner());
                        match guard.as_ref() {
                            Some(receiver) => receiver.try_recv(),
                            None => break,
                        }
                    };
                    match incoming {
                        Ok(paths) => {
                            let _ = weak.update_in(cx, |this, window, cx| {
                                this.open_sources(paths, window, cx)
                            });
                        }
                        Err(std::sync::mpsc::TryRecvError::Empty) => {
                            cx.background_executor()
                                .timer(std::time::Duration::from_millis(10))
                                .await;
                        }
                        Err(std::sync::mpsc::TryRecvError::Disconnected) => break,
                    }
                })
                .detach();
            }

            // 命令行传入的图片在首帧后后台解码，避免阻塞启动
            if let Some(path) = initial_path.clone() {
                let weak = view.downgrade();
                window.defer(cx, move |window, cx| {
                    weak.update(cx, |this, cx| this.open_source(path, window, cx))
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

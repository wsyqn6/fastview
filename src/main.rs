mod app;
mod fonts;
mod i18n;
mod loader;
mod types;
mod utils;

use app::FastViewApp;
use eframe::egui;

/// 仅在 debug 模式下输出日志
#[macro_export]
macro_rules! debug_log {
    ($($arg:tt)*) => {
        #[cfg(debug_assertions)]
        eprintln!($($arg)*);
    };
}

/// 性能监控宏 (仅 debug 模式)
#[macro_export]
macro_rules! perf_log {
    ($label:expr, $start:expr) => {
        #[cfg(debug_assertions)]
        eprintln!("[PERF] {}: {}ms", $label, $start.elapsed().as_millis());
    };
}

/// Windows 平台：在 release 模式下隐藏控制台窗口
#[cfg(all(windows, not(debug_assertions)))]
fn hide_console_window() {
    use windows_sys::Win32::System::Console::GetConsoleWindow;
    use windows_sys::Win32::UI::WindowsAndMessaging::{ShowWindow, SW_HIDE};
    unsafe {
        let hwnd = GetConsoleWindow();
        if !hwnd.is_null() {
            ShowWindow(hwnd, SW_HIDE);
        }
    }
}

fn main() -> eframe::Result<()> {
    // Windows release 模式：隐藏控制台窗口
    #[cfg(all(windows, not(debug_assertions)))]
    hide_console_window();
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1200.0, 800.0])
            .with_min_inner_size([400.0, 300.0])
            .with_decorations(true) // 使用系统原生窗口装饰
            .with_fullscreen(false),
        ..Default::default()
    };

    eframe::run_native(
        "FastView",
        options,
        Box::new(|cc| Ok(Box::new(FastViewApp::new(cc)))),
    )
}

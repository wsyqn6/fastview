//! Windows 窗口外观的原生微调

use core::ffi::c_void;

use windows_sys::Win32::Foundation::HWND;
use windows_sys::Win32::Graphics::Dwm::DwmSetWindowAttribute;

/// DWMWA_BORDER_COLOR（Windows 11 21H2+）
const DWMWA_BORDER_COLOR: u32 = 34;
/// DWMWA_COLOR_NONE：让 DWM 不绘制窗口边框
const DWMWA_COLOR_NONE: u32 = 0xFFFF_FFFE;

/// 去掉 DWM 给窗口画的 1px 边框
///
/// 自绘标题栏时，这条边框会表现为窗口顶部/四周的一条亮线，与自绘底色不协调。
pub fn hide_window_border(hwnd: HWND) {
    unsafe {
        DwmSetWindowAttribute(
            hwnd,
            DWMWA_BORDER_COLOR,
            &DWMWA_COLOR_NONE as *const u32 as *const c_void,
            size_of::<u32>() as u32,
        );
    }
}

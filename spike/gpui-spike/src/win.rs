//! Windows 窗口外观的原生微调

use core::ffi::c_void;

use windows_sys::Win32::Foundation::HWND;
use windows_sys::Win32::Graphics::Dwm::DwmSetWindowAttribute;
use windows_sys::Win32::UI::WindowsAndMessaging::{
    SWP_FRAMECHANGED, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOOWNERZORDER, SWP_NOSIZE, SWP_NOZORDER,
    SetWindowPos,
};

/// DWMWA_WINDOW_CORNER_PREFERENCE（Windows 11）
const DWMWA_WINDOW_CORNER_PREFERENCE: u32 = 33;
/// DWMWA_BORDER_COLOR（Windows 11 21H2+）
const DWMWA_BORDER_COLOR: u32 = 34;
/// DWMWCP_ROUND
const DWMWCP_ROUND: u32 = 2;

/// 设置窗口边框与圆角
///
/// Windows 11 的 DWM 会给窗口画一圈 1px 边框。自绘标题栏时它表现为顶部一条亮线：
/// 单纯设 `DWMWA_COLOR_NONE` 在部分版本/时序下不生效，所以这里直接把边框画成
/// **标题栏底色**——即使系统坚持要画，也看不出来。
///
/// `border_rgb` 是 `0xRRGGBB`。
pub fn apply_window_frame(hwnd: HWND, border_rgb: u32) {
    set_attribute(hwnd, DWMWA_BORDER_COLOR, colorref(border_rgb));
    set_attribute(hwnd, DWMWA_WINDOW_CORNER_PREFERENCE, DWMWCP_ROUND);

    // 让 DWM 立刻重建窗口框架，否则改动可能要等到下次尺寸变化才生效
    unsafe {
        SetWindowPos(
            hwnd,
            core::ptr::null_mut(),
            0,
            0,
            0,
            0,
            SWP_FRAMECHANGED
                | SWP_NOACTIVATE
                | SWP_NOMOVE
                | SWP_NOSIZE
                | SWP_NOZORDER
                | SWP_NOOWNERZORDER,
        );
    }
}

/// `0xRRGGBB` → COLORREF（`0x00BBGGRR`）
fn colorref(rgb: u32) -> u32 {
    ((rgb & 0x0000_00FF) << 16) | (rgb & 0x0000_FF00) | ((rgb & 0x00FF_0000) >> 16)
}

/// 设置一个 DWM 的 u32 属性，失败时打日志（不阻塞 UI）
fn set_attribute(hwnd: HWND, attribute: u32, value: u32) {
    let result = unsafe {
        DwmSetWindowAttribute(
            hwnd,
            attribute,
            &value as *const u32 as *const c_void,
            size_of::<u32>() as u32,
        )
    };

    if result != 0 {
        eprintln!("[SPIKE] DwmSetWindowAttribute({attribute}) failed: {result:#x}");
    }
}

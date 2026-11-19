//! Windows 窗口外观的原生微调
//!
//! 材质、暗色模式与边框都走 DWM：gpui 只暴露 Mica / MicaAlt，亚克力（系统瞬态背景）
//! 需要自己设 `DWMWA_SYSTEMBACKDROP_TYPE`。
//!
//! 这里**不拦截重绘**：gpui-windows 在拖动时每帧 present 才能让 DComp
//! 内容跟住窗口位置。只在拖动/缩放期间临时关掉后台模糊，避免每帧重采样背景。

use core::ffi::c_void;
use std::sync::atomic::{AtomicU32, Ordering};

use windows::Win32::Foundation::{HWND as SubclassHwnd, LPARAM, LRESULT, WPARAM};
use windows::Win32::UI::Shell::{DefSubclassProc, RemoveWindowSubclass, SetWindowSubclass};
use windows::Win32::UI::WindowsAndMessaging::{
    HTBOTTOM, HTBOTTOMLEFT, HTBOTTOMRIGHT, HTCAPTION, HTCLIENT, HTLEFT, HTMAXBUTTON, HTRIGHT,
    NCCALCSIZE_PARAMS, WM_CANCELMODE, WM_ENTERSIZEMOVE, WM_EXITSIZEMOVE, WM_NCDESTROY,
    WM_NCCALCSIZE, WM_NCHITTEST, WM_NCLBUTTONUP,
};
use windows_sys::Win32::Foundation::{HWND, POINT as SysPoint};
use windows_sys::Win32::Graphics::Dwm::{DwmGetWindowAttribute, DwmSetWindowAttribute};
use windows_sys::Win32::Graphics::Gdi::ScreenToClient;
use windows_sys::Win32::System::LibraryLoader::{GetModuleHandleA, GetProcAddress};
use windows_sys::Win32::UI::HiDpi::GetDpiForWindow;
use windows_sys::Win32::UI::WindowsAndMessaging::{
    GWL_STYLE, GetClientRect, GetWindowLongPtrW, IsZoomed, SW_MAXIMIZE, SW_NORMAL,
    SWP_FRAMECHANGED, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOOWNERZORDER, SWP_NOSIZE, SWP_NOZORDER,
    SetWindowPos, SetWindowLongPtrW, ShowWindowAsync, WS_MAXIMIZEBOX, WS_THICKFRAME,
};

use crate::app::theme::{TITLEBAR_HEIGHT, WINDOW_BUTTON_WIDTH};

/// DWMWA_NCRENDERING_POLICY
const DWMWA_NCRENDERING_POLICY: u32 = 2;
/// DWMNCRP_DISABLED：关掉 DWM native frame / shadow
const DWMNCRP_DISABLED: u32 = 1;
/// DWMWA_USE_IMMERSIVE_DARK_MODE
const DWMWA_USE_IMMERSIVE_DARK_MODE: u32 = 20;
/// DWMWA_WINDOW_CORNER_PREFERENCE（Windows 11）
const DWMWA_WINDOW_CORNER_PREFERENCE: u32 = 33;
/// DWMWA_BORDER_COLOR（Windows 11 21H2+）
const DWMWA_BORDER_COLOR: u32 = 34;
/// DWMWA_SYSTEMBACKDROP_TYPE（Windows 11 22H2 / build 22621+）
const DWMWA_SYSTEMBACKDROP_TYPE: u32 = 38;

/// DWMWCP_ROUND
const DWMWCP_ROUND: u32 = 2;
/// 不画边框
const DWMWA_COLOR_NONE: u32 = 0xffff_fffe;

/// DWMSBT_NONE
const DWMSBT_NONE: u32 = 1;
/// DWMSBT_MAINWINDOW（云母）
const DWMSBT_MAINWINDOW: u32 = 2;
/// DWMSBT_TRANSIENTWINDOW（亚克力）
const DWMSBT_TRANSIENTWINDOW: u32 = 3;

/// WCA_ACCENT_POLICY
const WCA_ACCENT_POLICY: u32 = 0x13;
/// ACCENT_DISABLED：没有系统背景模糊，窗口自身 alpha 仍保留
const ACCENT_DISABLED: u32 = 0;
/// ACCENT_ENABLE_BLURBEHIND：Win10 老式模糊，拖动时的低开销降级
const ACCENT_ENABLE_BLURBEHIND: u32 = 3;
/// ACCENT_ENABLE_ACRYLICBLURBEHIND：Win10 的老式亚克力
const ACCENT_ENABLE_ACRYLICBLURBEHIND: u32 = 4;

const DRAG_BACKDROP_SUBCLASS_ID: usize = 1;

#[repr(C)]
struct AccentPolicy {
    accent_state: u32,
    accent_flags: u32,
    gradient_color: u32,
    animation_id: u32,
}

#[repr(C)]
struct WindowCompositionAttributeData {
    attribute: u32,
    data: *mut c_void,
    data_size: u32,
}

type SetWindowCompositionAttributeFn = unsafe extern "system" fn(
    HWND,
    *mut WindowCompositionAttributeData,
) -> i32;

/// 期望的系统 DWM 材质（`DWMSBT_*`）；拖动时实际临时设为 NONE
static ACTIVE_BACKDROP: AtomicU32 = AtomicU32::new(DWMSBT_NONE);
/// 期望的 Win10 老式亚克力状态；拖动时实际临时降为 BLURBEHIND
static LEGACY_ACCENT_STATE: AtomicU32 = AtomicU32::new(ACCENT_DISABLED);
/// 是否处于系统 size/move 循环
static SIZE_MOVE_ACTIVE: AtomicU32 = AtomicU32::new(0);

/// 窗口背景材质
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Backdrop {
    None,
    Acrylic,
    Mica,
}

/// 设置窗口边框与圆角
///
/// gpui 创建窗口时以 `is_resizable=false` 量到零 `border_offset`，所以这里可以
/// 在 `install_window_hooks` 里恢复 `WS_THICKFRAME` 并把客户区扩到整窗而没有布局差。
pub fn apply_window_frame(hwnd: HWND, border_rgb: u32, prefer_none: bool) {
    let border = colorref(border_rgb);

    let hidden = prefer_none
        && set_attribute_quiet(hwnd, DWMWA_BORDER_COLOR, DWMWA_COLOR_NONE);
    if !hidden {
        set_attribute_quiet(hwnd, DWMWA_BORDER_COLOR, border);
    }
    if !supports_system_backdrop(hwnd) {
        set_attribute_quiet(hwnd, DWMWA_NCRENDERING_POLICY, DWMNCRP_DISABLED);
    }
    set_attribute_quiet(hwnd, DWMWA_WINDOW_CORNER_PREFERENCE, DWMWCP_ROUND);

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

/// 系统深浅色：DWM 的材质与标题栏色跟着切
pub fn apply_dark_mode(hwnd: HWND, dark: bool) {
    set_attribute(hwnd, DWMWA_USE_IMMERSIVE_DARK_MODE, u32::from(dark));
}

/// 当前系统是否支持 DWM 系统背景材质（云母 / 亚克力）
pub fn supports_system_backdrop(hwnd: HWND) -> bool {
    let mut value = 0u32;
    let result = unsafe {
        DwmGetWindowAttribute(
            hwnd,
            DWMWA_SYSTEMBACKDROP_TYPE,
            &mut value as *mut u32 as *mut c_void,
            size_of::<u32>() as u32,
        )
    };
    result == 0
}

/// 设置系统背景材质
///
/// 返回 `false` 表示当前系统不支持（22621 之前），调用方应改用 Win10 的
/// SWCA 亚克力（`set_legacy_accent`）。
pub fn apply_backdrop(hwnd: HWND, backdrop: Backdrop) -> bool {
    let value = match backdrop {
        Backdrop::None => DWMSBT_NONE,
        Backdrop::Acrylic => DWMSBT_TRANSIENTWINDOW,
        Backdrop::Mica => DWMSBT_MAINWINDOW,
    };
    ACTIVE_BACKDROP.store(value, Ordering::Relaxed);

    // 最大化/还原会在拖动过程中触发 apply_window_style；此时不能把 DWM 材质设回去。
    let effective = if SIZE_MOVE_ACTIVE.load(Ordering::Relaxed) != 0 {
        DWMSBT_NONE
    } else {
        value
    };
    set_attribute(hwnd, DWMWA_SYSTEMBACKDROP_TYPE, effective)
}

/// 设置 Win10 老式亚克力（SWCA `ACCENT_ENABLE_ACRYLICBLURBEHIND`）
///
/// 不通过 gpui 的 `set_background_appearance`：窗口创建时已是 `Transparent`，
/// 这里只负责系统背景模糊；拖动时才能单独把它关掉而不影响窗口 alpha。
pub fn set_legacy_accent(hwnd: HWND, acrylic: bool) {
    let desired = if acrylic {
        ACCENT_ENABLE_ACRYLICBLURBEHIND
    } else {
        ACCENT_DISABLED
    };
    LEGACY_ACCENT_STATE.store(desired, Ordering::Relaxed);

    // 同上：拖动过程中只应用低开销模糊，等 WM_EXITSIZEMOVE 再恢复亚克力。
    let effective = if SIZE_MOVE_ACTIVE.load(Ordering::Relaxed) != 0
        && desired == ACCENT_ENABLE_ACRYLICBLURBEHIND
    {
        ACCENT_ENABLE_BLURBEHIND
    } else {
        desired
    };
    set_accent_policy(hwnd, effective);
}

/// 安装边框与材质钩子
///
/// gpui 开窗时以 `is_resizable=false` 量到零 `border_offset`；这里再加回
/// `WS_THICKFRAME`，并在 `WM_NCCALCSIZE` 里把客户区扩到整窗。这样 gpui 的布局
/// 尺寸与实际 swapchain 一致，左右下不会留未绘制的透明边。
pub fn install_window_hooks(hwnd: HWND) {
    let subclass_hwnd = SubclassHwnd(hwnd);
    unsafe {
        let style = GetWindowLongPtrW(hwnd, GWL_STYLE);
        let resizable = (WS_THICKFRAME | WS_MAXIMIZEBOX) as isize;
        SetWindowLongPtrW(hwnd, GWL_STYLE, style | resizable);
        let _ = SetWindowSubclass(
            subclass_hwnd,
            Some(window_frame_proc),
            DRAG_BACKDROP_SUBCLASS_ID,
            0,
        );
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

unsafe extern "system" fn window_frame_proc(
    hwnd: SubclassHwnd,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
    _id: usize,
    _data: usize,
) -> LRESULT {
    let raw = hwnd.0;
    match msg {
        WM_ENTERSIZEMOVE => pause_backdrop(raw),
        WM_EXITSIZEMOVE | WM_CANCELMODE => resume_backdrop(raw),
        WM_NCDESTROY => {
            resume_backdrop(raw);
            unsafe {
                let _ = RemoveWindowSubclass(hwnd, Some(window_frame_proc), DRAG_BACKDROP_SUBCLASS_ID);
            }
        }
        WM_NCCALCSIZE => {
            // 进入时的 rgrc[0] 才是新窗口矩形；DefWindowProc 之后它会变成客户区。
            let window_rect = if wparam.0 != 0 && lparam.0 != 0 {
                unsafe {
                    let params = &*(lparam.0 as *const NCCALCSIZE_PARAMS);
                    Some(params.rgrc[0])
                }
            } else {
                None
            };
            let result = unsafe { DefSubclassProc(hwnd, msg, wparam, lparam) };
            if let Some(window) = window_rect
                && unsafe { IsZoomed(raw) } == 0
            {
                unsafe {
                    extend_client_area(lparam, window.left, window.right, window.bottom);
                }
            }
            return result;
        }
        WM_NCHITTEST => {
            let result = unsafe { DefSubclassProc(hwnd, msg, wparam, lparam) };
            // gpui 开窗时 is_resizable=false（为了 border_offset=0），把 Max 命中降级成 HTCAPTION。
            // 按按钮矩形补回 HTMAXBUTTON，保留系统贴靠布局与悬停行为。
            if result.0 == HTCAPTION as isize && unsafe { max_button_hit(hwnd, lparam) } {
                return LRESULT(HTMAXBUTTON as isize);
            }
            if result.0 == HTCLIENT as isize && unsafe { IsZoomed(raw) } == 0 {
                if let Some(hit) = unsafe { edge_hit(hwnd, lparam) } {
                    return LRESULT(hit);
                }
            }
            return result;
        }
        WM_NCLBUTTONUP => {
            // gpui 的 is_resizable=false 不会执行最大化；自己调系统动作。
            // 仍要把 UP 交给 gpui 清掉按钮的按下态，且 gpui 会吞掉该消息，
            // 不会让 DefWindowProc 重复执行最大化/还原。
            if wparam.0 as u32 == HTMAXBUTTON {
                let show = if unsafe { IsZoomed(raw) } != 0 {
                    SW_NORMAL
                } else {
                    SW_MAXIMIZE
                };
                unsafe { ShowWindowAsync(raw, show) };
            }
        }
        _ => {}
    }
    unsafe { DefSubclassProc(hwnd, msg, wparam, lparam) }
}

/// 客户区扩展到整个窗口矩形；gpui 此时记录的 border_offset 已为 0。
unsafe fn extend_client_area(lparam: LPARAM, left: i32, right: i32, bottom: i32) {
    let params = lparam.0 as *mut NCCALCSIZE_PARAMS;
    if params.is_null() {
        return;
    }
    unsafe {
        let top = (*params).rgrc[0].top;
        let client = &mut (*params).rgrc[0];
        client.left = left;
        client.right = right;
        client.bottom = bottom;
        client.top = top;
    }
}

/// 客户区盖住 native frame 后，补回左右下边缘缩放命中。
unsafe fn edge_hit(hwnd: SubclassHwnd, lparam: LPARAM) -> Option<isize> {
    let low = lparam.0 as i32;
    let mut point = SysPoint {
        x: (low & 0xffff) as i16 as i32,
        y: ((low >> 16) & 0xffff) as i16 as i32,
    };
    let mut rect: windows_sys::Win32::Foundation::RECT = unsafe { std::mem::zeroed() };

    unsafe {
        if ScreenToClient(hwnd.0, &mut point) == 0 {
            return None;
        }
        if GetClientRect(hwnd.0, &mut rect) == 0 {
            return None;
        }
    }
    let dpi = unsafe { GetDpiForWindow(hwnd.0) } as i32;
    let edge = ((8 * dpi) / 96).max(4);

    let left = point.x < edge;
    let right = point.x >= rect.right - edge;
    let bottom = point.y >= rect.bottom - edge;

    let hit = match (left, right, bottom) {
        (true, _, true) => HTBOTTOMLEFT,
        (_, true, true) => HTBOTTOMRIGHT,
        (true, _, _) => HTLEFT,
        (_, true, _) => HTRIGHT,
        (_, _, true) => HTBOTTOM,
        _ => return None,
    };
    Some(hit as isize)
}

/// gpui 把 Max 按钮降级成 HTCAPTION；用按钮矩形补回真实命中。
unsafe fn max_button_hit(hwnd: SubclassHwnd, lparam: LPARAM) -> bool {
    let low = lparam.0 as i32;
    let mut point = SysPoint {
        x: (low & 0xffff) as i16 as i32,
        y: ((low >> 16) & 0xffff) as i16 as i32,
    };
    let mut rect: windows_sys::Win32::Foundation::RECT = unsafe { std::mem::zeroed() };
    unsafe {
        if ScreenToClient(hwnd.0, &mut point) == 0 || GetClientRect(hwnd.0, &mut rect) == 0 {
            return false;
        }
    }
    let scale = unsafe { GetDpiForWindow(hwnd.0) } as f32 / 96.0;
    let width = (WINDOW_BUTTON_WIDTH * scale).round() as i32;
    let height = (TITLEBAR_HEIGHT * scale).round() as i32;
    point.x >= rect.right - 2 * width
        && point.x < rect.right - width
        && point.y >= 0
        && point.y < height
}

/// 移动 / 缩放期间：换成低开销模糊，保留窗口自身 alpha
fn pause_backdrop(hwnd: HWND) {
    SIZE_MOVE_ACTIVE.store(1, Ordering::Relaxed);
    if LEGACY_ACCENT_STATE.load(Ordering::Relaxed) == ACCENT_ENABLE_ACRYLICBLURBEHIND {
        set_accent_policy(hwnd, ACCENT_ENABLE_BLURBEHIND);
    }
    if ACTIVE_BACKDROP.load(Ordering::Relaxed) != DWMSBT_NONE {
        set_attribute(hwnd, DWMWA_SYSTEMBACKDROP_TYPE, DWMSBT_NONE);
    }
}

/// 移动 / 缩放结束：恢复原来的材质
fn resume_backdrop(hwnd: HWND) {
    SIZE_MOVE_ACTIVE.store(0, Ordering::Relaxed);
    if LEGACY_ACCENT_STATE.load(Ordering::Relaxed) == ACCENT_ENABLE_ACRYLICBLURBEHIND {
        set_accent_policy(hwnd, ACCENT_ENABLE_ACRYLICBLURBEHIND);
    }
    let value = ACTIVE_BACKDROP.load(Ordering::Relaxed);
    if value != DWMSBT_NONE {
        set_attribute(hwnd, DWMWA_SYSTEMBACKDROP_TYPE, value);
    }
}

/// 系统强调色（`0xRRGGBB`）
///
/// 注册表里存的是 `0xAABBGGRR`。
pub fn system_accent() -> Option<u32> {
    use windows::Win32::System::Registry::{
        HKEY_CURRENT_USER, RRF_RT_REG_DWORD, RegGetValueW,
    };
    use windows::core::w;

    let mut raw = 0u32;
    let mut size = size_of::<u32>() as u32;
    let status = unsafe {
        RegGetValueW(
            HKEY_CURRENT_USER,
            w!("Software\\Microsoft\\Windows\\DWM"),
            w!("AccentColor"),
            RRF_RT_REG_DWORD,
            None,
            Some(&mut raw as *mut u32 as *mut c_void),
            Some(&mut size),
        )
    };
    if status.0 != 0 {
        return None;
    }

    Some((raw & 0x0000_00ff) << 16 | (raw & 0x0000_ff00) | ((raw >> 16) & 0x0000_00ff))
}

/// `0xRRGGBB` → COLORREF（`0x00BBGGRR`）
fn colorref(rgb: u32) -> u32 {
    ((rgb & 0x0000_00FF) << 16) | (rgb & 0x0000_FF00) | ((rgb & 0x00FF_0000) >> 16)
}

/// 直接设置 SWCA 的 AccentPolicy（gpui 的 `Blurred` 走的同一路径）
fn set_accent_policy(hwnd: HWND, state: u32) -> bool {
    let user32 = unsafe { GetModuleHandleA(c"user32.dll".as_ptr().cast::<u8>()) };
    if user32.is_null() {
        return false;
    }
    let Some(address) = (unsafe {
        GetProcAddress(user32, c"SetWindowCompositionAttribute".as_ptr().cast::<u8>())
    }) else {
        return false;
    };
    let function: SetWindowCompositionAttributeFn = unsafe { std::mem::transmute(address) };

    let acrylic = state == ACCENT_ENABLE_ACRYLICBLURBEHIND;
    let policy = AccentPolicy {
        accent_state: state,
        accent_flags: if acrylic { 0 } else { 2 },
        // gpui 在 alpha 为 0 时会补 1，否则亚克力不生效
        gradient_color: if acrylic { 0x0100_0000 } else { 0 },
        animation_id: 0,
    };
    let mut data = WindowCompositionAttributeData {
        attribute: WCA_ACCENT_POLICY,
        data: &policy as *const _ as *mut c_void,
        data_size: size_of::<AccentPolicy>() as u32,
    };
    unsafe { function(hwnd, &mut data) != 0 }
}

fn dword_attribute(hwnd: HWND, attribute: u32, value: u32) -> i32 {
    unsafe {
        DwmSetWindowAttribute(
            hwnd,
            attribute,
            &value as *const u32 as *const c_void,
            size_of::<u32>() as u32,
        )
    }
}

/// 设置一个 DWM 的 u32 属性，失败时打日志（不阻塞 UI）
fn set_attribute(hwnd: HWND, attribute: u32, value: u32) -> bool {
    let result = dword_attribute(hwnd, attribute, value);
    if result != 0 {
        eprintln!("[SPIKE] DwmSetWindowAttribute({attribute}) failed: {result:#x}");
        return false;
    }
    true
}

/// 可选属性：失败是预期内的（例如 Win10 不支持边框/圆角属性）
fn set_attribute_quiet(hwnd: HWND, attribute: u32, value: u32) -> bool {
    dword_attribute(hwnd, attribute, value) == 0
}

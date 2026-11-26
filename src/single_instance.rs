//! Windows 轻量单实例：第二进程把命令行路径发给已运行进程后退出

use std::ffi::OsString;
use std::os::windows::ffi::{OsStrExt, OsStringExt};
use std::path::PathBuf;
use std::sync::mpsc::Sender;
use std::sync::{Mutex, OnceLock};
use std::time::Duration;

use windows::Win32::Foundation::{HINSTANCE, HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, FindWindowExW, HWND_MESSAGE, SendMessageTimeoutW,
    WINDOW_EX_STYLE, WINDOW_STYLE, WM_COPYDATA, WNDCLASSW,
};
use windows::core::PCWSTR;
use windows_sys::Win32::Foundation::{CloseHandle, GetLastError};
use windows_sys::Win32::System::Threading::CreateMutexW;

const MUTEX_NAME: &str = "Local\\FastView.SingleInstance";
const CLASS_NAME: &str = "FastView.SingleInstance.Window";
const WINDOW_NAME: &str = "FastView.SingleInstance.Target";
const ERROR_ALREADY_EXISTS: u32 = 183;
const COPY_DATA_OPEN: usize = 1;

static INCOMING: OnceLock<Mutex<Sender<Vec<PathBuf>>>> = OnceLock::new();

/// `COPYDATASTRUCT`：dwData 为指针宽度，cbData 为字节数
#[repr(C)]
struct CopyData {
    dw_data: usize,
    cb_data: u32,
    lp_data: *const u16,
}

fn wide(value: &str) -> Vec<u16> {
    value.encode_utf16().chain(Some(0)).collect()
}

fn encode_paths(paths: &[PathBuf]) -> Vec<u16> {
    let mut wide_paths = Vec::new();
    for path in paths {
        wide_paths.extend(path.as_os_str().encode_wide());
        wide_paths.push(0);
    }
    wide_paths.push(0);
    wide_paths
}

fn decode_paths(data: &CopyData) -> Vec<PathBuf> {
    if data.dw_data != COPY_DATA_OPEN || data.cb_data < 4 || data.lp_data.is_null() {
        return Vec::new();
    }
    let units = unsafe { std::slice::from_raw_parts(data.lp_data, data.cb_data as usize / 2) };
    let mut paths = Vec::new();
    let mut current = Vec::new();
    for &unit in units {
        if unit == 0 {
            if !current.is_empty() {
                paths.push(PathBuf::from(OsString::from_wide(&current)));
                current.clear();
            }
        } else {
            current.push(unit);
        }
    }
    if !current.is_empty() {
        paths.push(PathBuf::from(OsString::from_wide(&current)));
    }
    paths
}

unsafe extern "system" fn hidden_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    if msg == WM_COPYDATA {
        let data = unsafe { &*(lparam.0 as *const CopyData) };
        let paths = decode_paths(data);
        if let Some(sender) = INCOMING.get()
            && !paths.is_empty()
        {
            let _ = sender.lock().map(|sender| sender.send(paths));
        }
        return LRESULT(1);
    }
    unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) }
}

/// 首个进程返回 true 并保持互斥量句柄至进程退出；后续进程返回 false
pub fn acquire() -> bool {
    let name = wide(MUTEX_NAME);
    unsafe {
        let handle = CreateMutexW(std::ptr::null(), 1, name.as_ptr());
        if handle.is_null() {
            return true;
        }
        if GetLastError() == ERROR_ALREADY_EXISTS {
            let _ = CloseHandle(handle);
            return false;
        }
        // 故意不关闭：句柄随进程退出释放，避免第二个进程抢主
        true
    }
}

/// 首进程安装隐藏窗口；后续进程通过它找到首进程
pub fn install(sender: Sender<Vec<PathBuf>>) -> bool {
    if INCOMING.set(Mutex::new(sender)).is_err() {
        return false;
    }
    let class = wide(CLASS_NAME);
    let window = wide(WINDOW_NAME);
    let instance =
        unsafe { windows_sys::Win32::System::LibraryLoader::GetModuleHandleW(std::ptr::null()) };
    let wc = WNDCLASSW {
        lpfnWndProc: Some(hidden_proc),
        hInstance: HINSTANCE(instance),
        lpszClassName: PCWSTR(class.as_ptr()),
        ..Default::default()
    };
    unsafe {
        let _ = windows::Win32::UI::WindowsAndMessaging::RegisterClassW(&wc);
        let result = CreateWindowExW(
            WINDOW_EX_STYLE(0),
            PCWSTR(class.as_ptr()),
            PCWSTR(window.as_ptr()),
            WINDOW_STYLE(0),
            0,
            0,
            0,
            0,
            Some(HWND_MESSAGE),
            None,
            Some(HINSTANCE(instance)),
            None,
        );
        result.is_ok()
    }
}

/// 第二进程把路径发给首进程；等待隐藏窗口最多 1 秒
pub fn forward(paths: &[PathBuf]) -> bool {
    if paths.is_empty() {
        return false;
    }
    let class = wide(CLASS_NAME);
    let window = wide(WINDOW_NAME);
    let encoded = encode_paths(paths);
    for _ in 0..20 {
        let found = unsafe {
            FindWindowExW(
                Some(HWND_MESSAGE),
                None,
                PCWSTR(class.as_ptr()),
                PCWSTR(window.as_ptr()),
            )
        };
        if let Ok(hwnd) = found {
            let data = CopyData {
                dw_data: COPY_DATA_OPEN,
                cb_data: (encoded.len() * 2) as u32,
                lp_data: encoded.as_ptr(),
            };
            let result = unsafe {
                SendMessageTimeoutW(
                    hwnd,
                    WM_COPYDATA,
                    WPARAM(0),
                    LPARAM(&data as *const CopyData as isize),
                    windows::Win32::UI::WindowsAndMessaging::SMTO_ABORTIFHUNG,
                    1000,
                    None,
                )
            };
            return result.0 != 0;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    false
}

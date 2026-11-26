//! Windows 文件关联：启动时静默注册为“打开方式”候选
//!
//! 只写 HKCU，不碰 UserChoice / 默认程序；启动时检查，缺失或路径变化才写，移动后自愈。

use std::path::Path;

use windows::Win32::System::Registry::{
    HKEY, HKEY_CURRENT_USER, REG_SZ, RegCloseKey, RegCreateKeyW, RegDeleteTreeW, RegDeleteValueW,
    RegOpenKeyW, RegQueryValueW, RegSetValueW,
};
use windows::Win32::UI::Shell::{SHCNE_ASSOCCHANGED, SHCNF_IDLIST, SHChangeNotify};
use windows::core::{PCWSTR, PWSTR};

const PROG_ID: &str = "FastView.Image.1";
const FRIENDLY_NAME: &str = "FastView Image";
/// `RegDeleteTreeW` 删除不存在的键时返回该值，视为成功
const ERROR_FILE_NOT_FOUND: u32 = 2;

fn wide(value: &str) -> Vec<u16> {
    value.encode_utf16().chain(Some(0)).collect()
}

/// `"<exe>" "%1"`：路径带空格也可正确接收
fn command_for(exe: &Path) -> String {
    format!("\"{}\" \"%1\"", exe.to_string_lossy())
}

fn progids_path(ext: &str) -> String {
    format!("Software\\Classes\\.{ext}\\OpenWithProgids")
}

/// 通知 Explorer 立即刷新“打开方式”，无需注销或重启
fn notify_shell() {
    unsafe { SHChangeNotify(SHCNE_ASSOCCHANGED, SHCNF_IDLIST, None, None) };
}

fn create_key(path: &str) -> Result<HKEY, String> {
    let path = wide(path);
    let mut key = HKEY::default();
    let status = unsafe { RegCreateKeyW(HKEY_CURRENT_USER, PCWSTR(path.as_ptr()), &mut key) };
    if status.0 != 0 {
        return Err(format!("create key failed: {}", status.0));
    }
    Ok(key)
}

fn set_value(key: HKEY, name: &str, value: &str) -> Result<(), String> {
    let name = wide(name);
    let value = wide(value);
    let status = unsafe {
        RegSetValueW(
            key,
            PCWSTR(name.as_ptr()),
            REG_SZ,
            PCWSTR(value.as_ptr()),
            (value.len() * 2) as u32,
        )
    };
    if status.0 != 0 {
        Err(format!("set value failed: {}", status.0))
    } else {
        Ok(())
    }
}

fn set_value_in(path: &str, name: &str, value: &str) -> Result<(), String> {
    let key = create_key(path)?;
    let result = set_value(key, name, value);
    unsafe {
        let _ = RegCloseKey(key);
    };
    result
}

fn open_key(path: &str) -> Option<HKEY> {
    let path = wide(path);
    let mut key = HKEY::default();
    let status = unsafe { RegOpenKeyW(HKEY_CURRENT_USER, PCWSTR(path.as_ptr()), &mut key) };
    (status.0 == 0).then_some(key)
}

fn read_value(key: HKEY, name: &str) -> Option<String> {
    let name = wide(name);
    let mut size = 0i32;
    let status = unsafe { RegQueryValueW(key, PCWSTR(name.as_ptr()), None, Some(&mut size)) };
    if status.0 != 0 || size <= 0 {
        return None;
    }
    let mut buffer = vec![0u16; size as usize / 2 + 1];
    let status = unsafe {
        RegQueryValueW(
            key,
            PCWSTR(name.as_ptr()),
            Some(PWSTR(buffer.as_mut_ptr())),
            Some(&mut size),
        )
    };
    if status.0 != 0 {
        return None;
    }
    let mut len = (size as usize / 2).min(buffer.len());
    while len > 0 && buffer[len - 1] == 0 {
        len -= 1;
    }
    Some(String::from_utf16_lossy(&buffer[..len]))
}

fn read_value_in(path: &str, name: &str) -> Option<String> {
    let key = open_key(path)?;
    let value = read_value(key, name);
    unsafe {
        let _ = RegCloseKey(key);
    };
    value
}

/// 注册表内容已与当前 exe 一致时跳过写入
fn is_current(command: &str, icon: &str) -> bool {
    let root = format!("Software\\Classes\\{PROG_ID}");
    if read_value_in(&root, "") != Some(FRIENDLY_NAME.to_string())
        || read_value_in(&format!("{root}\\DefaultIcon"), "") != Some(icon.to_string())
        || read_value_in(&format!("{root}\\shell\\open\\command"), "") != Some(command.to_string())
        || read_value_in(&format!("{root}\\shell\\FastView\\command"), "")
            != Some(command.to_string())
    {
        return false;
    }
    crate::folder::EXTENSIONS
        .iter()
        .all(|ext| read_value_in(&progids_path(ext), PROG_ID).as_deref() == Some(""))
}

/// 注册 HKCU 候选关联；不写扩展名默认值，不接管系统默认程序
fn register() -> Result<(), String> {
    let exe = std::env::current_exe().map_err(|error| format!("current_exe: {error}"))?;
    let command = command_for(&exe);
    let icon = format!("\"{}\",0", exe.to_string_lossy());
    let root = format!("Software\\Classes\\{PROG_ID}");
    if is_current(&command, &icon) {
        return Ok(());
    }

    set_value_in(&root, "", FRIENDLY_NAME)?;
    set_value_in(&format!("{root}\\DefaultIcon"), "", &icon)?;
    set_value_in(&format!("{root}\\shell\\open\\command"), "", &command)?;
    set_value_in(&format!("{root}\\shell\\FastView\\command"), "", &command)?;
    for ext in crate::folder::EXTENSIONS {
        set_value_in(&progids_path(ext), PROG_ID, "")?;
    }
    notify_shell();
    Ok(())
}

/// 启动时静默注册；失败只记日志，不影响查看器
pub fn register_silent() {
    if let Err(error) = register() {
        eprintln!("[SPIKE] file association failed: {error}");
    }
}

/// 只删本程序写入的键/值，保留默认程序与其他 ProgID
pub fn unregister() -> Result<(), String> {
    let root = wide(&format!("Software\\Classes\\{PROG_ID}"));
    let status = unsafe { RegDeleteTreeW(HKEY_CURRENT_USER, PCWSTR(root.as_ptr())) };
    if status.0 != 0 && status.0 != ERROR_FILE_NOT_FOUND {
        return Err(format!("delete prog id failed: {}", status.0));
    }
    for ext in crate::folder::EXTENSIONS {
        if let Some(key) = open_key(&progids_path(ext)) {
            let name = wide(PROG_ID);
            unsafe {
                let _ = RegDeleteValueW(key, PCWSTR(name.as_ptr()));
                let _ = RegCloseKey(key);
            }
        }
    }
    notify_shell();
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn command_quotes_exe_and_keeps_placeholder() {
        let exe = Path::new(r"C:\Program Files\FastView\fastview.exe");
        assert_eq!(
            command_for(exe),
            r#""C:\Program Files\FastView\fastview.exe" "%1""#
        );
    }

    #[test]
    fn extensions_match_folder_scan_with_leading_dot() {
        let path = progids_path("jpg");
        assert_eq!(path, "Software\\Classes\\.jpg\\OpenWithProgids");
        assert!(crate::folder::EXTENSIONS.contains(&"jpg"));
    }
}

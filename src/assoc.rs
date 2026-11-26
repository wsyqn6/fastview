//! Windows 文件关联：只写 HKCU，不碰 UserChoice
//!
//! 注册候选 ProgID、图标与右键动词；默认程序仍由用户在系统设置中选择。

use std::path::Path;

use windows::Win32::System::Registry::{
    HKEY, HKEY_CURRENT_USER, REG_SZ, RegCloseKey, RegCreateKeyW, RegDeleteTreeW, RegDeleteValueW,
    RegOpenKeyW, RegQueryValueW, RegSetValueW,
};
use windows::core::{PCWSTR, PWSTR};

const PROG_ID: &str = "FastView.Image.1";
const FRIENDLY_NAME: &str = "FastView Image";
const EXTS: &[&str] = &[
    ".jpg", ".jpeg", ".png", ".gif", ".webp", ".bmp", ".tif", ".tiff",
];
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
    format!("Software\\Classes\\{ext}\\OpenWithProgids")
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

fn has_value(path: &str, name: &str) -> bool {
    let Some(key) = open_key(path) else {
        return false;
    };
    let name = wide(name);
    let mut buffer = [0u16; 128];
    let mut size = (buffer.len() * 2) as i32;
    let status = unsafe {
        RegQueryValueW(
            key,
            PCWSTR(name.as_ptr()),
            Some(PWSTR(buffer.as_mut_ptr())),
            Some(&mut size),
        )
    };
    unsafe {
        let _ = RegCloseKey(key);
    };
    status.0 == 0
}

/// 是否注册过候选 ProgID（以第一个扩展名为准）
pub fn is_registered() -> bool {
    EXTS.first()
        .is_some_and(|ext| has_value(&progids_path(ext), PROG_ID))
}

/// 注册 HKCU 候选关联；不写扩展名默认值，不接管系统默认程序
pub fn register() -> Result<(), String> {
    let exe = std::env::current_exe().map_err(|error| format!("current_exe: {error}"))?;
    let command = command_for(&exe);
    let icon = format!("\"{}\",0", exe.to_string_lossy());
    let root = format!("Software\\Classes\\{PROG_ID}");

    set_value_in(&root, "", FRIENDLY_NAME)?;
    set_value_in(&format!("{root}\\DefaultIcon"), "", &icon)?;
    set_value_in(&format!("{root}\\shell\\open\\command"), "", &command)?;
    set_value_in(&format!("{root}\\shell\\FastView\\command"), "", &command)?;
    for ext in EXTS {
        set_value_in(&progids_path(ext), PROG_ID, "")?;
    }
    Ok(())
}

/// 只删本程序写入的键/值，保留默认程序与其他 ProgID
pub fn unregister() -> Result<(), String> {
    let root = wide(&format!("Software\\Classes\\{PROG_ID}"));
    let status = unsafe { RegDeleteTreeW(HKEY_CURRENT_USER, PCWSTR(root.as_ptr())) };
    if status.0 != 0 && status.0 != ERROR_FILE_NOT_FOUND {
        return Err(format!("delete prog id failed: {}", status.0));
    }
    for ext in EXTS {
        if let Some(key) = open_key(&progids_path(ext)) {
            let name = wide(PROG_ID);
            unsafe {
                let _ = RegDeleteValueW(key, PCWSTR(name.as_ptr()));
                let _ = RegCloseKey(key);
            }
        }
    }
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
    fn extensions_are_registered_with_leading_dot() {
        assert!(EXTS.iter().all(|ext| ext.starts_with('.')));
        assert!(EXTS.contains(&".jpg"));
    }
}

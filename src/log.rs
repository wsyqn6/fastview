//! 轻量诊断日志：debug 输出终端，release 可选写入文件
//!
//! release 默认静默；设置 `FASTVIEW_LOG=1` 后写到
//! `%APPDATA%\fastview\fastview.log`，超过 1 MiB 自动截断。

use std::fs::{File, OpenOptions};
use std::io::Write;
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};

const MAX_LOG_BYTES: u64 = 1024 * 1024;

static LOG_FILE: OnceLock<Mutex<Option<File>>> = OnceLock::new();

fn enabled() -> bool {
    std::env::var("FASTVIEW_LOG").is_ok_and(|value| !value.is_empty() && value != "0")
}

fn log_path() -> Option<PathBuf> {
    let mut path = dirs::config_dir()?;
    path.push("fastview");
    path.push("fastview.log");
    Some(path)
}

fn open_log() -> Option<File> {
    if !enabled() {
        return None;
    }
    let path = log_path()?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).ok()?;
    }
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
        .ok()?;
    if file.metadata().map(|meta| meta.len()).unwrap_or(0) > MAX_LOG_BYTES {
        drop(file);
        std::fs::write(&path, "").ok()?;
        file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)
            .ok()?;
    }
    Some(file)
}

fn log_file() -> &'static Mutex<Option<File>> {
    LOG_FILE.get_or_init(|| Mutex::new(open_log()))
}

/// 写一条日志； debug 总输出终端，release 仅写启用后的文件
pub fn write(message: &str) {
    #[cfg(debug_assertions)]
    eprintln!("{message}");

    let mut guard = log_file()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    if let Some(file) = guard.as_mut() {
        let _ = writeln!(file, "{message}");
        let _ = file.flush();
    }
}

#[macro_export]
macro_rules! spike_log {
    ($($arg:tt)*) => {{
        $crate::log::write(&format!($($arg)*));
    }};
}

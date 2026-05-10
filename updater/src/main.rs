//! FastView Updater - 极简更新器
//!
//! 职责:
//! 1. 等待原进程退出
//! 2. 复制新文件覆盖旧文件
//! 3. 可选重启应用
//! 4. 清理临时文件

use std::env;
use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::thread;
use std::time::Duration;

fn main() {
    let args: Vec<String> = env::args().collect();

    // 解析参数
    let mut source_path: Option<PathBuf> = None;
    let mut target_path: Option<PathBuf> = None;
    let mut should_restart = false;

    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--source" => {
                if i + 1 < args.len() {
                    source_path = Some(PathBuf::from(&args[i + 1]));
                    i += 2;
                } else {
                    eprintln!("Error: --source requires a path argument");
                    std::process::exit(1);
                }
            }
            "--target" => {
                if i + 1 < args.len() {
                    target_path = Some(PathBuf::from(&args[i + 1]));
                    i += 2;
                } else {
                    eprintln!("Error: --target requires a path argument");
                    std::process::exit(1);
                }
            }
            "--restart" => {
                should_restart = true;
                i += 1;
            }
            _ => {
                eprintln!("Unknown argument: {}", args[i]);
                std::process::exit(1);
            }
        }
    }

    // 验证必需参数
    let source = match source_path {
        Some(p) => p,
        None => {
            eprintln!("Error: --source is required");
            std::process::exit(1);
        }
    };

    let target = match target_path {
        Some(p) => p,
        None => {
            eprintln!("Error: --target is required");
            std::process::exit(1);
        }
    };

    // 等待原进程完全退出 (2秒延迟)
    thread::sleep(Duration::from_secs(2));

    // 复制新文件覆盖旧文件
    if let Err(e) = fs::copy(&source, &target) {
        eprintln!("Failed to copy file: {}", e);
        std::process::exit(1);
    }

    // macOS/Linux: 设置可执行权限
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if let Ok(metadata) = fs::metadata(&target) {
            let mut perms = metadata.permissions();
            perms.set_mode(0o755); // rwxr-xr-x
            if let Err(e) = fs::set_permissions(&target, perms) {
                eprintln!("Warning: Failed to set executable permissions: {}", e);
            }
        }
    }

    // 如果需要,启动新版本
    if should_restart {
        if let Err(e) = Command::new(&target).spawn() {
            eprintln!("Failed to restart application: {}", e);
            // 即使重启失败,也不影响更新成功,继续执行
        }
    }

    // 清理临时文件
    if let Err(e) = fs::remove_file(&source) {
        eprintln!("Warning: Failed to remove temporary file: {}", e);
    }

    // 成功退出
    std::process::exit(0);
}

//! 更新功能测试脚本
//!
//! 用法: cargo run --example test_updater

use fastview::core::updater;
use std::io::Write;

fn main() {
    println!("=== FastView Updater Test ===\n");

    // 测试1: 检查更新
    println!("1. Checking for updates...");

    // 模拟旧版本以测试更新检测 (取消注释下一行来测试)
    // let current_version = "0.1.0";
    let current_version = env!("CARGO_PKG_VERSION");
    println!("   Current version: {}", current_version);

    match updater::check_for_updates(current_version) {
        Ok(status) => {
            match status {
                updater::UpdateStatus::UpToDate => {
                    println!("   ✓ Already up to date!");
                }
                updater::UpdateStatus::UpdateAvailable {
                    version,
                    changelog,
                    asset,
                } => {
                    println!("   ✓ Update available!");
                    println!("   New version: {}", version);
                    println!("   Asset name: {}", asset.name);
                    println!("   Asset size: {} bytes", asset.size);
                    println!("\n   Changelog preview:");
                    println!(
                        "   {}",
                        changelog.lines().take(5).collect::<Vec<_>>().join("\n   ")
                    );

                    // 测试2: 下载更新 (可选,会实际下载文件)
                    println!("\n2. Would you like to test download? (y/n)");
                    let mut input = String::new();
                    std::io::stdin().read_line(&mut input).ok();

                    if input.trim().to_lowercase() == "y" {
                        println!("   Downloading...");
                        match updater::download_update(&asset, |progress| {
                            print!("\r   Progress: {:.1}%", progress * 100.0);
                            std::io::stdout().flush().ok();
                        }) {
                            Ok(path) => {
                                println!("\n   ✓ Download complete!");
                                println!("   Saved to: {:?}", path);

                                // 清理测试文件
                                println!("   Cleaning up test file...");
                                std::fs::remove_file(&path).ok();
                            }
                            Err(e) => {
                                println!("\n   ✗ Download failed: {}", e);
                            }
                        }
                    }
                }
                _ => {
                    println!("   Unexpected status: {:?}", status);
                }
            }
        }
        Err(e) => {
            println!("   ✗ Error: {}", e);
        }
    }

    println!("\n=== Test Complete ===");
}

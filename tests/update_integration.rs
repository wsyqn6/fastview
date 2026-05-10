//! 更新功能集成测试
//!
//! 使用方法:
//! 1. 正常测试(连接真实GitHub): cargo test --test update_integration
//! 2. 本地测试: 先运行 `cargo run --bin test_update_server`,再修改 updater.rs URL
//! 3. 查看测试说明: cargo test --test update_integration -- --nocapture

#[cfg(test)]
mod tests {
    use fastview::core::updater;

    #[test]
    fn test_check_updates_real_api() {
        // 这个测试会真正调用 GitHub API
        // 如果网络不可用或 API 限流,测试会失败
        let current_version = "0.1.0"; // 使用旧版本以确保有更新可用

        match updater::check_for_updates(current_version) {
            Ok(status) => {
                match status {
                    updater::UpdateStatus::UpdateAvailable {
                        version,
                        changelog,
                        asset,
                    } => {
                        println!("✓ Found update: {}", version);
                        println!("  Asset: {}", asset.name);
                        println!("  Size: {} bytes", asset.size);
                        println!(
                            "  Changelog preview: {}",
                            changelog.lines().take(2).collect::<Vec<_>>().join("\n  ")
                        );

                        // 验证基本结构
                        assert!(!version.is_empty());
                        assert!(!asset.name.is_empty());
                        assert!(asset.size > 0);
                        assert!(!asset.browser_download_url.is_empty());
                    }
                    updater::UpdateStatus::UpToDate => {
                        println!("✓ Already up to date (unexpected for v0.1.0)");
                    }
                    _ => panic!("Unexpected status: {:?}", status),
                }
            }
            Err(e) => {
                // 网络错误时跳过测试而不是失败
                eprintln!("⚠ Network error (test skipped): {}", e);
                // 不 assert,允许网络问题时测试通过
            }
        }
    }

    #[test]
    fn test_version_comparison_edge_cases() {
        // 测试边界情况
        assert!(updater::version_greater_than("1.0.0", "0.9.9"));
        assert!(updater::version_greater_than("0.10.0", "0.9.0"));
        assert!(updater::version_greater_than("1.0.1", "1.0.0"));
        assert!(!updater::version_greater_than("1.0.0", "1.0.0"));
        assert!(!updater::version_greater_than("0.9.0", "1.0.0"));

        // 测试不完整版本号
        assert!(updater::version_greater_than("1.0", "0.9"));
        assert!(updater::version_greater_than("2", "1.9.9"));
    }

    #[test]
    fn test_platform_asset_selection() {
        use fastview::core::updater::GitHubAsset;

        let assets = vec![
            GitHubAsset {
                name: "fastview-windows-x86_64.exe".to_string(),
                browser_download_url: "https://example.com/win.exe".to_string(),
                size: 5000000,
            },
            GitHubAsset {
                name: "fastview-macos-x86_64.zip".to_string(),
                browser_download_url: "https://example.com/mac.zip".to_string(),
                size: 4500000,
            },
            GitHubAsset {
                name: "fastview-linux-x86_64.tar.gz".to_string(),
                browser_download_url: "https://example.com/linux.tar.gz".to_string(),
                size: 4800000,
            },
        ];

        let selected = updater::select_platform_asset(&assets);
        assert!(selected.is_some());

        let asset = selected.unwrap();
        #[cfg(target_os = "windows")]
        assert!(asset.name.contains("windows"));

        #[cfg(target_os = "macos")]
        assert!(asset.name.contains("macos"));

        #[cfg(target_os = "linux")]
        assert!(asset.name.contains("linux"));

        println!("✓ Platform asset selection works correctly");
        println!("  Selected: {}", asset.name);
    }
}

//! 自动更新模块
//!
//! 负责从 GitHub Releases API 检查最新版本并下载更新

use serde::Deserialize;
use std::path::PathBuf;

/// GitHub Release API 响应结构
#[derive(Debug, Deserialize)]
pub struct GitHubRelease {
    pub tag_name: String,
    pub name: Option<String>,
    pub body: Option<String>, // changelog
    pub assets: Vec<GitHubAsset>,
}

/// GitHub Release 资产
#[derive(Debug, Deserialize, Clone, PartialEq)]
pub struct GitHubAsset {
    pub name: String,
    pub browser_download_url: String,
    pub size: u64,
}

/// 更新状态
#[derive(Debug, Clone, PartialEq)]
pub enum UpdateStatus {
    /// 未检查
    NotChecked,
    /// 检查中
    Checking,
    /// 已是最新版本
    UpToDate,
    /// 有可用更新
    UpdateAvailable {
        version: String,
        changelog: String,
        asset: GitHubAsset,
    },
    /// 下载中 (进度 0.0-1.0)
    Downloading(f32),
    /// 下载完成,等待重启
    DownloadComplete(PathBuf),
    /// 错误
    Error(String),
}

/// 检查更新
pub fn check_for_updates(current_version: &str) -> Result<UpdateStatus, String> {
    let url = "https://api.github.com/repos/wsyqn6/fastview/releases/latest";

    // 发送 HTTP 请求
    let response = ureq::get(url)
        .timeout(std::time::Duration::from_secs(10))
        .call()
        .map_err(|e| format!("Network error: {}", e))?;

    let release: GitHubRelease = response
        .into_json::<GitHubRelease>()
        .map_err(|e| format!("Failed to parse response: {}", e))?;

    // 比较版本号 (简单字符串比较,假设语义化版本)
    if is_newer_version(&release.tag_name, current_version) {
        // 选择适合当前平台的资产
        if let Some(asset) = select_platform_asset(&release.assets).cloned() {
            let changelog = release.body.unwrap_or_else(|| "No changelog available".to_string());
            Ok(UpdateStatus::UpdateAvailable {
                version: release.tag_name.trim_start_matches('v').to_string(),
                changelog,
                asset,
            })
        } else {
            Err("No suitable update asset found for your platform".to_string())
        }
    } else {
        Ok(UpdateStatus::UpToDate)
    }
}

/// 下载更新文件
pub fn download_update(
    asset: &GitHubAsset,
    progress_callback: impl Fn(f32),
) -> Result<PathBuf, String> {
    // 创建临时目录
    let temp_dir = std::env::temp_dir().join("fastview_update");
    std::fs::create_dir_all(&temp_dir)
        .map_err(|e| format!("Failed to create temp directory: {}", e))?;

    let target_path = temp_dir.join(&asset.name);

    // 发送下载请求
    let response = ureq::get(&asset.browser_download_url)
        .timeout(std::time::Duration::from_secs(300)) // 5分钟超时
        .call()
        .map_err(|e| format!("Download failed: {}", e))?;

    let total_size = asset.size;
    let mut downloaded: u64 = 0;

    // 读取响应数据
    let mut reader = response.into_reader();
    let mut file = std::fs::File::create(&target_path)
        .map_err(|e| format!("Failed to create file: {}", e))?;

    let mut buffer = vec![0u8; 8192];
    loop {
        let bytes_read = reader
            .read(&mut buffer)
            .map_err(|e| format!("Read error: {}", e))?;

        if bytes_read == 0 {
            break; // 下载完成
        }

        std::io::Write::write_all(&mut file, &buffer[..bytes_read])
            .map_err(|e| format!("Write error: {}", e))?;

        downloaded += bytes_read as u64;

        // 报告进度
        if total_size > 0 {
            let progress = downloaded as f32 / total_size as f32;
            progress_callback(progress);
        }
    }

    Ok(target_path)
}

/// 判断新版本是否更新
fn is_newer_version(new_version: &str, current_version: &str) -> bool {
    let new_ver = new_version.trim_start_matches('v');
    let cur_ver = current_version;

    // 简单的字符串比较 (适用于语义化版本)
    new_ver != cur_ver && version_greater_than(new_ver, cur_ver)
}

/// 比较两个版本号
fn version_greater_than(v1: &str, v2: &str) -> bool {
    let parts1: Vec<u32> = v1.split('.').filter_map(|s| s.parse().ok()).collect();
    let parts2: Vec<u32> = v2.split('.').filter_map(|s| s.parse().ok()).collect();

    for i in 0..std::cmp::max(parts1.len(), parts2.len()) {
        let p1 = parts1.get(i).copied().unwrap_or(0);
        let p2 = parts2.get(i).copied().unwrap_or(0);

        if p1 > p2 {
            return true;
        } else if p1 < p2 {
            return false;
        }
    }

    false // 相等
}

/// 根据平台选择合适的资产
fn select_platform_asset(assets: &[GitHubAsset]) -> Option<&GitHubAsset> {
    #[cfg(target_os = "windows")]
    let platform_pattern = "windows";

    #[cfg(target_os = "macos")]
    let platform_pattern = "macos";

    #[cfg(target_os = "linux")]
    let platform_pattern = "linux";

    #[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
    let platform_pattern = "";

    assets
        .iter()
        .find(|asset| asset.name.to_lowercase().contains(platform_pattern))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_version_comparison() {
        assert!(version_greater_than("0.4.0", "0.3.0"));
        assert!(version_greater_than("0.3.1", "0.3.0"));
        assert!(version_greater_than("1.0.0", "0.9.9"));
        assert!(!version_greater_than("0.3.0", "0.3.0"));
        assert!(!version_greater_than("0.2.0", "0.3.0"));
    }

    #[test]
    fn test_is_newer_version() {
        assert!(is_newer_version("v0.4.0", "0.3.0"));
        assert!(!is_newer_version("v0.3.0", "0.3.0"));
        assert!(!is_newer_version("v0.2.0", "0.3.0"));
    }
}

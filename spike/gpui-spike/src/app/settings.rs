//! 用户配置：主题、窗口材质与透明度
//!
//! spike 使用独立文件：egui 主线也写 `%APPDATA%\fastview\settings.json`，
//! 共用一份会被对方保存时丢掉未知字段。

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

/// 主题：默认跟随系统
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Theme {
    #[default]
    System,
    Light,
    Dark,
}

/// 窗口背景材质：只决定是否请求系统 backdrop，不影响透明度
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Material {
    /// DWM 亚克力（系统瞬态背景），默认
    #[default]
    Acrylic,
    /// DWM 云母，开销更低
    Mica,
    /// 不请求系统 backdrop；低透明度时只保留普通透明，不模糊
    None,
}

/// 自绘背景层不透明度（百分比）
fn default_opacity() -> u8 {
    65
}

fn default_version() -> u32 {
    2
}

/// 配置
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Settings {
    #[serde(default = "default_version")]
    pub version: u32,
    #[serde(default)]
    pub theme: Theme,
    #[serde(default)]
    pub material: Material,
    #[serde(default = "default_opacity")]
    pub opacity: u8,
    #[serde(default)]
    pub show_thumbs: bool,
    #[serde(default)]
    pub status_pinned: bool,
    #[serde(default)]
    pub hide_chrome: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            version: default_version(),
            theme: Theme::System,
            material: Material::Acrylic,
            opacity: default_opacity(),
            show_thumbs: false,
            status_pinned: false,
            hide_chrome: false,
        }
    }
}

impl Settings {
    /// 读取配置：文件不存在或解析失败时回退默认值
    pub fn load() -> Self {
        let Some(path) = Self::path() else {
            return Self::default();
        };
        let text = match std::fs::read_to_string(&path) {
            Ok(text) => text,
            Err(_) => return Self::default(),
        };
        match serde_json::from_str::<Self>(&text) {
            Ok(mut settings) => {
                settings.normalize();
                settings
            }
            Err(error) => {
                eprintln!("[SPIKE] settings parse failed: {error}");
                Self::default()
            }
        }
    }

    /// 写盘：失败只打日志，不影响运行
    pub fn save(&self) {
        let Some(path) = Self::path() else {
            return;
        };
        if let Some(parent) = path.parent()
            && let Err(error) = std::fs::create_dir_all(parent)
        {
            eprintln!("[SPIKE] settings dir failed: {error}");
            return;
        }
        match serde_json::to_string_pretty(self) {
            Ok(text) => {
                if let Err(error) = std::fs::write(&path, text) {
                    eprintln!("[SPIKE] settings write failed: {error}");
                }
            }
            Err(error) => eprintln!("[SPIKE] settings encode failed: {error}"),
        }
    }

    /// 自绘背景层 alpha（0..=1），只由不透明度决定
    pub fn alpha(&self) -> f32 {
        f32::from(self.opacity.min(100)) / 100.0
    }

    /// 是否请求系统背景模糊（与透明度无关）
    pub fn uses_backdrop(&self) -> bool {
        self.material != Material::None
    }

    /// 自绘背景层是否透明；100% 时窗口用 Opaque，恢复 subpixel AA
    pub fn translucent(&self) -> bool {
        self.opacity < 100
    }

    fn normalize(&mut self) {
        // v1 的 material=none 同时表示「实色」；迁移后它由 opacity=100 表达。
        if self.version < 2 && self.material == Material::None {
            self.opacity = 100;
        }
        self.version = default_version();
        self.opacity = self.opacity.min(100);
    }

    fn path() -> Option<PathBuf> {
        Some(
            dirs::config_dir()?
                .join("fastview")
                .join("gpui-settings.json"),
        )
    }
}
